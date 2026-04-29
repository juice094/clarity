//! # Clarity Wire
//!
//! A broadcast-based communication channel between Soul (backend) and UI (frontend).
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────┐     WireMessage      ┌─────────────┐
//! │  WireSoul   │ ───────────────────► │   WireUI    │
//! │  (Producer) │   (broadcast channel) │  (Consumer) │
//! └─────────────┘                      └─────────────┘
//! ```
//!
//! The `Wire` provides a SPMC (Single Producer, Multiple Consumers) channel:
//! - **Soul side**: Produces messages (TurnBegin, StepBegin, ContentPart, etc.)
//! - **UI side**: Consumes messages for display

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use tracing::{debug, error, trace, warn};

/// Default capacity for broadcast channels.
const DEFAULT_CHANNEL_CAPACITY: usize = 1024;

/// Core message types flowing through the Wire.
///
/// These messages represent the lifecycle of a conversation turn,
/// from start to finish, including all intermediate steps.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WireMessage {
    /// Start of a new conversation turn with user input.
    TurnBegin {
        /// The user's input text.
        user_input: String,
    },

    /// Start of a tool execution step.
    StepBegin {
        /// Name of the tool being executed.
        tool_name: String,
    },

    /// A content part (text chunk from the model).
    ContentPart { text: String },

    /// A tool call initiated by the model.
    ToolCall {
        /// Unique identifier for this tool call.
        id: String,
        /// Name of the tool being called.
        name: String,
        /// Arguments passed to the tool (JSON object).
        arguments: Value,
    },

    /// Result returned from a tool execution.
    ToolResult {
        /// Identifier matching the original ToolCall.
        id: String,
        /// The result content (usually JSON string).
        result: String,
    },

    /// End of the current conversation turn.
    TurnEnd,

    /// Token usage report for the session.
    Usage {
        prompt_tokens: u32,
        completion_tokens: u32,
        total_tokens: u32,
    },

    /// Status update message (for UI feedback).
    StatusUpdate { message: String },

    /// Conversation history compaction has started.
    CompactionBegin,

    /// Conversation history compaction has finished.
    CompactionEnd,
}

impl WireMessage {
    /// Returns true if this message type is mergeable with subsequent messages.
    ///
    /// Currently, only `ContentPart` messages can be merged.
    fn is_mergeable(&self) -> bool {
        matches!(self, WireMessage::ContentPart { .. })
    }

    /// Attempts to merge another message into this one.
    ///
    /// Returns true if the merge was successful.
    fn try_merge(&mut self, other: &Self) -> bool {
        match (self, other) {
            (
                WireMessage::ContentPart { text: self_text },
                WireMessage::ContentPart { text: other_text },
            ) => {
                self_text.push_str(other_text);
                true
            }
            _ => false,
        }
    }
}

/// The main Wire struct that manages communication channels.
///
/// `Wire` maintains two broadcast channels:
/// - `raw_sender`: Unprocessed messages as they are produced
/// - `merged_sender`: Messages with consecutive ContentParts merged for efficiency
///
/// # Example
///
/// ```
/// use clarity_wire::Wire;
///
/// let wire = Wire::new();
/// let soul = wire.soul_side();
/// let mut ui = wire.ui_side(false);
/// ```
#[derive(Clone)]
pub struct Wire {
    /// Sender for raw (unmerged) messages.
    raw_sender: broadcast::Sender<WireMessage>,
    /// Sender for merged messages.
    merged_sender: broadcast::Sender<WireMessage>,
    /// The soul side handle.
    soul_side: WireSoulSide,
}

impl Wire {
    /// Creates a new Wire with default channel capacity.
    ///
    /// # Examples
    ///
    /// ```
    /// use clarity_wire::Wire;
    ///
    /// let wire = Wire::new();
    /// ```
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CHANNEL_CAPACITY)
    }

    /// Creates a new Wire with specified channel capacity.
    ///
    /// # Arguments
    ///
    /// * `capacity` - The buffer size for both broadcast channels.
    ///
    /// # Examples
    ///
    /// ```
    /// use clarity_wire::Wire;
    ///
    /// let wire = Wire::with_capacity(256);
    /// ```
    pub fn with_capacity(capacity: usize) -> Self {
        let (raw_sender, _) = broadcast::channel(capacity);
        let (merged_sender, _) = broadcast::channel(capacity);

        let soul_side = WireSoulSide {
            raw_sender: raw_sender.clone(),
            merged_sender: merged_sender.clone(),
            merge_buffer: Arc::new(Mutex::new(None)),
        };

        Self {
            raw_sender,
            merged_sender,
            soul_side,
        }
    }

    /// Returns a reference to the soul side (producer) of the wire.
    ///
    /// The soul side is used to send messages into the wire.
    pub fn soul_side(&self) -> &WireSoulSide {
        &self.soul_side
    }

    /// Consumes the Wire and returns the soul side.
    ///
    /// This is useful when you want to move ownership of the soul side
    /// while the wire itself is managed elsewhere.
    pub fn into_soul_side(self) -> WireSoulSide {
        self.soul_side
    }

    /// Creates a UI side (consumer) for receiving messages.
    ///
    /// # Arguments
    ///
    /// * `merge` - If true, receives from the merged channel (consecutive
    ///   ContentParts are combined). If false, receives raw messages.
    ///
    /// # Examples
    ///
    /// ```
    /// use clarity_wire::Wire;
    ///
    /// let wire = Wire::new();
    /// let mut ui_raw = wire.ui_side(false);
    /// let mut ui_merged = wire.ui_side(true);
    /// ```
    pub fn ui_side(&self, merge: bool) -> WireUISide {
        let receiver = if merge {
            self.merged_sender.subscribe()
        } else {
            self.raw_sender.subscribe()
        };

        WireUISide { receiver }
    }

    /// Shuts down the wire, closing all channels.
    ///
    /// This method flushes any pending merged messages and drops all senders,
    /// causing receivers to return `RecvError::Closed` on subsequent receives.
    ///
    /// # Examples
    ///
    /// ```
    /// use clarity_wire::Wire;
    ///
    /// let wire = Wire::new();
    /// // ... use wire ...
    /// wire.shutdown();
    /// ```
    pub fn shutdown(&self) {
        debug!("Shutting down wire");
        self.soul_side.flush();
        // Channels are automatically closed when all senders are dropped.
        // The senders in soul_side will be dropped when Wire is dropped.
    }

    /// Returns the number of active receivers on the raw channel.
    pub fn raw_receiver_count(&self) -> usize {
        self.raw_sender.receiver_count()
    }

    /// Returns the number of active receivers on the merged channel.
    pub fn merged_receiver_count(&self) -> usize {
        self.merged_sender.receiver_count()
    }
}

impl Default for Wire {
    fn default() -> Self {
        Self::new()
    }
}

/// The Soul side of the Wire - used for producing messages.
///
/// This handle allows sending messages into the wire. Messages are
/// automatically sent to both the raw and merged channels.
///
/// Uses interior mutability to allow sending from shared references.
#[derive(Clone)]
pub struct WireSoulSide {
    raw_sender: broadcast::Sender<WireMessage>,
    merged_sender: broadcast::Sender<WireMessage>,
    /// Buffer for accumulating mergeable messages (protected by mutex for interior mutability).
    merge_buffer: Arc<Mutex<Option<WireMessage>>>,
}

impl WireSoulSide {
    /// Sends a message through the wire.
    ///
    /// The message is sent to the raw channel immediately. For the merged
    /// channel, mergeable messages (ContentPart) are buffered and flushed
    /// when a non-mergeable message is sent or `flush()` is called.
    ///
    /// # Arguments
    ///
    /// * `msg` - The message to send.
    ///
    /// # Examples
    ///
    /// ```
    /// use clarity_wire::{Wire, WireMessage};
    ///
    /// let wire = Wire::new();
    /// let soul = wire.soul_side();
    ///
    /// soul.send(WireMessage::TurnBegin {
    ///     user_input: "Hello".to_string(),
    /// });
    /// ```
    pub fn send(&self, msg: WireMessage) {
        trace!("Sending wire message: {:?}", msg);

        // Always send to raw channel immediately.
        if let Err(e) = self.raw_sender.send(msg.clone()) {
            warn!("Failed to send raw message, no receivers: {}", e);
        }

        // Handle merging for the merged channel.
        let mut merge_buffer = self.merge_buffer.lock().unwrap();
        if msg.is_mergeable() {
            if let Some(ref mut buffer) = *merge_buffer {
                if !buffer.try_merge(&msg) {
                    // Cannot merge, flush buffer first.
                    drop(merge_buffer);
                    self.flush();
                    *self.merge_buffer.lock().unwrap() = Some(msg);
                }
            } else {
                *merge_buffer = Some(msg);
            }
        } else {
            // Non-mergeable message: flush any pending buffer first.
            drop(merge_buffer);
            self.flush();
            if let Err(e) = self.merged_sender.send(msg) {
                warn!("Failed to send merged message, no receivers: {}", e);
            }
        }
    }

    /// Flushes any buffered mergeable messages.
    ///
    /// This should be called when you want to ensure all pending ContentPart
    /// messages are sent to the merged channel, for example at the end of
    /// a conversation turn.
    ///
    /// # Examples
    ///
    /// ```
    /// use clarity_wire::{Wire, WireMessage};
    ///
    /// let wire = Wire::new();
    /// let soul = wire.soul_side();
    ///
    /// soul.send(WireMessage::ContentPart { text: "Hello ".to_string() });
    /// soul.send(WireMessage::ContentPart { text: "world".to_string() });
    /// soul.flush(); // Sends the merged "Hello world" message
    /// ```
    pub fn flush(&self) {
        if let Some(buffer) = self.merge_buffer.lock().unwrap().take() {
            debug!("Flushing merged message: {:?}", buffer);
            if let Err(e) = self.merged_sender.send(buffer) {
                warn!("Failed to send merged message, no receivers: {}", e);
            }
        }
    }
}

/// The UI side of the Wire - used for consuming messages.
///
/// This handle allows receiving messages from the wire. Create multiple
/// UI sides to broadcast messages to multiple consumers.
pub struct WireUISide {
    receiver: broadcast::Receiver<WireMessage>,
}

impl WireUISide {
    /// Receives a message from the wire.
    ///
    /// Returns `Some(WireMessage)` on success, or `None` if the channel
    /// is closed (all senders have been dropped).
    ///
    /// # Examples
    ///
    /// ```
    /// use clarity_wire::{Wire, WireMessage};
    ///
    /// # tokio::runtime::Runtime::new().unwrap().block_on(async {
    /// let wire = Wire::new();
    /// let soul = wire.soul_side();
    /// let mut ui = wire.ui_side(false);
    ///
    /// soul.send(WireMessage::TurnEnd);
    ///
    /// if let Some(msg) = ui.recv().await {
    ///     assert!(matches!(msg, WireMessage::TurnEnd));
    /// }
    /// # });
    /// ```
    pub async fn recv(&mut self) -> Option<WireMessage> {
        loop {
            match self.receiver.recv().await {
                Ok(msg) => {
                    trace!("Received wire message: {:?}", msg);
                    return Some(msg);
                }
                Err(broadcast::error::RecvError::Closed) => {
                    debug!("Wire channel closed");
                    return None;
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    error!("UI receiver lagged, skipped {} messages", n);
                    // Continue to receive the next available message.
                    continue;
                }
            }
        }
    }

    /// Attempts to receive a message without blocking.
    ///
    /// Returns `Ok(Some(WireMessage))` if a message is available,
    /// `Ok(None)` if the channel is empty, or `Err(())` if closed.
    ///
    /// # Examples
    ///
    /// ```
    /// use clarity_wire::{Wire, WireMessage};
    ///
    /// let wire = Wire::new();
    /// let soul = wire.soul_side();
    /// let mut ui = wire.ui_side(false);
    ///
    /// // Initially empty
    /// assert!(ui.try_recv().is_none());
    ///
    /// soul.send(WireMessage::TurnEnd);
    ///
    /// // Now available
    /// assert!(ui.try_recv().is_some());
    /// ```
    pub fn try_recv(&mut self) -> Option<WireMessage> {
        match self.receiver.try_recv() {
            Ok(msg) => {
                trace!("Received wire message (non-blocking): {:?}", msg);
                Some(msg)
            }
            Err(broadcast::error::TryRecvError::Empty) => None,
            Err(broadcast::error::TryRecvError::Closed) => {
                debug!("Wire channel closed");
                None
            }
            Err(broadcast::error::TryRecvError::Lagged(n)) => {
                error!("UI receiver lagged, skipped {} messages", n);
                None
            }
        }
    }
}

// ============================================================================
// Protocol-Driven UI Layer (Phase 2 Pilot)
// ============================================================================

/// Semantic text role — frontend maps to theme-specific styling.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextRole {
    Label,
    Body,
    Title,
}

/// Semantic button style — frontend maps to theme-specific coloring.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ButtonStyle {
    Primary,
    Secondary,
    Danger,
}

/// Declarative UI commands produced by a ViewModel.
/// The frontend translates these into native draw calls.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ViewCommand {
    /// Vertical stack of children.
    VStack { children: Vec<ViewCommand> },
    /// Horizontal stack of children.
    HStack { children: Vec<ViewCommand> },
    /// Static text label.
    Text {
        content: String,
        role: TextRole,
        size: f32,
    },
    /// Single-line text input.
    TextInput {
        id: String,
        value: String,
        placeholder: String,
        password: bool,
        width: f32,
    },
    /// Dropdown selector.
    ComboBox {
        id: String,
        selected_value: String,
        /// (value, label) pairs.
        options: Vec<(String, String)>,
        width: f32,
    },
    /// Clickable button.
    Button {
        id: String,
        label: String,
        style: ButtonStyle,
        min_width: f32,
        min_height: f32,
    },
    /// Vertical spacer.
    Space { height: f32 },
}

/// User interaction events captured by the frontend and sent to the ViewModel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UserAction {
    TextInputChange { id: String, value: String },
    ComboChange { id: String, selected: String },
    ButtonClick { id: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{timeout, Duration};

    /// Test basic send and receive functionality.
    #[tokio::test]
    async fn test_wire_basic() {
        let wire = Wire::new();
        let soul = wire.soul_side();
        let mut ui = wire.ui_side(false);

        // Send a message
        soul.send(WireMessage::TurnBegin {
            user_input: "Hello, world!".to_string(),
        });

        // Receive the message
        let msg = timeout(Duration::from_millis(100), ui.recv())
            .await
            .expect("timeout")
            .expect("channel closed");

        assert!(matches!(
            msg,
            WireMessage::TurnBegin { user_input } if user_input == "Hello, world!"
        ));
    }

    /// Test broadcasting to multiple consumers.
    #[tokio::test]
    async fn test_wire_broadcast() {
        let wire = Wire::new();
        let soul = wire.soul_side();

        // Create multiple UI sides
        let mut ui1 = wire.ui_side(false);
        let mut ui2 = wire.ui_side(false);
        let mut ui3 = wire.ui_side(false);

        // Send a message
        soul.send(WireMessage::StatusUpdate {
            message: "Test broadcast".to_string(),
        });

        // All consumers should receive the message
        let msg1 = ui1.recv().await.expect("ui1 should receive");
        let msg2 = ui2.recv().await.expect("ui2 should receive");
        let msg3 = ui3.recv().await.expect("ui3 should receive");

        assert_eq!(msg1, msg2);
        assert_eq!(msg2, msg3);
    }

    /// Test graceful shutdown behavior.
    #[tokio::test]
    async fn test_wire_shutdown() {
        let wire = Wire::new();
        let soul = wire.soul_side();
        let mut ui = wire.ui_side(false);

        // Send some messages
        soul.send(WireMessage::StepBegin {
            tool_name: "test_tool".to_string(),
        });
        soul.send(WireMessage::TurnEnd);

        // Shutdown (flushes buffers)
        wire.shutdown();

        // Drop the wire to close channels (drops all senders)
        drop(wire);

        // Consume remaining messages
        let mut received = 0;
        while let Some(_msg) = ui.recv().await {
            received += 1;
        }

        // Should have received 2 messages before channel closed
        assert_eq!(received, 2);
    }

    /// Test message merging in merged channel.
    #[tokio::test]
    async fn test_wire_merging() {
        let wire = Wire::new();
        let soul = wire.soul_side();

        // Raw channel gets all messages separately
        let mut ui_raw = wire.ui_side(false);
        // Merged channel combines consecutive ContentParts
        let mut ui_merged = wire.ui_side(true);

        // Send interleaved messages
        soul.send(WireMessage::TurnBegin {
            user_input: "Hi".to_string(),
        });
        soul.send(WireMessage::ContentPart {
            text: "Hello ".to_string(),
        });
        soul.send(WireMessage::ContentPart {
            text: "world".to_string(),
        });
        soul.send(WireMessage::ContentPart {
            text: "!".to_string(),
        });
        soul.send(WireMessage::TurnEnd);

        // Raw channel: should receive all 5 messages
        let mut raw_count = 0;
        while let Some(msg) = ui_raw.recv().await {
            if matches!(msg, WireMessage::TurnEnd) {
                raw_count += 1;
                break;
            }
            raw_count += 1;
        }
        assert_eq!(raw_count, 5, "Raw channel should receive 5 messages");

        // Merged channel: TurnBegin, ContentPart (merged), TurnEnd = 3 messages
        let msg1 = ui_merged.recv().await.expect("should receive TurnBegin");
        assert!(matches!(msg1, WireMessage::TurnBegin { .. }));

        let msg2 = ui_merged
            .recv()
            .await
            .expect("should receive merged ContentPart");
        assert_eq!(
            msg2,
            WireMessage::ContentPart {
                text: "Hello world!".to_string(),
            }
        );

        let msg3 = ui_merged.recv().await.expect("should receive TurnEnd");
        assert!(matches!(msg3, WireMessage::TurnEnd));
    }

    /// Test ToolCall and ToolResult messages.
    #[tokio::test]
    async fn test_wire_tool_messages() {
        let wire = Wire::new();
        let soul = wire.soul_side();
        let mut ui = wire.ui_side(false);

        let tool_call = WireMessage::ToolCall {
            id: "call_123".to_string(),
            name: "read_file".to_string(),
            arguments: serde_json::json!({"path": "/tmp/test.txt"}),
        };

        let tool_result = WireMessage::ToolResult {
            id: "call_123".to_string(),
            result: "File contents here".to_string(),
        };

        soul.send(tool_call.clone());
        soul.send(tool_result.clone());

        assert_eq!(ui.recv().await.unwrap(), tool_call);
        assert_eq!(ui.recv().await.unwrap(), tool_result);
    }

    /// Test non-blocking receive.
    #[tokio::test]
    async fn test_wire_try_recv() {
        let wire = Wire::new();
        let soul = wire.soul_side();
        let mut ui = wire.ui_side(false);

        // Should be empty initially
        assert!(ui.try_recv().is_none());

        // Send and receive
        soul.send(WireMessage::TurnEnd);
        assert!(ui.try_recv().is_some());

        // Should be empty again
        assert!(ui.try_recv().is_none());
    }

    /// Test Wire::with_capacity.
    #[test]
    fn test_wire_with_capacity() {
        let wire = Wire::with_capacity(256);
        let soul = wire.soul_side();
        let mut ui = wire.ui_side(false);

        soul.send(WireMessage::TurnEnd);

        // Use try_recv since we're in a sync test
        assert!(ui.try_recv().is_some());
    }

    /// Test that flush sends buffered mergeable messages.
    #[tokio::test]
    async fn test_wire_flush() {
        let wire = Wire::new();
        let soul = wire.soul_side();
        let mut ui = wire.ui_side(true); // merged channel

        // Send only mergeable messages
        soul.send(WireMessage::ContentPart {
            text: "A".to_string(),
        });
        soul.send(WireMessage::ContentPart {
            text: "B".to_string(),
        });

        // Flush to send the merged message
        soul.flush();

        let msg = ui.recv().await.expect("should receive after flush");
        assert_eq!(
            msg,
            WireMessage::ContentPart {
                text: "AB".to_string()
            }
        );
    }

    // ------------------------------------------------------------------------
    // Protocol-Driven UI Layer tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_text_role_serde_roundtrip() {
        for role in [TextRole::Label, TextRole::Body, TextRole::Title] {
            let json = serde_json::to_string(&role).unwrap();
            let decoded: TextRole = serde_json::from_str(&json).unwrap();
            assert_eq!(role, decoded);
        }
    }

    #[test]
    fn test_button_style_serde_roundtrip() {
        for style in [ButtonStyle::Primary, ButtonStyle::Secondary, ButtonStyle::Danger] {
            let json = serde_json::to_string(&style).unwrap();
            let decoded: ButtonStyle = serde_json::from_str(&json).unwrap();
            assert_eq!(style, decoded);
        }
    }

    #[test]
    fn test_view_command_nested_roundtrip() {
        let cmd = ViewCommand::VStack {
            children: vec![
                ViewCommand::HStack {
                    children: vec![
                        ViewCommand::Text {
                            content: "Provider".into(),
                            role: TextRole::Label,
                            size: 13.0,
                        },
                        ViewCommand::ComboBox {
                            id: "provider".into(),
                            selected_value: "openai".into(),
                            options: vec![
                                ("openai".into(), "OpenAI".into()),
                                ("kimi".into(), "Kimi".into()),
                            ],
                            width: 200.0,
                        },
                    ],
                },
                ViewCommand::Space { height: 8.0 },
                ViewCommand::Button {
                    id: "save".into(),
                    label: "Save".into(),
                    style: ButtonStyle::Primary,
                    min_width: 80.0,
                    min_height: 32.0,
                },
            ],
        };
        let json = serde_json::to_string(&cmd).unwrap();
        let decoded: ViewCommand = serde_json::from_str(&json).unwrap();
        assert_eq!(cmd, decoded);
    }

    #[test]
    fn test_user_action_roundtrip() {
        for action in [
            UserAction::TextInputChange { id: "api_key".into(), value: "secret".into() },
            UserAction::ComboChange { id: "provider".into(), selected: "openai".into() },
            UserAction::ButtonClick { id: "save".into() },
        ] {
            let json = serde_json::to_string(&action).unwrap();
            let decoded: UserAction = serde_json::from_str(&json).unwrap();
            assert_eq!(action, decoded);
        }
    }
}
