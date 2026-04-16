use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::app::GenerationMetrics;

/// 生成中指示器组件
pub struct GeneratingIndicator;

impl GeneratingIndicator {
    pub fn render(f: &mut Frame, _area: Rect, metrics: Option<&GenerationMetrics>) {
        let size = f.size();
        let popup_area = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(40),
                Constraint::Length(7),
                Constraint::Percentage(40),
            ])
            .split(size)[1];

        let popup_area = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(15),
                Constraint::Min(50),
                Constraint::Percentage(15),
            ])
            .split(popup_area)[1];

        let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let frame_idx = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
            / 80) as usize
            % frames.len();
        let spinner = frames[frame_idx];

        let (lines, border_color) = match metrics {
            Some(m) if m.first_token_time.is_some() => {
                let ttft = m
                    .first_token_time
                    .unwrap()
                    .duration_since(m.start_time)
                    .as_secs_f64();
                let lines = vec![
                    Line::from(vec![
                        Span::styled(
                            spinner,
                            Style::default()
                                .fg(Color::Rgb(100, 220, 150))
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            " 生成中 ",
                            Style::default()
                                .fg(Color::Rgb(220, 220, 240))
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("首字耗时: ", Style::default().fg(Color::Rgb(160, 160, 180))),
                        Span::styled(
                            format!("{:.1}s", ttft),
                            Style::default().fg(Color::Rgb(100, 220, 150)),
                        ),
                        Span::styled(" | 已生成 ", Style::default().fg(Color::Rgb(160, 160, 180))),
                        Span::styled(
                            format!("{} 字", m.total_chars),
                            Style::default().fg(Color::Rgb(100, 200, 255)),
                        ),
                    ]),
                    Line::from(vec![Span::styled(
                        "按 Ctrl+C 停止",
                        Style::default().fg(Color::Rgb(140, 140, 160)),
                    )]),
                ];
                (lines, Color::Rgb(80, 160, 120))
            }
            Some(m) => {
                let elapsed = m.start_time.elapsed().as_secs();
                if elapsed < 10 {
                    let lines = vec![
                        Line::from(vec![
                            Span::styled(
                                spinner,
                                Style::default()
                                    .fg(Color::Rgb(255, 200, 80))
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(
                                " 等待响应 ",
                                Style::default()
                                    .fg(Color::Rgb(220, 220, 240))
                                    .add_modifier(Modifier::BOLD),
                            ),
                        ]),
                        Line::from(""),
                        Line::from(vec![
                            Span::styled("已等待 ", Style::default().fg(Color::Rgb(160, 160, 180))),
                            Span::styled(
                                format!("{}s", elapsed),
                                Style::default().fg(Color::Rgb(255, 200, 80)),
                            ),
                        ]),
                        Line::from(vec![Span::styled(
                            "按 Ctrl+C 停止",
                            Style::default().fg(Color::Rgb(140, 140, 160)),
                        )]),
                    ];
                    (lines, Color::Rgb(180, 150, 60))
                } else {
                    let lines = vec![
                        Line::from(vec![
                            Span::styled(
                                "⏳",
                                Style::default()
                                    .fg(Color::Rgb(255, 100, 100))
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(
                                " 模型响应较慢 ",
                                Style::default()
                                    .fg(Color::Rgb(220, 220, 240))
                                    .add_modifier(Modifier::BOLD),
                            ),
                        ]),
                        Line::from(""),
                        Line::from(vec![
                            Span::styled("已等待 ", Style::default().fg(Color::Rgb(160, 160, 180))),
                            Span::styled(
                                format!("{}s", elapsed),
                                Style::default()
                                    .fg(Color::Rgb(255, 100, 100))
                                    .add_modifier(Modifier::BOLD),
                            ),
                        ]),
                        Line::from(vec![Span::styled(
                            "请耐心等待或按 Ctrl+C 停止",
                            Style::default().fg(Color::Rgb(140, 140, 160)),
                        )]),
                    ];
                    (lines, Color::Rgb(180, 80, 80))
                }
            }
            None => {
                let lines = vec![
                    Line::from(vec![
                        Span::styled(
                            spinner,
                            Style::default()
                                .fg(Color::Rgb(100, 200, 255))
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            " 准备中 ",
                            Style::default()
                                .fg(Color::Rgb(220, 220, 240))
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]),
                    Line::from(""),
                    Line::from(vec![Span::styled(
                        "按 Ctrl+C 停止",
                        Style::default().fg(Color::Rgb(140, 140, 160)),
                    )]),
                ];
                (lines, Color::Rgb(60, 120, 180))
            }
        };

        let popup = Paragraph::new(Text::from(lines))
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border_color))
                    .title(" Generating ")
                    .title_alignment(Alignment::Center),
            );

        f.render_widget(Clear, popup_area);
        f.render_widget(popup, popup_area);
    }
}
