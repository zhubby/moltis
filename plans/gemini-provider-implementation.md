# Google Gemini Provider Implementation

This document describes the implementation of Google Gemini providers for moltis, including both API key and OAuth authentication methods.

## Overview

Two Gemini providers were implemented:

| Provider | Auth Method | File | Feature Flag |
|----------|-------------|------|--------------|
| `gemini` | API Key | `gemini.rs` | Always enabled |
| `gemini-oauth` | OAuth 2.0 Device Flow | `gemini_oauth.rs` | `provider-gemini-oauth` |

Both providers support:
- Full tool/function calling
- Streaming responses via SSE
- System instructions
- 1M context window (all Gemini models)

## Architecture

### Provider Trait

All providers implement `LlmProvider` from `crates/agents/src/model.rs`:

```rust
#[async_trait]
pub trait LlmProvider: Send + Sync {
    fn name(&self) -> &str;           // Provider name (e.g., "gemini")
    fn id(&self) -> &str;             // Model ID (e.g., "gemini-2.0-flash")
    fn supports_tools(&self) -> bool; // Tool calling capability
    fn context_window(&self) -> u32;  // Context window size

    async fn complete(
        &self,
        messages: &[serde_json::Value],
        tools: &[serde_json::Value],
    ) -> anyhow::Result<CompletionResponse>;

    fn stream(
        &self,
        messages: Vec<serde_json::Value>,
    ) -> Pin<Box<dyn Stream<Item = StreamEvent> + Send + '_>>;
}
```

### File Structure

```
crates/agents/src/providers/
├── mod.rs              # Provider registry and registration
├── gemini.rs           # API key provider (new)
├── gemini_oauth.rs     # OAuth provider (new)
├── anthropic.rs        # Reference implementation
├── openai.rs           # Reference implementation
├── github_copilot.rs   # OAuth reference
└── ...
```

## API Key Provider (`gemini.rs`)

### Configuration

```bash
# Environment variable
export GEMINI_API_KEY="your-api-key"

# Or in moltis.toml
[providers.gemini]
api_key = "your-api-key"
model = "gemini-2.0-flash"  # Optional, defaults to all models
base_url = "https://generativelanguage.googleapis.com"  # Optional
```

### Implementation Details

#### Struct Definition

```rust
pub struct GeminiProvider {
    api_key: secrecy::Secret<String>,  // Wrapped for security
    model: String,
    base_url: String,
    client: reqwest::Client,
}
```

#### Message Format Conversion

Gemini uses a different message format than OpenAI/Anthropic:

| Generic Format | Gemini Format |
|----------------|---------------|
| `role: "system"` | `systemInstruction: { parts: [{ text }] }` |
| `role: "user"` | `role: "user", parts: [{ text }]` |
| `role: "assistant"` | `role: "model", parts: [{ text }]` |
| `role: "tool"` | `role: "user", parts: [{ functionResponse }]` |

**System Instructions**: Extracted from messages and placed in top-level `systemInstruction` field.

**Tool Calls**: Assistant messages with `tool_calls` are converted to:
```json
{
  "role": "model",
  "parts": [
    { "functionCall": { "name": "...", "args": {...} } }
  ]
}
```

**Tool Results**: Tool messages are converted to:
```json
{
  "role": "user",
  "parts": [{
    "functionResponse": {
      "name": "tool_name",
      "response": { /* parsed JSON or wrapped text */ }
    }
  }]
}
```

#### Tool Schema Conversion

Gemini requires uppercase type names in JSON Schema:

```rust
fn convert_json_schema_types(schema: &Value) -> Value {
    // Recursively converts:
    // "type": "object" → "type": "OBJECT"
    // "type": "string" → "type": "STRING"
    // etc.
}
```

Tool schemas are wrapped in `functionDeclarations`:
```json
{
  "tools": [{
    "functionDeclarations": [{
      "name": "get_weather",
      "description": "...",
      "parameters": { "type": "OBJECT", ... }
    }]
  }]
}
```

#### API Endpoints

- **Non-streaming**: `POST /v1beta/models/{model}:generateContent`
- **Streaming**: `POST /v1beta/models/{model}:streamGenerateContent?alt=sse`

#### Authentication

```rust
.header("x-goog-api-key", self.api_key.expose_secret())
```

#### Streaming Implementation

Uses `async_stream::stream!` macro with SSE parsing:

```rust
fn stream(&self, messages: Vec<Value>) -> Pin<Box<dyn Stream<Item = StreamEvent>>> {
    Box::pin(async_stream::stream! {
        // 1. Make HTTP request with ?alt=sse
        // 2. Parse SSE events (data: {...}\n\n)
        // 3. Yield StreamEvent::Delta for text chunks
        // 4. Track usage from usageMetadata
        // 5. Yield StreamEvent::Done on finishReason: "STOP"
    })
}
```

## OAuth Provider (`gemini_oauth.rs`)

### Configuration

```bash
# Required: Google Cloud OAuth client ID
export GOOGLE_CLIENT_ID="your-client-id.apps.googleusercontent.com"

# Optional: Client secret (for confidential clients)
export GOOGLE_CLIENT_SECRET="your-secret"
```

### Setup Steps

1. Go to [Google Cloud Console](https://console.cloud.google.com)
2. Create a new project or select existing
3. Enable "Generative Language API"
4. Go to APIs & Services → Credentials
5. Create OAuth 2.0 Client ID (Desktop app type)
6. Copy the client ID

### OAuth Device Flow

The device flow allows authentication without a browser redirect:

```
┌──────────────┐                              ┌─────────────────┐
│   moltis     │                              │  Google OAuth   │
└──────┬───────┘                              └────────┬────────┘
       │                                               │
       │  POST /device/code                            │
       │  {client_id, scope}                           │
       │──────────────────────────────────────────────>│
       │                                               │
       │  {device_code, user_code, verification_url}   │
       │<──────────────────────────────────────────────│
       │                                               │
       │  Display to user:                             │
       │  "Go to {url} and enter code: {user_code}"    │
       │                                               │
       │  Poll: POST /token                            │
       │  {client_id, device_code, grant_type}         │
       │──────────────────────────────────────────────>│
       │                                               │
       │  (user enters code in browser)                │
       │                                               │
       │  {access_token, refresh_token, expires_in}    │
       │<──────────────────────────────────────────────│
       │                                               │
       │  Store tokens in ~/.config/moltis/            │
       │  oauth_tokens.json                            │
       │                                               │
```

### Implementation Details

#### Token Management

```rust
pub struct GeminiOAuthProvider {
    model: String,
    client: reqwest::Client,
    token_store: TokenStore,  // From moltis-oauth crate
}
```

**Token Storage**: `~/.config/moltis/oauth_tokens.json`
```json
{
  "gemini-oauth": {
    "access_token": "ya29...",
    "refresh_token": "1//...",
    "expires_at": 1707123456
  }
}
```

#### Token Refresh

Tokens are automatically refreshed when expiring (5 minute buffer):

```rust
const REFRESH_THRESHOLD_SECS: u64 = 300;

async fn get_valid_token(&self) -> Result<String> {
    let tokens = self.token_store.load(PROVIDER_NAME)?;

    if let Some(expires_at) = tokens.expires_at {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

        if now + REFRESH_THRESHOLD_SECS >= expires_at {
            // Token expiring soon — refresh it
            let new_tokens = self.refresh_access_token(
                tokens.refresh_token.expose_secret()
            ).await?;
            self.token_store.save(PROVIDER_NAME, &new_tokens)?;
            return Ok(new_tokens.access_token.expose_secret().clone());
        }
    }

    Ok(tokens.access_token.expose_secret().clone())
}
```

#### API Authentication

Unlike API key auth, OAuth uses Bearer token:

```rust
.header("Authorization", format!("Bearer {token}"))
```

## Provider Registration

### mod.rs Registration

Providers are registered in `ProviderRegistry::from_env_with_config()`:

```rust
// Built-in providers (always available)
reg.register_builtin_providers(config);  // Includes gemini with API key

// Feature-gated OAuth providers
#[cfg(feature = "provider-gemini-oauth")]
{
    reg.register_gemini_oauth_providers(config);
}
```

### Registration Logic

```rust
fn register_gemini_oauth_providers(&mut self, config: &ProvidersConfig) {
    // 1. Check if provider is enabled in config
    if !config.is_enabled("gemini-oauth") { return; }

    // 2. Require GOOGLE_CLIENT_ID
    if GeminiOAuthProvider::get_client_id().is_none() { return; }

    // 3. Only register if tokens exist (user has authenticated)
    if !has_stored_tokens() { return; }

    // 4. Register all models or specific configured model
    for (model_id, display_name) in GEMINI_OAUTH_MODELS {
        let provider = Arc::new(GeminiOAuthProvider::new(model_id.into()));
        self.register(ModelInfo { ... }, provider);
    }
}
```

## Supported Models

Both providers support the same models:

| Model ID | Display Name | Context Window |
|----------|--------------|----------------|
| `gemini-2.5-pro-preview-06-05` | Gemini 2.5 Pro | 1M |
| `gemini-2.5-flash-preview-05-20` | Gemini 2.5 Flash | 1M |
| `gemini-2.0-flash` | Gemini 2.0 Flash | 1M |
| `gemini-2.0-flash-lite` | Gemini 2.0 Flash Lite | 1M |
| `gemini-1.5-pro` | Gemini 1.5 Pro | 1M |
| `gemini-1.5-flash` | Gemini 1.5 Flash | 1M |

## Testing

### Unit Tests

Both providers have comprehensive tests in their respective files:

```bash
# Run all Gemini tests
cargo test --all-features -p moltis-agents providers::gemini

# Run OAuth-specific tests
cargo test --all-features -p moltis-agents providers::gemini_oauth
```

### Test Coverage

- Message format conversion (user, assistant, tool messages)
- Tool schema conversion (JSON Schema type uppercasing)
- Tool call parsing from responses
- System instruction extraction
- Provider metadata (name, id, supports_tools)
- Context window lookup
- Registration with config

## Security Considerations

1. **API Keys**: Wrapped in `secrecy::Secret<String>` to prevent accidental logging
2. **OAuth Tokens**: Stored with 0600 permissions on Unix
3. **Token Refresh**: Automatic refresh prevents token expiry issues
4. **No Hardcoded Credentials**: All credentials come from environment or config

## Comparison with Other Providers

| Feature | Anthropic | OpenAI | Gemini | Gemini OAuth |
|---------|-----------|--------|--------|--------------|
| Auth | API Key | API Key | API Key | OAuth 2.0 |
| Tools | ✅ | ✅ | ✅ | ✅ |
| Streaming | ✅ | ✅ | ✅ | ✅ |
| System Prompt | Top-level | In messages | `systemInstruction` | `systemInstruction` |
| Message Format | Custom | OpenAI | Custom | Custom |
| Context Window | 200k | 128k | 1M | 1M |

## Future Improvements

1. **Vertex AI Support**: Add support for Google Cloud's Vertex AI endpoint
2. **Dynamic Model List**: Query `/models` endpoint at startup
3. **Multimodal Support**: Add image/audio input support
4. **Caching**: Implement context caching for repeated prompts
5. **Code Execution**: Support Gemini's code execution tool
