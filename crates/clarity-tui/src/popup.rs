use crossterm::event::Event;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    Frame,
};

pub enum EventState {
    Consumed,
    NotConsumed,
}

pub trait Popup {
    fn draw(&self, frame: &mut Frame, area: Rect);
    fn handle_event(&mut self, event: Event) -> EventState;
    fn is_done(&self) -> bool {
        false
    }
    fn preferred_size(&self) -> (u16, u16) {
        (60, 40)
    }
}

pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
