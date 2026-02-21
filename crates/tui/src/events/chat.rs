use {
    crate::state::{AppState, ApprovalRequest, DisplayMessage, MessageRole, ToolCallCard},
    serde_json::Value,
    tracing::debug,
};

/// Handle a `chat` event payload.
pub fn handle_chat_event(state: &mut AppState, payload: &Value) {
    let Some(event_state) = payload.get("state").and_then(|v| v.as_str()) else {
        return;
    };
    let session_key = payload
        .get("sessionKey")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Only process events for the active session
    if !session_key.is_empty() && session_key != state.active_session {
        // Mark the session as replying in sidebar
        if let Some(entry) = state.sessions.iter_mut().find(|s| s.key == session_key) {
            entry.replying = matches!(
                event_state,
                "thinking" | "delta" | "tool_call_start" | "iteration"
            );
        }
        return;
    }

    match event_state {
        "thinking" => {
            let run_id = payload
                .get("runId")
                .and_then(|v| v.as_str())
                .map(String::from);
            state.active_run_id = run_id;
            state.thinking_active = true;
            state.thinking_text.clear();
            state.stream_buffer.clear();
            state.scroll_to_bottom();
            state.dirty = true;
        },
        "thinking_text" => {
            if let Some(text) = payload.get("text").and_then(|v| v.as_str()) {
                state.thinking_text.push_str(text);
                state.dirty = true;
            }
        },
        "thinking_done" => {
            state.thinking_active = false;
            state.dirty = true;
        },
        "delta" => {
            if let Some(text) = payload.get("text").and_then(|v| v.as_str()) {
                state.stream_buffer.push_str(text);
                state.thinking_active = false;
                state.dirty = true;
            }
        },
        "tool_call_start" => {
            let card = ToolCallCard {
                id: payload
                    .get("toolCallId")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .into(),
                name: payload
                    .get("toolName")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .into(),
                arguments: payload.get("arguments").cloned().unwrap_or(Value::Null),
                execution_mode: payload
                    .get("executionMode")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                success: None,
                result_summary: None,
            };

            // If we have accumulated text, finalize it as a message before the tool call
            if !state.stream_buffer.is_empty() {
                let content = std::mem::take(&mut state.stream_buffer);
                state.messages.push(DisplayMessage {
                    role: MessageRole::Assistant,
                    content,
                    tool_calls: Vec::new(),
                    thinking: None,
                });
            }

            // Add tool call to a new or last assistant message
            if let Some(last) = state
                .messages
                .last_mut()
                .filter(|m| m.role == MessageRole::Assistant)
            {
                last.tool_calls.push(card);
            } else {
                state.messages.push(DisplayMessage {
                    role: MessageRole::Assistant,
                    content: String::new(),
                    tool_calls: vec![card],
                    thinking: None,
                });
            }
            state.dirty = true;
        },
        "tool_call_end" => {
            let tool_id = payload
                .get("toolCallId")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let success = payload.get("success").and_then(|v| v.as_bool());
            let result_summary = payload
                .get("result")
                .and_then(|v| v.get("stdout").and_then(|s| s.as_str()))
                .or_else(|| {
                    payload
                        .get("error")
                        .and_then(|v| v.get("detail").and_then(|s| s.as_str()))
                })
                .map(String::from);

            // Find and update the tool call card
            for msg in state.messages.iter_mut().rev() {
                if let Some(card) = msg.tool_calls.iter_mut().find(|c| c.id == tool_id) {
                    card.success = success;
                    card.result_summary = result_summary;
                    break;
                }
            }
            state.dirty = true;
        },
        "iteration" => {
            debug!(
                iteration = payload.get("iteration").and_then(|v| v.as_u64()),
                "agent iteration"
            );
        },
        "sub_agent_start" => {
            debug!(
                task = payload.get("task").and_then(|v| v.as_str()),
                "sub-agent started"
            );
        },
        "sub_agent_end" => {
            debug!(
                task = payload.get("task").and_then(|v| v.as_str()),
                "sub-agent ended"
            );
        },
        "retrying" => {
            let retry_ms = payload
                .get("retryAfterMs")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let error_msg = payload
                .get("error")
                .and_then(|v| v.get("title").and_then(|t| t.as_str()))
                .unwrap_or("rate limited");
            state.messages.push(DisplayMessage {
                role: MessageRole::System,
                content: format!("Retrying in {:.1}s: {error_msg}", retry_ms as f64 / 1000.0),
                tool_calls: Vec::new(),
                thinking: None,
            });
            state.dirty = true;
        },
        "final" => {
            let text = payload
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned();
            let model = payload
                .get("model")
                .and_then(|v| v.as_str())
                .map(String::from);
            let provider = payload
                .get("provider")
                .and_then(|v| v.as_str())
                .map(String::from);

            // Update token counts
            if let Some(input) = payload.get("inputTokens").and_then(|v| v.as_u64()) {
                state.token_usage.session_input =
                    state.token_usage.session_input.saturating_add(input);
            }
            if let Some(output) = payload.get("outputTokens").and_then(|v| v.as_u64()) {
                state.token_usage.session_output =
                    state.token_usage.session_output.saturating_add(output);
            }

            state.finalize_stream(&text, model, provider);

            // Mark session as no longer replying
            if let Some(entry) = state.sessions.iter_mut().find(|s| s.key == session_key) {
                entry.replying = false;
            }
        },
        "error" => {
            let error_msg = payload
                .get("error")
                .and_then(|v| v.get("detail").or(v.get("title")))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown error");
            state.messages.push(DisplayMessage {
                role: MessageRole::System,
                content: format!("Error: {error_msg}"),
                tool_calls: Vec::new(),
                thinking: None,
            });
            state.active_run_id = None;
            state.thinking_active = false;
            state.stream_buffer.clear();

            if let Some(entry) = state.sessions.iter_mut().find(|s| s.key == session_key) {
                entry.replying = false;
            }
            state.dirty = true;
        },
        "session_cleared" => {
            if session_key == state.active_session {
                state.messages.clear();
                state.stream_buffer.clear();
                state.active_run_id = None;
                state.thinking_active = false;
                state.dirty = true;
            }
        },
        "notice" => {
            if let Some(message) = payload.get("message").and_then(|v| v.as_str()) {
                let title = payload
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Notice");
                state.messages.push(DisplayMessage {
                    role: MessageRole::System,
                    content: format!("{title}: {message}"),
                    tool_calls: Vec::new(),
                    thinking: None,
                });
                state.dirty = true;
            }
        },
        other => {
            debug!(state = other, "unhandled chat event state");
        },
    }
}

/// Handle an `exec.approval.requested` event.
pub fn handle_approval_requested(state: &mut AppState, payload: &Value) {
    let request_id = payload
        .get("requestId")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .into();
    let command = payload
        .get("command")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .into();
    state.pending_approval = Some(ApprovalRequest {
        request_id,
        command,
    });
    state.dirty = true;
}

/// Handle an `exec.approval.resolved` event.
pub fn handle_approval_resolved(state: &mut AppState, _payload: &Value) {
    state.pending_approval = None;
    state.dirty = true;
}
