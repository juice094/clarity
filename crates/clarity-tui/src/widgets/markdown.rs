use pulldown_cmark::{Event, Parser, Tag};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

/// Parse basic Markdown and return a list of ratatui `Line`s.
pub fn render_markdown(text: &str, base_style: Style) -> Vec<Line<'static>> {
    let parser = Parser::new(text);
    let mut lines: Vec<Line> = vec![];
    let mut current_spans: Vec<Span> = vec![];
    let mut style_stack: Vec<Style> = vec![];
    let mut in_code_block = false;
    let mut code_block_buffer = String::new();
    let mut just_ended_paragraph = false;

    for event in parser {
        match event {
            Event::Start(tag) => {
                if just_ended_paragraph {
                    lines.push(Line::from(""));
                    just_ended_paragraph = false;
                }

                match tag {
                    Tag::Heading(..) => {
                        style_stack.push(
                            Style::default()
                                .add_modifier(Modifier::BOLD)
                                .fg(Color::Rgb(150, 200, 255)),
                        );
                    }
                    Tag::CodeBlock(_) => {
                        in_code_block = true;
                        code_block_buffer.clear();
                    }
                    Tag::Strong => {
                        style_stack.push(
                            Style::default()
                                .add_modifier(Modifier::BOLD)
                                .fg(Color::Rgb(220, 230, 255)),
                        );
                    }
                    Tag::Emphasis => {
                        style_stack.push(
                            Style::default()
                                .add_modifier(Modifier::ITALIC)
                                .fg(Color::Rgb(180, 190, 220)),
                        );
                    }
                    _ => {}
                }
            }
            Event::End(tag) => match tag {
                Tag::Heading(..) => {
                    if !current_spans.is_empty() {
                        lines.push(Line::from(std::mem::take(&mut current_spans)));
                    }
                    style_stack.pop();
                }
                Tag::CodeBlock(_) => {
                    if !code_block_buffer.is_empty() {
                        let style = Style::default()
                            .fg(Color::Rgb(210, 230, 220))
                            .bg(Color::Rgb(30, 30, 45));
                        for line in code_block_buffer.lines() {
                            lines.push(Line::styled(line.to_string(), style));
                        }
                    }
                    in_code_block = false;
                    code_block_buffer.clear();
                }
                Tag::Paragraph => {
                    if !current_spans.is_empty() {
                        lines.push(Line::from(std::mem::take(&mut current_spans)));
                    }
                    just_ended_paragraph = true;
                }
                Tag::Strong | Tag::Emphasis => {
                    style_stack.pop();
                }
                _ => {}
            },
            Event::Text(text_content) => {
                if in_code_block {
                    code_block_buffer.push_str(&text_content);
                } else {
                    let style = style_stack.last().copied().unwrap_or(base_style);
                    current_spans.push(Span::styled(text_content.into_string(), style));
                }
            }
            Event::Code(text_content) => {
                if !in_code_block {
                    let style = Style::default()
                        .fg(Color::Rgb(255, 200, 150))
                        .bg(Color::Rgb(40, 40, 60));
                    current_spans.push(Span::styled(text_content.into_string(), style));
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if in_code_block {
                    code_block_buffer.push('\n');
                } else {
                    lines.push(Line::from(std::mem::take(&mut current_spans)));
                }
            }
            _ => {}
        }
    }

    if in_code_block && !code_block_buffer.is_empty() {
        let style = Style::default()
            .fg(Color::Rgb(210, 230, 220))
            .bg(Color::Rgb(30, 30, 45));
        for line in code_block_buffer.lines() {
            lines.push(Line::styled(line.to_string(), style));
        }
    } else if !current_spans.is_empty() {
        lines.push(Line::from(current_spans));
    }

    lines
}
