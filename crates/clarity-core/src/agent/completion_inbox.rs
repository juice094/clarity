//! Completion inbox: carries background completion summaries into the parent
//! agent's conversation.
//!
//! Background producers (e.g. [`crate::background::BackgroundTaskManager`])
//! `push` a one-line summary when a task finishes; the
//! [`CompletionInboxHook`] drains the inbox on the next LLM input and injects
//! each summary as a `Message::system`, so the parent agent sees background
//! completions on its following turn — the same injection mechanism the LSP
//! hook uses for diagnostics.
//!
//! ponytail: summaries surface only at the next LLM input of this agent;
//! there is no wake-up of an idle agent and no UI-level surfacing. Upgrade
//! path: emit a wire-level completion event and render it in the chat panel.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use clarity_contract::Message;

use crate::agent::hooks::{AgentHook, HookResult};
use crate::types::ToolCall;

/// Shared queue of background completion summaries awaiting injection into
/// the parent agent's conversation context.
///
/// Cheap to clone-free share via `Arc`; all operations are synchronous and
/// lock-free of await points, so it is safe to call from background task
/// contexts.
#[derive(Debug, Default)]
pub struct CompletionInbox {
    pending: Mutex<VecDeque<String>>,
}

impl CompletionInbox {
    /// Create an empty inbox wrapped in an [`Arc`].
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Push a completion summary. Called from background task contexts;
    /// lock poisoning is tolerated by dropping the message.
    pub fn push(&self, message: impl Into<String>) {
        if let Ok(mut guard) = self.pending.lock() {
            guard.push_back(message.into());
        }
    }

    /// Drain all pending summaries.
    pub fn drain(&self) -> Vec<String> {
        match self.pending.lock() {
            Ok(mut guard) => guard.drain(..).collect(),
            Err(_) => Vec::new(),
        }
    }
}

/// [`AgentHook`] that injects drained completion summaries as system
/// messages before each LLM call.
pub struct CompletionInboxHook {
    inbox: Arc<CompletionInbox>,
}

impl CompletionInboxHook {
    /// Create a hook draining the given inbox.
    pub fn new(inbox: Arc<CompletionInbox>) -> Self {
        Self { inbox }
    }
}

#[async_trait::async_trait]
impl AgentHook for CompletionInboxHook {
    async fn before_tool_call(&self, _tool_call: &mut ToolCall) -> HookResult {
        HookResult::Continue
    }

    async fn after_tool_call(&self, _tool_call: &ToolCall, _result: &mut serde_json::Value) {}

    async fn on_llm_input(&self, messages: &mut Vec<Message>) {
        for summary in self.inbox.drain() {
            messages.push(Message::system(summary));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inbox_push_then_drain_empties_queue() {
        let inbox = CompletionInbox::new();
        assert!(inbox.drain().is_empty());

        inbox.push("[background task completed] demo: done");
        inbox.push("[background task failed] other: boom");

        let drained = inbox.drain();
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0], "[background task completed] demo: done");
        assert!(inbox.drain().is_empty());
    }

    #[tokio::test]
    async fn hook_injects_summaries_as_system_messages() {
        let inbox = CompletionInbox::new();
        inbox.push("[subagent completed] task X: summary");
        let hook = CompletionInboxHook::new(inbox);

        let mut messages = vec![Message::user("hello")];
        hook.on_llm_input(&mut messages).await;

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[1].role, clarity_contract::MessageRole::System);
        assert!(messages[1].content.contains("[subagent completed]"));
    }
}
