use {
    super::{common, theme::Theme},
    crate::{
        onboarding::{
            ChannelProvider, EditTarget, OnboardingState, OnboardingStep, ProviderConfigurePhase,
            supports_endpoint,
        },
        state::InputMode,
    },
    ratatui::{
        Frame,
        layout::{Constraint, Layout, Rect},
        style::{Color, Modifier, Style},
        text::{Line, Span},
        widgets::{
            Block, BorderType, Borders, Cell, Clear, Paragraph, Row, Table, TableState, Wrap,
        },
    },
    tui_textarea::TextArea,
};

pub fn draw(
    frame: &mut Frame,
    area: Rect,
    onboarding: &OnboardingState,
    input_mode: InputMode,
    textarea: &mut TextArea<'_>,
    theme: &Theme,
) {
    match onboarding.current_step() {
        OnboardingStep::Llm => {
            draw_llm_screen(frame, area, onboarding, input_mode, textarea, theme);
            return;
        },
        OnboardingStep::Voice => {
            draw_voice_screen(frame, area, onboarding, input_mode, textarea, theme);
            return;
        },
        OnboardingStep::Channel => {
            draw_channel_screen(frame, area, onboarding, input_mode, textarea, theme);
            return;
        },
        OnboardingStep::Identity => {
            draw_identity_screen(frame, area, onboarding, input_mode, textarea, theme);
            return;
        },
        OnboardingStep::Summary => {
            draw_summary_screen(frame, area, onboarding, theme);
            return;
        },
        _ => {},
    }

    let mut lines: Vec<Line<'_>> = vec![
        step_indicator(onboarding, theme),
        Line::from(""),
        onboarding_intro_line(theme),
        Line::from(""),
        Line::from(vec![Span::styled(
            onboarding.current_step().title(),
            theme.heading,
        )]),
        Line::from(""),
    ];

    match onboarding.current_step() {
        OnboardingStep::Security => draw_security(&mut lines, onboarding, theme),
        OnboardingStep::Llm => draw_llm_compact(&mut lines, onboarding, theme),
        OnboardingStep::Voice => draw_voice_compact(&mut lines, onboarding, theme),
        OnboardingStep::Channel => draw_channel_compact(&mut lines, onboarding, theme),
        OnboardingStep::Identity => draw_identity_compact(&mut lines, onboarding, theme),
        OnboardingStep::Summary => draw_summary(&mut lines, onboarding, theme),
    }

    append_feedback(&mut lines, onboarding, theme);
    lines.push(Line::from(""));
    lines.push(hints_line(onboarding, theme));

    let paragraph = Paragraph::new(lines)
        .block(onboarding_block(theme))
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);

    if onboarding.editing.is_some() {
        draw_edit_modal(frame, area, onboarding, input_mode, textarea, theme);
    }
}

fn onboarding_block(theme: &Theme) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.border_focused)
        .title(" Onboarding ")
}

fn render_step_compact<'a>(
    frame: &mut Frame,
    area: Rect,
    onboarding: &'a OnboardingState,
    theme: &Theme,
    draw_content: impl FnOnce(&mut Vec<Line<'a>>),
) {
    let mut lines: Vec<Line<'a>> = vec![
        step_indicator(onboarding, theme),
        Line::from(""),
        onboarding_intro_line(theme),
        Line::from(""),
        Line::from(vec![Span::styled(
            onboarding.current_step().title(),
            theme.heading,
        )]),
        Line::from(""),
    ];
    draw_content(&mut lines);
    append_feedback(&mut lines, onboarding, theme);
    lines.push(Line::from(""));
    lines.push(hints_line(onboarding, theme));

    let paragraph = Paragraph::new(lines)
        .block(onboarding_block(theme))
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

fn append_feedback<'a>(lines: &mut Vec<Line<'a>>, onboarding: &'a OnboardingState, theme: &Theme) {
    if let Some(error) = onboarding.error_message.as_deref() {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("Error: ", theme.tool_error.add_modifier(Modifier::BOLD)),
            Span::styled(error, theme.tool_error),
        ]));
    }
    if let Some(status) = onboarding.status_message.as_deref() {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("Status: ", theme.tool_success.add_modifier(Modifier::BOLD)),
            Span::styled(status, theme.tool_success),
        ]));
    }
}

fn feedback_line<'a>(onboarding: &'a OnboardingState, theme: &Theme) -> Line<'a> {
    if let Some(error) = onboarding.error_message.as_deref() {
        Line::from(vec![
            Span::styled("Error: ", theme.tool_error.add_modifier(Modifier::BOLD)),
            Span::styled(error, theme.tool_error),
        ])
    } else if let Some(status) = onboarding.status_message.as_deref() {
        Line::from(vec![
            Span::styled("Status: ", theme.tool_success.add_modifier(Modifier::BOLD)),
            Span::styled(status, theme.tool_success),
        ])
    } else {
        Line::from("")
    }
}

fn render_wide_header(
    frame: &mut Frame,
    area: Rect,
    onboarding: &OnboardingState,
    theme: &Theme,
    summary_line: Line<'_>,
) -> std::rc::Rc<[Rect]> {
    let block = onboarding_block(theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let sections = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(8),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(inner);

    frame.render_widget(
        Paragraph::new(step_indicator(onboarding, theme)),
        sections[0],
    );
    frame.render_widget(Paragraph::new(onboarding_intro_line(theme)), sections[1]);
    frame.render_widget(
        Paragraph::new(Line::from(vec![Span::styled(
            onboarding.current_step().title(),
            theme.heading,
        )])),
        sections[2],
    );
    frame.render_widget(Paragraph::new(summary_line), sections[3]);

    sections
}

fn render_wide_footer(
    frame: &mut Frame,
    sections: &[Rect],
    onboarding: &OnboardingState,
    theme: &Theme,
    actions_hint: &str,
) {
    frame.render_widget(
        Paragraph::new(feedback_line(onboarding, theme)),
        sections[5],
    );
    frame.render_widget(Paragraph::new(actions_hint), sections[6]);
    frame.render_widget(Paragraph::new(hints_line(onboarding, theme)), sections[7]);
}

fn draw_llm_screen(
    frame: &mut Frame,
    area: Rect,
    onboarding: &OnboardingState,
    input_mode: InputMode,
    textarea: &mut TextArea<'_>,
    theme: &Theme,
) {
    if area.width < 90 || area.height < 16 {
        render_step_compact(frame, area, onboarding, theme, |lines| {
            draw_llm_compact(lines, onboarding, theme);
        });
        if onboarding.llm.configuring.is_some() {
            draw_llm_config_modal(frame, area, onboarding, textarea, theme);
        }
        if onboarding.editing.is_some() && provider_inline_edit_target(onboarding).is_none() {
            draw_edit_modal(frame, area, onboarding, input_mode, textarea, theme);
        }
        return;
    }

    let sections = render_wide_header(frame, area, onboarding, theme, llm_summary_line(onboarding));
    let body = Layout::horizontal([Constraint::Percentage(63), Constraint::Percentage(37)])
        .split(sections[4]);

    let llm = &onboarding.llm;
    if llm.providers.is_empty() {
        let empty = Paragraph::new("No providers returned by gateway. Press r to refresh.").block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(" Providers "),
        );
        frame.render_widget(empty, body[0]);
    } else {
        let rows = llm
            .providers
            .iter()
            .map(|provider| {
                let status = if provider.configured {
                    "configured"
                } else {
                    "not configured"
                };

                let status_cell = if provider.configured {
                    Cell::from(status).style(theme.tool_success)
                } else {
                    Cell::from(status).style(theme.system_msg)
                };

                Row::new(vec![
                    Cell::from(provider.display_name.clone()),
                    Cell::from(provider.auth_type.clone()),
                    status_cell,
                ])
            })
            .collect::<Vec<Row>>();

        let table = Table::new(rows, [
            Constraint::Percentage(45),
            Constraint::Length(10),
            Constraint::Length(16),
        ])
        .header(Row::new(vec!["Provider", "Auth", "Status"]).style(theme.bold))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(" Providers "),
        )
        .row_highlight_style(theme.mode_insert.add_modifier(Modifier::BOLD))
        .highlight_symbol("▶ ");

        let mut table_state = TableState::default();
        table_state.select(Some(llm.selected_provider.min(llm.providers.len() - 1)));
        frame.render_stateful_widget(table, body[0], &mut table_state);
    }

    let mut details: Vec<Line<'_>> = Vec::new();
    if let Some(provider) = llm.providers.get(llm.selected_provider) {
        details.push(Line::from(vec![
            Span::styled("Selected: ", theme.bold),
            Span::raw(provider.display_name.clone()),
        ]));
        details.push(Line::from(format!("Auth: {}", provider.auth_type)));
        details.push(Line::from(if provider.configured {
            "Status: configured"
        } else {
            "Status: not configured"
        }));

        if let Some(base) = provider.base_url.as_deref() {
            details.push(Line::from(format!("Endpoint: {base}")));
        } else if let Some(base) = provider.default_base_url.as_deref() {
            details.push(Line::from(format!("Default endpoint: {base}")));
        }

        if !provider.models.is_empty() {
            let preview = provider
                .models
                .iter()
                .take(3)
                .cloned()
                .collect::<Vec<String>>()
                .join(", ");
            details.push(Line::from(format!(
                "Models: {}{}",
                preview,
                if provider.models.len() > 3 {
                    ", ..."
                } else {
                    ""
                }
            )));
        }

        if llm.configuring.is_none() {
            details.push(Line::from(""));
            details.push(Line::from(vec![
                Span::styled("Next: ", theme.bold),
                Span::raw("Press Enter to configure this provider."),
            ]));
        }
    }

    if let Some(config) = llm.configuring.as_ref() {
        details.push(Line::from(""));
        details.push(Line::from(vec![Span::styled(
            format!("Configuring {}", config.provider_display_name),
            theme.bold,
        )]));

        match &config.phase {
            ProviderConfigurePhase::Form => {
                let mut row_index = 0usize;
                details.push(Line::from(format!(
                    "{} API key: {}",
                    if config.field_index == row_index {
                        "▶"
                    } else {
                        " "
                    },
                    common::mask_secret(&config.api_key)
                )));

                if supports_endpoint(&config.provider_name) {
                    row_index += 1;
                    details.push(Line::from(format!(
                        "{} Endpoint: {}",
                        if config.field_index == row_index {
                            "▶"
                        } else {
                            " "
                        },
                        if config.endpoint.is_empty() {
                            "(empty)"
                        } else {
                            &config.endpoint
                        }
                    )));
                }

                if config.requires_model {
                    row_index += 1;
                    details.push(Line::from(format!(
                        "{} Model: {}",
                        if config.field_index == row_index {
                            "▶"
                        } else {
                            " "
                        },
                        if config.model.is_empty() {
                            "(empty)"
                        } else {
                            &config.model
                        }
                    )));
                }
            },
            ProviderConfigurePhase::ModelSelect {
                models,
                selected,
                cursor,
            } => {
                details.push(Line::from(format!(
                    "Model prefs: {} selected",
                    selected.len()
                )));
                for (index, model) in models.iter().take(6).enumerate() {
                    let marker = if index == *cursor {
                        "▶"
                    } else {
                        " "
                    };
                    let check = if selected.contains(&model.id) {
                        "[x]"
                    } else {
                        "[ ]"
                    };
                    details.push(Line::from(format!(
                        "{marker} {check} {}",
                        model.display_name
                    )));
                }
                if models.len() > 6 {
                    details.push(Line::from(format!("... {} more models", models.len() - 6)));
                }
            },
            ProviderConfigurePhase::OAuth {
                auth_url,
                verification_uri,
                user_code,
            } => {
                if let Some(url) = auth_url {
                    details.push(Line::from("Open auth URL:"));
                    details.push(Line::from(url.clone()));
                }
                if let Some(uri) = verification_uri {
                    details.push(Line::from("Verify URL:"));
                    details.push(Line::from(uri.clone()));
                }
                if let Some(code) = user_code {
                    details.push(Line::from(format!("Code: {code}")));
                }
            },
            ProviderConfigurePhase::Local {
                backend,
                models,
                cursor,
                note,
            } => {
                details.push(Line::from(format!("Backend: {backend}")));
                if let Some(note) = note {
                    details.push(Line::from(note.clone()));
                }
                for (index, model) in models.iter().take(5).enumerate() {
                    let marker = if index == *cursor {
                        "▶"
                    } else {
                        " "
                    };
                    details.push(Line::from(format!("{marker} {}", model.display_name)));
                }
                if models.len() > 5 {
                    details.push(Line::from(format!("... {} more models", models.len() - 5)));
                }
            },
        }
    } else if details.is_empty() {
        details.push(Line::from("Select a provider to view details."));
    }

    let details_widget = Paragraph::new(details)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(" Details "),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(details_widget, body[1]);

    render_wide_footer(
        frame,
        &sections,
        onboarding,
        theme,
        llm_actions_hint(onboarding),
    );

    if onboarding.llm.configuring.is_some() {
        draw_llm_config_modal(frame, area, onboarding, textarea, theme);
    }
    if onboarding.editing.is_some() && provider_inline_edit_target(onboarding).is_none() {
        draw_edit_modal(frame, area, onboarding, input_mode, textarea, theme);
    }
}

fn llm_summary_line(onboarding: &OnboardingState) -> Line<'static> {
    let llm = &onboarding.llm;
    let configured = llm
        .providers
        .iter()
        .filter(|provider| provider.configured)
        .map(|provider| provider.display_name.clone())
        .collect::<Vec<String>>();

    if configured.is_empty() {
        return Line::from(format!(
            "Providers: {}/{} configured",
            0,
            llm.providers.len()
        ));
    }

    Line::from(format!(
        "Providers: {}/{} configured ({})",
        configured.len(),
        llm.providers.len(),
        configured.join(", ")
    ))
}

fn llm_actions_hint(onboarding: &OnboardingState) -> &'static str {
    let Some(config) = onboarding.llm.configuring.as_ref() else {
        return "Actions: Enter configure  r refresh  c continue  s skip  b back";
    };

    match config.phase {
        ProviderConfigurePhase::Form => {
            "Actions: j/k move fields  e edit  m models  v validate/save  Esc close"
        },
        ProviderConfigurePhase::ModelSelect { .. } => {
            "Actions: j/k move models  Space toggle  Enter save  Esc close"
        },
        ProviderConfigurePhase::OAuth { .. } => "Actions: Enter/p poll status  Esc close",
        ProviderConfigurePhase::Local { .. } => {
            "Actions: j/k move models  Enter configure  Esc close"
        },
    }
}

fn draw_voice_screen(
    frame: &mut Frame,
    area: Rect,
    onboarding: &OnboardingState,
    input_mode: InputMode,
    textarea: &mut TextArea<'_>,
    theme: &Theme,
) {
    if area.width < 90 || area.height < 16 {
        render_step_compact(frame, area, onboarding, theme, |lines| {
            draw_voice_compact(lines, onboarding, theme);
        });
        if onboarding.editing.is_some() {
            draw_edit_modal(frame, area, onboarding, input_mode, textarea, theme);
        }
        return;
    }

    let sections = render_wide_header(
        frame,
        area,
        onboarding,
        theme,
        voice_summary_line(onboarding),
    );
    let body = Layout::horizontal([Constraint::Percentage(63), Constraint::Percentage(37)])
        .split(sections[4]);

    let voice = &onboarding.voice;
    if voice.providers.is_empty() {
        let empty = Paragraph::new("No voice providers returned by gateway. Press r to refresh.")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(" Providers "),
            );
        frame.render_widget(empty, body[0]);
    } else {
        let rows = voice
            .providers
            .iter()
            .map(|provider| {
                let (status_text, status_style) = if !provider.available {
                    ("needs key", theme.tool_error)
                } else if provider.enabled {
                    ("enabled", theme.tool_success)
                } else {
                    ("available", theme.system_msg)
                };

                Row::new(vec![
                    Cell::from(provider.name.clone()),
                    Cell::from(provider.provider_type.clone()),
                    Cell::from(provider.category.clone()),
                    Cell::from(status_text).style(status_style),
                ])
            })
            .collect::<Vec<Row>>();

        let table = Table::new(rows, [
            Constraint::Percentage(42),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(12),
        ])
        .header(Row::new(vec!["Provider", "Type", "Mode", "Status"]).style(theme.bold))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(" Providers "),
        )
        .row_highlight_style(theme.mode_insert.add_modifier(Modifier::BOLD))
        .highlight_symbol("▶ ");

        let mut table_state = TableState::default();
        table_state.select(Some(voice.selected_provider.min(voice.providers.len() - 1)));
        frame.render_stateful_widget(table, body[0], &mut table_state);
    }

    let mut details: Vec<Line<'_>> = Vec::new();
    if let Some(provider) = voice.providers.get(voice.selected_provider) {
        details.push(Line::from(vec![
            Span::styled("Selected: ", theme.bold),
            Span::raw(provider.name.clone()),
        ]));
        details.push(Line::from(format!("ID: {}", provider.id)));
        details.push(Line::from(format!("Type: {}", provider.provider_type)));
        details.push(Line::from(format!("Mode: {}", provider.category)));
        details.push(Line::from(if provider.enabled {
            "Status: enabled"
        } else {
            "Status: disabled"
        }));
        details.push(Line::from(if provider.available {
            "Availability: ready"
        } else {
            "Availability: API key required"
        }));

        if let Some(source) = provider.key_source.as_deref() {
            details.push(Line::from(format!("Key source: {source}")));
        }

        if let Some(description) = provider.description.as_deref() {
            details.push(Line::from(""));
            details.push(Line::from(description.to_string()));
        }

        details.push(Line::from(""));
        details.push(Line::from(format!(
            "Pending API key: {}",
            common::mask_secret(&voice.pending_api_key)
        )));
        details.push(Line::from(""));
        details.push(Line::from(vec![
            Span::styled("Next: ", theme.bold),
            Span::raw("e edit key  v save key  t toggle provider"),
        ]));
    } else {
        details.push(Line::from("Select a provider to view details."));
    }

    let details_widget = Paragraph::new(details)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(" Details "),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(details_widget, body[1]);

    render_wide_footer(frame, &sections, onboarding, theme, voice_actions_hint());

    if onboarding.editing.is_some() {
        draw_edit_modal(frame, area, onboarding, input_mode, textarea, theme);
    }
}

fn voice_summary_line(onboarding: &OnboardingState) -> Line<'static> {
    let voice = &onboarding.voice;
    let enabled = voice
        .providers
        .iter()
        .filter(|provider| provider.enabled)
        .map(|provider| provider.name.clone())
        .collect::<Vec<String>>();

    if enabled.is_empty() {
        return Line::from(format!(
            "Voice providers: {}/{} enabled",
            0,
            voice.providers.len()
        ));
    }

    Line::from(format!(
        "Voice providers: {}/{} enabled ({})",
        enabled.len(),
        voice.providers.len(),
        enabled.join(", ")
    ))
}

fn voice_actions_hint() -> &'static str {
    "Actions: t toggle  e edit key  v save key  r refresh  c continue  s skip  b back"
}

fn draw_channel_screen(
    frame: &mut Frame,
    area: Rect,
    onboarding: &OnboardingState,
    input_mode: InputMode,
    textarea: &mut TextArea<'_>,
    theme: &Theme,
) {
    if area.width < 90 || area.height < 16 {
        render_step_compact(frame, area, onboarding, theme, |lines| {
            draw_channel_compact(lines, onboarding, theme);
        });
        if onboarding.channel.configuring {
            draw_channel_config_modal(frame, area, onboarding, textarea, theme);
        }
        if onboarding.editing.is_some() && channel_inline_edit_target(onboarding).is_none() {
            draw_edit_modal(frame, area, onboarding, input_mode, textarea, theme);
        }
        return;
    }

    let sections = render_wide_header(
        frame,
        area,
        onboarding,
        theme,
        channel_summary_line(onboarding),
    );
    let body = Layout::horizontal([Constraint::Percentage(63), Constraint::Percentage(37)])
        .split(sections[4]);

    let channel = &onboarding.channel;

    let rows = ChannelProvider::ALL
        .iter()
        .map(|provider| {
            let (status, status_style) = match provider {
                ChannelProvider::Telegram => {
                    if channel.connected {
                        ("connected", theme.tool_success)
                    } else {
                        ("not configured", theme.system_msg)
                    }
                },
                ChannelProvider::Slack | ChannelProvider::Discord => {
                    ("coming soon", theme.system_msg)
                },
            };

            Row::new(vec![
                Cell::from(provider.name()),
                Cell::from(provider.auth()),
                Cell::from(status).style(status_style),
            ])
        })
        .collect::<Vec<Row>>();

    let table = Table::new(rows, [
        Constraint::Percentage(52),
        Constraint::Length(12),
        Constraint::Length(16),
    ])
    .header(Row::new(vec!["Channel", "Auth", "Status"]).style(theme.bold))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Providers "),
    )
    .row_highlight_style(theme.mode_insert.add_modifier(Modifier::BOLD))
    .highlight_symbol("▶ ");

    let mut table_state = TableState::default();
    table_state.select(Some(
        channel
            .selected_provider
            .min(ChannelProvider::ALL.len() - 1),
    ));
    frame.render_stateful_widget(table, body[0], &mut table_state);

    let selected = ChannelProvider::from_index(channel.selected_provider);
    let mut details: Vec<Line<'_>> = Vec::new();
    details.push(Line::from(vec![
        Span::styled("Selected: ", theme.bold),
        Span::raw(selected.name()),
    ]));
    details.push(Line::from(format!("Auth: {}", selected.auth())));
    details.push(Line::from(if selected.available() {
        "Status: available"
    } else {
        "Status: coming soon"
    }));
    details.push(Line::from(""));
    details.push(Line::from(selected.description()));

    if selected == ChannelProvider::Telegram {
        details.push(Line::from(""));
        details.push(Line::from(format!(
            "Bot username: {}",
            if channel.account_id.trim().is_empty() {
                "(empty)"
            } else {
                channel.account_id.as_str()
            }
        )));
        details.push(Line::from(format!(
            "Bot token: {}",
            common::mask_secret(&channel.token)
        )));
        details.push(Line::from(format!("DM policy: {}", channel.dm_policy)));
        if channel.connected {
            details.push(Line::from(format!(
                "Connected as: @{}",
                channel.connected_name
            )));
        }
        details.push(Line::from(""));
        details.push(Line::from(vec![
            Span::styled("Next: ", theme.bold),
            Span::raw("Press Enter to configure Telegram."),
        ]));
    } else {
        details.push(Line::from(""));
        details.push(Line::from(
            "This channel is not yet available in TUI onboarding.",
        ));
    }

    let details_widget = Paragraph::new(details)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(" Details "),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(details_widget, body[1]);

    render_wide_footer(
        frame,
        &sections,
        onboarding,
        theme,
        channel_actions_hint(onboarding),
    );

    if onboarding.channel.configuring {
        draw_channel_config_modal(frame, area, onboarding, textarea, theme);
    }
    if onboarding.editing.is_some() && channel_inline_edit_target(onboarding).is_none() {
        draw_edit_modal(frame, area, onboarding, input_mode, textarea, theme);
    }
}

fn channel_summary_line(onboarding: &OnboardingState) -> Line<'static> {
    let channel = &onboarding.channel;
    if channel.connected {
        return Line::from(format!(
            "Channels: 1/{} connected (Telegram: @{})",
            ChannelProvider::ALL.len(),
            channel.connected_name
        ));
    }

    Line::from(format!(
        "Channels: 0/{} connected",
        ChannelProvider::ALL.len()
    ))
}

fn channel_actions_hint(onboarding: &OnboardingState) -> &'static str {
    if onboarding.channel.configuring {
        "Actions: j/k move  e edit  [ ] change DM policy  x connect  Esc close"
    } else {
        "Actions: Enter configure  c continue  s skip  b back"
    }
}

fn draw_channel_config_modal(
    frame: &mut Frame,
    area: Rect,
    onboarding: &OnboardingState,
    textarea: &mut TextArea<'_>,
    theme: &Theme,
) {
    if !onboarding.channel.configuring {
        return;
    }
    if ChannelProvider::from_index(onboarding.channel.selected_provider)
        != ChannelProvider::Telegram
    {
        return;
    }

    let popup = common::centered_rect(60, 58, area);
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.border_focused)
        .title(" Configure Telegram ");
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let mut lines: Vec<Line<'_>> = Vec::new();
    let channel = &onboarding.channel;
    let editing_target = channel_inline_edit_target(onboarding);
    let mut inline_input = None;

    lines.push(Line::from("Insert Telegram bot credentials, then connect."));
    lines.push(Line::from(""));

    {
        let mut push_row = |label: &str, target: EditTarget, active: bool, value: String| {
            let marker = if active {
                "▶"
            } else {
                " "
            };
            let prefix = format!("{marker} {label}: ");

            if editing_target == Some(target) {
                inline_input = Some(InlineProviderField {
                    line_index: lines.len() as u16,
                    value_column: prefix.chars().count() as u16,
                    placeholder: target.placeholder(),
                });
                let padding_width = inner
                    .width
                    .saturating_sub(prefix.chars().count() as u16)
                    .saturating_sub(1) as usize;
                lines.push(Line::from(format!(
                    "{}{}",
                    prefix,
                    " ".repeat(padding_width)
                )));
            } else {
                lines.push(Line::from(format!("{prefix}{value}")));
            }
        };

        push_row(
            "Bot username",
            EditTarget::ChannelAccountId,
            channel.field_index == 0,
            if channel.account_id.trim().is_empty() {
                "(empty)".to_string()
            } else {
                channel.account_id.clone()
            },
        );
        push_row(
            "Bot token",
            EditTarget::ChannelToken,
            channel.field_index == 1,
            common::mask_secret(&channel.token),
        );
    }

    lines.push(Line::from(format!(
        "{} DM policy: {}",
        if channel.field_index == 2 {
            "▶"
        } else {
            " "
        },
        channel.dm_policy
    )));

    lines.push(Line::from(format!(
        "{} Allowlist: {}",
        if channel.field_index == 3 {
            "▶"
        } else {
            " "
        },
        if channel.allowlist.trim().is_empty() {
            "(empty)".to_string()
        } else {
            format!(
                "{} entries",
                channel
                    .allowlist
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                    .count()
            )
        }
    )));

    if channel.connected {
        lines.push(Line::from(""));
        lines.push(Line::from(format!(
            "Connected as @{}",
            channel.connected_name
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(
        "Actions: j/k move  e edit  [ ] change DM policy  x connect  Esc close",
    ));

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);

    if let Some(input) = inline_input {
        let x = inner.x.saturating_add(input.value_column);
        let y = inner.y.saturating_add(input.line_index);
        let width = inner
            .width
            .saturating_sub(input.value_column)
            .saturating_sub(1);

        if width > 0 {
            let input_rect = Rect {
                x,
                y,
                width,
                height: 1,
            };
            let input_style = Style::default().add_modifier(Modifier::UNDERLINED);
            textarea.set_style(input_style);
            textarea.set_placeholder_text(input.placeholder);
            textarea.set_cursor_line_style(input_style);
            textarea.set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
            textarea.set_block(Block::default());
            frame.render_widget(&*textarea, input_rect);
        }
    }
}

fn channel_inline_edit_target(onboarding: &OnboardingState) -> Option<EditTarget> {
    let target = onboarding.editing?;
    let is_channel_target = matches!(
        target,
        EditTarget::ChannelAccountId | EditTarget::ChannelToken
    );
    if is_channel_target && onboarding.channel.configuring {
        Some(target)
    } else {
        None
    }
}

fn draw_identity_screen(
    frame: &mut Frame,
    area: Rect,
    onboarding: &OnboardingState,
    input_mode: InputMode,
    textarea: &mut TextArea<'_>,
    theme: &Theme,
) {
    if area.width < 90 || area.height < 16 {
        render_step_compact(frame, area, onboarding, theme, |lines| {
            draw_identity_compact(lines, onboarding, theme);
        });
        if onboarding.editing.is_some() && identity_inline_edit_target(onboarding).is_none() {
            draw_edit_modal(frame, area, onboarding, input_mode, textarea, theme);
        }
        return;
    }

    let sections = render_wide_header(
        frame,
        area,
        onboarding,
        theme,
        identity_summary_line(onboarding),
    );
    let body = Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(sections[4]);

    let identity = &onboarding.identity;

    let field_rows = [
        (
            EditTarget::IdentityUserName,
            "Your name",
            identity.user_name.clone(),
        ),
        (
            EditTarget::IdentityAgentName,
            "Agent name",
            identity.agent_name.clone(),
        ),
        (EditTarget::IdentityEmoji, "Emoji", identity.emoji.clone()),
        (
            EditTarget::IdentityCreature,
            "Creature",
            identity.creature.clone(),
        ),
        (EditTarget::IdentityVibe, "Vibe", identity.vibe.clone()),
    ];

    // --- Left column: field list ---
    let editing_target = identity_inline_edit_target(onboarding);
    let mut inline_input = None;
    let mut lines: Vec<Line<'_>> = Vec::new();

    for (index, (target, label, value)) in field_rows.iter().enumerate() {
        let active = index == identity.field_index;
        let marker = if active {
            "▶ "
        } else {
            "  "
        };
        let label_text = format!("{:<12}", label);

        if editing_target == Some(*target) && active {
            let prefix_len = (marker.len() + label_text.trim_end().len() + 2) as u16;
            inline_input = Some(InlineProviderField {
                line_index: lines.len() as u16,
                value_column: prefix_len,
                placeholder: target.placeholder(),
            });
            lines.push(Line::from(vec![
                Span::styled(marker, theme.sidebar_active),
                Span::styled(label_text, theme.bold),
                Span::raw("  "),
            ]));
        } else {
            let (display, value_style) = if value.trim().is_empty() {
                ("(empty)".to_string(), theme.system_msg)
            } else {
                (value.clone(), Style::default())
            };
            let marker_style = if active {
                theme.sidebar_active
            } else {
                Style::default()
            };
            lines.push(Line::from(vec![
                Span::styled(marker, marker_style),
                Span::styled(label_text, theme.bold),
                Span::styled(display, value_style),
            ]));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  e", theme.footer_key),
        Span::styled(" edit field  ", theme.footer_desc),
        Span::styled("j/k", theme.footer_key),
        Span::styled(" navigate", theme.footer_desc),
    ]));

    let fields_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" Fields ");
    let fields_inner = fields_block.inner(body[0]);
    let fields_widget = Paragraph::new(lines)
        .block(fields_block)
        .wrap(Wrap { trim: false });
    frame.render_widget(fields_widget, body[0]);

    if let Some(input) = inline_input {
        let x = fields_inner.x.saturating_add(input.value_column);
        let y = fields_inner.y.saturating_add(input.line_index);
        let width = fields_inner
            .width
            .saturating_sub(input.value_column)
            .saturating_sub(1);

        if width > 0 {
            let input_rect = Rect {
                x,
                y,
                width,
                height: 1,
            };
            let input_style = Style::default().add_modifier(Modifier::UNDERLINED);
            textarea.set_style(input_style);
            textarea.set_placeholder_text(input.placeholder);
            textarea.set_cursor_line_style(input_style);
            textarea.set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
            textarea.set_block(Block::default());
            frame.render_widget(&*textarea, input_rect);
        }
    }

    // --- Right column: agent preview ---
    let emoji = if identity.emoji.trim().is_empty() {
        "🤖"
    } else {
        identity.emoji.trim()
    };
    let name = if identity.agent_name.trim().is_empty() {
        "Moltis"
    } else {
        identity.agent_name.trim()
    };
    let vibe = if identity.vibe.trim().is_empty() {
        "default"
    } else {
        identity.vibe.trim()
    };

    let mut preview: Vec<Line<'_>> = Vec::new();
    preview.push(Line::from(""));
    preview.push(Line::from(vec![Span::styled(
        format!("  {emoji}  {name}"),
        theme.heading,
    )]));
    preview.push(Line::from(""));

    if !identity.user_name.trim().is_empty() {
        preview.push(Line::from(vec![
            Span::styled("  Owner     ", theme.bold),
            Span::raw(identity.user_name.trim().to_string()),
        ]));
    }
    preview.push(Line::from(vec![
        Span::styled("  Vibe      ", theme.bold),
        Span::raw(vibe.to_string()),
    ]));
    if !identity.creature.trim().is_empty() {
        preview.push(Line::from(vec![
            Span::styled("  Creature  ", theme.bold),
            Span::raw(identity.creature.trim().to_string()),
        ]));
    }

    let preview_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" Agent Preview ");
    frame.render_widget(
        Paragraph::new(preview)
            .block(preview_block)
            .wrap(Wrap { trim: false }),
        body[1],
    );

    render_wide_footer(frame, &sections, onboarding, theme, identity_actions_hint());

    if onboarding.editing.is_some() && identity_inline_edit_target(onboarding).is_none() {
        draw_edit_modal(frame, area, onboarding, input_mode, textarea, theme);
    }
}

fn identity_summary_line(onboarding: &OnboardingState) -> Line<'static> {
    let identity = &onboarding.identity;
    let values = [
        identity.user_name.as_str(),
        identity.agent_name.as_str(),
        identity.emoji.as_str(),
        identity.creature.as_str(),
        identity.vibe.as_str(),
    ];
    let filled = values
        .iter()
        .filter(|value| !value.trim().is_empty())
        .count();
    Line::from(format!("Identity fields: {filled}/5 filled"))
}

fn identity_actions_hint() -> &'static str {
    "Actions: j/k move  e edit field  c save and continue  b back"
}

fn identity_inline_edit_target(onboarding: &OnboardingState) -> Option<EditTarget> {
    let target = onboarding.editing?;
    let is_identity_target = matches!(
        target,
        EditTarget::IdentityUserName
            | EditTarget::IdentityAgentName
            | EditTarget::IdentityEmoji
            | EditTarget::IdentityCreature
            | EditTarget::IdentityVibe
    );
    if is_identity_target {
        Some(target)
    } else {
        None
    }
}

#[derive(Clone, Copy)]
struct InlineProviderField {
    line_index: u16,
    value_column: u16,
    placeholder: &'static str,
}

fn draw_llm_config_modal(
    frame: &mut Frame,
    area: Rect,
    onboarding: &OnboardingState,
    textarea: &mut TextArea<'_>,
    theme: &Theme,
) {
    let Some(config) = onboarding.llm.configuring.as_ref() else {
        return;
    };

    let popup = common::centered_rect(56, 58, area);
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.border_focused)
        .title(format!(" Configure {} ", config.provider_display_name));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let mut lines: Vec<Line<'_>> = Vec::new();
    let mut inline_input = None;

    match &config.phase {
        ProviderConfigurePhase::Form => {
            let editing_target = provider_inline_edit_target(onboarding);
            lines.push(Line::from(
                "Provider selected. Edit fields, then validate/save.",
            ));
            lines.push(Line::from(""));

            let mut push_row = |label: &str, target: EditTarget, active: bool, value: String| {
                let marker = if active {
                    "▶"
                } else {
                    " "
                };
                let prefix = format!("{marker} {label}: ");

                if editing_target == Some(target) {
                    inline_input = Some(InlineProviderField {
                        line_index: lines.len() as u16,
                        value_column: prefix.chars().count() as u16,
                        placeholder: target.placeholder(),
                    });

                    let padding_width = inner
                        .width
                        .saturating_sub(prefix.chars().count() as u16)
                        .saturating_sub(1) as usize;
                    lines.push(Line::from(format!(
                        "{}{}",
                        prefix,
                        " ".repeat(padding_width)
                    )));
                } else {
                    lines.push(Line::from(format!("{prefix}{value}")));
                }
            };

            let mut row_index = 0usize;
            push_row(
                "API key",
                EditTarget::ProviderApiKey,
                config.field_index == row_index,
                common::mask_secret(&config.api_key),
            );

            if supports_endpoint(&config.provider_name) {
                row_index += 1;
                push_row(
                    "Endpoint",
                    EditTarget::ProviderEndpoint,
                    config.field_index == row_index,
                    if config.endpoint.is_empty() {
                        "(empty)".to_string()
                    } else {
                        config.endpoint.clone()
                    },
                );
            }

            if config.requires_model {
                row_index += 1;
                push_row(
                    "Model",
                    EditTarget::ProviderModel,
                    config.field_index == row_index,
                    if config.model.is_empty() {
                        "(empty)".to_string()
                    } else {
                        config.model.clone()
                    },
                );
            }

            lines.push(Line::from(""));
            lines.push(Line::from(
                "Actions: j/k move  e edit  m models  v validate/save  Esc close",
            ));
        },
        ProviderConfigurePhase::ModelSelect {
            models,
            selected,
            cursor,
        } => {
            lines.push(Line::from("Choose preferred models."));
            lines.push(Line::from(""));
            for (index, model) in models.iter().enumerate().take(10) {
                let marker = if index == *cursor {
                    "▶"
                } else {
                    " "
                };
                let check = if selected.contains(&model.id) {
                    "[x]"
                } else {
                    "[ ]"
                };
                lines.push(Line::from(format!(
                    "{marker} {check} {} ({})",
                    model.display_name, model.id
                )));
            }
            if models.len() > 10 {
                lines.push(Line::from(format!("... {} more models", models.len() - 10)));
            }
            lines.push(Line::from(""));
            lines.push(Line::from(
                "Actions: j/k move  Space toggle  Enter save  Esc close",
            ));
        },
        ProviderConfigurePhase::OAuth {
            auth_url,
            verification_uri,
            user_code,
        } => {
            lines.push(Line::from("Complete OAuth in your browser."));
            lines.push(Line::from(""));
            if let Some(url) = auth_url {
                lines.push(Line::from("Auth URL:"));
                lines.push(Line::from(url.clone()));
                lines.push(Line::from(""));
            }
            if let Some(uri) = verification_uri {
                lines.push(Line::from("Verification URL:"));
                lines.push(Line::from(uri.clone()));
            }
            if let Some(code) = user_code {
                lines.push(Line::from(format!("Code: {code}")));
            }
            lines.push(Line::from(""));
            lines.push(Line::from("Actions: Enter/p poll status  Esc close"));
        },
        ProviderConfigurePhase::Local {
            backend,
            models,
            cursor,
            note,
        } => {
            lines.push(Line::from(format!("Backend: {backend}")));
            if let Some(note) = note {
                lines.push(Line::from(note.clone()));
            }
            lines.push(Line::from(""));
            lines.push(Line::from("Recommended models:"));
            for (index, model) in models.iter().enumerate().take(8) {
                let marker = if index == *cursor {
                    "▶"
                } else {
                    " "
                };
                lines.push(Line::from(format!(
                    "{marker} {} ({}GB RAM, {}k ctx)",
                    model.display_name,
                    model.min_ram_gb,
                    model.context_window / 1000
                )));
            }
            if models.len() > 8 {
                lines.push(Line::from(format!("... {} more models", models.len() - 8)));
            }
            lines.push(Line::from(""));
            lines.push(Line::from("Actions: j/k move  Enter configure  Esc close"));
        },
    }

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);

    if let Some(input) = inline_input {
        let x = inner.x.saturating_add(input.value_column);
        let y = inner.y.saturating_add(input.line_index);
        let width = inner
            .width
            .saturating_sub(input.value_column)
            .saturating_sub(1);

        if width > 0 {
            let input_rect = Rect {
                x,
                y,
                width,
                height: 1,
            };
            let input_style = Style::default().add_modifier(Modifier::UNDERLINED);
            textarea.set_style(input_style);
            textarea.set_placeholder_text(input.placeholder);
            textarea.set_cursor_line_style(input_style);
            textarea.set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
            textarea.set_block(Block::default());
            frame.render_widget(&*textarea, input_rect);
        }
    }
}

fn draw_edit_modal(
    frame: &mut Frame,
    area: Rect,
    onboarding: &OnboardingState,
    input_mode: InputMode,
    textarea: &mut TextArea<'_>,
    theme: &Theme,
) {
    let Some(target) = onboarding.editing else {
        return;
    };

    let surface = Color::Rgb(46, 58, 78);
    let content_style = Style::default().fg(Color::White).bg(surface);
    let popup = common::centered_rect(62, 44, area);
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.border_focused)
        .style(content_style)
        .title(" Edit Field ");
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let layout = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(1),
    ])
    .split(inner);

    let headline = Line::from(vec![
        Span::styled("Editing: ", theme.bold),
        Span::raw(target.placeholder()),
    ]);
    frame.render_widget(Paragraph::new(headline).style(content_style), layout[0]);

    let input_style = Style::default()
        .fg(Color::White)
        .bg(Color::Rgb(60, 78, 104));
    textarea.set_style(input_style);
    textarea.set_cursor_line_style(
        Style::default()
            .fg(Color::White)
            .bg(Color::Rgb(84, 108, 140)),
    );
    textarea.set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
    textarea.set_block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(theme.mode_insert)
            .style(input_style)
            .title(" Value "),
    );
    frame.render_widget(&*textarea, layout[1]);

    let footer = if input_mode == InputMode::Insert {
        "Enter save  Shift+Enter newline  Esc cancel"
    } else {
        "Press Enter to save or Esc to cancel"
    };
    frame.render_widget(Paragraph::new(footer).style(content_style), layout[2]);
}

fn onboarding_intro_line(theme: &Theme) -> Line<'static> {
    Line::from(vec![Span::styled(
        "Please proceed to onboarding to finish setup before chatting.",
        theme.approval_highlight.add_modifier(Modifier::BOLD),
    )])
}

fn provider_inline_edit_target(onboarding: &OnboardingState) -> Option<EditTarget> {
    let target = onboarding.editing?;
    let is_provider_target = matches!(
        target,
        EditTarget::ProviderApiKey | EditTarget::ProviderEndpoint | EditTarget::ProviderModel
    );
    if !is_provider_target {
        return None;
    }

    let is_form = onboarding
        .llm
        .configuring
        .as_ref()
        .is_some_and(|config| matches!(config.phase, ProviderConfigurePhase::Form));

    if is_form {
        Some(target)
    } else {
        None
    }
}

fn step_indicator(onboarding: &OnboardingState, theme: &Theme) -> Line<'static> {
    let mut spans = Vec::new();

    for (idx, step) in onboarding.steps.iter().enumerate() {
        let label = format!(" {}.{} ", idx + 1, step.label());
        let style = if idx < onboarding.step_index {
            theme.tool_success
        } else if idx == onboarding.step_index {
            theme.mode_insert
        } else {
            theme.system_msg
        };
        spans.push(Span::styled(label, style));

        if idx + 1 < onboarding.steps.len() {
            spans.push(Span::raw(" "));
        }
    }

    Line::from(spans)
}

fn draw_security<'a>(lines: &mut Vec<Line<'a>>, onboarding: &'a OnboardingState, theme: &Theme) {
    let security = &onboarding.security;

    if security.setup_complete {
        lines.push(Line::from("Authentication is already configured."));
        lines.push(Line::from("Press c to continue."));
        return;
    }

    if security.webauthn_available {
        lines.push(Line::from(
            "Passkeys are available in web onboarding. This TUI flow currently supports password setup.",
        ));
    }

    if security.localhost_only {
        lines.push(Line::from(
            "Localhost mode detected, password can be left empty to skip setup.",
        ));
    }

    if security.setup_code_required {
        let active = security.field_index == 0;
        editable_row(
            lines,
            active,
            "Setup code",
            &security.setup_code,
            false,
            theme,
        );
    }

    let pw_index = if security.setup_code_required {
        1
    } else {
        0
    };
    let confirm_index = pw_index + 1;

    editable_row(
        lines,
        security.field_index == pw_index,
        "Password",
        &security.password,
        true,
        theme,
    );

    editable_row(
        lines,
        security.field_index == confirm_index,
        "Confirm password",
        &security.confirm_password,
        true,
        theme,
    );

    lines.push(Line::from(""));
    lines.push(Line::from("Actions: e edit field, c save and continue"));
    if security.skippable || security.localhost_only {
        lines.push(Line::from("Skip: s"));
    }
}

fn draw_llm_compact<'a>(lines: &mut Vec<Line<'a>>, onboarding: &'a OnboardingState, theme: &Theme) {
    let llm = &onboarding.llm;

    if llm.providers.is_empty() {
        lines.push(Line::from("No providers returned by gateway."));
        lines.push(Line::from("Press r to refresh, c to continue, s to skip."));
        return;
    }

    let configured = llm
        .providers
        .iter()
        .filter(|p| p.configured)
        .map(|p| p.display_name.as_str())
        .collect::<Vec<&str>>();

    if !configured.is_empty() {
        lines.push(Line::from(format!(
            "Detected LLM providers: {}",
            configured.join(", ")
        )));
        lines.push(Line::from(""));
    }

    for (idx, provider) in llm.providers.iter().enumerate() {
        let selected = idx == llm.selected_provider;
        let marker = if selected {
            ">"
        } else {
            " "
        };
        let configured_marker = if provider.configured {
            Span::styled("configured", theme.tool_success)
        } else {
            Span::styled("not configured", theme.system_msg)
        };
        lines.push(Line::from(vec![
            Span::raw(format!("{marker} {} ", provider.display_name)),
            Span::styled(format!("[{}] ", provider.auth_type), theme.system_msg),
            configured_marker,
        ]));
    }

    if let Some(config) = llm.configuring.as_ref() {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled(
            format!("Configuring {}", config.provider_display_name),
            theme.bold,
        )]));

        match &config.phase {
            ProviderConfigurePhase::Form => {
                let mut row_index = 0usize;
                editable_row(
                    lines,
                    config.field_index == row_index,
                    "API key",
                    &config.api_key,
                    true,
                    theme,
                );

                if supports_endpoint(&config.provider_name) {
                    row_index += 1;
                    editable_row(
                        lines,
                        config.field_index == row_index,
                        "Endpoint",
                        &config.endpoint,
                        false,
                        theme,
                    );
                }

                if config.requires_model {
                    row_index += 1;
                    editable_row(
                        lines,
                        config.field_index == row_index,
                        "Model",
                        &config.model,
                        false,
                        theme,
                    );
                }

                lines.push(Line::from(""));
                lines.push(Line::from(
                    "Actions: e edit, v validate and save, esc cancel",
                ));
            },
            ProviderConfigurePhase::ModelSelect {
                models,
                selected,
                cursor,
            } => {
                lines.push(Line::from("Select preferred models:"));
                for (idx, model) in models.iter().enumerate() {
                    let marker = if idx == *cursor {
                        ">"
                    } else {
                        " "
                    };
                    let checked = if selected.contains(&model.id) {
                        "[x]"
                    } else {
                        "[ ]"
                    };
                    let tools = if model.supports_tools {
                        " tools"
                    } else {
                        ""
                    };
                    lines.push(Line::from(format!(
                        "{marker} {checked} {} ({}){tools}",
                        model.display_name, model.id
                    )));
                }
                lines.push(Line::from(
                    "Actions: space toggle, enter save selection, esc cancel",
                ));
            },
            ProviderConfigurePhase::OAuth {
                auth_url,
                verification_uri,
                user_code,
            } => {
                if let Some(url) = auth_url {
                    lines.push(Line::from("Open this URL to continue OAuth:"));
                    lines.push(Line::from(url.clone()));
                }
                if let Some(uri) = verification_uri {
                    lines.push(Line::from("Device verification URL:"));
                    lines.push(Line::from(uri.clone()));
                }
                if let Some(code) = user_code {
                    lines.push(Line::from(format!("User code: {code}")));
                }
                lines.push(Line::from("Actions: p poll status, esc cancel"));
            },
            ProviderConfigurePhase::Local {
                backend,
                models,
                cursor,
                note,
            } => {
                lines.push(Line::from(format!("Backend: {backend}")));
                if let Some(note) = note {
                    lines.push(Line::from(note.clone()));
                }
                lines.push(Line::from("Recommended local models:"));
                for (idx, model) in models.iter().enumerate() {
                    let marker = if idx == *cursor {
                        ">"
                    } else {
                        " "
                    };
                    let suggested = if model.suggested {
                        " recommended"
                    } else {
                        ""
                    };
                    lines.push(Line::from(format!(
                        "{marker} {} ({}GB RAM, {}k ctx){suggested}",
                        model.display_name,
                        model.min_ram_gb,
                        model.context_window / 1000
                    )));
                }
                lines.push(Line::from("Actions: enter configure model, esc cancel"));
            },
        }
    } else {
        lines.push(Line::from(""));
        lines.push(Line::from(
            "Actions: enter configure provider, c continue, s skip, r refresh",
        ));
    }
}

fn draw_voice_compact(lines: &mut Vec<Line<'_>>, onboarding: &OnboardingState, _theme: &Theme) {
    let voice = &onboarding.voice;

    if voice.providers.is_empty() {
        lines.push(Line::from("No voice providers available."));
        lines.push(Line::from(
            "Press r to refresh, c to continue, or s to skip.",
        ));
        return;
    }

    lines.push(Line::from(
        "Configure optional voice providers. You can set this up later in Settings.",
    ));
    lines.push(Line::from(""));

    for (idx, provider) in voice.providers.iter().enumerate() {
        let marker = if idx == voice.selected_provider {
            ">"
        } else {
            " "
        };
        let enabled = if provider.enabled {
            "enabled"
        } else {
            "disabled"
        };
        let available = if provider.available {
            "available"
        } else {
            "needs key"
        };
        lines.push(Line::from(format!(
            "{marker} {} [{} {}] {}",
            provider.name, provider.provider_type, provider.category, enabled
        )));
        lines.push(Line::from(format!("    {available}")));
        if let Some(source) = provider.key_source.as_deref() {
            lines.push(Line::from(format!("    key source: {source}")));
        }
        if let Some(desc) = provider.description.as_deref() {
            lines.push(Line::from(format!("    {desc}")));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(
        "Actions: t toggle provider, e edit API key, v save key, r refresh, c continue, s skip, b back",
    ));
}

fn draw_channel_compact(lines: &mut Vec<Line<'_>>, onboarding: &OnboardingState, _theme: &Theme) {
    let channel = &onboarding.channel;

    let selected = ChannelProvider::from_index(channel.selected_provider);
    lines.push(Line::from("Choose a channel integration for onboarding."));

    for (idx, provider) in ChannelProvider::ALL.iter().enumerate() {
        let marker = if idx == channel.selected_provider {
            ">"
        } else {
            " "
        };
        let status = match provider {
            ChannelProvider::Telegram => {
                if channel.connected {
                    "connected"
                } else {
                    "not configured"
                }
            },
            ChannelProvider::Slack | ChannelProvider::Discord => "coming soon",
        };
        lines.push(Line::from(format!(
            "{marker} {} [{}] {status}",
            provider.name(),
            provider.auth()
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(selected.description()));
    if selected == ChannelProvider::Telegram {
        lines.push(Line::from(format!(
            "Telegram user: {}",
            if channel.account_id.trim().is_empty() {
                "(empty)"
            } else {
                channel.account_id.as_str()
            }
        )));
        lines.push(Line::from(format!(
            "Telegram token: {}",
            common::mask_secret(&channel.token)
        )));
    }

    if channel.configuring {
        lines.push(Line::from(""));
        lines.push(Line::from(
            "Actions: j/k move fields, e edit, [ ] DM policy, x connect, Esc close",
        ));
    } else {
        lines.push(Line::from(""));
        lines.push(Line::from(
            "Actions: enter configure, c continue, s skip, b back",
        ));
    }
}

fn draw_identity_compact<'a>(
    lines: &mut Vec<Line<'a>>,
    onboarding: &'a OnboardingState,
    theme: &Theme,
) {
    let identity = &onboarding.identity;

    lines.push(Line::from(
        "Tell us about yourself and customize your agent.",
    ));
    lines.push(Line::from(""));

    editable_row(
        lines,
        identity.field_index == 0,
        "Your name",
        &identity.user_name,
        false,
        theme,
    );
    editable_row(
        lines,
        identity.field_index == 1,
        "Agent name",
        &identity.agent_name,
        false,
        theme,
    );
    editable_row(
        lines,
        identity.field_index == 2,
        "Emoji",
        &identity.emoji,
        false,
        theme,
    );
    editable_row(
        lines,
        identity.field_index == 3,
        "Creature",
        &identity.creature,
        false,
        theme,
    );
    editable_row(
        lines,
        identity.field_index == 4,
        "Vibe",
        &identity.vibe,
        false,
        theme,
    );

    lines.push(Line::from(""));
    lines.push(Line::from(
        "Actions: e edit field, c save and continue, b back",
    ));
}

fn draw_summary(lines: &mut Vec<Line<'_>>, onboarding: &OnboardingState, theme: &Theme) {
    let summary = &onboarding.summary;

    lines.push(Line::from(
        "Review your setup, then finish onboarding to open chat.",
    ));
    lines.push(Line::from(""));

    lines.push(Line::from("Identity:"));
    if let Some(line) = summary.identity_line.as_deref() {
        lines.push(Line::from(format!("  {line}")));
    } else {
        lines.push(Line::from("  Identity not fully configured"));
    }

    lines.push(Line::from(""));
    lines.push(Line::from("LLMs:"));
    if summary.provider_badges.is_empty() {
        lines.push(Line::from("  No providers configured"));
    } else {
        lines.push(Line::from(format!(
            "  {}",
            summary.provider_badges.join(", ")
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from("Channels:"));
    if summary.channels.is_empty() {
        lines.push(Line::from("  No channels configured"));
    } else {
        for channel in &summary.channels {
            lines.push(Line::from(format!(
                "  {} ({})",
                channel.name, channel.status
            )));
        }
    }

    if !summary.voice_enabled.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from("Voice:"));
        lines.push(Line::from(format!(
            "  {}",
            summary.voice_enabled.join(", ")
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![Span::styled(
        " Finish Onboarding (Enter) ",
        theme.mode_insert.add_modifier(Modifier::BOLD),
    )]));
    lines.push(Line::from("Secondary actions: r refresh summary, b back"));
}

fn draw_summary_screen(frame: &mut Frame, area: Rect, onboarding: &OnboardingState, theme: &Theme) {
    if area.width < 90 || area.height < 16 {
        render_step_compact(frame, area, onboarding, theme, |lines| {
            draw_summary(lines, onboarding, theme);
        });
        return;
    }

    let sections = render_wide_header(
        frame,
        area,
        onboarding,
        theme,
        summary_summary_line(onboarding),
    );
    let body = Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(sections[4]);

    // --- Left column: Setup Review ---
    let summary = &onboarding.summary;
    let mut lines: Vec<Line<'_>> = Vec::new();

    // Identity section
    let (identity_icon, identity_style) = if summary.identity_line.is_some() {
        ("\u{2713}", theme.tool_success) // checkmark
    } else {
        ("\u{2717}", theme.system_msg) // cross
    };
    lines.push(Line::from(vec![
        Span::styled(format!(" {identity_icon} "), identity_style),
        Span::styled("Identity", theme.bold),
    ]));
    if let Some(line) = summary.identity_line.as_deref() {
        lines.push(Line::from(format!("     {line}")));
    } else {
        lines.push(Line::from(vec![Span::styled(
            "     Not configured",
            theme.system_msg,
        )]));
    }
    lines.push(Line::from(""));

    // LLMs section
    let (llm_icon, llm_style) = if !summary.provider_badges.is_empty() {
        ("\u{2713}", theme.tool_success)
    } else {
        ("\u{2717}", theme.system_msg)
    };
    lines.push(Line::from(vec![
        Span::styled(format!(" {llm_icon} "), llm_style),
        Span::styled("LLMs", theme.bold),
    ]));
    if summary.provider_badges.is_empty() {
        lines.push(Line::from(vec![Span::styled(
            "     No providers configured",
            theme.system_msg,
        )]));
    } else {
        lines.push(Line::from(format!(
            "     {}",
            summary.provider_badges.join(", ")
        )));
    }
    lines.push(Line::from(""));

    // Channels section
    let has_channels = !summary.channels.is_empty();
    let (ch_icon, ch_style) = if has_channels {
        ("\u{2713}", theme.tool_success)
    } else {
        ("\u{2717}", theme.system_msg)
    };
    lines.push(Line::from(vec![
        Span::styled(format!(" {ch_icon} "), ch_style),
        Span::styled("Channels", theme.bold),
    ]));
    if summary.channels.is_empty() {
        lines.push(Line::from(vec![Span::styled(
            "     No channels configured",
            theme.system_msg,
        )]));
    } else {
        for channel in &summary.channels {
            lines.push(Line::from(format!(
                "     {} ({})",
                channel.name, channel.status
            )));
        }
    }
    lines.push(Line::from(""));

    // Voice section
    let (voice_icon, voice_style) = if !summary.voice_enabled.is_empty() {
        ("\u{2713}", theme.tool_success)
    } else {
        ("\u{2717}", theme.system_msg)
    };
    lines.push(Line::from(vec![
        Span::styled(format!(" {voice_icon} "), voice_style),
        Span::styled("Voice", theme.bold),
    ]));
    if summary.voice_enabled.is_empty() {
        lines.push(Line::from(vec![Span::styled(
            "     No voice providers enabled",
            theme.system_msg,
        )]));
    } else {
        lines.push(Line::from(format!(
            "     {}",
            summary.voice_enabled.join(", ")
        )));
    }

    let review_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" Setup Review ");
    frame.render_widget(
        Paragraph::new(lines)
            .block(review_block)
            .wrap(Wrap { trim: false }),
        body[0],
    );

    // --- Right column: Get Started ---
    let right: Vec<Line<'_>> = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            "  Finish Onboarding (Enter) ",
            theme.mode_insert.add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from("  This will save your configuration and open chat."),
        Line::from("  You can change settings any time from the"),
        Line::from("  Settings tab."),
        Line::from(""),
        Line::from(vec![
            Span::styled("  r", theme.footer_key),
            Span::styled(" refresh summary  ", theme.footer_desc),
            Span::styled("b", theme.footer_key),
            Span::styled(" back", theme.footer_desc),
        ]),
    ];

    let started_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" Get Started ");
    frame.render_widget(
        Paragraph::new(right)
            .block(started_block)
            .wrap(Wrap { trim: false }),
        body[1],
    );

    render_wide_footer(
        frame,
        &sections,
        onboarding,
        theme,
        "Actions: Enter finish  r refresh  b back",
    );
}

fn summary_summary_line(onboarding: &OnboardingState) -> Line<'static> {
    let summary = &onboarding.summary;
    let mut configured = 0u8;

    if summary.identity_line.is_some() {
        configured += 1;
    }
    if !summary.provider_badges.is_empty() {
        configured += 1;
    }
    if !summary.channels.is_empty() {
        configured += 1;
    }
    if !summary.voice_enabled.is_empty() {
        configured += 1;
    }

    Line::from(format!("{configured}/4 sections configured"))
}

fn editable_row<'a>(
    lines: &mut Vec<Line<'a>>,
    active: bool,
    label: &'a str,
    value: &'a str,
    secret: bool,
    theme: &Theme,
) {
    lines.extend(common::form_field(label, value, active, "", secret, theme));
}

fn hints_line(onboarding: &OnboardingState, theme: &Theme) -> Line<'static> {
    let mut parts = vec![
        Span::styled("Keys: ", theme.bold),
        Span::raw("j/k move  "),
        Span::raw("e edit  "),
        Span::raw("b back  "),
    ];

    if onboarding.busy {
        parts.push(Span::styled("working...", theme.thinking));
    } else if onboarding.llm.configuring.is_some()
        || onboarding.channel.configuring
        || onboarding.editing.is_some()
    {
        parts.push(Span::raw("Esc close modal"));
    } else {
        parts.push(Span::raw("Esc quit"));
    }

    Line::from(parts)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::onboarding::{
            LlmState, ProviderConfigurePhase, ProviderConfigureState, ProviderEntry,
        },
    };

    #[test]
    fn llm_summary_shows_configured_count() {
        let mut onboarding = OnboardingState::new(false, false, false, None);
        onboarding.llm = LlmState {
            providers: vec![
                ProviderEntry {
                    name: "openai".into(),
                    display_name: "OpenAI".into(),
                    auth_type: "api-key".into(),
                    configured: true,
                    default_base_url: None,
                    base_url: None,
                    models: Vec::new(),
                    requires_model: false,
                    key_optional: false,
                },
                ProviderEntry {
                    name: "anthropic".into(),
                    display_name: "Anthropic".into(),
                    auth_type: "api-key".into(),
                    configured: false,
                    default_base_url: None,
                    base_url: None,
                    models: Vec::new(),
                    requires_model: false,
                    key_optional: false,
                },
            ],
            selected_provider: 0,
            configuring: None,
        };

        let line = llm_summary_line(&onboarding);
        let text = line
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>();
        assert!(text.contains("1/2 configured"));
    }

    #[test]
    fn llm_actions_change_for_model_selection() {
        let mut onboarding = OnboardingState::new(false, false, false, None);
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
            phase: ProviderConfigurePhase::ModelSelect {
                models: Vec::new(),
                selected: std::collections::BTreeSet::new(),
                cursor: 0,
            },
        });

        assert!(llm_actions_hint(&onboarding).contains("Space toggle"));
    }

    #[test]
    fn llm_actions_show_model_picker_shortcut_in_form() {
        let mut onboarding = OnboardingState::new(false, false, false, None);
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

        assert!(llm_actions_hint(&onboarding).contains("m models"));
    }
}
