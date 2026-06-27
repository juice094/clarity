//! Channel lifecycle helpers.
//!
//! Provides `run_channel_listener` which spawns a listen task and a receive
//! loop on separate tokio tasks, returning a `JoinHandle`.

use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::chkit::channel::{Channel, ChannelMessage};

/// Spawn a channel listener on two tokio tasks and return a handle that
/// completes when both tasks have finished.
///
/// - One task runs `channel.listen(tx)` to populate the mpsc channel.
/// - The other task drains the mpsc receiver and calls `on_message` for
///   each inbound message.
///
/// The returned [`JoinHandle`] resolves once the receive loop ends (i.e.
/// when the sender is dropped and the channel is drained) and the listen
/// task has been awaited.
pub fn run_channel_listener(
    channel: Arc<dyn Channel>,
    on_message: impl Fn(ChannelMessage) + Send + 'static,
) -> JoinHandle<()> {
    let (tx, mut rx) = mpsc::channel::<ChannelMessage>(256);

    let listen_task = {
        let channel = Arc::clone(&channel);
        tokio::spawn(async move {
            if let Err(e) = channel.listen(tx).await {
                tracing::error!(error = %e, "channel listen task exited with error");
            }
        })
    };

    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            on_message(msg);
        }
        // Ensure the listen task is awaited so we don't drop it early.
        let _ = listen_task.await;
    })
}
