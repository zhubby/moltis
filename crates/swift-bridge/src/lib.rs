//! C ABI bridge for embedding Moltis Rust functionality into native Swift apps.

#![allow(unsafe_code)]

use std::{
    collections::HashMap,
    ffi::{CStr, CString, c_char},
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{LazyLock, RwLock},
};

use {
    moltis_agents::model::{ChatMessage as AgentChatMessage, LlmProvider, UserContent},
    moltis_config::{schema::ProvidersConfig, validate::Severity},
    moltis_provider_setup::{
        KeyStore, config_with_saved_keys, detect_auto_provider_sources_with_overrides,
        known_providers,
    },
    moltis_providers::ProviderRegistry,
    serde::{Deserialize, Serialize},
};

// ── Global bridge state ────────────────────────────────────────────────────

struct BridgeState {
    runtime: tokio::runtime::Runtime,
    registry: RwLock<ProviderRegistry>,
}

impl BridgeState {
    fn new() -> Self {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap_or_else(|e| panic!("failed to create tokio runtime: {e}"));

        let registry = build_registry();
        Self {
            runtime,
            registry: RwLock::new(registry),
        }
    }
}

fn build_registry() -> ProviderRegistry {
    let base = ProvidersConfig::default();
    let key_store = KeyStore::new();
    let config = config_with_saved_keys(&base, &key_store, &[]);
    ProviderRegistry::from_env_with_config(&config)
}

static BRIDGE: LazyLock<BridgeState> = LazyLock::new(BridgeState::new);

// ── Request / Response types ───────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ChatRequest {
    message: String,
    #[serde(default)]
    model: Option<String>,
    /// Reserved for future provider-hint resolution; deserialized so Swift
    /// can pass it but not yet used for routing.
    #[serde(default)]
    #[allow(dead_code)]
    provider: Option<String>,
    #[serde(default)]
    config_toml: Option<String>,
}

#[derive(Debug, Serialize)]
struct ChatResponse {
    reply: String,
    model: Option<String>,
    provider: Option<String>,
    config_dir: String,
    default_soul: String,
    validation: Option<ValidationSummary>,
    input_tokens: Option<u32>,
    output_tokens: Option<u32>,
    duration_ms: Option<u64>,
}

#[derive(Debug, Serialize)]
struct ValidationSummary {
    errors: usize,
    warnings: usize,
    info: usize,
    has_errors: bool,
}

#[derive(Debug, Serialize)]
struct VersionResponse {
    bridge_version: &'static str,
    moltis_version: &'static str,
    config_dir: String,
}

#[derive(Debug, Serialize)]
struct ErrorEnvelope<'a> {
    error: ErrorPayload<'a>,
}

#[derive(Debug, Serialize)]
struct ErrorPayload<'a> {
    code: &'a str,
    message: &'a str,
}

// ── Bridge serde types for provider data ───────────────────────────────────

#[derive(Debug, Serialize)]
struct BridgeKnownProvider {
    name: &'static str,
    display_name: &'static str,
    auth_type: &'static str,
    env_key: Option<&'static str>,
    default_base_url: Option<&'static str>,
    requires_model: bool,
    key_optional: bool,
}

#[derive(Debug, Serialize)]
struct BridgeDetectedSource {
    provider: String,
    source: String,
}

#[derive(Debug, Serialize)]
struct BridgeModelInfo {
    id: String,
    provider: String,
    display_name: String,
    created_at: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct SaveProviderRequest {
    provider: String,
    #[serde(default)]
    api_key: Option<String>,
    #[serde(default)]
    base_url: Option<String>,
    #[serde(default)]
    models: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
struct OkResponse {
    ok: bool,
}

// ── Encoding helpers ───────────────────────────────────────────────────────

fn encode_json<T: Serialize>(value: &T) -> String {
    match serde_json::to_string(value) {
        Ok(json) => json,
        Err(_) => {
            "{\"error\":{\"code\":\"serialization_error\",\"message\":\"failed to serialize response\"}}"
                .to_owned()
        }
    }
}

fn encode_error(code: &str, message: &str) -> String {
    encode_json(&ErrorEnvelope {
        error: ErrorPayload { code, message },
    })
}

fn into_c_ptr(payload: String) -> *mut c_char {
    match CString::new(payload) {
        Ok(value) => value.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

fn with_ffi_boundary<F>(work: F) -> *mut c_char
where
    F: FnOnce() -> String,
{
    match catch_unwind(AssertUnwindSafe(work)) {
        Ok(payload) => into_c_ptr(payload),
        Err(_) => into_c_ptr(encode_error(
            "panic",
            "unexpected panic occurred in Rust FFI boundary",
        )),
    }
}

fn read_c_string(ptr: *const c_char) -> Result<String, String> {
    if ptr.is_null() {
        return Err("request_json pointer was null".to_owned());
    }

    // SAFETY: pointer nullability is checked above, and callers guarantee a
    // valid NUL-terminated C string for the duration of the call.
    let c_str = unsafe { CStr::from_ptr(ptr) };
    match c_str.to_str() {
        Ok(text) => Ok(text.to_owned()),
        Err(_) => Err("request_json was not valid UTF-8".to_owned()),
    }
}

fn build_validation_summary(config_toml: Option<&str>) -> Option<ValidationSummary> {
    let config_toml = config_toml?;
    let result = moltis_config::validate::validate_toml_str(config_toml);

    Some(ValidationSummary {
        errors: result.count(Severity::Error),
        warnings: result.count(Severity::Warning),
        info: result.count(Severity::Info),
        has_errors: result.has_errors(),
    })
}

fn config_dir_string() -> String {
    match moltis_config::config_dir() {
        Some(path) => path.display().to_string(),
        None => "unavailable".to_owned(),
    }
}

// ── Chat with real LLM ────────────────────────────────────────────────────

fn resolve_provider(request: &ChatRequest) -> Option<std::sync::Arc<dyn LlmProvider>> {
    let registry = BRIDGE.registry.read().unwrap_or_else(|e| e.into_inner());

    // Try explicit model first
    if let Some(model_id) = &request.model {
        if let Some(provider) = registry.get(model_id) {
            return Some(provider);
        }
    }

    // Fall back to first available provider
    registry.first()
}

fn build_chat_response(request: ChatRequest) -> String {
    let validation = build_validation_summary(request.config_toml.as_deref());

    let (reply, model, provider_name, input_tokens, output_tokens, duration_ms) =
        match resolve_provider(&request) {
            Some(provider) => {
                let model_id = provider.id().to_string();
                let provider_name = provider.name().to_string();
                let messages = vec![AgentChatMessage::User {
                    content: UserContent::text(&request.message),
                }];

                let start = std::time::Instant::now();
                match BRIDGE.runtime.block_on(provider.complete(&messages, &[])) {
                    Ok(response) => {
                        let elapsed = start.elapsed().as_millis() as u64;
                        let text = response
                            .text
                            .unwrap_or_else(|| "(empty response)".to_owned());
                        let in_tok = response.usage.input_tokens;
                        let out_tok = response.usage.output_tokens;
                        (
                            text,
                            Some(model_id),
                            Some(provider_name),
                            Some(in_tok),
                            Some(out_tok),
                            Some(elapsed),
                        )
                    },
                    Err(error) => {
                        let msg = format!("LLM error: {error}");
                        (msg, Some(model_id), Some(provider_name), None, None, None)
                    },
                }
            },
            None => {
                let msg = format!(
                    "No LLM provider configured. Rust bridge received: {}",
                    request.message
                );
                (msg, None, None, None, None, None)
            },
        };

    let response = ChatResponse {
        reply,
        model,
        provider: provider_name,
        config_dir: config_dir_string(),
        default_soul: moltis_config::DEFAULT_SOUL.to_owned(),
        validation,
        input_tokens,
        output_tokens,
        duration_ms,
    };
    encode_json(&response)
}

// ── Metrics / tracing helpers ──────────────────────────────────────────────

#[cfg(feature = "metrics")]
fn record_call(function: &'static str) {
    metrics::counter!("moltis_swift_bridge_calls_total", "function" => function).increment(1);
}

#[cfg(not(feature = "metrics"))]
fn record_call(_function: &'static str) {}

#[cfg(feature = "metrics")]
fn record_error(function: &'static str, code: &'static str) {
    metrics::counter!(
        "moltis_swift_bridge_errors_total",
        "function" => function,
        "code" => code
    )
    .increment(1);
}

#[cfg(not(feature = "metrics"))]
fn record_error(_function: &'static str, _code: &'static str) {}

#[cfg(feature = "tracing")]
fn trace_call(function: &'static str) {
    tracing::debug!(target: "moltis_swift_bridge", function, "ffi call");
}

#[cfg(not(feature = "tracing"))]
fn trace_call(_function: &'static str) {}

// ── FFI exports ────────────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn moltis_version() -> *mut c_char {
    record_call("moltis_version");
    trace_call("moltis_version");

    with_ffi_boundary(|| {
        let response = VersionResponse {
            bridge_version: env!("CARGO_PKG_VERSION"),
            moltis_version: env!("CARGO_PKG_VERSION"),
            config_dir: config_dir_string(),
        };
        encode_json(&response)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn moltis_chat_json(request_json: *const c_char) -> *mut c_char {
    record_call("moltis_chat_json");
    trace_call("moltis_chat_json");

    with_ffi_boundary(|| {
        let raw = match read_c_string(request_json) {
            Ok(value) => value,
            Err(message) => {
                record_error("moltis_chat_json", "null_pointer_or_invalid_utf8");
                return encode_error("null_pointer_or_invalid_utf8", &message);
            },
        };

        let request = match serde_json::from_str::<ChatRequest>(&raw) {
            Ok(request) => request,
            Err(error) => {
                record_error("moltis_chat_json", "invalid_json");
                return encode_error("invalid_json", &error.to_string());
            },
        };

        build_chat_response(request)
    })
}

/// Returns JSON array of all known providers.
#[unsafe(no_mangle)]
pub extern "C" fn moltis_known_providers() -> *mut c_char {
    record_call("moltis_known_providers");
    trace_call("moltis_known_providers");

    with_ffi_boundary(|| {
        let providers: Vec<BridgeKnownProvider> = known_providers()
            .into_iter()
            .map(|p| BridgeKnownProvider {
                name: p.name,
                display_name: p.display_name,
                auth_type: p.auth_type,
                env_key: p.env_key,
                default_base_url: p.default_base_url,
                requires_model: p.requires_model,
                key_optional: p.key_optional,
            })
            .collect();
        encode_json(&providers)
    })
}

/// Returns JSON array of auto-detected provider sources.
#[unsafe(no_mangle)]
pub extern "C" fn moltis_detect_providers() -> *mut c_char {
    record_call("moltis_detect_providers");
    trace_call("moltis_detect_providers");

    with_ffi_boundary(|| {
        let config = ProvidersConfig::default();
        let env_overrides = HashMap::new();
        let sources = detect_auto_provider_sources_with_overrides(&config, None, &env_overrides);
        let bridge_sources: Vec<BridgeDetectedSource> = sources
            .into_iter()
            .map(|s| BridgeDetectedSource {
                provider: s.provider,
                source: s.source,
            })
            .collect();
        encode_json(&bridge_sources)
    })
}

/// Saves provider configuration (API key, base URL, models).
#[unsafe(no_mangle)]
pub extern "C" fn moltis_save_provider_config(request_json: *const c_char) -> *mut c_char {
    record_call("moltis_save_provider_config");
    trace_call("moltis_save_provider_config");

    with_ffi_boundary(|| {
        let raw = match read_c_string(request_json) {
            Ok(value) => value,
            Err(message) => {
                record_error(
                    "moltis_save_provider_config",
                    "null_pointer_or_invalid_utf8",
                );
                return encode_error("null_pointer_or_invalid_utf8", &message);
            },
        };

        let request = match serde_json::from_str::<SaveProviderRequest>(&raw) {
            Ok(request) => request,
            Err(error) => {
                record_error("moltis_save_provider_config", "invalid_json");
                return encode_error("invalid_json", &error.to_string());
            },
        };

        let key_store = KeyStore::new();
        match key_store.save_config(
            &request.provider,
            request.api_key,
            request.base_url,
            request.models,
        ) {
            Ok(()) => encode_json(&OkResponse { ok: true }),
            Err(error) => encode_error("save_failed", &error),
        }
    })
}

/// Lists all discovered models from the current provider registry.
#[unsafe(no_mangle)]
pub extern "C" fn moltis_list_models() -> *mut c_char {
    record_call("moltis_list_models");
    trace_call("moltis_list_models");

    with_ffi_boundary(|| {
        let registry = BRIDGE.registry.read().unwrap_or_else(|e| e.into_inner());
        let models: Vec<BridgeModelInfo> = registry
            .list_models()
            .iter()
            .map(|m| BridgeModelInfo {
                id: m.id.clone(),
                provider: m.provider.clone(),
                display_name: m.display_name.clone(),
                created_at: m.created_at,
            })
            .collect();
        encode_json(&models)
    })
}

/// Rebuilds the global provider registry from saved config + env.
#[unsafe(no_mangle)]
pub extern "C" fn moltis_refresh_registry() -> *mut c_char {
    record_call("moltis_refresh_registry");
    trace_call("moltis_refresh_registry");

    with_ffi_boundary(|| {
        let new_registry = build_registry();
        let mut guard = BRIDGE.registry.write().unwrap_or_else(|e| e.into_inner());
        *guard = new_registry;
        encode_json(&OkResponse { ok: true })
    })
}

#[unsafe(no_mangle)]
/// # Safety
///
/// `ptr` must either be null or a pointer previously returned by one of the
/// `moltis_*` FFI functions from this crate. Passing any other pointer, or
/// freeing the same pointer more than once, is undefined behavior.
pub unsafe extern "C" fn moltis_free_string(ptr: *mut c_char) {
    record_call("moltis_free_string");

    if ptr.is_null() {
        return;
    }

    // SAFETY: pointer must originate from `CString::into_raw` in this crate.
    let _ = unsafe { CString::from_raw(ptr) };
}

#[unsafe(no_mangle)]
pub extern "C" fn moltis_shutdown() {
    record_call("moltis_shutdown");
    trace_call("moltis_shutdown");
}

#[cfg(test)]
mod tests {
    use {super::*, serde_json::Value};

    fn text_from_ptr(ptr: *mut c_char) -> String {
        assert!(!ptr.is_null(), "ffi returned null pointer");

        // SAFETY: pointer returned by this crate, converted back exactly once.
        let owned = unsafe { CString::from_raw(ptr) };

        match owned.into_string() {
            Ok(text) => text,
            Err(error) => panic!("failed to decode UTF-8 from ffi pointer: {error}"),
        }
    }

    fn json_from_ptr(ptr: *mut c_char) -> Value {
        let text = text_from_ptr(ptr);
        match serde_json::from_str::<Value>(&text) {
            Ok(value) => value,
            Err(error) => panic!("failed to parse ffi json payload: {error}; payload={text}"),
        }
    }

    #[test]
    fn version_returns_expected_payload() {
        let payload = json_from_ptr(moltis_version());

        let version = payload
            .get("bridge_version")
            .and_then(Value::as_str)
            .unwrap_or_default();
        assert_eq!(version, env!("CARGO_PKG_VERSION"));

        let config_dir = payload
            .get("config_dir")
            .and_then(Value::as_str)
            .unwrap_or_default();
        assert!(!config_dir.is_empty(), "config_dir should be populated");
    }

    #[test]
    fn chat_returns_error_for_null_pointer() {
        let payload = json_from_ptr(moltis_chat_json(std::ptr::null()));

        let code = payload
            .get("error")
            .and_then(|value| value.get("code"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        assert_eq!(code, "null_pointer_or_invalid_utf8");
    }

    #[test]
    fn chat_returns_validation_counts() {
        let request =
            r#"{"message":"hello from swift","config_toml":"[server]\nport = \"invalid\""}"#;
        let c_request = match CString::new(request) {
            Ok(value) => value,
            Err(error) => panic!("failed to build c string for test request: {error}"),
        };

        let payload = json_from_ptr(moltis_chat_json(c_request.as_ptr()));

        // Chat response should have a reply (either from LLM or fallback)
        assert!(
            payload.get("reply").and_then(Value::as_str).is_some(),
            "response should contain a reply field"
        );

        let has_errors = payload
            .get("validation")
            .and_then(|value| value.get("has_errors"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        assert!(has_errors, "validation should detect invalid config value");
    }

    #[test]
    fn known_providers_returns_array() {
        let payload = json_from_ptr(moltis_known_providers());

        let providers = payload.as_array();
        assert!(
            providers.is_some(),
            "known_providers should return a JSON array"
        );
        let providers = providers.unwrap_or_else(|| panic!("not an array"));
        assert!(!providers.is_empty(), "should have at least one provider");

        // Check first provider has expected fields
        let first = &providers[0];
        assert!(first.get("name").and_then(Value::as_str).is_some());
        assert!(first.get("display_name").and_then(Value::as_str).is_some());
        assert!(first.get("auth_type").and_then(Value::as_str).is_some());
    }

    #[test]
    fn detect_providers_returns_array() {
        let payload = json_from_ptr(moltis_detect_providers());

        // Should always return a JSON array (possibly empty)
        assert!(
            payload.as_array().is_some(),
            "detect_providers should return a JSON array"
        );
    }

    #[test]
    fn save_provider_config_returns_error_for_null() {
        let payload = json_from_ptr(moltis_save_provider_config(std::ptr::null()));

        let code = payload
            .get("error")
            .and_then(|value| value.get("code"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        assert_eq!(code, "null_pointer_or_invalid_utf8");
    }

    #[test]
    fn list_models_returns_array() {
        let payload = json_from_ptr(moltis_list_models());

        assert!(
            payload.as_array().is_some(),
            "list_models should return a JSON array"
        );
    }

    #[test]
    fn refresh_registry_returns_ok() {
        let payload = json_from_ptr(moltis_refresh_registry());

        let ok = payload.get("ok").and_then(Value::as_bool).unwrap_or(false);
        assert!(ok, "refresh_registry should return ok: true");
    }

    #[test]
    fn free_string_tolerates_null_pointer() {
        // SAFETY: null pointers are explicitly accepted and treated as no-op.
        unsafe {
            moltis_free_string(std::ptr::null_mut());
        }
    }
}
