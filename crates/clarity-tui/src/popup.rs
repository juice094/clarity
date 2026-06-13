use crossterm::event::Event;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
};

/// Result of routing an input event to a popup.
pub enum EventState {
    /// The popup consumed the event; do not process it further.
    Consumed,
    /// The popup did not handle the event; route it to the next handler.
    NotConsumed,
}

/// Modal popup that can draw itself and handle crossterm events.
pub trait Popup {
    /// Draw the popup inside the provided area.
    fn draw(&self, frame: &mut Frame, area: Rect);
    /// Handle a terminal event, returning whether it was consumed.
    fn handle_event(&mut self, event: Event) -> EventState;
    /// Whether the popup should be closed.
    fn is_done(&self) -> bool {
        false
    }
    /// Preferred popup size as `(width_percent, height_percent)`.
    fn preferred_size(&self) -> (u16, u16) {
        (60, 40)
    }
}

/// Compute a rectangle centered inside `r` using the given percentages.
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
