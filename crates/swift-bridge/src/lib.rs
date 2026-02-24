//! C ABI bridge for embedding Moltis Rust functionality into native Swift apps.

#![allow(unsafe_code)]

use std::{
    ffi::{CStr, CString, c_char},
    panic::{AssertUnwindSafe, catch_unwind},
};

use {
    moltis_config::validate::Severity,
    serde::{Deserialize, Serialize},
};

#[derive(Debug, Deserialize)]
struct ChatRequest {
    message: String,
    #[serde(default)]
    config_toml: Option<String>,
}

#[derive(Debug, Serialize)]
struct ChatResponse {
    reply: String,
    config_dir: String,
    default_soul: String,
    validation: Option<ValidationSummary>,
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

fn build_chat_response(request: ChatRequest) -> String {
    let response = ChatResponse {
        reply: format!("Rust bridge received: {}", request.message),
        config_dir: config_dir_string(),
        default_soul: moltis_config::DEFAULT_SOUL.to_owned(),
        validation: build_validation_summary(request.config_toml.as_deref()),
    };
    encode_json(&response)
}

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

#[unsafe(no_mangle)]
/// # Safety
///
/// `ptr` must either be null or a pointer previously returned by
/// `moltis_version` or `moltis_chat_json` from this crate. Passing any other
/// pointer, or freeing the same pointer more than once, is undefined behavior.
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

        let reply = payload
            .get("reply")
            .and_then(Value::as_str)
            .unwrap_or_default();
        assert!(reply.contains("hello from swift"));

        let has_errors = payload
            .get("validation")
            .and_then(|value| value.get("has_errors"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        assert!(has_errors, "validation should detect invalid config value");
    }

    #[test]
    fn free_string_tolerates_null_pointer() {
        // SAFETY: null pointers are explicitly accepted and treated as no-op.
        unsafe {
            moltis_free_string(std::ptr::null_mut());
        }
    }
}
