pub mod chat;

use {
    crate::state::{AppState, SessionEntry},
    serde_json::Value,
    tracing::debug,
};

/// Route a gateway event to the appropriate handler.
pub fn handle_event(state: &mut AppState, event_name: &str, payload: &Value) {
    match event_name {
        "chat" => chat::handle_chat_event(state, payload),
        "exec.approval.requested" => chat::handle_approval_requested(state, payload),
        "exec.approval.resolved" => chat::handle_approval_resolved(state, payload),
        "session" => handle_session_event(state, payload),
        "tick" | "presence" | "health" => {
            // System events — silently handled, no UI change needed yet
        },
        "shutdown" => {
            state.messages.push(crate::state::DisplayMessage {
                role: crate::state::MessageRole::System,
                content: "Server is shutting down.".into(),
                tool_calls: Vec::new(),
                thinking: None,
            });
            state.dirty = true;
        },
        other => {
            debug!(event = other, "unhandled event");
        },
    }
}

/// Handle a `session` event.
fn handle_session_event(state: &mut AppState, payload: &Value) {
    let kind = payload.get("kind").and_then(|v| v.as_str()).unwrap_or("");
    let session_key = payload
        .get("sessionKey")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    match kind {
        "patched" | "created" => {
            // Refresh will happen via sessions.list RPC — just mark dirty
            if !state.sessions.iter().any(|s| s.key == session_key) {
                state.sessions.push(SessionEntry {
                    key: session_key.into(),
                    label: None,
                    model: None,
                    message_count: 0,
                    replying: false,
                });
            }
            state.dirty = true;
        },
        "deleted" => {
            state.sessions.retain(|s| s.key != session_key);
            state.dirty = true;
        },
        _ => {},
    }
}
