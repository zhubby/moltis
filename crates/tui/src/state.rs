use serde_json::Value;

/// Vim-like input modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Insert,
    Command,
}

/// Which panel has focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    Chat,
    Sessions,
}

/// Role of a chat message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

/// A tool call displayed as a card in the chat.
#[derive(Debug, Clone)]
pub struct ToolCallCard {
    pub id: String,
    pub name: String,
    pub arguments: Value,
    pub execution_mode: Option<String>,
    pub success: Option<bool>,
    pub result_summary: Option<String>,
}

/// A pending approval request.
#[derive(Debug, Clone)]
pub struct ApprovalRequest {
    pub request_id: String,
    pub command: String,
}

/// A single message displayed in the chat view.
#[derive(Debug, Clone)]
pub struct DisplayMessage {
    pub role: MessageRole,
    pub content: String,
    pub tool_calls: Vec<ToolCallCard>,
    pub thinking: Option<String>,
}

/// Token usage tracking.
#[derive(Debug, Clone, Default)]
pub struct TokenUsage {
    pub session_input: u64,
    pub session_output: u64,
    pub context_window: u64,
}

/// Session entry for the sidebar.
#[derive(Debug, Clone)]
pub struct SessionEntry {
    pub key: String,
    pub label: Option<String>,
    pub model: Option<String>,
    pub message_count: u64,
    pub replying: bool,
}

impl SessionEntry {
    /// Display name: label if set, otherwise key.
    pub fn display_name(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.key)
    }
}

/// Full application state.
pub struct AppState {
    pub input_mode: InputMode,
    pub active_panel: Panel,
    pub messages: Vec<DisplayMessage>,
    pub stream_buffer: String,
    pub thinking_active: bool,
    pub thinking_text: String,
    pub active_run_id: Option<String>,
    pub scroll_offset: usize,
    pub sidebar_visible: bool,
    pub sessions: Vec<SessionEntry>,
    pub active_session: String,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub token_usage: TokenUsage,
    pub pending_approval: Option<ApprovalRequest>,
    pub command_buffer: String,
    pub dirty: bool,
    pub server_version: Option<String>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            input_mode: InputMode::Normal,
            active_panel: Panel::Chat,
            messages: Vec::new(),
            stream_buffer: String::new(),
            thinking_active: false,
            thinking_text: String::new(),
            active_run_id: None,
            scroll_offset: 0,
            sidebar_visible: true,
            sessions: Vec::new(),
            active_session: "main".into(),
            model: None,
            provider: None,
            token_usage: TokenUsage::default(),
            pending_approval: None,
            command_buffer: String::new(),
            dirty: true,
            server_version: None,
        }
    }
}

impl AppState {
    /// Whether the assistant is currently streaming a response.
    pub fn is_streaming(&self) -> bool {
        self.active_run_id.is_some()
    }

    /// Finalize the current stream: move stream_buffer into a message.
    pub fn finalize_stream(&mut self, text: &str, model: Option<String>, provider: Option<String>) {
        let content = if self.stream_buffer.is_empty() {
            text.to_owned()
        } else {
            std::mem::take(&mut self.stream_buffer)
        };

        let thinking = if self.thinking_text.is_empty() {
            None
        } else {
            Some(std::mem::take(&mut self.thinking_text))
        };

        self.messages.push(DisplayMessage {
            role: MessageRole::Assistant,
            content,
            tool_calls: Vec::new(),
            thinking,
        });

        self.active_run_id = None;
        self.thinking_active = false;

        if let Some(m) = model {
            self.model = Some(m);
        }
        if let Some(p) = provider {
            self.provider = Some(p);
        }
        self.dirty = true;
    }

    /// Add a user message to the history.
    pub fn add_user_message(&mut self, text: String) {
        self.messages.push(DisplayMessage {
            role: MessageRole::User,
            content: text,
            tool_calls: Vec::new(),
            thinking: None,
        });
        self.dirty = true;
    }

    /// Scroll chat messages up.
    pub fn scroll_up(&mut self, amount: usize) {
        self.scroll_offset = self.scroll_offset.saturating_add(amount);
        self.dirty = true;
    }

    /// Scroll chat messages down (towards newest).
    pub fn scroll_down(&mut self, amount: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(amount);
        self.dirty = true;
    }

    /// Scroll to the bottom (newest messages).
    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = 0;
        self.dirty = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state() {
        let state = AppState::default();
        assert_eq!(state.input_mode, InputMode::Normal);
        assert_eq!(state.active_panel, Panel::Chat);
        assert!(state.messages.is_empty());
        assert!(!state.is_streaming());
    }

    #[test]
    fn finalize_stream_creates_message() {
        let mut state = AppState {
            stream_buffer: "Hello world".into(),
            active_run_id: Some("run-1".into()),
            ..AppState::default()
        };

        state.finalize_stream("", Some("claude-3".into()), Some("anthropic".into()));

        assert_eq!(state.messages.len(), 1);
        assert_eq!(state.messages[0].content, "Hello world");
        assert_eq!(state.model.as_deref(), Some("claude-3"));
        assert!(!state.is_streaming());
    }

    #[test]
    fn scroll_bounds() {
        let mut state = AppState::default();
        state.scroll_down(10); // Can't go below 0
        assert_eq!(state.scroll_offset, 0);

        state.scroll_up(5);
        assert_eq!(state.scroll_offset, 5);

        state.scroll_to_bottom();
        assert_eq!(state.scroll_offset, 0);
    }

    #[test]
    fn session_display_name() {
        let s1 = SessionEntry {
            key: "main".into(),
            label: None,
            model: None,
            message_count: 0,
            replying: false,
        };
        assert_eq!(s1.display_name(), "main");

        let s2 = SessionEntry {
            key: "abc123".into(),
            label: Some("My Chat".into()),
            model: None,
            message_count: 5,
            replying: true,
        };
        assert_eq!(s2.display_name(), "My Chat");
    }
}
