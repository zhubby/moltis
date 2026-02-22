use {
    super::{common, theme::Theme},
    crate::state::{AppState, SettingsSection},
    ratatui::{
        Frame,
        layout::{Constraint, Layout, Rect},
        text::{Line, Span},
        widgets::{List, ListItem, Paragraph, Wrap},
    },
};

/// Render the Settings tab: section nav + detail panel.
pub fn draw(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    let nav_width = 22u16.min(area.width / 3);
    let layout =
        Layout::horizontal([Constraint::Length(nav_width), Constraint::Min(30)]).split(area);

    draw_section_nav(frame, layout[0], state, theme);
    draw_section_detail(frame, layout[1], state, theme);
}

fn draw_section_nav(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    let items: Vec<ListItem<'_>> = state
        .settings
        .sections
        .iter()
        .map(|section| {
            let style = if *section == state.settings.active_section {
                theme.sidebar_active
            } else {
                theme.sidebar_item
            };
            let marker = if *section == state.settings.active_section {
                "▶ "
            } else {
                "  "
            };
            ListItem::new(Line::from(vec![
                Span::raw(marker),
                Span::styled(section.label(), style),
            ]))
        })
        .collect();

    let list = List::new(items).block(common::rounded_block_focused(" Sections ", true, theme));
    frame.render_widget(list, area);
}

fn draw_section_detail(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    let title = format!(" {} ", state.settings.active_section.label());
    let block = common::rounded_block_focused(&title, false, theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let content = match state.settings.active_section {
        SettingsSection::Identity => vec![
            Line::from(vec![Span::styled("Identity Configuration", theme.heading)]),
            Line::from(""),
            Line::from("Manage your name, agent name, emoji, creature, and vibe."),
            Line::from(""),
            Line::from("Data will load when connected to gateway."),
            Line::from("Press Enter to edit the selected field."),
        ],
        SettingsSection::Providers => vec![
            Line::from(vec![Span::styled("LLM Providers", theme.heading)]),
            Line::from(""),
            Line::from("Configure API keys and endpoints for LLM providers."),
            Line::from(""),
            Line::from("Data will load when connected to gateway."),
        ],
        SettingsSection::Voice => vec![
            Line::from(vec![Span::styled("Voice Configuration", theme.heading)]),
            Line::from(""),
            Line::from("Enable or disable speech-to-text and text-to-speech providers."),
        ],
        SettingsSection::Channels => vec![
            Line::from(vec![Span::styled("Channel Integrations", theme.heading)]),
            Line::from(""),
            Line::from("Manage Telegram, Slack, and Discord connections."),
        ],
        SettingsSection::EnvVars => vec![
            Line::from(vec![Span::styled("Environment Variables", theme.heading)]),
            Line::from(""),
            Line::from("Key-value pairs available to tools and scripts."),
        ],
        SettingsSection::McpServers => vec![
            Line::from(vec![Span::styled("MCP Servers", theme.heading)]),
            Line::from(""),
            Line::from("Model Context Protocol server connections."),
        ],
        SettingsSection::Memory => vec![
            Line::from(vec![Span::styled("Memory", theme.heading)]),
            Line::from(""),
            Line::from("Memory store status and configuration."),
        ],
    };

    let paragraph = Paragraph::new(content).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}
