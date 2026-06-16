//! Structured log emission surface for channel events.
//!
//! Emits `tracing` events with structured fields so the Clarity Gateway can
//! observe channel activity. A future extension can add JSONL persistence / broadcast.

/// Re-exported `tracing` helpers for span propagation in spawned channel tasks.
pub use tracing::{Instrument, Span, debug_span, error_span, info_span, trace_span, warn_span};

/// Action taxonomy for log events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Action {
    /// Default action when none is specified.
    #[default]
    Generic,
    /// A message was sent or received.
    Message,
    /// A tool call was executed.
    ToolCall,
    /// A state transition happened.
    StateChange,
    /// A spawn/completion lifecycle event.
    Lifecycle,
    /// An explicit note or annotation.
    Note,
    /// Start of an operation.
    Start,
    /// Completion of an operation.
    Complete,
    /// Failure of an operation.
    Fail,
    /// Spawn of a background task.
    Spawn,
    /// Rejection of invalid input.
    Reject,
}

impl Action {
    /// Return the canonical string representation of this action.
    pub const fn as_str(self) -> &'static str {
        match self {
            Action::Message => "message",
            Action::ToolCall => "tool_call",
            Action::StateChange => "state_change",
            Action::Lifecycle => "lifecycle",
            Action::Note => "note",
            Action::Generic => "generic",
            Action::Start => "start",
            Action::Complete => "complete",
            Action::Fail => "fail",
            Action::Spawn => "spawn",
            Action::Reject => "reject",
        }
    }
}

/// Outcome of an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventOutcome {
    /// The operation completed successfully.
    Success,
    /// The operation failed.
    Failure,
    /// No outcome was recorded.
    Unknown,
}

impl EventOutcome {
    /// Return the canonical string representation of this outcome.
    pub const fn as_str(self) -> &'static str {
        match self {
            EventOutcome::Success => "success",
            EventOutcome::Failure => "failure",
            EventOutcome::Unknown => "unknown",
        }
    }
}

/// Structured event payload.
#[derive(Debug, Clone, Default)]
pub struct Event {
    /// Event name, typically the module path or task identifier.
    pub name: String,
    /// Action taxonomy bucket.
    pub action: Action,
    /// Optional success/failure outcome.
    pub outcome: Option<EventOutcome>,
    /// Free-form structured attributes emitted as tracing fields.
    pub attrs: serde_json::Value,
}

impl Event {
    /// Create a new event with the given name and action.
    pub fn new(name: impl Into<String>, action: Action) -> Self {
        Self {
            name: name.into(),
            action,
            outcome: None,
            attrs: serde_json::Value::Object(serde_json::Map::new()),
        }
    }

    /// Attach an outcome to this event.
    pub fn with_outcome(mut self, outcome: EventOutcome) -> Self {
        self.outcome = Some(outcome);
        self
    }

    /// Attach structured attributes to this event.
    pub fn with_attrs(mut self, attrs: serde_json::Value) -> Self {
        self.attrs = attrs;
        self
    }
}
