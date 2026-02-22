use {
    super::{common, theme::Theme},
    crate::state::{AppState, ModelSwitcherState},
    ratatui::{
        Frame,
        layout::{Constraint, Layout, Rect},
        style::Modifier,
        text::{Line, Span},
        widgets::{Block, BorderType, Borders, Clear, Paragraph, Row, Table, TableState, Wrap},
    },
};

pub fn draw(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    switcher: &ModelSwitcherState,
    theme: &Theme,
) {
    let popup = common::centered_rect(78, 76, area);
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.modal_border)
        .style(theme.modal_surface)
        .title(" Switch Provider/Model ");
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let sections = Layout::vertical([
        Constraint::Length(1), // current
        Constraint::Length(1), // hint
        Constraint::Length(3), // search
        Constraint::Min(8),    // list
        Constraint::Length(1), // actions
        Constraint::Length(1), // error/status
    ])
    .split(inner);

    let current = format!(
        "Current: {} · {}",
        state.provider.as_deref().unwrap_or("(provider auto)"),
        state.model.as_deref().unwrap_or("(model auto)")
    );
    frame.render_widget(Paragraph::new(current), sections[0]);
    frame.render_widget(
        Paragraph::new("Configured providers and models. Type to filter."),
        sections[1],
    );

    let search = Paragraph::new(Line::from(vec![
        Span::styled("Search: ", theme.bold),
        if switcher.query.is_empty() {
            Span::styled("(type to filter)", theme.system_msg)
        } else {
            Span::raw(switcher.query.as_str())
        },
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(theme.mode_insert)
            .title(" Filter "),
    )
    .wrap(Wrap { trim: false });
    frame.render_widget(search, sections[2]);

    let filtered = switcher.filtered_indices();
    if filtered.is_empty() {
        let empty = Paragraph::new("No matching configured models.").block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(" Models "),
        );
        frame.render_widget(empty, sections[3]);
    } else {
        let query = &switcher.query;
        let rows = filtered
            .iter()
            .enumerate()
            .filter_map(|(row_index, index)| {
                let item = switcher.items.get(*index)?;
                let provider_spans = common::highlight_match(
                    &item.provider_display,
                    query,
                    theme.sidebar_item,
                    theme.sidebar_active,
                );
                let display = if item.model_display == item.model_id {
                    item.model_display.clone()
                } else {
                    format!("{} ({})", item.model_display, item.model_id)
                };
                let model_spans = common::highlight_match(
                    &display,
                    query,
                    theme.sidebar_item,
                    theme.sidebar_active,
                );

                // Zebra striping
                let row_style = if row_index % 2 == 1 {
                    theme.zebra_odd
                } else {
                    ratatui::style::Style::default()
                };

                Some(
                    Row::new(vec![Line::from(provider_spans), Line::from(model_spans)])
                        .style(row_style),
                )
            })
            .collect::<Vec<Row>>();

        let table = Table::new(rows, [Constraint::Length(16), Constraint::Min(20)])
            .header(Row::new(vec!["Provider", "Model"]).style(theme.bold))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(" Models "),
            )
            .row_highlight_style(theme.mode_insert.add_modifier(Modifier::BOLD))
            .highlight_symbol("▶ ");

        let selected_pos = filtered
            .iter()
            .position(|index| *index == switcher.selected)
            .unwrap_or(0)
            .min(filtered.len().saturating_sub(1));
        let mut table_state = TableState::default();
        table_state.select(Some(selected_pos));
        frame.render_stateful_widget(table, sections[3], &mut table_state);
    }

    let actions = Line::from(vec![
        Span::styled(
            " Enter switch ",
            theme.mode_insert.add_modifier(Modifier::BOLD),
        ),
        Span::raw("  Esc close  j/k move  Backspace delete"),
    ]);
    frame.render_widget(Paragraph::new(actions), sections[4]);

    if let Some(error) = switcher.error_message.as_deref() {
        let line = Line::from(vec![
            Span::styled("Error: ", theme.tool_error.add_modifier(Modifier::BOLD)),
            Span::styled(error, theme.tool_error),
        ]);
        frame.render_widget(Paragraph::new(line), sections[5]);
    }
}
