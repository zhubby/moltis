use {
    super::theme::Theme,
    crate::state::{AppState, InputMode},
    ratatui::{
        Frame,
        layout::{Constraint, Layout, Rect},
        style::{Color, Modifier, Style},
        text::{Line, Span},
        widgets::{Block, BorderType, Borders, Paragraph},
    },
    tui_textarea::TextArea,
};

/// Render the input area.
pub fn draw(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    textarea: &mut TextArea<'_>,
    theme: &Theme,
) {
    let mut input_area = area;
    let mut slash_menu_area = None;
    if matches!(state.input_mode, InputMode::Insert)
        && !state.slash_menu_items.is_empty()
        && area.height > 3
    {
        let chunks = Layout::vertical([Constraint::Length(3), Constraint::Min(1)]).split(area);
        input_area = chunks[0];
        slash_menu_area = Some(chunks[1]);
    }

    // Configure textarea style based on mode
    match state.input_mode {
        InputMode::Insert => {
            textarea.set_cursor_line_style(Style::default());
            textarea.set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
            let title = if state.shell_mode_enabled {
                " Input (/sh mode, Enter to send, Shift+Enter for newline) "
            } else {
                " Input (Enter to send, Shift+Enter for newline, /help for commands) "
            };
            textarea.set_block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(theme.border_focused)
                    .style(theme.input_bg)
                    .title(title),
            );
        },
        InputMode::Normal => {
            textarea.set_cursor_line_style(Style::default());
            textarea.set_cursor_style(Style::default().fg(Color::DarkGray));
            textarea.set_block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(theme.border)
                    .title(" Navigate (i to type) "),
            );
        },
        InputMode::Command => {
            textarea.set_cursor_line_style(Style::default());
            textarea.set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
            textarea.set_block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(theme.border_focused)
                    .style(theme.input_bg)
                    .title(format!(" :{} ", state.command_buffer)),
            );
        },
    }

    frame.render_widget(&*textarea, input_area);

    if let Some(menu_area) = slash_menu_area {
        let max_items = menu_area.height.saturating_sub(2) as usize;
        let lines: Vec<Line<'_>> = state
            .slash_menu_items
            .iter()
            .take(max_items)
            .enumerate()
            .map(|(index, item)| {
                let selected = index == state.slash_menu_selected;
                let marker_style = if selected {
                    theme.mode_insert
                } else {
                    theme.footer_desc
                };
                let name_style = if selected {
                    theme.mode_insert.add_modifier(Modifier::BOLD)
                } else {
                    theme.footer_key
                };
                let desc_style = if selected {
                    theme.footer_desc.add_modifier(Modifier::BOLD)
                } else {
                    theme.footer_desc
                };
                Line::from(vec![
                    Span::styled(
                        if selected {
                            "▶ "
                        } else {
                            "  "
                        },
                        marker_style,
                    ),
                    Span::styled(format!("/{}", item.name), name_style),
                    Span::raw(" "),
                    Span::styled(item.description.as_str(), desc_style),
                ])
            })
            .collect();

        let menu = Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(theme.border)
                .title(" Slash Commands "),
        );
        frame.render_widget(menu, menu_area);
    }
}
