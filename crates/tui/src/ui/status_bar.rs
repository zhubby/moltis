use {
    super::theme::Theme,
    crate::state::{AppState, InputMode},
    ratatui::{
        Frame,
        layout::{Constraint, Layout, Rect},
        text::{Line, Span},
        widgets::Paragraph,
    },
};

/// Connection status for display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionDisplay {
    Disconnected,
    Connecting,
    Connected,
}

/// Render the status bar at the bottom of the screen.
pub fn draw(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    connection: &ConnectionDisplay,
    theme: &Theme,
) {
    let layout = Layout::horizontal([
        Constraint::Length(10), // mode indicator
        Constraint::Min(1),     // status info
    ])
    .split(area);

    // Mode indicator
    let (mode_text, mode_style) = match state.input_mode {
        InputMode::Normal => (" NORMAL ", theme.mode_normal),
        InputMode::Insert => (" INSERT ", theme.mode_insert),
        InputMode::Command => (" COMMAND", theme.mode_command),
    };
    let mode = Paragraph::new(Line::from(Span::styled(mode_text, mode_style)));
    frame.render_widget(mode, layout[0]);

    // Status info
    let mut parts: Vec<Span<'_>> = Vec::new();

    // Connection
    let (conn_text, conn_style) = match connection {
        ConnectionDisplay::Disconnected => (" Disconnected", theme.status_disconnected),
        ConnectionDisplay::Connecting => (" Connecting...", theme.status_connecting),
        ConnectionDisplay::Connected => (" Connected", theme.status_connected),
    };
    parts.push(Span::styled(conn_text, conn_style));

    // Server version
    if let Some(ref ver) = state.server_version {
        parts.push(Span::styled(format!(" v{ver}"), conn_style));
    }

    parts.push(Span::raw(" "));

    // Model
    if let Some(ref model) = state.model {
        parts.push(Span::raw(format!(" | {model}")));
    }

    // Session
    parts.push(Span::raw(format!(" | {} ", state.active_session)));

    // Token usage
    let total = state.token_usage.session_input + state.token_usage.session_output;
    if total > 0 || state.token_usage.context_window > 0 {
        let total_k = total / 1000;
        let window_k = state.token_usage.context_window / 1000;
        if window_k > 0 {
            parts.push(Span::raw(format!("| {total_k}K/{window_k}K tokens ")));
        } else {
            parts.push(Span::raw(format!("| {total_k}K tokens ")));
        }
    }

    // Streaming indicator
    if state.is_streaming() {
        parts.push(Span::styled(" streaming... ", theme.thinking));
    }

    let status = Paragraph::new(Line::from(parts));
    frame.render_widget(status, layout[1]);
}
