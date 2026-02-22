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
    // New Phase 1 fields
    pub footer_key: Style,
    pub footer_desc: Style,
    pub header_title: Style,
    pub header_tab_active: Style,
    pub header_tab_inactive: Style,
    pub zebra_odd: Style,
    pub input_bg: Style,
    pub modal_surface: Style,
    pub modal_border: Style,
    pub msg_card_user: Style,
    pub msg_card_assistant: Style,
    pub msg_card_system: Style,
    pub status_dot_active: Style,
    pub status_dot_inactive: Style,
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
            border_focused: Style::default().fg(Color::Yellow),
            // New fields
            footer_key: Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            footer_desc: Style::default().fg(Color::DarkGray),
            header_title: Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            header_tab_active: Style::default().bg(Color::Yellow).fg(Color::Black),
            header_tab_inactive: Style::default().fg(Color::DarkGray),
            zebra_odd: Style::default().bg(Color::Rgb(30, 30, 40)),
            input_bg: Style::default().bg(Color::Rgb(40, 40, 40)),
            modal_surface: Style::default().fg(Color::White).bg(Color::Rgb(24, 28, 40)),
            modal_border: Style::default().fg(Color::Cyan),
            msg_card_user: Style::default().bg(Color::Rgb(25, 30, 40)),
            msg_card_assistant: Style::default().bg(Color::Rgb(20, 30, 35)),
            msg_card_system: Style::default().bg(Color::Rgb(30, 25, 25)),
            status_dot_active: Style::default().fg(Color::Green),
            status_dot_inactive: Style::default().fg(Color::DarkGray),
        }
    }
}
