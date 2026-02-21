use {
    super::{markdown, theme::Theme},
    crate::state::{AppState, DisplayMessage, MessageRole},
    ratatui::{
        Frame,
        layout::Rect,
        style::{Modifier, Style},
        text::{Line, Span},
        widgets::{Block, Borders, Paragraph, Wrap},
    },
};

/// Render the chat message list.
pub fn draw(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    let mut all_lines: Vec<Line<'_>> = Vec::new();

    // Render existing messages
    for msg in &state.messages {
        render_message(&mut all_lines, msg, theme);
        all_lines.push(Line::from("")); // spacing between messages
    }

    // Render current streaming content
    if state.is_streaming() {
        // Thinking indicator
        if state.thinking_active {
            let spinner = thinking_spinner();
            all_lines.push(Line::from(vec![
                Span::styled(format!("{spinner} "), theme.thinking),
                Span::styled("Thinking...", theme.thinking),
            ]));
            if !state.thinking_text.is_empty() {
                let thinking_lines = markdown::render_markdown(&state.thinking_text, theme);
                for line in thinking_lines {
                    let dimmed: Vec<Span<'_>> = line
                        .spans
                        .into_iter()
                        .map(|s| {
                            Span::styled(s.content.to_string(), s.style.add_modifier(Modifier::DIM))
                        })
                        .collect();
                    all_lines.push(Line::from(dimmed));
                }
            }
        }

        // Streaming text
        if !state.stream_buffer.is_empty() {
            all_lines.push(Line::from(vec![Span::styled(
                "assistant ",
                Style::default()
                    .fg(ratatui::style::Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )]));
            let stream_lines = markdown::render_markdown(&state.stream_buffer, theme);
            all_lines.extend(stream_lines);
        } else if !state.thinking_active {
            // Show a waiting indicator
            let spinner = thinking_spinner();
            all_lines.push(Line::from(Span::styled(
                format!("{spinner} Waiting for response..."),
                theme.thinking,
            )));
        }
    }

    // Approval card
    if let Some(ref approval) = state.pending_approval {
        all_lines.push(Line::from(""));
        all_lines.push(Line::from(vec![Span::styled(
            " APPROVAL REQUIRED ",
            theme.approval_highlight,
        )]));
        all_lines.push(Line::from(vec![Span::raw(format!(
            "  Command: {}",
            approval.command
        ))]));
        all_lines.push(Line::from(vec![
            Span::styled("  [y] ", theme.tool_success),
            Span::raw("Approve  "),
            Span::styled("[n] ", theme.tool_error),
            Span::raw("Deny"),
        ]));
        all_lines.push(Line::from(""));
    }

    // Apply scroll offset (scroll from bottom)
    let visible_height = area.height.saturating_sub(2) as usize; // account for borders
    let total_lines = all_lines.len();
    let max_scroll = total_lines.saturating_sub(visible_height);
    let effective_scroll = state.scroll_offset.min(max_scroll);
    let start = total_lines.saturating_sub(visible_height + effective_scroll);
    let end = total_lines.saturating_sub(effective_scroll);
    let visible: Vec<Line<'_>> = all_lines
        .into_iter()
        .skip(start)
        .take(end - start)
        .collect();

    let title = format!(" Chat: {} ", state.active_session);
    let chat = Paragraph::new(visible)
        .block(Block::default().borders(Borders::ALL).title(title))
        .wrap(Wrap { trim: false });

    frame.render_widget(chat, area);
}

/// Render a single message into lines.
fn render_message<'a>(lines: &mut Vec<Line<'a>>, msg: &'a DisplayMessage, theme: &Theme) {
    // Role header
    let (role_label, role_style) = match msg.role {
        MessageRole::User => ("you ", theme.user_msg),
        MessageRole::Assistant => ("assistant ", theme.assistant_msg),
        MessageRole::System => ("system ", theme.system_msg),
    };
    lines.push(Line::from(vec![Span::styled(
        role_label,
        role_style.add_modifier(Modifier::BOLD),
    )]));

    // Thinking (collapsed, dimmed)
    if let Some(ref thinking) = msg.thinking {
        let preview: String = thinking.chars().take(80).collect();
        lines.push(Line::from(vec![Span::styled(
            format!("  (thinking: {preview}...)"),
            theme.thinking,
        )]));
    }

    // Content with markdown rendering
    let md_lines = markdown::render_markdown(&msg.content, theme);
    lines.extend(md_lines);

    // Tool calls
    for tool in &msg.tool_calls {
        lines.push(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(&tool.name, theme.tool_name),
            if let Some(ref mode) = tool.execution_mode {
                Span::raw(format!(" ({mode})"))
            } else {
                Span::raw("")
            },
        ]));

        // Arguments summary (truncated)
        let args_str = tool.arguments.to_string();
        let args_preview: String = args_str.chars().take(100).collect();
        lines.push(Line::from(Span::raw(format!("    {args_preview}"))));

        // Result
        if let Some(success) = tool.success {
            let (icon, style) = if success {
                ("done", theme.tool_success)
            } else {
                ("failed", theme.tool_error)
            };
            let mut result_span = vec![Span::styled(format!("    {icon}"), style)];
            if let Some(ref summary) = tool.result_summary {
                let preview: String = summary.chars().take(120).collect();
                result_span.push(Span::raw(format!(": {preview}")));
            }
            lines.push(Line::from(result_span));
        }
    }
}

/// Simple spinning animation based on elapsed time.
fn thinking_spinner() -> char {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let frames = ['|', '/', '-', '\\'];
    let idx = (now / 150) as usize % frames.len();
    frames[idx]
}
