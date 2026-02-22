use serde_json::Value;

/// Input modes for the TUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    /// Navigation mode: scrolling, switching tabs, quitting.
    Normal,
    /// Default typing mode: text input is active.
    Insert,
    /// Command-line mode (`:quit`, `:model`, etc.).
    Command,
}

/// Which panel has focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    Chat,
    Sessions,
}

/// Top-level tab navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MainTab {
    Chat,
    Settings,
    Projects,
    Crons,
}

/// Settings navigation sections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsSection {
    Identity,
    Providers,
    Voice,
    Channels,
    EnvVars,
    McpServers,
    Memory,
}

impl SettingsSection {
    pub const ALL: [Self; 7] = [
        Self::Identity,
        Self::Providers,
        Self::Voice,
        Self::Channels,
        Self::EnvVars,
        Self::McpServers,
        Self::Memory,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Identity => "Identity",
            Self::Providers => "Providers",
            Self::Voice => "Voice",
            Self::Channels => "Channels",
            Self::EnvVars => "Env Vars",
            Self::McpServers => "MCP Servers",
            Self::Memory => "Memory",
        }
    }
}

/// State for the Settings tab.
#[derive(Debug, Clone)]
pub struct SettingsState {
    pub active_section: SettingsSection,
    pub sections: Vec<SettingsSection>,
    #[allow(dead_code)] // Used when Settings tab loads data via RPC
    pub section_data: Option<Value>,
    #[allow(dead_code)] // Used when Settings form editing is implemented
    pub editing_field: Option<usize>,
}

impl Default for SettingsState {
    fn default() -> Self {
        Self {
            active_section: SettingsSection::Identity,
            sections: SettingsSection::ALL.to_vec(),
            section_data: None,
            editing_field: None,
        }
    }
}

/// Entry in the projects list.
#[derive(Debug, Clone)]
pub struct ProjectEntry {
    pub name: String,
    pub description: String,
    pub path: String,
    pub active: bool,
}

/// State for the Projects tab.
#[derive(Debug, Clone, Default)]
pub struct ProjectsState {
    pub projects: Vec<ProjectEntry>,
    pub selected: usize,
}

/// Entry in the cron jobs list.
#[derive(Debug, Clone)]
pub struct CronJobEntry {
    pub name: String,
    pub schedule: String,
    pub last_run: Option<String>,
    pub next_run: Option<String>,
    pub enabled: bool,
}

/// State for the Crons tab.
#[derive(Debug, Clone, Default)]
pub struct CronsState {
    pub jobs: Vec<CronJobEntry>,
    pub selected: usize,
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

/// Selectable model option for the session model switcher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelSwitchItem {
    pub provider_name: String,
    pub provider_display: String,
    pub model_id: String,
    pub model_display: String,
}

/// Modal state for provider/model switching with search.
#[derive(Debug, Clone, Default)]
pub struct ModelSwitcherState {
    pub query: String,
    pub selected: usize,
    pub items: Vec<ModelSwitchItem>,
    pub error_message: Option<String>,
}

impl ModelSwitcherState {
    #[must_use]
    pub fn filtered_indices(&self) -> Vec<usize> {
        let q = self.query.trim().to_lowercase();
        if q.is_empty() {
            return (0..self.items.len()).collect();
        }

        self.items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| {
                let provider = item.provider_display.to_lowercase();
                let model = item.model_display.to_lowercase();
                let model_id = item.model_id.to_lowercase();
                if provider.contains(&q) || model.contains(&q) || model_id.contains(&q) {
                    Some(index)
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn reset_selection_to_visible(&mut self) {
        let filtered = self.filtered_indices();
        if let Some(first) = filtered.first().copied() {
            self.selected = first;
        } else {
            self.selected = 0;
        }
    }
}

/// Full application state.
pub struct AppState {
    pub input_mode: InputMode,
    pub active_panel: Panel,
    pub active_tab: MainTab,
    pub messages: Vec<DisplayMessage>,
    pub stream_buffer: String,
    pub thinking_active: bool,
    pub thinking_text: String,
    pub active_run_id: Option<String>,
    pub scroll_offset: usize,
    pub sidebar_visible: bool,
    pub sessions: Vec<SessionEntry>,
    pub active_session: String,
    pub selected_session: usize,
    pub session_scroll_offset: usize,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub token_usage: TokenUsage,
    pub pending_approval: Option<ApprovalRequest>,
    pub command_buffer: String,
    pub dirty: bool,
    pub server_version: Option<String>,
    pub settings: SettingsState,
    pub projects: ProjectsState,
    pub crons: CronsState,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            input_mode: InputMode::Insert,
            active_panel: Panel::Chat,
            active_tab: MainTab::Chat,
            messages: Vec::new(),
            stream_buffer: String::new(),
            thinking_active: false,
            thinking_text: String::new(),
            active_run_id: None,
            scroll_offset: 0,
            sidebar_visible: true,
            sessions: Vec::new(),
            active_session: "main".into(),
            selected_session: 0,
            session_scroll_offset: 0,
            model: None,
            provider: None,
            token_usage: TokenUsage::default(),
            pending_approval: None,
            command_buffer: String::new(),
            dirty: true,
            server_version: None,
            settings: SettingsState::default(),
            projects: ProjectsState::default(),
            crons: CronsState::default(),
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
        assert_eq!(state.input_mode, InputMode::Insert);
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

    #[test]
    fn model_switcher_filters_by_query() {
        let mut switcher = ModelSwitcherState {
            query: "openai".into(),
            selected: 0,
            items: vec![
                ModelSwitchItem {
                    provider_name: "openai".into(),
                    provider_display: "OpenAI".into(),
                    model_id: "openai/gpt-5".into(),
                    model_display: "GPT-5".into(),
                },
                ModelSwitchItem {
                    provider_name: "anthropic".into(),
                    provider_display: "Anthropic".into(),
                    model_id: "anthropic/claude-sonnet-4".into(),
                    model_display: "Claude Sonnet 4".into(),
                },
            ],
            error_message: None,
        };

        let filtered = switcher.filtered_indices();
        assert_eq!(filtered, vec![0]);

        switcher.query = "claude".into();
        assert_eq!(switcher.filtered_indices(), vec![1]);

        switcher.query = "missing".into();
        assert!(switcher.filtered_indices().is_empty());
    }
}
