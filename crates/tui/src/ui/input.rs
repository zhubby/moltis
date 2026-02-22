use {
    super::theme::Theme,
    crate::state::{AppState, InputMode},
    ratatui::{
        Frame,
        layout::Rect,
        style::{Color, Modifier, Style},
        widgets::{Block, BorderType, Borders},
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
    // Configure textarea style based on mode
    match state.input_mode {
        InputMode::Insert => {
            textarea.set_cursor_line_style(Style::default());
            textarea.set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
            textarea.set_block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(theme.border_focused)
                    .style(theme.input_bg)
                    .title(" Input (Enter to send, Shift+Enter for newline) "),
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

    frame.render_widget(&*textarea, area);
}
