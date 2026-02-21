use ratatui::style::{Color, Modifier, Style};

/// Color theme for the TUI.
pub struct Theme {
    pub user_msg: Style,
    pub assistant_msg: Style,
    pub system_msg: Style,
    pub thinking: Style,
    pub tool_name: Style,
    pub tool_success: Style,
    pub tool_error: Style,
    pub approval_highlight: Style,
    pub code_inline: Style,
    pub code_block_border: Style,
    pub heading: Style,
    pub link: Style,
    pub bold: Style,
    pub italic: Style,
    pub status_connected: Style,
    pub status_connecting: Style,
    pub status_disconnected: Style,
    pub mode_normal: Style,
    pub mode_insert: Style,
    pub mode_command: Style,
    pub sidebar_active: Style,
    pub sidebar_item: Style,
    pub session_replying: Style,
    pub border: Style,
    pub border_focused: Style,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            user_msg: Style::default().fg(Color::White),
            assistant_msg: Style::default().fg(Color::Cyan),
            system_msg: Style::default().fg(Color::DarkGray),
            thinking: Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::DIM),
            tool_name: Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
            tool_success: Style::default().fg(Color::Green),
            tool_error: Style::default().fg(Color::Red),
            approval_highlight: Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            code_inline: Style::default().fg(Color::Green),
            code_block_border: Style::default().fg(Color::DarkGray),
            heading: Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            link: Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::UNDERLINED),
            bold: Style::default().add_modifier(Modifier::BOLD),
            italic: Style::default().add_modifier(Modifier::ITALIC),
            status_connected: Style::default().bg(Color::Green).fg(Color::Black),
            status_connecting: Style::default().bg(Color::Yellow).fg(Color::Black),
            status_disconnected: Style::default().bg(Color::DarkGray).fg(Color::White),
            mode_normal: Style::default().bg(Color::Blue).fg(Color::White),
            mode_insert: Style::default().bg(Color::Green).fg(Color::Black),
            mode_command: Style::default().bg(Color::Magenta).fg(Color::White),
            sidebar_active: Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            sidebar_item: Style::default().fg(Color::White),
            session_replying: Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::DIM),
            border: Style::default().fg(Color::DarkGray),
            border_focused: Style::default().fg(Color::Cyan),
        }
    }
}
