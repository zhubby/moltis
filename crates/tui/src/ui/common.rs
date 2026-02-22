use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders},
};

use super::theme::Theme;

/// Center a rectangle within `area` using percentage-based sizing.
pub fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(area);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(vertical[1])[1]
}

/// Block with rounded borders and a title.
#[allow(dead_code)] // Public API used by onboarding refactor
pub fn rounded_block(title: &str) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(title)
}

/// Block with rounded borders, title, and focus-aware border style.
pub fn rounded_block_focused<'a>(title: &'a str, focused: bool, theme: &Theme) -> Block<'a> {
    let border_style = if focused {
        theme.border_focused
    } else {
        theme.border
    };
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style)
        .title(title)
}

/// Draw a context-aware help bar (footer) with keyboard shortcut hints.
pub fn draw_help_bar<'a>(hints: &[(&'a str, &'a str)], theme: &Theme) -> Line<'a> {
    let mut spans: Vec<Span<'a>> = Vec::new();
    for (index, (key, desc)) in hints.iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled(
                "  ",
                Style::default().fg(theme
                    .footer_desc
                    .fg
                    .unwrap_or(ratatui::style::Color::DarkGray)),
            ));
        }
        spans.push(Span::styled(*key, theme.footer_key));
        spans.push(Span::styled(format!(" {desc}"), theme.footer_desc));
    }
    Line::from(spans)
}

/// Format a count with K/M suffixes for compact display.
#[must_use]
pub fn format_count(n: u64) -> String {
    if n >= 1_000_000 {
        let m = n as f64 / 1_000_000.0;
        if m >= 10.0 {
            format!("{}M", m as u64)
        } else {
            format!("{m:.1}M")
        }
    } else if n >= 1_000 {
        let k = n as f64 / 1_000.0;
        if k >= 10.0 {
            format!("{}K", k as u64)
        } else {
            format!("{k:.1}K")
        }
    } else {
        n.to_string()
    }
}

/// Render a form field as lines suitable for onboarding/settings forms.
#[allow(dead_code)] // Public API for onboarding/settings form rendering
pub fn form_field<'a>(
    label: &'a str,
    value: &'a str,
    active: bool,
    _description: &str,
    secret: bool,
    _theme: &Theme,
) -> Vec<Line<'a>> {
    let marker = if active {
        "▶"
    } else {
        " "
    };
    let display = if secret {
        mask_secret(value)
    } else if value.trim().is_empty() {
        "(empty)".to_string()
    } else {
        value.to_string()
    };

    vec![Line::from(format!("{marker} {label}: {display}"))]
}

/// Highlight matching substrings in text for search results.
///
/// All returned spans own their content, so the result is `'static`.
pub fn highlight_match(
    text: &str,
    query: &str,
    normal_style: Style,
    match_style: Style,
) -> Vec<Span<'static>> {
    if query.is_empty() {
        return vec![Span::styled(text.to_string(), normal_style)];
    }

    let lower_text = text.to_lowercase();
    let lower_query = query.to_lowercase();
    let mut spans = Vec::new();
    let mut last_end = 0;

    for (start, _) in lower_text.match_indices(&lower_query) {
        if start > last_end {
            spans.push(Span::styled(
                text[last_end..start].to_string(),
                normal_style,
            ));
        }
        let end = start + query.len();
        spans.push(Span::styled(
            text[start..end].to_string(),
            match_style.add_modifier(Modifier::BOLD),
        ));
        last_end = end;
    }

    if last_end < text.len() {
        spans.push(Span::styled(text[last_end..].to_string(), normal_style));
    }

    if spans.is_empty() {
        spans.push(Span::styled(text.to_string(), normal_style));
    }

    spans
}

/// Mask a secret value for display.
#[allow(dead_code)] // Used by form_field for secret values
pub fn mask_secret(value: &str) -> String {
    if value.is_empty() {
        return "(empty)".into();
    }
    "*".repeat(value.chars().count().min(32))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_count_below_thousand() {
        assert_eq!(format_count(0), "0");
        assert_eq!(format_count(999), "999");
    }

    #[test]
    fn format_count_thousands() {
        assert_eq!(format_count(1_000), "1.0K");
        assert_eq!(format_count(1_500), "1.5K");
        assert_eq!(format_count(10_000), "10K");
        assert_eq!(format_count(99_999), "99K");
    }

    #[test]
    fn format_count_millions() {
        assert_eq!(format_count(1_000_000), "1.0M");
        assert_eq!(format_count(2_500_000), "2.5M");
        assert_eq!(format_count(10_000_000), "10M");
    }

    #[test]
    fn mask_secret_empty_and_filled() {
        assert_eq!(mask_secret(""), "(empty)");
        assert_eq!(mask_secret("abc"), "***");
    }

    #[test]
    fn highlight_match_no_query() {
        let theme = Theme::default();
        let spans = highlight_match("Hello World", "", theme.sidebar_item, theme.sidebar_active);
        assert_eq!(spans.len(), 1);
    }

    #[test]
    fn highlight_match_finds_substring() {
        let theme = Theme::default();
        let spans = highlight_match(
            "Hello World",
            "world",
            theme.sidebar_item,
            theme.sidebar_active,
        );
        assert_eq!(spans.len(), 2); // "Hello " + "World"
    }

    #[test]
    fn centered_rect_produces_smaller_rect() {
        let area = Rect::new(0, 0, 100, 50);
        let center = centered_rect(80, 60, area);
        assert!(center.width < area.width);
        assert!(center.height < area.height);
    }
}
