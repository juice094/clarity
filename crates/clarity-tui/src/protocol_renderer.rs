//! Declarative UI command renderer for `clarity-tui`.
//!
//! **ADR-006 status (2026-05-11)**: All `clarity_wire::View*` types are
//! deprecated. This module is scheduled for migration to
//! `clarity-frontend-ir` in Phase D. The file-level `#![allow(deprecated)]`
//! suppresses the deprecation noise during the grace period; external
//! callers (e.g. `commands.rs`) still see the deprecation when invoking
//! `render_view_commands`.
//!
//! See `docs/adr/ADR-006-protocol-layer-convergence.md`.

#![allow(deprecated)]

use clarity_wire::{ButtonStyle, UserAction, ViewCommand};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

/// Render a slice of `ViewCommand`s into ratatui widgets within `area`,
/// returning any `UserAction`s triggered by interactive elements.
///
/// TUI simplification: each leaf widget occupies one row; `HStack` splits
/// horizontally, `VStack` stacks vertically.  Keyboard focus / input is
/// handled externally by `InputPane`; this renderer is display-only for
/// settings forms received over the wire view channel.
pub fn render_view_commands(
    f: &mut Frame,
    area: Rect,
    commands: &[ViewCommand],
) -> Vec<UserAction> {
    let mut actions = Vec::new();
    let mut current = area;

    for cmd in commands {
        let (_used, remaining) = render_single(f, current, cmd, &mut actions);
        current = remaining;
        if current.height == 0 {
            break;
        }
    }

    actions
}

fn render_single(
    f: &mut Frame,
    area: Rect,
    cmd: &ViewCommand,
    _actions: &mut Vec<UserAction>,
) -> (Rect, Rect) {
    match cmd {
        ViewCommand::VStack { children } => {
            let constraints: Vec<Constraint> =
                std::iter::repeat_n(Constraint::Length(1), children.len()).collect();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(constraints)
                .split(area);

            let mut remaining = area;
            for (i, child) in children.iter().enumerate() {
                if i < chunks.len() {
                    let (_u, r) = render_single(f, chunks[i], child, _actions);
                    remaining = r;
                }
            }
            (area, remaining)
        }

        ViewCommand::HStack { children } => {
            let n = children.len().max(1) as u32;
            let constraints: Vec<Constraint> =
                std::iter::repeat_n(Constraint::Ratio(1, n), children.len()).collect();
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints(constraints)
                .split(area);

            let mut remaining = area;
            for (i, child) in children.iter().enumerate() {
                if i < chunks.len() {
                    let (_u, r) = render_single(f, chunks[i], child, _actions);
                    remaining = r;
                }
            }
            (area, remaining)
        }

        ViewCommand::Text { content, role, .. } => {
            let color = match role {
                clarity_wire::TextRole::Label => Color::Rgb(140, 140, 160),
                clarity_wire::TextRole::Body => Color::Rgb(220, 220, 240),
                clarity_wire::TextRole::Title => Color::Rgb(255, 255, 255),
            };
            let style = if *role == clarity_wire::TextRole::Title {
                Style::default().fg(color).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(color)
            };
            let para = Paragraph::new(content.clone()).style(style);
            f.render_widget(para, area);
            let remaining = Rect {
                x: area.x,
                y: area.y + 1,
                width: area.width,
                height: area.height.saturating_sub(1),
            };
            (area, remaining)
        }

        ViewCommand::TextInput {
            id: _,
            value,
            password,
            ..
        } => {
            let display = if *password {
                "*".repeat(value.chars().count())
            } else {
                value.clone()
            };
            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Rgb(100, 100, 120)));
            let para = Paragraph::new(display).block(block);
            f.render_widget(para, area);

            // TUI does not capture live typing here; InputPane handles that.
            let remaining = Rect {
                x: area.x,
                y: area.y + area.height,
                width: area.width,
                height: 0,
            };
            (area, remaining)
        }

        ViewCommand::ComboBox {
            id: _,
            selected_value,
            options,
            ..
        } => {
            let label = options
                .iter()
                .find(|(v, _)| v == selected_value)
                .map(|(_, l)| l.as_str())
                .unwrap_or(selected_value.as_str());

            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Rgb(100, 140, 180)));
            let para = Paragraph::new(label.to_string())
                .block(block)
                .style(Style::default().fg(Color::Rgb(220, 220, 240)));
            f.render_widget(para, area);
            let remaining = Rect {
                x: area.x,
                y: area.y + area.height,
                width: area.width,
                height: 0,
            };
            (area, remaining)
        }

        ViewCommand::Button {
            id: _,
            label,
            style,
            ..
        } => {
            let (fg, bg) = match style {
                ButtonStyle::Primary => (Color::Rgb(30, 30, 30), Color::Rgb(100, 180, 255)),
                ButtonStyle::Secondary => (Color::Rgb(220, 220, 240), Color::Rgb(80, 80, 100)),
                ButtonStyle::Danger => (Color::Rgb(255, 255, 255), Color::Rgb(220, 80, 80)),
            };
            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(bg))
                .style(Style::default().bg(bg));
            let para = Paragraph::new(label.clone())
                .block(block)
                .style(Style::default().fg(fg).add_modifier(Modifier::BOLD))
                .alignment(ratatui::layout::Alignment::Center);
            f.render_widget(para, area);
            let remaining = Rect {
                x: area.x,
                y: area.y + area.height,
                width: area.width,
                height: 0,
            };
            (area, remaining)
        }

        ViewCommand::Space { height } => {
            let h = *height as u16 / 10; // scale down for terminal rows
            let remaining = Rect {
                x: area.x,
                y: area.y + h,
                width: area.width,
                height: area.height.saturating_sub(h),
            };
            (area, remaining)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clarity_wire::{ButtonStyle, TextRole, ViewCommand};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn test_render_text() {
        let backend = TestBackend::new(40, 5);
        let mut terminal = Terminal::new(backend).unwrap();
        let cmds = vec![ViewCommand::Text {
            content: "Hello".into(),
            role: TextRole::Title,
            size: 14.0,
        }];
        terminal
            .draw(|f| {
                let area = f.area();
                render_view_commands(f, area, &cmds);
            })
            .unwrap();
    }

    #[test]
    fn test_render_button() {
        let backend = TestBackend::new(40, 5);
        let mut terminal = Terminal::new(backend).unwrap();
        let cmds = vec![ViewCommand::Button {
            id: "save".into(),
            label: "Save".into(),
            style: ButtonStyle::Primary,
            min_width: 10.0,
            min_height: 1.0,
        }];
        let mut captured = Vec::new();
        terminal
            .draw(|f| {
                let area = f.area();
                captured = render_view_commands(f, area, &cmds);
            })
            .unwrap();
        assert!(captured.is_empty()); // display-only in current impl
    }
}
