use crate::diff::{DiffHunk, DiffLine, parse_unified_diff};
use crate::popup::{EventState, Popup};
use crossterm::event::{Event, KeyCode};
use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

/// Popup that previews a unified diff and lets the user confirm or cancel.
pub struct DiffPopup {
    file_path: String,
    hunks: Vec<DiffHunk>,
    scroll_offset: usize,
    done: bool,
    confirmed: bool,
}

impl DiffPopup {
    /// Create a diff popup from parsed hunks.
    pub fn new(file_path: impl Into<String>, hunks: Vec<DiffHunk>) -> Self {
        Self {
            file_path: file_path.into(),
            hunks,
            scroll_offset: 0,
            done: false,
            confirmed: false,
        }
    }

    /// Create a DiffPopup from a unified diff patch string.
    pub fn from_patch(file_path: impl Into<String>, patch: impl Into<String>) -> Self {
        let hunks = parse_unified_diff(&patch.into());
        Self::new(file_path, hunks)
    }
}

impl Popup for DiffPopup {
    fn draw(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" Diff Preview: {} ", self.file_path))
            .title_alignment(Alignment::Center)
            .border_style(Style::default().fg(Color::Cyan));
        let inner = block.inner(area);
        frame.render_widget(Clear, area);
        frame.render_widget(block, area);

        let mut lines: Vec<Line> = Vec::new();
        for hunk in &self.hunks {
            let old_count = hunk
                .lines
                .iter()
                .filter(|l| !matches!(l, DiffLine::Added(_)))
                .count();
            let new_count = hunk
                .lines
                .iter()
                .filter(|l| !matches!(l, DiffLine::Removed(_)))
                .count();
            lines.push(Line::from(vec![Span::styled(
                format!(
                    "@@ -{},{} +{},{} @@",
                    hunk.old_start, old_count, hunk.new_start, new_count
                ),
                Style::default().fg(Color::Yellow),
            )]));
            for line in &hunk.lines {
                let (prefix, content, style) = match line {
                    DiffLine::Context(s) => (" ", s.as_str(), Style::default()),
                    DiffLine::Removed(s) => ("-", s.as_str(), Style::default().fg(Color::Red)),
                    DiffLine::Added(s) => ("+", s.as_str(), Style::default().fg(Color::Green)),
                };
                let display = format!(
                    "{}{}",
                    prefix,
                    content.strip_suffix('\n').unwrap_or(content)
                );
                lines.push(Line::from(vec![Span::styled(display, style)]));
            }
        }

        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled(
            "Enter = Apply  q/Esc = Cancel  ↑/↓ = Scroll",
            Style::default().fg(Color::Cyan),
        )]));

        let visible_height = inner.height as usize;
        let total = lines.len();
        let max_scroll = total.saturating_sub(visible_height);
        let scroll = self.scroll_offset.min(max_scroll);

        let visible_lines: Vec<Line> = lines
            .into_iter()
            .skip(scroll)
            .take(visible_height)
            .collect();
        let paragraph = Paragraph::new(Text::from(visible_lines)).wrap(Wrap { trim: false });
        frame.render_widget(paragraph, inner);
    }

    fn handle_event(&mut self, event: Event) -> EventState {
        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Enter => {
                    self.confirmed = true;
                    self.done = true;
                    return EventState::Consumed;
                }
                KeyCode::Esc | KeyCode::Char('q') => {
                    self.confirmed = false;
                    self.done = true;
                    return EventState::Consumed;
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.scroll_offset = self.scroll_offset.saturating_sub(1);
                    return EventState::Consumed;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.scroll_offset = self.scroll_offset.saturating_add(1);
                    return EventState::Consumed;
                }
                _ => return EventState::Consumed,
            }
        }
        EventState::NotConsumed
    }

    fn is_done(&self) -> bool {
        self.done
    }

    fn preferred_size(&self) -> (u16, u16) {
        (80, 80)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn test_diff_popup_scroll() {
        let hunks = vec![DiffHunk {
            old_start: 1,
            new_start: 1,
            lines: vec![
                DiffLine::Context("a\n".into()),
                DiffLine::Removed("b\n".into()),
                DiffLine::Added("c\n".into()),
            ],
        }];
        let mut popup = DiffPopup::new("test.txt", hunks);
        assert_eq!(popup.scroll_offset, 0);

        let action = popup.handle_event(Event::Key(KeyEvent::new(
            KeyCode::Down,
            KeyModifiers::empty(),
        )));
        assert!(matches!(action, EventState::Consumed));
        assert_eq!(popup.scroll_offset, 1);

        let action = popup.handle_event(Event::Key(KeyEvent::new(
            KeyCode::Up,
            KeyModifiers::empty(),
        )));
        assert!(matches!(action, EventState::Consumed));
        assert_eq!(popup.scroll_offset, 0);

        let action = popup.handle_event(Event::Key(KeyEvent::new(
            KeyCode::Enter,
            KeyModifiers::empty(),
        )));
        assert!(matches!(action, EventState::Consumed));
        assert!(popup.is_done());
        assert!(popup.confirmed);

        let mut popup2 = DiffPopup::new("test.txt", vec![]);
        let action = popup2.handle_event(Event::Key(KeyEvent::new(
            KeyCode::Esc,
            KeyModifiers::empty(),
        )));
        assert!(matches!(action, EventState::Consumed));
        assert!(popup2.is_done());
        assert!(!popup2.confirmed);
    }
}
