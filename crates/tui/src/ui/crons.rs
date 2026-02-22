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

/// Render the Crons tab: job list + detail panel.
pub fn draw(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    let layout =
        Layout::horizontal([Constraint::Percentage(40), Constraint::Percentage(60)]).split(area);

    draw_job_list(frame, layout[0], state, theme);
    draw_job_detail(frame, layout[1], state, theme);
}

fn draw_job_list(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    if state.crons.jobs.is_empty() {
        let block = common::rounded_block_focused(" Cron Jobs ", true, theme);
        let content = Paragraph::new(vec![
            Line::from(""),
            Line::from("  No cron jobs found."),
            Line::from(""),
            Line::from("  Jobs will load from gateway."),
            Line::from("  Press n to create a new cron job."),
        ])
        .block(block)
        .wrap(Wrap { trim: false });
        frame.render_widget(content, area);
        return;
    }

    let items: Vec<ListItem<'_>> = state
        .crons
        .jobs
        .iter()
        .enumerate()
        .map(|(index, job)| {
            let is_selected = index == state.crons.selected;
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
            let dot = if job.enabled {
                "●"
            } else {
                "○"
            };

            ListItem::new(Line::from(vec![
                Span::raw(marker),
                Span::styled(
                    dot,
                    if job.enabled {
                        theme.status_dot_active
                    } else {
                        theme.status_dot_inactive
                    },
                ),
                Span::raw(" "),
                Span::styled(&job.name, style),
            ]))
        })
        .collect();

    let list = List::new(items).block(common::rounded_block_focused(" Cron Jobs ", true, theme));
    frame.render_widget(list, area);
}

fn draw_job_detail(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    let block = common::rounded_block_focused(" Details ", false, theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let content = if let Some(job) = state.crons.jobs.get(state.crons.selected) {
        vec![
            Line::from(vec![Span::styled(&job.name, theme.heading)]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Schedule: ", theme.bold),
                Span::raw(&job.schedule),
            ]),
            Line::from(vec![
                Span::styled("Enabled: ", theme.bold),
                Span::raw(if job.enabled {
                    "Yes"
                } else {
                    "No"
                }),
            ]),
            Line::from(vec![
                Span::styled("Last run: ", theme.bold),
                Span::raw(job.last_run.as_deref().unwrap_or("Never")),
            ]),
            Line::from(vec![
                Span::styled("Next run: ", theme.bold),
                Span::raw(job.next_run.as_deref().unwrap_or("N/A")),
            ]),
        ]
    } else {
        vec![Line::from("Select a cron job to view details.")]
    };

    let paragraph = Paragraph::new(content).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}
