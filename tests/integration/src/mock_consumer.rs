use clarity_wire::{Wire, WireMessage};
use std::sync::Arc;
use tokio::sync::Mutex;

/// A test helper that subscribes to a [`Wire`] and collects all
/// [`WireMessage`] variants into a buffer for later assertions.
pub struct MockConsumer {
    received: Arc<Mutex<Vec<WireMessage>>>,
}

impl MockConsumer {
    /// Subscribe to the raw (unmerged) channel of `wire` and start a
    /// background task that records every message.
    pub fn subscribe(wire: &Wire) -> Self {
        let mut ui = wire.ui_side(false);
        let received = Arc::new(Mutex::new(Vec::new()));
        let r = received.clone();
        tokio::spawn(async move {
            while let Some(msg) = ui.recv().await {
                r.lock().await.push(msg);
            }
        });
        Self { received }
    }

    /// Return a snapshot of the messages received so far.
    pub async fn messages(&self) -> Vec<WireMessage> {
        self.received.lock().await.clone()
    }

    /// Assert that at least one `ContentPart` contains `expected`.
    pub async fn assert_received_content(&self, expected: &str) {
        let msgs = self.messages().await;
        let found = msgs.iter().any(|m| match m {
            WireMessage::ContentPart { text, .. } => text.contains(expected),
            _ => false,
        });
        assert!(
            found,
            "Expected content '{}' not found in wire messages: {:?}",
            expected, msgs
        );
    }

    /// Assert that the message list satisfies the given predicate.
    pub async fn assert_has(&self, predicate: impl Fn(&WireMessage) -> bool) {
        let msgs = self.messages().await;
        assert!(
            msgs.iter().any(predicate),
            "Expected message not found in: {:?}",
            msgs
        );
    }
}
