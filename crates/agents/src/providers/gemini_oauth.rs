//! Google Gemini OAuth provider.
//!
//! Authentication uses Google's device-flow OAuth to obtain an access token,
//! which is then used to call the Gemini API.
//!
//! Users need to:
//! 1. Create a Google Cloud project
//! 2. Enable the Generative Language API
//! 3. Create OAuth 2.0 credentials (Desktop app or TV/Limited Input Device)
//! 4. Set GOOGLE_CLIENT_ID and optionally GOOGLE_CLIENT_SECRET environment variables

use std::pin::Pin;

use {
    async_trait::async_trait,
    futures::StreamExt,
    moltis_oauth::{OAuthTokens, TokenStore},
    secrecy::{ExposeSecret, Secret},
    tokio_stream::Stream,
    tracing::{debug, trace, warn},
};

use crate::model::{CompletionResponse, LlmProvider, StreamEvent, ToolCall, Usage};

// ── Constants ────────────────────────────────────────────────────────────────

const GOOGLE_DEVICE_CODE_URL: &str = "https://oauth2.googleapis.com/device/code";
const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const GEMINI_API_BASE: &str = "https://generativelanguage.googleapis.com";

/// OAuth scope for Gemini API access.
const GEMINI_SCOPE: &str = "https://www.googleapis.com/auth/generative-language.retriever https://www.googleapis.com/auth/cloud-platform";

const PROVIDER_NAME: &str = "gemini-oauth";

/// Buffer before token expiry to trigger refresh (5 minutes).
const REFRESH_THRESHOLD_SECS: u64 = 300;

// ── Device flow types ────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_url: String,
    pub interval: u64,
    pub expires_in: u64,
}

#[derive(Debug, serde::Deserialize)]
struct GoogleTokenResponse {
    access_token: Option<String>,
    refresh_token: Option<String>,
    expires_in: Option<u64>,
    error: Option<String>,
    error_description: Option<String>,
}

// ── Provider ─────────────────────────────────────────────────────────────────

pub struct GeminiOAuthProvider {
    model: String,
    client: reqwest::Client,
    token_store: TokenStore,
}

impl GeminiOAuthProvider {
    pub fn new(model: String) -> Self {
        Self {
            model,
            client: reqwest::Client::new(),
            token_store: TokenStore::new(),
        }
    }

    /// Get the Google client ID from environment.
    /// Users must set GOOGLE_CLIENT_ID from their Google Cloud project.
    pub fn get_client_id() -> Option<String> {
        std::env::var("GOOGLE_CLIENT_ID")
            .ok()
            .filter(|s| !s.is_empty())
    }

    /// Get the optional Google client secret from environment.
    pub fn get_client_secret() -> Option<String> {
        std::env::var("GOOGLE_CLIENT_SECRET")
            .ok()
            .filter(|s| !s.is_empty())
    }

    /// Start the Google device-flow: request a device code.
    pub async fn request_device_code(
        client: &reqwest::Client,
    ) -> anyhow::Result<DeviceCodeResponse> {
        let client_id = Self::get_client_id()
            .ok_or_else(|| anyhow::anyhow!("GOOGLE_CLIENT_ID environment variable not set"))?;

        let resp = client
            .post(GOOGLE_DEVICE_CODE_URL)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(&[("client_id", client_id.as_str()), ("scope", GEMINI_SCOPE)])
            .send()
            .await?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Google device code request failed: {body}");
        }

        Ok(resp.json().await?)
    }

    /// Poll Google for the access token after the user has entered the code.
    pub async fn poll_for_token(
        client: &reqwest::Client,
        device_code: &str,
        interval: u64,
    ) -> anyhow::Result<OAuthTokens> {
        let client_id = Self::get_client_id()
            .ok_or_else(|| anyhow::anyhow!("GOOGLE_CLIENT_ID environment variable not set"))?;
        let client_secret = Self::get_client_secret();

        loop {
            tokio::time::sleep(std::time::Duration::from_secs(interval)).await;

            let mut form_params = vec![
                ("client_id", client_id.clone()),
                ("device_code", device_code.to_string()),
                (
                    "grant_type",
                    "urn:ietf:params:oauth:grant-type:device_code".to_string(),
                ),
            ];

            if let Some(ref secret) = client_secret {
                form_params.push(("client_secret", secret.clone()));
            }

            let resp = client
                .post(GOOGLE_TOKEN_URL)
                .header("Content-Type", "application/x-www-form-urlencoded")
                .form(&form_params)
                .send()
                .await?;

            let body: GoogleTokenResponse = resp.json().await?;

            if let Some(token) = body.access_token {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();

                return Ok(OAuthTokens {
                    access_token: Secret::new(token),
                    refresh_token: body.refresh_token.map(Secret::new),
                    expires_at: body.expires_in.map(|e| now + e),
                });
            }

            match body.error.as_deref() {
                Some("authorization_pending") => continue,
                Some("slow_down") => {
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    continue;
                },
                Some(err) => {
                    let desc = body.error_description.unwrap_or_default();
                    anyhow::bail!("Google device flow error: {err} - {desc}");
                },
                None => anyhow::bail!("unexpected response from Google token endpoint"),
            }
        }
    }

    /// Refresh the access token using the refresh token.
    async fn refresh_access_token(&self, refresh_token: &str) -> anyhow::Result<OAuthTokens> {
        let client_id = Self::get_client_id()
            .ok_or_else(|| anyhow::anyhow!("GOOGLE_CLIENT_ID environment variable not set"))?;
        let client_secret = Self::get_client_secret();

        let mut form_params = vec![
            ("client_id", client_id),
            ("refresh_token", refresh_token.to_string()),
            ("grant_type", "refresh_token".to_string()),
        ];

        if let Some(secret) = client_secret {
            form_params.push(("client_secret", secret));
        }

        let resp = self
            .client
            .post(GOOGLE_TOKEN_URL)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(&form_params)
            .send()
            .await?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Google token refresh failed: {body}");
        }

        let body: GoogleTokenResponse = resp.json().await?;

        let access_token = body
            .access_token
            .ok_or_else(|| anyhow::anyhow!("no access_token in refresh response"))?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Ok(OAuthTokens {
            access_token: Secret::new(access_token),
            // Google may or may not return a new refresh token
            refresh_token: body
                .refresh_token
                .map(Secret::new)
                .or_else(|| Some(Secret::new(refresh_token.to_string()))),
            expires_at: body.expires_in.map(|e| now + e),
        })
    }

    /// Get a valid access token, refreshing if needed.
    async fn get_valid_token(&self) -> anyhow::Result<String> {
        let tokens = self.token_store.load(PROVIDER_NAME).ok_or_else(|| {
            anyhow::anyhow!("not logged in to gemini-oauth — run OAuth device flow first")
        })?;

        // Check if token needs refresh
        if let Some(expires_at) = tokens.expires_at {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();

            if now + REFRESH_THRESHOLD_SECS >= expires_at {
                // Token expiring soon — refresh it
                if let Some(ref refresh_token) = tokens.refresh_token {
                    let new_tokens = self
                        .refresh_access_token(refresh_token.expose_secret())
                        .await?;
                    self.token_store.save(PROVIDER_NAME, &new_tokens)?;
                    return Ok(new_tokens.access_token.expose_secret().clone());
                }
                anyhow::bail!("token expired and no refresh token available");
            }
        }

        Ok(tokens.access_token.expose_secret().clone())
    }
}

/// Check if we have stored tokens for Google Gemini OAuth.
pub fn has_stored_tokens() -> bool {
    TokenStore::new().load(PROVIDER_NAME).is_some()
}

/// Save tokens after successful authentication.
pub fn save_tokens(tokens: &OAuthTokens) -> anyhow::Result<()> {
    TokenStore::new().save(PROVIDER_NAME, tokens)?;
    Ok(())
}

/// Known Gemini models available via OAuth.
pub const GEMINI_OAUTH_MODELS: &[(&str, &str)] = &[
    ("gemini-2.5-pro-preview-06-05", "Gemini 2.5 Pro (OAuth)"),
    ("gemini-2.5-flash-preview-05-20", "Gemini 2.5 Flash (OAuth)"),
    ("gemini-2.0-flash", "Gemini 2.0 Flash (OAuth)"),
    ("gemini-2.0-flash-lite", "Gemini 2.0 Flash Lite (OAuth)"),
    ("gemini-1.5-pro", "Gemini 1.5 Pro (OAuth)"),
    ("gemini-1.5-flash", "Gemini 1.5 Flash (OAuth)"),
];

// ── Gemini API helpers (reused from gemini.rs) ──────────────────────────────

/// Convert JSON Schema types (lowercase) to Gemini types (uppercase).
fn convert_json_schema_types(schema: &serde_json::Value) -> serde_json::Value {
    match schema {
        serde_json::Value::Object(obj) => {
            let mut result = serde_json::Map::new();
            for (key, value) in obj {
                if key == "type" {
                    if let Some(type_str) = value.as_str() {
                        result.insert(
                            key.clone(),
                            serde_json::Value::String(type_str.to_uppercase()),
                        );
                    } else {
                        result.insert(key.clone(), value.clone());
                    }
                } else if key == "properties" {
                    if let serde_json::Value::Object(props) = value {
                        let converted_props: serde_json::Map<String, serde_json::Value> = props
                            .iter()
                            .map(|(k, v)| (k.clone(), convert_json_schema_types(v)))
                            .collect();
                        result.insert(key.clone(), serde_json::Value::Object(converted_props));
                    } else {
                        result.insert(key.clone(), value.clone());
                    }
                } else if key == "items" {
                    result.insert(key.clone(), convert_json_schema_types(value));
                } else {
                    result.insert(key.clone(), value.clone());
                }
            }
            serde_json::Value::Object(result)
        },
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(convert_json_schema_types).collect())
        },
        _ => schema.clone(),
    }
}

/// Convert tool schemas to Gemini's functionDeclarations format.
fn to_gemini_tools(tools: &[serde_json::Value]) -> serde_json::Value {
    let declarations: Vec<serde_json::Value> = tools
        .iter()
        .map(|t| {
            let params = convert_json_schema_types(&t["parameters"]);
            serde_json::json!({
                "name": t["name"],
                "description": t["description"],
                "parameters": params,
            })
        })
        .collect();

    serde_json::json!({ "functionDeclarations": declarations })
}

/// Extract system instruction from messages.
fn extract_system_instruction(
    messages: &[serde_json::Value],
) -> (Option<String>, Vec<&serde_json::Value>) {
    let mut system_text = None;
    let mut remaining = Vec::new();

    for msg in messages {
        if msg["role"].as_str() == Some("system") {
            system_text = msg["content"].as_str().map(|s| s.to_string());
        } else {
            remaining.push(msg);
        }
    }

    (system_text, remaining)
}

/// Convert messages to Gemini's content format.
fn to_gemini_messages(messages: &[&serde_json::Value]) -> Vec<serde_json::Value> {
    messages
        .iter()
        .map(|msg| {
            let role = msg["role"].as_str().unwrap_or("user");

            match role {
                "assistant" => {
                    if let Some(tool_calls) = msg["tool_calls"].as_array() {
                        let mut parts = Vec::new();

                        if let Some(text) = msg["content"].as_str()
                            && !text.is_empty()
                        {
                            parts.push(serde_json::json!({ "text": text }));
                        }

                        for tc in tool_calls {
                            let name = tc["function"]["name"].as_str().unwrap_or("");
                            let args_str = tc["function"]["arguments"].as_str().unwrap_or("{}");
                            let args: serde_json::Value =
                                serde_json::from_str(args_str).unwrap_or(serde_json::json!({}));
                            parts.push(serde_json::json!({
                                "functionCall": {
                                    "name": name,
                                    "args": args,
                                }
                            }));
                        }

                        serde_json::json!({
                            "role": "model",
                            "parts": parts,
                        })
                    } else {
                        serde_json::json!({
                            "role": "model",
                            "parts": [{ "text": msg["content"].as_str().unwrap_or("") }],
                        })
                    }
                },
                "tool" => {
                    let tool_call_id = msg["tool_call_id"].as_str().unwrap_or("");
                    let content = msg["content"].as_str().unwrap_or("");

                    let response: serde_json::Value = serde_json::from_str(content)
                        .unwrap_or_else(|_| serde_json::json!({ "result": content }));

                    serde_json::json!({
                        "role": "user",
                        "parts": [{
                            "functionResponse": {
                                "name": tool_call_id,
                                "response": response,
                            }
                        }],
                    })
                },
                _ => {
                    serde_json::json!({
                        "role": "user",
                        "parts": [{ "text": msg["content"].as_str().unwrap_or("") }],
                    })
                },
            }
        })
        .collect()
}

/// Parse tool calls from Gemini response parts.
fn parse_tool_calls(parts: &[serde_json::Value]) -> Vec<ToolCall> {
    parts
        .iter()
        .filter_map(|part| {
            if let Some(fc) = part.get("functionCall") {
                let name = fc["name"].as_str().unwrap_or("").to_string();
                let args = fc["args"].clone();
                Some(ToolCall {
                    id: name.clone(),
                    name,
                    arguments: args,
                })
            } else {
                None
            }
        })
        .collect()
}

/// Extract text content from Gemini response parts.
fn extract_text(parts: &[serde_json::Value]) -> Option<String> {
    let texts: Vec<&str> = parts
        .iter()
        .filter_map(|part| part["text"].as_str())
        .collect();

    if texts.is_empty() {
        None
    } else {
        Some(texts.join(""))
    }
}

// ── LlmProvider impl ────────────────────────────────────────────────────────

#[async_trait]
impl LlmProvider for GeminiOAuthProvider {
    fn name(&self) -> &str {
        PROVIDER_NAME
    }

    fn id(&self) -> &str {
        &self.model
    }

    fn supports_tools(&self) -> bool {
        true
    }

    fn context_window(&self) -> u32 {
        super::context_window_for_model(&self.model)
    }

    async fn complete(
        &self,
        messages: &[serde_json::Value],
        tools: &[serde_json::Value],
    ) -> anyhow::Result<CompletionResponse> {
        let token = self.get_valid_token().await?;

        let (system_text, conv_messages) = extract_system_instruction(messages);
        let gemini_messages = to_gemini_messages(&conv_messages);

        let mut body = serde_json::json!({
            "contents": gemini_messages,
            "generationConfig": {
                "maxOutputTokens": 8192,
            },
        });

        if let Some(ref sys) = system_text {
            body["systemInstruction"] = serde_json::json!({
                "parts": [{ "text": sys }]
            });
        }

        if !tools.is_empty() {
            body["tools"] = serde_json::Value::Array(vec![to_gemini_tools(tools)]);
        }

        debug!(
            model = %self.model,
            messages_count = gemini_messages.len(),
            tools_count = tools.len(),
            has_system = system_text.is_some(),
            "gemini-oauth complete request"
        );
        trace!(body = %serde_json::to_string(&body).unwrap_or_default(), "gemini-oauth request body");

        let url = format!(
            "{}/v1beta/models/{}:generateContent",
            GEMINI_API_BASE, self.model
        );

        let http_resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {token}"))
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = http_resp.status();
        if !status.is_success() {
            let body_text = http_resp.text().await.unwrap_or_default();
            warn!(status = %status, body = %body_text, "gemini-oauth API error");
            anyhow::bail!("Gemini OAuth API error HTTP {status}: {body_text}");
        }

        let resp = http_resp.json::<serde_json::Value>().await?;
        trace!(response = %resp, "gemini-oauth raw response");

        let parts = resp["candidates"][0]["content"]["parts"]
            .as_array()
            .cloned()
            .unwrap_or_default();

        let text = extract_text(&parts);
        let tool_calls = parse_tool_calls(&parts);

        let usage = Usage {
            input_tokens: resp["usageMetadata"]["promptTokenCount"]
                .as_u64()
                .unwrap_or(0) as u32,
            output_tokens: resp["usageMetadata"]["candidatesTokenCount"]
                .as_u64()
                .unwrap_or(0) as u32,
        };

        Ok(CompletionResponse {
            text,
            tool_calls,
            usage,
        })
    }

    #[allow(clippy::collapsible_if)]
    fn stream(
        &self,
        messages: Vec<serde_json::Value>,
    ) -> Pin<Box<dyn Stream<Item = StreamEvent> + Send + '_>> {
        Box::pin(async_stream::stream! {
            let token = match self.get_valid_token().await {
                Ok(t) => t,
                Err(e) => {
                    yield StreamEvent::Error(e.to_string());
                    return;
                }
            };

            let (system_text, conv_messages) = extract_system_instruction(&messages);
            let conv_refs: Vec<&serde_json::Value> = conv_messages.into_iter().collect();
            let gemini_messages = to_gemini_messages(&conv_refs);

            let mut body = serde_json::json!({
                "contents": gemini_messages,
                "generationConfig": {
                    "maxOutputTokens": 8192,
                },
            });

            if let Some(ref sys) = system_text {
                body["systemInstruction"] = serde_json::json!({
                    "parts": [{ "text": sys }]
                });
            }

            let url = format!(
                "{}/v1beta/models/{}:streamGenerateContent?alt=sse",
                GEMINI_API_BASE, self.model
            );

            let resp = match self
                .client
                .post(&url)
                .header("Authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .json(&body)
                .send()
                .await
            {
                Ok(r) => {
                    if let Err(e) = r.error_for_status_ref() {
                        let status = e.status().map(|s| s.as_u16()).unwrap_or(0);
                        let body_text = r.text().await.unwrap_or_default();
                        yield StreamEvent::Error(format!("HTTP {status}: {body_text}"));
                        return;
                    }
                    r
                }
                Err(e) => {
                    yield StreamEvent::Error(e.to_string());
                    return;
                }
            };

            let mut byte_stream = resp.bytes_stream();
            let mut buf = String::new();
            let mut input_tokens: u32 = 0;
            let mut output_tokens: u32 = 0;

            while let Some(chunk) = byte_stream.next().await {
                let chunk = match chunk {
                    Ok(c) => c,
                    Err(e) => {
                        yield StreamEvent::Error(e.to_string());
                        return;
                    }
                };
                buf.push_str(&String::from_utf8_lossy(&chunk));

                while let Some(pos) = buf.find("\n\n") {
                    let block = buf[..pos].to_string();
                    buf = buf[pos + 2..].to_string();

                    for line in block.lines() {
                        let Some(data) = line.strip_prefix("data: ") else {
                            continue;
                        };

                        if let Ok(evt) = serde_json::from_str::<serde_json::Value>(data) {
                            if let Some(usage) = evt.get("usageMetadata") {
                                if let Some(pt) = usage["promptTokenCount"].as_u64() {
                                    input_tokens = pt as u32;
                                }
                                if let Some(ct) = usage["candidatesTokenCount"].as_u64() {
                                    output_tokens = ct as u32;
                                }
                            }

                            if let Some(parts) = evt["candidates"][0]["content"]["parts"].as_array() {
                                for part in parts {
                                    if let Some(text) = part["text"].as_str() {
                                        if !text.is_empty() {
                                            yield StreamEvent::Delta(text.to_string());
                                        }
                                    }
                                }
                            }

                            if let Some(finish_reason) = evt["candidates"][0]["finishReason"].as_str() {
                                if finish_reason == "STOP" || finish_reason == "MAX_TOKENS" {
                                    yield StreamEvent::Done(Usage { input_tokens, output_tokens });
                                    return;
                                }
                            }
                        }
                    }
                }
            }

            yield StreamEvent::Done(Usage { input_tokens, output_tokens });
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn has_stored_tokens_returns_false_without_tokens() {
        // Just verify it doesn't panic
        let _ = has_stored_tokens();
    }

    #[test]
    fn gemini_oauth_models_not_empty() {
        assert!(!GEMINI_OAUTH_MODELS.is_empty());
    }

    #[test]
    fn gemini_oauth_models_have_unique_ids() {
        let mut ids: Vec<&str> = GEMINI_OAUTH_MODELS.iter().map(|(id, _)| *id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), GEMINI_OAUTH_MODELS.len());
    }

    #[test]
    fn provider_name_and_id() {
        let provider = GeminiOAuthProvider::new("gemini-2.0-flash".into());
        assert_eq!(provider.name(), "gemini-oauth");
        assert_eq!(provider.id(), "gemini-2.0-flash");
        assert!(provider.supports_tools());
    }

    #[test]
    fn to_gemini_tools_converts_correctly() {
        let tools = vec![serde_json::json!({
            "name": "test_tool",
            "description": "A test tool",
            "parameters": {"type": "object", "properties": {"x": {"type": "string"}}}
        })];
        let converted = to_gemini_tools(&tools);
        assert!(converted["functionDeclarations"].is_array());
        let decls = converted["functionDeclarations"].as_array().unwrap();
        assert_eq!(decls.len(), 1);
        assert_eq!(decls[0]["name"], "test_tool");
        assert_eq!(decls[0]["parameters"]["type"], "OBJECT");
    }

    #[test]
    fn convert_json_schema_types_works() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" }
            }
        });
        let converted = convert_json_schema_types(&schema);
        assert_eq!(converted["type"], "OBJECT");
        assert_eq!(converted["properties"]["name"]["type"], "STRING");
    }

    #[test]
    fn extract_system_instruction_works() {
        let messages = vec![
            serde_json::json!({ "role": "system", "content": "You are helpful" }),
            serde_json::json!({ "role": "user", "content": "Hello" }),
        ];
        let (system, remaining) = extract_system_instruction(&messages);
        assert_eq!(system, Some("You are helpful".to_string()));
        assert_eq!(remaining.len(), 1);
    }

    #[test]
    fn to_gemini_messages_converts_user() {
        let msg = serde_json::json!({ "role": "user", "content": "Hello" });
        let messages = vec![&msg];
        let gemini = to_gemini_messages(&messages);
        assert_eq!(gemini.len(), 1);
        assert_eq!(gemini[0]["role"], "user");
        assert_eq!(gemini[0]["parts"][0]["text"], "Hello");
    }

    #[test]
    fn to_gemini_messages_converts_assistant() {
        let msg = serde_json::json!({ "role": "assistant", "content": "Hi" });
        let messages = vec![&msg];
        let gemini = to_gemini_messages(&messages);
        assert_eq!(gemini[0]["role"], "model");
        assert_eq!(gemini[0]["parts"][0]["text"], "Hi");
    }

    #[test]
    fn parse_tool_calls_works() {
        let parts = vec![serde_json::json!({
            "functionCall": {
                "name": "get_weather",
                "args": { "city": "SF" }
            }
        })];
        let calls = parse_tool_calls(&parts);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "get_weather");
        assert_eq!(calls[0].arguments["city"], "SF");
    }

    #[test]
    fn extract_text_works() {
        let parts = vec![
            serde_json::json!({ "text": "Hello " }),
            serde_json::json!({ "text": "world" }),
        ];
        assert_eq!(extract_text(&parts), Some("Hello world".to_string()));
    }

    #[test]
    fn extract_text_empty() {
        let parts: Vec<serde_json::Value> = vec![];
        assert_eq!(extract_text(&parts), None);
    }

    #[test]
    fn context_window_uses_lookup() {
        let provider = GeminiOAuthProvider::new("gemini-2.0-flash".into());
        assert_eq!(provider.context_window(), 1_000_000);
    }
}
