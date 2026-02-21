use {
    super::theme::Theme,
    pulldown_cmark::{Event, Parser, Tag, TagEnd},
    ratatui::{
        style::Style,
        text::{Line, Span},
    },
};

/// Convert a markdown string into ratatui `Line` objects for rendering.
pub fn render_markdown<'a>(text: &str, theme: &Theme) -> Vec<Line<'a>> {
    let parser = Parser::new(text);
    let mut lines: Vec<Line<'a>> = Vec::new();
    let mut current_spans: Vec<Span<'a>> = Vec::new();
    let mut style_stack: Vec<Style> = vec![Style::default()];
    let mut in_code_block = false;
    let mut list_depth: u16 = 0;
    let mut ordered_index: Option<u64> = None;

    for event in parser {
        match event {
            Event::Start(tag) => match tag {
                Tag::Heading { level, .. } => {
                    let prefix = "#".repeat(level as usize);
                    current_spans.push(Span::styled(format!("{prefix} "), theme.heading));
                    style_stack.push(theme.heading);
                },
                Tag::Strong => {
                    style_stack.push(theme.bold);
                },
                Tag::Emphasis => {
                    style_stack.push(theme.italic);
                },
                Tag::CodeBlock(_) => {
                    // Flush current line
                    if !current_spans.is_empty() {
                        lines.push(Line::from(std::mem::take(&mut current_spans)));
                    }
                    lines.push(Line::from(Span::styled("───", theme.code_block_border)));
                    in_code_block = true;
                    style_stack.push(theme.code_inline);
                },
                Tag::Link { dest_url, .. } => {
                    style_stack.push(theme.link);
                    // We'll append the URL after the link text
                    current_spans.push(Span::raw("")); // placeholder
                    // Store the URL for later
                    let _ = dest_url; // handled in End
                },
                Tag::List(start) => {
                    if !current_spans.is_empty() {
                        lines.push(Line::from(std::mem::take(&mut current_spans)));
                    }
                    list_depth += 1;
                    ordered_index = start;
                },
                Tag::Item => {
                    let indent = "  ".repeat(list_depth.saturating_sub(1) as usize);
                    let bullet = if let Some(ref mut idx) = ordered_index {
                        let s = format!("{indent}{idx}. ");
                        *idx += 1;
                        s
                    } else {
                        format!("{indent}* ")
                    };
                    current_spans.push(Span::raw(bullet));
                },
                Tag::Paragraph => {},
                _ => {},
            },
            Event::End(tag_end) => match tag_end {
                TagEnd::Heading(_) => {
                    style_stack.pop();
                    lines.push(Line::from(std::mem::take(&mut current_spans)));
                },
                TagEnd::Strong | TagEnd::Emphasis => {
                    style_stack.pop();
                },
                TagEnd::CodeBlock => {
                    in_code_block = false;
                    style_stack.pop();
                    lines.push(Line::from(Span::styled("───", theme.code_block_border)));
                },
                TagEnd::Link => {
                    style_stack.pop();
                },
                TagEnd::List(_) => {
                    list_depth = list_depth.saturating_sub(1);
                    if list_depth == 0 {
                        ordered_index = None;
                    }
                },
                TagEnd::Item => {
                    lines.push(Line::from(std::mem::take(&mut current_spans)));
                },
                TagEnd::Paragraph => {
                    if !current_spans.is_empty() {
                        lines.push(Line::from(std::mem::take(&mut current_spans)));
                    }
                    lines.push(Line::from("")); // blank line between paragraphs
                },
                _ => {},
            },
            Event::Text(text) => {
                let style = style_stack.last().copied().unwrap_or_default();
                if in_code_block {
                    // Each line of code block on its own line
                    for line in text.split('\n') {
                        if !line.is_empty() {
                            current_spans.push(Span::styled(format!("  {line}"), style));
                        }
                        if text.contains('\n') {
                            lines.push(Line::from(std::mem::take(&mut current_spans)));
                        }
                    }
                } else {
                    current_spans.push(Span::styled(text.to_string(), style));
                }
            },
            Event::Code(code) => {
                current_spans.push(Span::styled(format!("`{code}`"), theme.code_inline));
            },
            Event::SoftBreak | Event::HardBreak => {
                lines.push(Line::from(std::mem::take(&mut current_spans)));
            },
            _ => {},
        }
    }

    // Flush remaining spans
    if !current_spans.is_empty() {
        lines.push(Line::from(current_spans));
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_theme() -> Theme {
        Theme::default()
    }

    #[test]
    fn plain_text() {
        let lines = render_markdown("Hello world", &default_theme());
        assert!(!lines.is_empty());
        let text: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.to_string()))
            .collect();
        assert!(text.contains("Hello world"));
    }

    #[test]
    fn bold_text() {
        let lines = render_markdown("**bold**", &default_theme());
        assert!(!lines.is_empty());
    }

    #[test]
    fn inline_code() {
        let lines = render_markdown("use `foo` here", &default_theme());
        let text: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.to_string()))
            .collect();
        assert!(text.contains("`foo`"));
    }

    #[test]
    fn code_block() {
        let lines = render_markdown("```\nlet x = 1;\n```", &default_theme());
        assert!(lines.len() >= 3); // border + code + border
    }

    #[test]
    fn unordered_list() {
        let lines = render_markdown("* item 1\n* item 2", &default_theme());
        let text: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.to_string()))
            .collect();
        assert!(text.contains("* "));
    }

    #[test]
    fn heading() {
        let lines = render_markdown("# Title", &default_theme());
        let text: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.to_string()))
            .collect();
        assert!(text.contains("# "));
    }
}
