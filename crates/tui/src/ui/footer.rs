use {
    super::{common, theme::Theme},
    crate::state::{AppState, InputMode, MainTab, Panel},
    ratatui::{Frame, layout::Rect, widgets::Paragraph},
};

/// Render the context-aware footer help bar.
pub fn draw(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    let hints = footer_hints(state);
    let line = common::draw_help_bar(&hints, theme);
    frame.render_widget(Paragraph::new(line), area);
}

fn footer_hints(state: &AppState) -> Vec<(&'static str, &'static str)> {
    if state.pending_approval.is_some() {
        return vec![("y", "Approve"), ("n", "Deny"), ("Esc", "Normal")];
    }

    match state.input_mode {
        InputMode::Insert => {
            return vec![
                ("Enter", "Send"),
                ("S+Enter", "Newline"),
                ("Esc", "Navigate"),
            ];
        },
        InputMode::Command => {
            return vec![("Enter", "Execute"), ("Esc", "Cancel")];
        },
        InputMode::Normal => {},
    }

    match state.active_tab {
        MainTab::Chat => match state.active_panel {
            Panel::Sessions => vec![
                ("j/k", "Nav"),
                ("Enter", "Select"),
                ("Tab", "Chat"),
                ("Ctrl+b", "Hide"),
                ("q", "Quit"),
            ],
            Panel::Chat => vec![
                ("i", "Type"),
                (":", "Cmd"),
                ("j/k", "Scroll"),
                ("m", "Model"),
                ("Tab", "Sidebar"),
                ("q", "Quit"),
            ],
        },
        MainTab::Settings => vec![
            ("j/k", "Nav"),
            ("Enter", "Edit"),
            ("Tab", "Section"),
            ("Esc", "Back"),
            ("1-4", "Tabs"),
        ],
        MainTab::Projects => vec![
            ("j/k", "Nav"),
            ("Enter", "Select"),
            ("n", "New"),
            ("Esc", "Back"),
            ("1-4", "Tabs"),
        ],
        MainTab::Crons => vec![
            ("j/k", "Nav"),
            ("Enter", "Edit"),
            ("r", "Run"),
            ("Esc", "Back"),
            ("1-4", "Tabs"),
        ],
    }
}
