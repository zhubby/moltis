use {
    super::{common, theme::Theme},
    crate::state::AppState,
    ratatui::{
        Frame,
        layout::{Constraint, Layout, Rect},
        style::Modifier,
        text::{Line, Span},
        widgets::{List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
    },
};

/// Render the session sidebar.
pub fn draw(frame: &mut Frame, area: Rect, state: &AppState, focused: bool, theme: &Theme) {
    if area.height <= 5 {
        let items = build_session_items(state, theme);
        let list =
            List::new(items).block(common::rounded_block_focused(" Sessions ", focused, theme));
        frame.render_widget(list, area);
        return;
    }

    let sections = Layout::vertical([Constraint::Length(4), Constraint::Min(3)]).split(area);
    let provider = state.provider.as_deref().unwrap_or("(auto)");
    let model = state.model.as_deref().unwrap_or("(auto)");
    let header_lines = vec![
        Line::from(vec![
            Span::styled("Active: ", theme.bold),
            Span::raw(format!("{provider} · {model}")),
        ]),
        Line::from(vec![
            Span::styled("[m] ", theme.mode_insert.add_modifier(Modifier::BOLD)),
            Span::raw("Switch provider/model"),
        ]),
    ];
    let header = Paragraph::new(header_lines)
        .block(common::rounded_block_focused(" Model ", focused, theme));
    frame.render_widget(header, sections[0]);

    let items = build_session_items(state, theme);
    let item_count = items.len();
    let list = List::new(items).block(common::rounded_block_focused(" Sessions ", focused, theme));
    frame.render_widget(list, sections[1]);

    // Scrollbar for session list
    let visible_height = sections[1].height.saturating_sub(2) as usize;
    if item_count > visible_height {
        let max_scroll = item_count.saturating_sub(visible_height);
        let mut scrollbar_state =
            ScrollbarState::new(max_scroll).position(state.session_scroll_offset.min(max_scroll));
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight),
            sections[1],
            &mut scrollbar_state,
        );
    }
}

fn build_session_items<'a>(state: &'a AppState, theme: &Theme) -> Vec<ListItem<'a>> {
    state
        .sessions
        .iter()
        .enumerate()
        .map(|(index, session)| {
            let is_active = session.key == state.active_session;
            let is_selected = index == state.selected_session;
            let style = if is_active {
                theme.sidebar_active
            } else if index % 2 == 1 {
                theme.zebra_odd
            } else {
                theme.sidebar_item
            };

            let mut spans = Vec::new();

            // Selection / active indicator
            if is_selected {
                spans.push(Span::styled("▶ ", theme.sidebar_active));
            } else if is_active {
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
        .collect()
}
