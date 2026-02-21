mod onboarding;

use {
    crate::{
        Error,
        connection::{ConnectionEvent, ConnectionManager},
        events,
        onboarding::OnboardingState,
        rpc::RpcClient,
        state::{
            AppState, DisplayMessage, InputMode, MessageRole, Panel, SessionEntry, TokenUsage,
        },
        ui::{self, status_bar::ConnectionDisplay, theme::Theme},
    },
    crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyModifiers},
    futures::StreamExt,
    moltis_protocol::ConnectAuth,
    ratatui::DefaultTerminal,
    std::{sync::Arc, time::Duration},
    tokio::sync::mpsc,
    tracing::{debug, warn},
    tui_textarea::TextArea,
};

/// Events that drive the application state machine.
#[derive(Debug)]
pub enum AppEvent {
    /// Terminal key press.
    Key(KeyEvent),
    /// Terminal resize or focus-regained — forces a full redraw.
    Redraw,
    /// Periodic tick for animations/status updates.
    Tick,
    /// Connection lifecycle event.
    Connection(ConnectionEvent),
    /// Initial data loaded from gateway (non-blocking).
    InitialData(InitialData),
}

/// Data loaded from the gateway after a successful connection.
#[derive(Debug, Default)]
pub struct InitialData {
    pub sessions: Option<Vec<SessionEntry>>,
    pub messages: Option<Vec<DisplayMessage>>,
    pub active_session: Option<String>,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub token_usage: Option<TokenUsage>,
}

/// Top-level application.
pub struct App {
    state: AppState,
    onboarding: Option<OnboardingState>,
    onboarding_check_pending: bool,
    connection_display: ConnectionDisplay,
    connection: Option<Arc<ConnectionManager>>,
    should_quit: bool,
    url: String,
    auth: ConnectAuth,
    theme: Theme,
}

impl App {
    pub fn new(url: String, auth: ConnectAuth) -> Self {
        Self {
            state: AppState::default(),
            onboarding: None,
            onboarding_check_pending: true,
            connection_display: ConnectionDisplay::Connecting,
            connection: None,
            should_quit: false,
            url,
            auth,
            theme: Theme::default(),
        }
    }

    /// Main event loop: reads terminal events, dispatches, and re-renders.
    pub async fn run(mut self, mut terminal: DefaultTerminal) -> Result<(), Error> {
        let (event_tx, mut event_rx) = mpsc::unbounded_channel::<AppEvent>();

        // Spawn terminal event reader
        let term_tx = event_tx.clone();
        tokio::spawn(async move {
            let mut reader = EventStream::new();
            while let Some(Ok(event)) = reader.next().await {
                let app_event = match event {
                    Event::Key(key) => AppEvent::Key(key),
                    Event::Resize(..) | Event::FocusGained => AppEvent::Redraw,
                    _ => continue,
                };
                if term_tx.send(app_event).is_err() {
                    break;
                }
            }
        });

        // Spawn tick timer (60ms for smooth streaming)
        let tick_tx = event_tx.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(60));
            loop {
                interval.tick().await;
                if tick_tx.send(AppEvent::Tick).is_err() {
                    break;
                }
            }
        });

        // Spawn connection manager
        let (conn_event_tx, mut conn_event_rx) = mpsc::unbounded_channel::<ConnectionEvent>();
        let connection = Arc::new(ConnectionManager::spawn(
            self.url.clone(),
            self.auth.clone(),
            conn_event_tx,
        ));
        let rpc = Arc::new(RpcClient::new(Arc::clone(&connection)));
        self.connection = Some(Arc::clone(&connection));

        // Forward connection events to main event loop
        let conn_fwd_tx = event_tx.clone();
        let rpc_resolver = Arc::clone(&rpc);
        tokio::spawn(async move {
            while let Some(event) = conn_event_rx.recv().await {
                match event {
                    ConnectionEvent::Frame(text) => {
                        // Resolve RPC responses off the UI thread so `rpc.call()`
                        // can complete even while the app loop is busy.
                        if let Ok(response) =
                            serde_json::from_str::<moltis_protocol::ResponseFrame>(&text)
                            && response.r#type == "res"
                        {
                            rpc_resolver.resolve_response(response).await;
                            continue;
                        }

                        if conn_fwd_tx
                            .send(AppEvent::Connection(ConnectionEvent::Frame(text)))
                            .is_err()
                        {
                            break;
                        }
                    },
                    other => {
                        if conn_fwd_tx.send(AppEvent::Connection(other)).is_err() {
                            break;
                        }
                    },
                }
            }
        });

        // Text input area
        let mut textarea = TextArea::default();
        textarea.set_placeholder_text("Press 'i' to type a message...");

        // Main loop
        while !self.should_quit {
            if self.state.dirty {
                terminal.draw(|frame| {
                    ui::draw(
                        frame,
                        &self.state,
                        self.onboarding.as_ref(),
                        self.onboarding_check_pending,
                        &self.connection_display,
                        &mut textarea,
                        &self.theme,
                    );
                })?;
                self.state.dirty = false;
            }

            if let Some(event) = event_rx.recv().await {
                self.handle_event(event, &rpc, &event_tx, &mut textarea)
                    .await;
            }
        }

        Ok(())
    }

    async fn handle_event(
        &mut self,
        event: AppEvent,
        rpc: &Arc<RpcClient>,
        event_tx: &mpsc::UnboundedSender<AppEvent>,
        textarea: &mut TextArea<'_>,
    ) {
        match event {
            AppEvent::Key(key) => self.handle_key(key, rpc, textarea).await,
            AppEvent::Redraw => {
                self.state.dirty = true;
            },
            AppEvent::Tick => {
                // Re-render on tick if streaming (for spinner animation)
                if self.state.is_streaming() || self.state.pending_approval.is_some() {
                    self.state.dirty = true;
                }
            },
            AppEvent::Connection(conn_event) => {
                self.handle_connection_event(conn_event, rpc, event_tx)
                    .await;
            },
            AppEvent::InitialData(data) => {
                self.apply_initial_data(data);
            },
        }
    }

    async fn handle_connection_event(
        &mut self,
        event: ConnectionEvent,
        rpc: &Arc<RpcClient>,
        event_tx: &mpsc::UnboundedSender<AppEvent>,
    ) {
        match event {
            ConnectionEvent::Connected(hello_ok) => {
                self.connection_display = ConnectionDisplay::Connected;
                self.state.server_version = Some(hello_ok.server.version.clone());
                self.state.dirty = true;

                if self.initialize_onboarding(rpc).await {
                    // Load sessions and history in background (non-blocking).
                    spawn_initial_data_load(Arc::clone(rpc), event_tx.clone());
                } else {
                    self.state.input_mode = InputMode::Normal;
                }
                self.onboarding_check_pending = false;
                self.state.dirty = true;
            },
            ConnectionEvent::Disconnected => {
                self.connection_display = ConnectionDisplay::Disconnected;
                self.state.active_run_id = None;
                self.state.thinking_active = false;
                self.onboarding_check_pending = false;
                self.state.dirty = true;
            },
            ConnectionEvent::Error(msg) => {
                self.connection_display = ConnectionDisplay::Disconnected;
                self.onboarding_check_pending = false;
                // Provide actionable hints for common errors.
                let content = if msg.contains("authentication failed") {
                    "Authentication failed. Run the gateway's web UI to complete \
                     setup, or pass --api-key."
                        .into()
                } else {
                    format!("Connection error: {msg}")
                };
                self.state.messages.push(DisplayMessage {
                    role: MessageRole::System,
                    content,
                    tool_calls: Vec::new(),
                    thinking: None,
                });
                self.state.dirty = true;
            },
            ConnectionEvent::Frame(text) => {
                self.handle_frame(&text);
            },
        }
    }

    fn handle_frame(&mut self, text: &str) {
        // Try as event frame
        if let Ok(event) = serde_json::from_str::<moltis_protocol::EventFrame>(text)
            && event.r#type == "event"
        {
            let payload = event.payload.unwrap_or(serde_json::Value::Null);
            events::handle_event(&mut self.state, &event.event, &payload);
        }
    }

    fn apply_initial_data(&mut self, data: InitialData) {
        if let Some(sessions) = data.sessions {
            self.state.sessions = sessions;
        }
        if let Some(messages) = data.messages {
            self.state.messages = messages;
        }
        if let Some(session) = data.active_session {
            self.state.active_session = session;
        }
        if data.model.is_some() {
            self.state.model = data.model;
        }
        if data.provider.is_some() {
            self.state.provider = data.provider;
        }
        if let Some(usage) = data.token_usage {
            self.state.token_usage = usage;
        }
        self.state.dirty = true;
    }

    async fn handle_key(
        &mut self,
        key: KeyEvent,
        rpc: &Arc<RpcClient>,
        textarea: &mut TextArea<'_>,
    ) {
        if let Some(onboarding) = self.onboarding.as_ref() {
            let modal_open = onboarding_modal_open(onboarding);
            if should_quit_onboarding(key, modal_open) {
                self.should_quit = true;
                return;
            }
        }

        if self.onboarding_check_pending {
            if should_quit_onboarding(key, false) {
                self.should_quit = true;
            }
            return;
        }

        match self.state.input_mode {
            InputMode::Normal => self.handle_normal_key(key, rpc, textarea).await,
            InputMode::Insert => self.handle_insert_key(key, rpc, textarea).await,
            InputMode::Command => self.handle_command_key(key, rpc),
        }
    }

    async fn handle_normal_key(
        &mut self,
        key: KeyEvent,
        rpc: &Arc<RpcClient>,
        textarea: &mut TextArea<'_>,
    ) {
        if self.onboarding.is_some() {
            self.handle_onboarding_normal_key(key, rpc, textarea).await;
            return;
        }

        match (key.code, key.modifiers) {
            // Quit
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                if self.state.is_streaming() {
                    // Abort current stream
                    rpc.fire_and_forget(
                        "chat.abort",
                        serde_json::json!({"sessionKey": self.state.active_session}),
                    );
                    self.state.active_run_id = None;
                    self.state.thinking_active = false;
                    self.state.dirty = true;
                } else {
                    self.should_quit = true;
                }
            },
            (KeyCode::Char('q'), _) => {
                if self.state.pending_approval.is_none() {
                    self.should_quit = true;
                }
            },

            // Enter insert mode
            (KeyCode::Char('i') | KeyCode::Char('a'), _) => {
                self.state.input_mode = InputMode::Insert;
                self.state.dirty = true;
            },

            // Enter command mode
            (KeyCode::Char(':'), _) => {
                self.state.input_mode = InputMode::Command;
                self.state.command_buffer.clear();
                self.state.dirty = true;
            },

            // Scrolling
            (KeyCode::Char('j') | KeyCode::Down, _) => {
                self.state.scroll_down(1);
            },
            (KeyCode::Char('k') | KeyCode::Up, _) => {
                self.state.scroll_up(1);
            },
            (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
                self.state.scroll_down(10);
            },
            (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
                self.state.scroll_up(10);
            },
            (KeyCode::Char('g'), _) => {
                // Scroll to top
                self.state.scroll_offset = usize::MAX;
                self.state.dirty = true;
            },
            (KeyCode::Char('G'), KeyModifiers::SHIFT) | (KeyCode::End, _) => {
                self.state.scroll_to_bottom();
            },

            // Toggle sidebar
            (KeyCode::Char('b'), KeyModifiers::CONTROL) => {
                self.state.sidebar_visible = !self.state.sidebar_visible;
                self.state.dirty = true;
            },

            // Tab: cycle focus
            (KeyCode::Tab, _) => {
                self.state.active_panel = match self.state.active_panel {
                    Panel::Chat => Panel::Sessions,
                    Panel::Sessions => Panel::Chat,
                };
                if self.state.active_panel == Panel::Sessions {
                    self.state.sidebar_visible = true;
                }
                self.state.dirty = true;
            },

            // Approval handling
            (KeyCode::Char('y'), _) => {
                if let Some(approval) = self.state.pending_approval.take() {
                    rpc.fire_and_forget(
                        "exec.approval.resolve",
                        serde_json::json!({
                            "requestId": approval.request_id,
                            "decision": "approved"
                        }),
                    );
                    self.state.dirty = true;
                }
            },
            (KeyCode::Char('n'), _) => {
                if let Some(approval) = self.state.pending_approval.take() {
                    rpc.fire_and_forget(
                        "exec.approval.resolve",
                        serde_json::json!({
                            "requestId": approval.request_id,
                            "decision": "denied"
                        }),
                    );
                    self.state.dirty = true;
                }
            },

            _ => {},
        }
    }

    async fn handle_insert_key(
        &mut self,
        key: KeyEvent,
        rpc: &Arc<RpcClient>,
        textarea: &mut TextArea<'_>,
    ) {
        if self.onboarding.is_some() {
            self.handle_onboarding_insert_key(key, rpc, textarea).await;
            return;
        }

        match (key.code, key.modifiers) {
            (KeyCode::Esc, _) => {
                self.state.input_mode = InputMode::Normal;
                self.state.dirty = true;
            },
            (KeyCode::Enter, KeyModifiers::NONE) => {
                // Send message
                let text: String = textarea.lines().join("\n");
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    self.state.add_user_message(trimmed.to_owned());
                    self.state.scroll_to_bottom();

                    rpc.fire_and_forget("chat.send", serde_json::json!({"text": trimmed}));

                    // Clear textarea
                    *textarea = TextArea::default();
                    textarea.set_placeholder_text("Press 'i' to type a message...");
                }
                self.state.input_mode = InputMode::Normal;
                self.state.dirty = true;
            },
            (KeyCode::Enter, KeyModifiers::SHIFT) => {
                // Insert newline
                textarea.insert_newline();
                self.state.dirty = true;
            },
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                if self.state.is_streaming() {
                    rpc.fire_and_forget(
                        "chat.abort",
                        serde_json::json!({"sessionKey": self.state.active_session}),
                    );
                    self.state.active_run_id = None;
                    self.state.thinking_active = false;
                } else {
                    self.state.input_mode = InputMode::Normal;
                }
                self.state.dirty = true;
            },
            _ => {
                // Forward to textarea
                textarea.input(key);
                self.state.dirty = true;
            },
        }
    }

    fn handle_command_key(&mut self, key: KeyEvent, rpc: &Arc<RpcClient>) {
        match key.code {
            KeyCode::Esc => {
                self.state.input_mode = InputMode::Normal;
                self.state.command_buffer.clear();
                self.state.dirty = true;
            },
            KeyCode::Enter => {
                let cmd = std::mem::take(&mut self.state.command_buffer);
                self.execute_command(&cmd, rpc);
                self.state.input_mode = InputMode::Normal;
                self.state.dirty = true;
            },
            KeyCode::Backspace => {
                self.state.command_buffer.pop();
                self.state.dirty = true;
            },
            KeyCode::Char(c) => {
                self.state.command_buffer.push(c);
                self.state.dirty = true;
            },
            _ => {},
        }
    }

    fn execute_command(&mut self, cmd: &str, rpc: &Arc<RpcClient>) {
        let parts: Vec<&str> = cmd.trim().splitn(2, ' ').collect();
        match parts.first().copied() {
            Some("q" | "quit") => self.should_quit = true,
            Some("clear") => {
                rpc.fire_and_forget("chat.clear", serde_json::json!({}));
                self.state.messages.clear();
            },
            Some("model") => {
                if let Some(model_id) = parts.get(1) {
                    rpc.fire_and_forget(
                        "sessions.patch",
                        serde_json::json!({
                            "key": self.state.active_session,
                            "model": model_id
                        }),
                    );
                    self.state.model = Some(model_id.to_string());
                }
            },
            Some("session") => {
                if let Some(key) = parts.get(1) {
                    rpc.fire_and_forget("sessions.switch", serde_json::json!({"key": key}));
                    self.state.active_session = key.to_string();
                    self.state.messages.clear();
                }
            },
            _ => {
                self.state.messages.push(DisplayMessage {
                    role: MessageRole::System,
                    content: format!("Unknown command: {cmd}"),
                    tool_calls: Vec::new(),
                    thinking: None,
                });
            },
        }
    }
}

fn onboarding_modal_open(onboarding: &OnboardingState) -> bool {
    onboarding.llm.configuring.is_some() || onboarding.editing.is_some()
}

fn should_quit_onboarding(key: KeyEvent, modal_open: bool) -> bool {
    is_force_quit_key(key) || (key.code == KeyCode::Esc && !modal_open)
}

fn is_force_quit_key(key: KeyEvent) -> bool {
    key.code == KeyCode::Char('q')
        || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
}

/// Load initial data (sessions, history, context) in a background task.
/// Results are sent back to the event loop via `event_tx`.
fn spawn_initial_data_load(rpc: Arc<RpcClient>, event_tx: mpsc::UnboundedSender<AppEvent>) {
    tokio::spawn(async move {
        let mut data = InitialData::default();

        // Run all 3 RPC calls concurrently.
        let (sessions_res, history_res, context_res) = tokio::join!(
            rpc.call("sessions.list", serde_json::json!({})),
            rpc.call("chat.history", serde_json::json!({})),
            rpc.call("chat.context", serde_json::json!({})),
        );

        // Parse sessions
        if let Ok(sessions) = sessions_res {
            if let Some(arr) = sessions.as_array() {
                data.sessions = Some(
                    arr.iter()
                        .filter_map(|s| {
                            let key = s.get("key").and_then(|v| v.as_str())?;
                            Some(SessionEntry {
                                key: key.into(),
                                label: s.get("label").and_then(|v| v.as_str()).map(String::from),
                                model: s.get("model").and_then(|v| v.as_str()).map(String::from),
                                message_count: s
                                    .get("message_count")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0),
                                replying: s
                                    .get("replying")
                                    .and_then(|v| v.as_bool())
                                    .unwrap_or(false),
                            })
                        })
                        .collect(),
                );
            }
        } else if let Err(e) = sessions_res {
            warn!(error = %e, "failed to load sessions");
        }

        // Parse chat history
        if let Ok(history) = history_res {
            if let Some(arr) = history.as_array() {
                data.messages = Some(
                    arr.iter()
                        .filter_map(|msg| {
                            let role = msg.get("role").and_then(|v| v.as_str())?;
                            let content = msg
                                .get("content")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_owned();
                            let role = match role {
                                "user" => MessageRole::User,
                                "assistant" => MessageRole::Assistant,
                                _ => MessageRole::System,
                            };
                            Some(DisplayMessage {
                                role,
                                content,
                                tool_calls: Vec::new(),
                                thinking: None,
                            })
                        })
                        .collect(),
                );
            }
        } else if let Err(e) = history_res {
            warn!(error = %e, "failed to load chat history");
        }

        // Parse context
        if let Ok(ctx) = context_res {
            if let Some(session) = ctx.get("session") {
                data.active_session = session
                    .get("key")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                data.model = session
                    .get("model")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                data.provider = session
                    .get("provider")
                    .and_then(|v| v.as_str())
                    .map(String::from);
            }
            let mut usage = TokenUsage::default();
            if let Some(u) = ctx.get("usage") {
                usage.session_input = u
                    .get("sessionInputTokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                usage.session_output = u
                    .get("sessionOutputTokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
            }
            usage.context_window = ctx
                .get("contextWindow")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            data.token_usage = Some(usage);
        } else if let Err(e) = context_res {
            debug!(error = %e, "failed to load context");
        }

        let _ = event_tx.send(AppEvent::InitialData(data));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn onboarding_escape_quits_only_without_modal() {
        assert!(should_quit_onboarding(
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
            false
        ));
        assert!(!should_quit_onboarding(
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
            true
        ));
    }

    #[test]
    fn onboarding_force_quit_keys_always_quit() {
        assert!(should_quit_onboarding(
            KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE),
            false
        ));
        assert!(should_quit_onboarding(
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
            true
        ));
        assert!(!should_quit_onboarding(
            KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
            false
        ));
    }
}
