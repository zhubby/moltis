pub mod chat;
pub mod common;
pub mod crons;
pub mod footer;
pub mod header;
pub mod input;
pub mod markdown;
pub mod model_switcher;
pub mod onboarding;
pub mod projects;
pub mod sessions;
pub mod settings;
pub mod status_bar;
pub mod theme;

use {
    crate::{
        onboarding::OnboardingState,
        state::{AppState, MainTab, ModelSwitcherState, Panel},
    },
    ratatui::{
        Frame,
        layout::{Constraint, Layout, Rect},
        style::{Color, Modifier, Style},
        text::{Line, Span},
        widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
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
    model_switcher_state: Option<&ModelSwitcherState>,
    onboarding_check_pending: bool,
    connection: &ConnectionDisplay,
    textarea: &mut TextArea<'_>,
    theme: &Theme,
) {
    let area = frame.area();

    // 5-row layout: Header | Main content | Input | Footer | Status bar
    let show_input = matches!(state.active_tab, MainTab::Chat);
    let slash_menu_lines = state.slash_menu_items.len().min(4) as u16;
    let input_height = if show_input {
        if slash_menu_lines > 0 {
            3 + slash_menu_lines + 2
        } else {
            3
        }
    } else {
        0
    };
    let vertical = Layout::vertical([
        Constraint::Length(1),            // header
        Constraint::Min(5),               // main content
        Constraint::Length(input_height), // input area (Chat only)
        Constraint::Length(1),            // footer help
        Constraint::Length(1),            // status bar
    ])
    .split(area);

    // Header
    header::draw(frame, vertical[0], state, theme);

    // Main content: tab-dependent
    match state.active_tab {
        MainTab::Chat => {
            if state.sidebar_visible {
                let sidebar_width = (vertical[1].width / 4).clamp(20, 35);
                let horizontal =
                    Layout::horizontal([Constraint::Length(sidebar_width), Constraint::Min(30)])
                        .split(vertical[1]);

                let sidebar_focused = state.active_panel == Panel::Sessions;
                sessions::draw(frame, horizontal[0], state, sidebar_focused, theme);
                chat::draw(frame, horizontal[1], state, theme);
            } else {
                chat::draw(frame, vertical[1], state, theme);
            }
        },
        MainTab::Settings => {
            settings::draw(frame, vertical[1], state, theme);
        },
        MainTab::Projects => {
            projects::draw(frame, vertical[1], state, theme);
        },
        MainTab::Crons => {
            crons::draw(frame, vertical[1], state, theme);
        },
    }

    // Input area (Chat tab only)
    if show_input {
        input::draw(frame, vertical[2], state, textarea, theme);
    }

    // Footer help
    footer::draw(frame, vertical[3], state, theme);

    // Status bar
    status_bar::draw(frame, vertical[4], state, connection, theme);

    // Modals overlay
    if let Some(onboarding) = onboarding_state {
        let modal = common::centered_rect(94, 92, area);
        onboarding::draw(frame, modal, onboarding, state.input_mode, textarea, theme);
    } else if let Some(switcher) = model_switcher_state {
        model_switcher::draw(frame, area, state, switcher, theme);
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
    let popup = common::centered_rect(74, 46, area);
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
        .border_type(BorderType::Rounded)
        .border_style(theme.border_focused)
        .style(surface)
        .title(" Onboarding ");
    let paragraph = Paragraph::new(lines)
        .style(surface)
        .block(block)
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, popup);
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            onboarding::{
                EditTarget, ModelOption, OnboardingState, ProviderConfigurePhase,
                ProviderConfigureState, ProviderEntry, VoiceProviderEntry,
            },
            state::{InputMode, ModelSwitchItem, ModelSwitcherState, SessionEntry, SlashMenuItem},
        },
        ratatui::{Terminal, backend::TestBackend},
        status_bar::ConnectionDisplay,
        std::collections::BTreeSet,
    };

    fn render_to_text_with_size(
        state: &AppState,
        onboarding: Option<&OnboardingState>,
        model_switcher: Option<&crate::state::ModelSwitcherState>,
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
                model_switcher,
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
        model_switcher: Option<&crate::state::ModelSwitcherState>,
        onboarding_pending: bool,
        connection: &ConnectionDisplay,
    ) -> String {
        render_to_text_with_size(
            state,
            onboarding,
            model_switcher,
            onboarding_pending,
            connection,
            80,
            26, // +2 for header + footer
        )
    }

    #[test]
    fn onboarding_mode_renders_as_overlay_modal() {
        let state = AppState::default();
        let onboarding = OnboardingState::new(false, false, true, None);

        let text = render_to_text(
            &state,
            Some(&onboarding),
            None,
            false,
            &ConnectionDisplay::Connected,
        );

        assert!(text.contains("Onboarding"));
        assert!(text.contains("Please proceed to onboarding"));
    }

    #[test]
    fn regular_mode_shows_input_and_status() {
        let state = AppState::default();
        let text = render_to_text(&state, None, None, false, &ConnectionDisplay::Connecting);

        assert!(text.contains("Enter to send"));
        assert!(text.contains(" INSERT "));
        assert!(text.contains("Connecting..."));
    }

    #[test]
    fn slash_menu_renders_command_suggestions() {
        let state = AppState {
            slash_menu_items: vec![
                SlashMenuItem {
                    name: "context".into(),
                    description: "Show session context and project info".into(),
                },
                SlashMenuItem {
                    name: "compact".into(),
                    description: "Summarize conversation to save tokens".into(),
                },
            ],
            slash_menu_selected: 0,
            ..AppState::default()
        };
        let text = render_to_text(&state, None, None, false, &ConnectionDisplay::Connected);

        assert!(text.contains("Slash Commands"));
        assert!(text.contains("/context"));
        assert!(text.contains("/compact"));
    }

    #[test]
    fn startup_pending_shows_onboarding_gate_modal() {
        let state = AppState::default();
        let text = render_to_text(&state, None, None, true, &ConnectionDisplay::Connecting);

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
            None,
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
            None,
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
            None,
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
            None,
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
            None,
            false,
            &ConnectionDisplay::Connected,
            120,
            34,
        );
        assert!(text.contains("Next:"));
    }

    #[test]
    fn voice_step_renders_provider_listing_layout() {
        let state = AppState::default();
        let mut onboarding = OnboardingState::new(false, false, true, None);
        onboarding.step_index = 1;
        onboarding.voice.providers = vec![
            VoiceProviderEntry {
                id: "whisper-openai".into(),
                name: "OpenAI Whisper".into(),
                provider_type: "stt".into(),
                category: "cloud".into(),
                available: true,
                enabled: true,
                key_source: Some("config".into()),
                description: Some("Speech-to-text provider".into()),
            },
            VoiceProviderEntry {
                id: "openai-tts".into(),
                name: "OpenAI TTS".into(),
                provider_type: "tts".into(),
                category: "cloud".into(),
                available: false,
                enabled: false,
                key_source: None,
                description: Some("Text-to-speech provider".into()),
            },
        ];
        onboarding.voice.selected_provider = 0;

        let text = render_to_text_with_size(
            &state,
            Some(&onboarding),
            None,
            false,
            &ConnectionDisplay::Connected,
            120,
            34,
        );

        assert!(text.contains("Voice (optional)"));
        assert!(text.contains("Providers"));
        assert!(text.contains("Details"));
        assert!(text.contains("OpenAI Whisper"));
        assert!(text.contains("Actions: t toggle"));
    }

    #[test]
    fn channel_step_renders_provider_listing_layout() {
        let state = AppState::default();
        let mut onboarding = OnboardingState::new(false, false, true, None);
        onboarding.step_index = 2;
        onboarding.channel.selected_provider = 0;

        let text = render_to_text_with_size(
            &state,
            Some(&onboarding),
            None,
            false,
            &ConnectionDisplay::Connected,
            120,
            34,
        );

        assert!(text.contains("Connect Channels"));
        assert!(text.contains("Providers"));
        assert!(text.contains("Details"));
        assert!(text.contains("Telegram"));
        assert!(text.contains("Actions: Enter configure"));
    }

    #[test]
    fn channel_step_opens_telegram_config_modal() {
        let state = AppState::default();
        let mut onboarding = OnboardingState::new(false, false, true, None);
        onboarding.step_index = 2;
        onboarding.channel.selected_provider = 0;
        onboarding.channel.configuring = true;

        let text = render_to_text_with_size(
            &state,
            Some(&onboarding),
            None,
            false,
            &ConnectionDisplay::Connected,
            120,
            34,
        );

        assert!(text.contains("Configure Telegram"));
        assert!(text.contains("Bot username"));
        assert!(text.contains("Bot token"));
    }

    #[test]
    fn identity_step_renders_fields_and_details_layout() {
        let state = AppState::default();
        let mut onboarding = OnboardingState::new(false, false, true, None);
        onboarding.step_index = 3;
        onboarding.identity.user_name = "Alice".into();
        onboarding.identity.agent_name = "Moltis".into();
        onboarding.identity.emoji = "🤖".into();

        let text = render_to_text_with_size(
            &state,
            Some(&onboarding),
            None,
            false,
            &ConnectionDisplay::Connected,
            120,
            34,
        );

        assert!(text.contains("Set up your identity"));
        assert!(text.contains("Fields"));
        assert!(text.contains("Your name"));
        assert!(text.contains("Actions: j/k move"));
        assert!(text.contains("Agent Preview"));
    }

    #[test]
    fn identity_edit_uses_inline_textfield_not_generic_modal() {
        let state = AppState {
            input_mode: InputMode::Insert,
            ..AppState::default()
        };
        let mut onboarding = OnboardingState::new(false, false, true, None);
        onboarding.step_index = 3;
        onboarding.identity.field_index = 0;
        onboarding.editing = Some(EditTarget::IdentityUserName);

        let text = render_to_text_with_size(
            &state,
            Some(&onboarding),
            None,
            false,
            &ConnectionDisplay::Connected,
            120,
            34,
        );

        assert!(text.contains("Agent Preview"));
        assert!(text.contains("Fields"));
        assert!(!text.contains("Edit Field"));
    }

    #[test]
    fn summary_step_highlights_primary_finish_action() {
        let state = AppState::default();
        let mut onboarding = OnboardingState::new(false, false, true, None);
        onboarding.step_index = onboarding.steps.len().saturating_sub(1);
        onboarding.summary.provider_badges = vec!["OpenAI".into()];

        let text = render_to_text_with_size(
            &state,
            Some(&onboarding),
            None,
            false,
            &ConnectionDisplay::Connected,
            120,
            34,
        );

        assert!(text.contains("Finish Onboarding (Enter)"));
        assert!(text.contains("Setup Review"));
        assert!(text.contains("Get Started"));
    }

    #[test]
    fn sessions_panel_shows_active_model_and_switch_hint() {
        let state = AppState {
            provider: Some("openai".into()),
            model: Some("openai/gpt-5".into()),
            sessions: vec![SessionEntry {
                key: "main".into(),
                label: Some("Main".into()),
                model: Some("openai/gpt-5".into()),
                message_count: 2,
                replying: false,
            }],
            ..AppState::default()
        };

        let text = render_to_text_with_size(
            &state,
            None,
            None,
            false,
            &ConnectionDisplay::Connected,
            120,
            34,
        );

        assert!(text.contains("Active:"));
        assert!(text.contains("openai"));
        assert!(text.contains("gpt-5"));
        assert!(text.contains("[m]"));
    }

    #[test]
    fn model_switcher_modal_renders_filter_and_actions() {
        let state = AppState {
            provider: Some("openai".into()),
            model: Some("openai/gpt-5".into()),
            ..AppState::default()
        };
        let switcher = ModelSwitcherState {
            query: "gpt".into(),
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

        let text = render_to_text_with_size(
            &state,
            None,
            Some(&switcher),
            false,
            &ConnectionDisplay::Connected,
            120,
            34,
        );

        assert!(text.contains("Switch Provider/Model"));
        assert!(text.contains("Search:"));
        assert!(text.contains("GPT-5"));
        assert!(text.contains("Enter switch"));
    }

    #[test]
    fn header_shows_tabs_and_model_info() {
        let state = AppState {
            provider: Some("openai".into()),
            model: Some("openai/gpt-5".into()),
            ..AppState::default()
        };
        let text = render_to_text_with_size(
            &state,
            None,
            None,
            false,
            &ConnectionDisplay::Connected,
            120,
            34,
        );

        assert!(text.contains("moltis"));
        assert!(text.contains("Chat"));
        assert!(text.contains("Settings"));
        assert!(text.contains("Projects"));
        assert!(text.contains("Crons"));
    }

    #[test]
    fn settings_tab_renders_sections() {
        let state = AppState {
            active_tab: MainTab::Settings,
            ..AppState::default()
        };
        let text = render_to_text(&state, None, None, false, &ConnectionDisplay::Connected);

        assert!(text.contains("Sections"));
        assert!(text.contains("Identity"));
        assert!(text.contains("Providers"));
    }
}
