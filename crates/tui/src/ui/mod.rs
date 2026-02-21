pub mod chat;
pub mod input;
pub mod markdown;
pub mod onboarding;
pub mod sessions;
pub mod status_bar;
pub mod theme;

use {
    crate::{
        onboarding::OnboardingState,
        state::{AppState, Panel},
    },
    ratatui::{
        Frame,
        layout::{Constraint, Layout, Rect},
        style::{Color, Modifier, Style},
        text::{Line, Span},
        widgets::{Block, Borders, Clear, Paragraph, Wrap},
    },
    status_bar::ConnectionDisplay,
    theme::Theme,
    tui_textarea::TextArea,
};

/// Draw the entire UI.
pub fn draw(
    frame: &mut Frame,
    state: &AppState,
    onboarding_state: Option<&OnboardingState>,
    onboarding_check_pending: bool,
    connection: &ConnectionDisplay,
    textarea: &mut TextArea<'_>,
    theme: &Theme,
) {
    let area = frame.area();

    // Vertical: main content + input + status bar
    let vertical = Layout::vertical([
        Constraint::Min(5),    // main content (chat + optional sidebar)
        Constraint::Length(3), // input area
        Constraint::Length(1), // status bar
    ])
    .split(area);

    // Main content: optional sidebar + chat
    if state.sidebar_visible {
        let horizontal = Layout::horizontal([
            Constraint::Length(25), // sidebar
            Constraint::Min(30),    // chat
        ])
        .split(vertical[0]);

        let sidebar_focused = state.active_panel == Panel::Sessions;
        sessions::draw(frame, horizontal[0], state, sidebar_focused, theme);
        chat::draw(frame, horizontal[1], state, theme);
    } else {
        chat::draw(frame, vertical[0], state, theme);
    }

    // Input area
    input::draw(frame, vertical[1], state, textarea, theme);

    // Status bar
    status_bar::draw(frame, vertical[2], state, connection, theme);

    if let Some(onboarding) = onboarding_state {
        // Onboarding is blocking, but appears as an intentional overlay above
        // the main app chrome so startup does not feel like a rendering glitch.
        let modal = centered_rect(94, 92, area);
        onboarding::draw(frame, modal, onboarding, state.input_mode, textarea, theme);
    } else if onboarding_check_pending {
        draw_onboarding_pending_modal(frame, area, connection, theme);
    }
}

fn draw_onboarding_pending_modal(
    frame: &mut Frame,
    area: Rect,
    connection: &ConnectionDisplay,
    theme: &Theme,
) {
    let popup = centered_rect(74, 46, area);
    let surface = Style::default().fg(Color::White).bg(Color::Rgb(24, 28, 40));
    let status_line = match connection {
        ConnectionDisplay::Connecting => "Connecting to gateway and checking setup status...",
        ConnectionDisplay::Connected => "Connected. Loading onboarding state...",
        ConnectionDisplay::Disconnected => "Waiting for gateway connection before onboarding.",
    };

    let lines = vec![
        Line::from(vec![Span::styled(
            "Please proceed to onboarding before using chat.",
            theme.heading.add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(status_line),
        Line::from(""),
        Line::from("This setup configures providers, identity, and channels."),
        Line::from("Press Esc, Ctrl+C, or q to quit."),
    ];

    frame.render_widget(Clear, popup);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border_focused)
        .style(surface)
        .title(" Onboarding ");
    let paragraph = Paragraph::new(lines)
        .style(surface)
        .block(block)
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, popup);
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(area);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(vertical[1])[1]
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            onboarding::{
                EditTarget, ModelOption, OnboardingState, ProviderConfigurePhase,
                ProviderConfigureState, ProviderEntry,
            },
            state::InputMode,
        },
        ratatui::{Terminal, backend::TestBackend},
        status_bar::ConnectionDisplay,
        std::collections::BTreeSet,
    };

    fn render_to_text_with_size(
        state: &AppState,
        onboarding: Option<&OnboardingState>,
        onboarding_pending: bool,
        connection: &ConnectionDisplay,
        width: u16,
        height: u16,
    ) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = match Terminal::new(backend) {
            Ok(t) => t,
            Err(error) => panic!("failed to create test terminal: {error}"),
        };
        let mut textarea = TextArea::default();
        let theme = Theme::default();

        if let Err(error) = terminal.draw(|frame| {
            draw(
                frame,
                state,
                onboarding,
                onboarding_pending,
                connection,
                &mut textarea,
                &theme,
            );
        }) {
            panic!("failed to draw test frame: {error}");
        }

        let buffer = terminal.backend().buffer();
        let area = buffer.area;
        let mut text = String::new();

        for y in 0..area.height {
            for x in 0..area.width {
                text.push_str(buffer[(x, y)].symbol());
            }
            text.push('\n');
        }

        text
    }

    fn render_to_text(
        state: &AppState,
        onboarding: Option<&OnboardingState>,
        onboarding_pending: bool,
        connection: &ConnectionDisplay,
    ) -> String {
        render_to_text_with_size(state, onboarding, onboarding_pending, connection, 80, 24)
    }

    #[test]
    fn onboarding_mode_renders_as_overlay_modal() {
        let state = AppState::default();
        let onboarding = OnboardingState::new(false, false, true, None);

        let text = render_to_text(
            &state,
            Some(&onboarding),
            false,
            &ConnectionDisplay::Connected,
        );

        assert!(text.contains("Onboarding"));
        assert!(text.contains("Please proceed to onboarding"));
    }

    #[test]
    fn regular_mode_shows_input_and_status() {
        let state = AppState::default();
        let text = render_to_text(&state, None, false, &ConnectionDisplay::Connecting);

        assert!(text.contains("Press 'i' to type"));
        assert!(text.contains(" NORMAL "));
        assert!(text.contains("Connecting..."));
    }

    #[test]
    fn startup_pending_shows_onboarding_gate_modal() {
        let state = AppState::default();
        let text = render_to_text(&state, None, true, &ConnectionDisplay::Connecting);

        assert!(text.contains("Please proceed to onboarding"));
        assert!(text.contains("checking setup status"));
    }

    #[test]
    fn llm_selection_opens_config_modal() {
        let state = AppState::default();
        let mut onboarding = OnboardingState::new(false, false, true, None);
        onboarding.llm.configuring = Some(ProviderConfigureState {
            provider_name: "openai".into(),
            provider_display_name: "OpenAI".into(),
            auth_type: "api-key".into(),
            requires_model: false,
            key_optional: false,
            field_index: 0,
            api_key: String::new(),
            endpoint: String::new(),
            model: String::new(),
            phase: ProviderConfigurePhase::Form,
        });

        let text = render_to_text(
            &state,
            Some(&onboarding),
            false,
            &ConnectionDisplay::Connected,
        );
        assert!(text.contains("Configure OpenAI"));
    }

    #[test]
    fn configured_provider_shows_model_selector_without_api_key_form() {
        let state = AppState::default();
        let mut onboarding = OnboardingState::new(false, false, true, None);
        onboarding.llm.configuring = Some(ProviderConfigureState {
            provider_name: "openai".into(),
            provider_display_name: "OpenAI".into(),
            auth_type: "api-key".into(),
            requires_model: false,
            key_optional: false,
            field_index: 0,
            api_key: String::new(),
            endpoint: String::new(),
            model: "openai/gpt-5".into(),
            phase: ProviderConfigurePhase::ModelSelect {
                models: vec![ModelOption {
                    id: "openai/gpt-5".into(),
                    display_name: "GPT-5".into(),
                    supports_tools: true,
                }],
                selected: BTreeSet::from(["openai/gpt-5".to_string()]),
                cursor: 0,
            },
        });

        let text = render_to_text(
            &state,
            Some(&onboarding),
            false,
            &ConnectionDisplay::Connected,
        );
        assert!(text.contains("Choose preferred models"));
        assert!(!text.contains("API key:"));
    }

    #[test]
    fn onboarding_edit_uses_modal_textfield() {
        let state = AppState {
            input_mode: InputMode::Insert,
            ..AppState::default()
        };

        let mut onboarding = OnboardingState::new(false, false, true, None);
        onboarding.editing = Some(EditTarget::ProviderApiKey);

        let text = render_to_text(
            &state,
            Some(&onboarding),
            false,
            &ConnectionDisplay::Connected,
        );
        assert!(text.contains("Edit Field"));
        assert!(text.contains("Value"));
    }

    #[test]
    fn provider_edit_stays_inside_config_modal() {
        let state = AppState {
            input_mode: InputMode::Insert,
            ..AppState::default()
        };
        let mut onboarding = OnboardingState::new(false, false, true, None);
        onboarding.editing = Some(EditTarget::ProviderApiKey);
        onboarding.llm.configuring = Some(ProviderConfigureState {
            provider_name: "openai".into(),
            provider_display_name: "OpenAI".into(),
            auth_type: "api-key".into(),
            requires_model: false,
            key_optional: false,
            field_index: 0,
            api_key: String::new(),
            endpoint: String::new(),
            model: String::new(),
            phase: ProviderConfigurePhase::Form,
        });

        let text = render_to_text(
            &state,
            Some(&onboarding),
            false,
            &ConnectionDisplay::Connected,
        );
        assert!(text.contains("Configure OpenAI"));
        assert!(text.contains("API key"));
        assert!(!text.contains("Edit Field"));
    }

    #[test]
    fn llm_details_show_next_action_hint() {
        let state = AppState::default();
        let mut onboarding = OnboardingState::new(false, false, true, None);
        onboarding.llm.providers.push(ProviderEntry {
            name: "openai".into(),
            display_name: "OpenAI".into(),
            auth_type: "api-key".into(),
            configured: false,
            default_base_url: None,
            base_url: None,
            models: Vec::new(),
            requires_model: false,
            key_optional: false,
        });

        let text = render_to_text_with_size(
            &state,
            Some(&onboarding),
            false,
            &ConnectionDisplay::Connected,
            120,
            32,
        );
        assert!(text.contains("Next:"));
    }
}
