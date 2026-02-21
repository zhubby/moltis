use {
    super::theme::Theme,
    crate::state::AppState,
    ratatui::{
        Frame,
        layout::Rect,
        text::{Line, Span},
        widgets::{Block, Borders, List, ListItem},
    },
};

/// Render the session sidebar.
pub fn draw(frame: &mut Frame, area: Rect, state: &AppState, focused: bool, theme: &Theme) {
    let items: Vec<ListItem<'_>> = state
        .sessions
        .iter()
        .map(|session| {
            let is_active = session.key == state.active_session;
            let style = if is_active {
                theme.sidebar_active
            } else {
                theme.sidebar_item
            };

            let mut spans = Vec::new();

            // Active indicator
            if is_active {
                spans.push(Span::styled("> ", theme.sidebar_active));
            } else {
                spans.push(Span::raw("  "));
            }

            // Session name
            spans.push(Span::styled(session.display_name().to_owned(), style));

            // Replying indicator
            if session.replying {
                spans.push(Span::styled(" ...", theme.session_replying));
            }

            // Message count
            if session.message_count > 0 {
                spans.push(Span::raw(format!(" ({})", session.message_count)));
            }

            // Model hint
            if let Some(model) = session.model.as_deref() {
                spans.push(Span::raw(format!(" · {model}")));
            }

            ListItem::new(Line::from(spans))
        })
        .collect();

    let border_style = if focused {
        theme.border_focused
    } else {
        theme.border
    };

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(" Sessions "),
    );

    frame.render_widget(list, area);
}
