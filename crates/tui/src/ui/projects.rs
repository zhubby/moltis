use {
    super::{common, theme::Theme},
    crate::state::AppState,
    ratatui::{
        Frame,
        layout::{Constraint, Layout, Rect},
        text::{Line, Span},
        widgets::{List, ListItem, Paragraph, Wrap},
    },
};

/// Render the Projects tab: list + detail panel.
pub fn draw(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    let layout =
        Layout::horizontal([Constraint::Percentage(40), Constraint::Percentage(60)]).split(area);

    draw_project_list(frame, layout[0], state, theme);
    draw_project_detail(frame, layout[1], state, theme);
}

fn draw_project_list(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    if state.projects.projects.is_empty() {
        let block = common::rounded_block_focused(" Projects ", true, theme);
        let content = Paragraph::new(vec![
            Line::from(""),
            Line::from("  No projects found."),
            Line::from(""),
            Line::from("  Projects will load from gateway."),
            Line::from("  Press n to create a new project."),
        ])
        .block(block)
        .wrap(Wrap { trim: false });
        frame.render_widget(content, area);
        return;
    }

    let items: Vec<ListItem<'_>> = state
        .projects
        .projects
        .iter()
        .enumerate()
        .map(|(index, project)| {
            let is_selected = index == state.projects.selected;
            let style = if is_selected {
                theme.sidebar_active
            } else if index % 2 == 1 {
                theme.zebra_odd
            } else {
                theme.sidebar_item
            };
            let marker = if is_selected {
                "▶ "
            } else {
                "  "
            };
            let active = if project.active {
                "●"
            } else {
                "○"
            };

            ListItem::new(Line::from(vec![
                Span::raw(marker),
                Span::styled(
                    active,
                    if project.active {
                        theme.status_dot_active
                    } else {
                        theme.status_dot_inactive
                    },
                ),
                Span::raw(" "),
                Span::styled(&project.name, style),
            ]))
        })
        .collect();

    let list = List::new(items).block(common::rounded_block_focused(" Projects ", true, theme));
    frame.render_widget(list, area);
}

fn draw_project_detail(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    let block = common::rounded_block_focused(" Details ", false, theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let content = if let Some(project) = state.projects.projects.get(state.projects.selected) {
        vec![
            Line::from(vec![Span::styled(&project.name, theme.heading)]),
            Line::from(""),
            Line::from(project.description.as_str()),
            Line::from(""),
            Line::from(vec![
                Span::styled("Path: ", theme.bold),
                Span::raw(&project.path),
            ]),
            Line::from(vec![
                Span::styled("Status: ", theme.bold),
                Span::raw(if project.active {
                    "Active"
                } else {
                    "Inactive"
                }),
            ]),
        ]
    } else {
        vec![Line::from("Select a project to view details.")]
    };

    let paragraph = Paragraph::new(content).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}
