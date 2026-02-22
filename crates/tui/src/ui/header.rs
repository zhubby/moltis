use {
    super::theme::Theme,
    crate::state::{AppState, MainTab},
    ratatui::{
        Frame,
        layout::{Constraint, Layout, Rect},
        text::{Line, Span},
        widgets::Paragraph,
    },
};

/// Render the header bar with app name, tabs, and model info.
pub fn draw(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    let layout = Layout::horizontal([
        Constraint::Min(40),    // tabs
        Constraint::Length(30), // model info
    ])
    .split(area);

    // Left: app name + tabs
    let mut spans: Vec<Span<'_>> = Vec::new();
    spans.push(Span::styled(" moltis ", theme.header_title));
    spans.push(Span::raw(" "));

    let tabs = [
        (MainTab::Chat, "Chat", "1"),
        (MainTab::Settings, "Settings", "2"),
        (MainTab::Projects, "Projects", "3"),
        (MainTab::Crons, "Crons", "4"),
    ];

    for (tab, label, key) in &tabs {
        let style = if state.active_tab == *tab {
            theme.header_tab_active
        } else {
            theme.header_tab_inactive
        };
        spans.push(Span::styled(format!(" {label} [{key}] "), style));
        spans.push(Span::raw(" "));
    }

    frame.render_widget(Paragraph::new(Line::from(spans)), layout[0]);

    // Right: model/provider info
    let provider = state.provider.as_deref().unwrap_or("(auto)");
    let model = state.model.as_deref().unwrap_or("(auto)");
    let info = format!("{provider} · {model} ");
    frame.render_widget(
        Paragraph::new(Line::from(Span::raw(info))).alignment(ratatui::layout::Alignment::Right),
        layout[1],
    );
}
