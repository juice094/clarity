//! ChatDriver trait — decouples message-building strategies from the core Agent loop.
//!
//! This allows frontends (Gateway, TUI, Headless) to inject their own
//! conversation-history formats without extending the `Op` enum.

use crate::llm::api::Message;

/// A driver that decides how a user query is turned into a message list
/// and how the final response is post-processed.
pub trait ChatDriver: Send + Sync {
    /// Build the message list for a new user turn.
    ///
    /// The returned vector must contain at least one user message.
    /// The driver is free to prepend system messages, file context,
    /// or conversation history in whatever format the target LLM expects.
    fn build_messages(&self, query: &str, system_prompt: &str) -> Vec<Message>;

    /// Post-process the final response before returning to the caller.
    ///
    /// Default implementation is a no-op pass-through.
    fn post_process(&self, response: &str) -> String {
        response.to_string()
    }
}

/// Default driver: single system prompt + single user message.
#[derive(Debug, Clone, Default)]
pub struct DefaultChatDriver;

impl ChatDriver for DefaultChatDriver {
    fn build_messages(&self, query: &str, system_prompt: &str) -> Vec<Message> {
        vec![Message::system(system_prompt), Message::user(query)]
    }
}

/// Gateway-compatible driver: accepts a full message history (OpenAI-style).
#[derive(Debug, Clone, Default)]
pub struct ConversationChatDriver {
    /// Pre-built message history, including system / user / assistant turns.
    pub history: Vec<Message>,
}

impl ChatDriver for ConversationChatDriver {
    fn build_messages(&self, query: &str, system_prompt: &str) -> Vec<Message> {
        let mut messages = self.history.clone();
        // Inject or update the system prompt at the front.
        if let Some(first) = messages.first_mut() {
            if first.role == crate::llm::api::MessageRole::System {
                first.content = system_prompt.to_string();
            } else {
                messages.insert(0, Message::system(system_prompt));
            }
        } else {
            messages.push(Message::system(system_prompt));
        }
        messages.push(Message::user(query));
        messages
    }
}
