pub mod diff_popup;

use crate::popup::{EventState, Popup};
use crossterm::event::{Event, KeyCode};
use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

pub struct HelpPopup {
    commands: Vec<(&'static str, &'static str)>,
    done: bool,
}

impl HelpPopup {
    pub fn new() -> Self {
        Self {
            commands: vec![
                ("q", "quit"),
                ("Enter", "send"),
                ("?", "help"),
                ("Esc", "close popup"),
                ("↑/↓", "scroll"),
                ("Ctrl+C", "stop gen / quit"),
            ],
            done: false,
        }
    }
}

impl Popup for HelpPopup {
    fn draw(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Help ")
            .title_alignment(Alignment::Center)
            .border_style(Style::default().fg(Color::Cyan));
        let inner = block.inner(area);
        frame.render_widget(Clear, area);
        frame.render_widget(block, area);

        let lines: Vec<Line> = self
            .commands
            .iter()
            .map(|(k, d)| {
                Line::from(vec![
                    Span::styled(
                        format!("{:10}", k),
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(*d, Style::default().fg(Color::White)),
                ])
            })
            .collect();
        let paragraph = Paragraph::new(Text::from(lines)).wrap(Wrap { trim: true });
        frame.render_widget(paragraph, inner);
    }

    fn handle_event(&mut self, event: Event) -> EventState {
        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Esc | KeyCode::Enter | KeyCode::Char('?') => {
                    self.done = true;
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
}

pub struct ToolResultPopup {
    title: String,
    body: String,
    done: bool,
}

impl ToolResultPopup {
    pub fn new(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            body: body.into(),
            done: false,
        }
    }
}

impl Popup for ToolResultPopup {
    fn draw(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" {} ", self.title))
            .title_alignment(Alignment::Center)
            .border_style(Style::default().fg(Color::Green));
        let inner = block.inner(area);
        frame.render_widget(Clear, area);
        frame.render_widget(block, area);

        let paragraph = Paragraph::new(self.body.clone())
            .wrap(Wrap { trim: true })
            .scroll((0, 0));
        frame.render_widget(paragraph, inner);
    }

    fn handle_event(&mut self, event: Event) -> EventState {
        if let Event::Key(key) = event {
            if key.code == KeyCode::Esc || key.code == KeyCode::Enter {
                self.done = true;
                return EventState::Consumed;
            }
        }
        EventState::NotConsumed
    }

    fn is_done(&self) -> bool {
        self.done
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn test_help_popup_consumes_esc() {
        let mut popup = HelpPopup::new();
        let event = Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));
        assert!(matches!(popup.handle_event(event), EventState::Consumed));
        assert!(popup.is_done());
    }

    #[test]
    fn test_help_popup_consumes_question() {
        let mut popup = HelpPopup::new();
        let event = Event::Key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::empty()));
        assert!(matches!(popup.handle_event(event), EventState::Consumed));
        assert!(popup.is_done());
    }

    #[test]
    fn test_tool_result_popup_closes_on_enter() {
        let mut popup = ToolResultPopup::new("title", "body");
        let event = Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
        assert!(matches!(popup.handle_event(event), EventState::Consumed));
        assert!(popup.is_done());
    }

    #[test]
    fn test_tool_result_popup_not_consumed_random_key() {
        let mut popup = ToolResultPopup::new("title", "body");
        let event = Event::Key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::empty()));
        assert!(matches!(popup.handle_event(event), EventState::NotConsumed));
        assert!(!popup.is_done());
    }
}
