//! Compaction service for intelligently compressing conversation history
//!
//! This module provides `CompactionService`, which proactively compresses
//! old messages before the conversation hits token limits.

use crate::error::AgentError;
use crate::llm::api::{LlmProvider, Message, MessageRole};

/// Configuration for the compaction service
#[derive(Debug, Clone, PartialEq)]
pub struct CompactionServiceConfig {
    /// Token threshold that triggers compaction
    pub token_limit: usize,
    /// Model identifier used for summarization (e.g. "kimi-latest")
    pub compaction_model: String,
    /// Number of tokens worth of recent messages to keep intact
    pub history_retention_tokens: usize,
}

/// Service that compacts conversation history by summarizing old messages
#[derive(Debug, Clone, PartialEq)]
pub struct CompactionService {
    config: CompactionServiceConfig,
    tier1_enabled: bool,
}

impl CompactionService {
    /// Create a new compaction service from configuration
    pub fn new(config: CompactionServiceConfig) -> Self {
        Self {
            config,
            tier1_enabled: true,
        }
    }

    /// Convenience constructor (alias for `new`)
    pub fn with_config(config: CompactionServiceConfig) -> Self {
        Self::new(config)
    }

    /// Enable or disable Tier-1 (fast local truncation) compaction.
    pub fn with_tier1(mut self, enabled: bool) -> Self {
        self.tier1_enabled = enabled;
        self
    }

    /// Tier-1 compaction: fast local truncation of old assistant text messages.
    ///
    /// Preserves system prompt, tool calls, and recent messages within the
    /// retention window. Only truncates plain assistant content (no tool_calls)
    /// that exceeds 120 characters.
    fn tier1_compact(&self, messages: &mut [Message]) {
        let total = Self::estimate_tokens(messages);
        let mut accumulated = 0;
        let mut split_index = messages.len();
        for (i, msg) in messages.iter().enumerate() {
            let tokens = crate::compaction::estimate_text_tokens(&msg.content);
            accumulated += tokens;
            if total.saturating_sub(accumulated) <= self.config.history_retention_tokens {
                split_index = i + 1;
                break;
            }
        }
        if split_index == 0 || split_index >= messages.len() {
            return;
        }
        for msg in messages[..split_index].iter_mut() {
            if msg.role == MessageRole::Assistant
                && msg.tool_calls.is_none()
                && msg.content.len() > 120
            {
                let orig_len = msg.content.len();
                msg.content.truncate(120);
                msg.content
                    .push_str(&format!(" [...truncated, {} total chars]", orig_len));
            }
        }
    }

    /// Estimate token count for a slice of messages.
    /// Delegates to the shared weighted heuristic in `crate::compaction`.
    pub fn estimate_tokens(messages: &[Message]) -> usize {
        crate::compaction::estimate_message_tokens(messages)
    }

    /// Returns `true` when the total estimated tokens exceed `token_limit`
    pub fn needs_compaction(&self, messages: &[Message]) -> bool {
        Self::estimate_tokens(messages) >= self.config.token_limit
    }

    /// Compact `messages` in-place if they exceed the token limit.
    ///
    /// On LLM failure a warning is logged and the original history is left
    /// untouched (graceful degradation).
    pub async fn maybe_compact(
        &self,
        messages: &mut Vec<Message>,
        llm: &dyn LlmProvider,
    ) -> Result<(), AgentError> {
        if !self.needs_compaction(messages) {
            return Ok(());
        }

        // Tier 1: fast local truncation (no LLM call).
        if self.tier1_enabled {
            self.tier1_compact(messages);
            if !self.needs_compaction(messages) {
                tracing::info!("Tier-1 compaction resolved context pressure");
                return Ok(());
            }
        }

        // Tier 2: LLM summarization.
        // Locate the original system prompt (if any).
        let system_idx = messages.iter().position(|m| m.role == MessageRole::System);

        // Determine how many messages are "old" by accumulating from oldest
        // to newest until the remaining messages fit inside the retention window.
        let total = Self::estimate_tokens(messages);
        let mut accumulated = 0;
        let mut split_index = messages.len();

        for (i, msg) in messages.iter().enumerate() {
            let tokens = crate::compaction::estimate_text_tokens(&msg.content);
            accumulated += tokens;
            if total.saturating_sub(accumulated) <= self.config.history_retention_tokens {
                split_index = i + 1;
                break;
            }
        }

        // Nothing to compact.
        if split_index == 0 || split_index > messages.len() {
            return Ok(());
        }

        // Build the list of old messages to summarise, skipping the original
        // system prompt so it stays intact.
        let mut old_messages = Vec::new();
        for (i, msg) in messages[..split_index].iter().enumerate() {
            if Some(i) == system_idx {
                continue;
            }
            old_messages.push(msg);
        }

        if old_messages.is_empty() {
            return Ok(());
        }

        // Build summarization prompt.
        let mut prompt_text = String::from(
            "Summarize the following conversation concisely, preserving key facts and decisions:\n\n",
        );
        for msg in old_messages {
            let role_label = match msg.role {
                MessageRole::System => "system",
                MessageRole::User => "user",
                MessageRole::Assistant => "assistant",
                MessageRole::Tool => "tool",
            };
            prompt_text.push_str(&format!("**{}**: {}\n\n", role_label, msg.content));
        }

        let prompt = vec![
            Message::system("You are a helpful assistant that summarizes conversation history."),
            Message::user(prompt_text),
        ];
        let tools = serde_json::json!({ "functions": [] });

        match llm.complete(&prompt, &tools).await {
            Ok(response) => {
                let summary = response.content;

                let mut new_messages = Vec::new();

                // Keep the original system prompt first.
                if let Some(idx) = system_idx {
                    new_messages.push(messages[idx].clone());
                }

                // Insert the summary immediately after the original system prompt.
                new_messages.push(Message::system(format!(
                    "Previous conversation summary: {}",
                    summary
                )));

                // Append the recent messages that were left intact.
                new_messages.extend(messages[split_index..].iter().cloned());

                *messages = new_messages;
                Ok(())
            }
            Err(e) => {
                tracing::warn!("Compaction failed: {}", e);
                Ok(()) // graceful degradation
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::AgentError;
    use async_trait::async_trait;
    use serde_json::Value;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct MockLlm {
        response: String,
    }

    #[async_trait]
    impl LlmProvider for MockLlm {
        async fn complete(
            &self,
            _messages: &[Message],
            _tools: &Value,
        ) -> Result<crate::agent::LlmResponse, AgentError> {
            Ok(crate::agent::LlmResponse {
                content: self.response.clone(),
                tool_calls: vec![],
                is_complete: true,
            })
        }

        fn stream(
            &self,
            _messages: &[Message],
            _tools: &Value,
        ) -> Result<
            tokio::sync::mpsc::Receiver<Result<crate::llm::StreamDelta, AgentError>>,
            AgentError,
        > {
            let (tx, rx) = tokio::sync::mpsc::channel(1);
            tokio::spawn(async move {
                let _ = tx
                    .send(Ok(crate::llm::StreamDelta {
                        content: Some("chunk".to_string()),
                        tool_calls: vec![],
                    }))
                    .await;
            });
            Ok(rx)
        }

        fn set_prompt_cache_key(&mut self, _key: &str) {}
    }

    struct FailingMockLlm;

    #[async_trait]
    impl LlmProvider for FailingMockLlm {
        async fn complete(
            &self,
            _messages: &[Message],
            _tools: &Value,
        ) -> Result<crate::agent::LlmResponse, AgentError> {
            Err(AgentError::Llm("mock llm error".to_string()))
        }

        fn stream(
            &self,
            _messages: &[Message],
            _tools: &Value,
        ) -> Result<
            tokio::sync::mpsc::Receiver<Result<crate::llm::StreamDelta, AgentError>>,
            AgentError,
        > {
            let (tx, rx) = tokio::sync::mpsc::channel(1);
            tokio::spawn(async move {
                let _ = tx
                    .send(Err(AgentError::Llm("mock stream error".to_string())))
                    .await;
            });
            Ok(rx)
        }

        fn set_prompt_cache_key(&mut self, _key: &str) {}
    }

    fn make_messages(n: usize, content_len: usize) -> Vec<Message> {
        (0..n)
            .map(|i| {
                let content = "x".repeat(content_len);
                if i % 2 == 0 {
                    Message::user(content)
                } else {
                    Message::assistant(content)
                }
            })
            .collect()
    }

    #[test]
    fn test_estimate_tokens() {
        assert_eq!(CompactionService::estimate_tokens(&[]), 0);

        let msg = Message::user("abcd"); // cl100k: 1 token
        assert_eq!(CompactionService::estimate_tokens(&[msg]), 1);

        let msgs = vec![
            Message::user("abcdefgh"),      // cl100k: 1 token
            Message::assistant("abcdefgh"), // cl100k: 1 token
        ];
        assert_eq!(CompactionService::estimate_tokens(&msgs), 2);
    }

    #[test]
    fn test_needs_compaction_true() {
        let config = CompactionServiceConfig {
            token_limit: 10,
            compaction_model: "kimi-latest".to_string(),
            history_retention_tokens: 5,
        };
        let service = CompactionService::new(config);
        // 15 short messages * ~1 token each = ~15 tokens > 10
        let messages = make_messages(15, 8);
        assert!(service.needs_compaction(&messages));
    }

    #[test]
    fn test_needs_compaction_false() {
        let config = CompactionServiceConfig {
            token_limit: 100,
            compaction_model: "kimi-latest".to_string(),
            history_retention_tokens: 20,
        };
        let service = CompactionService::new(config);
        let messages = make_messages(3, 8); // 3 * 2 = 6 tokens
        assert!(!service.needs_compaction(&messages));
    }

    #[tokio::test]
    async fn test_compact_retains_recent_messages() {
        let config = CompactionServiceConfig {
            token_limit: 10,
            compaction_model: "kimi-latest".to_string(),
            history_retention_tokens: 6, // keep ~2 messages (each ~3 tokens via cl100k)
        };
        let service = CompactionService::with_config(config);

        let mut messages = vec![
            Message::system("You are helpful"),
            Message::user("msg1".to_string() + &"x".repeat(4)), // ~3 tokens
            Message::assistant("msg2".to_string() + &"x".repeat(4)),
            Message::user("msg3".to_string() + &"x".repeat(4)),
            Message::assistant("msg4".to_string() + &"x".repeat(4)),
            Message::user("msg5".to_string() + &"x".repeat(4)),
        ];
        // total = system(~3) + 5*~3 = ~18 tokens > 10 -> triggers compaction
        // retention = 6 -> keep last 2 messages (~6 tokens)
        // old = system + first 3 user/assistant pairs -> compacted into summary

        let llm = MockLlm {
            response: "Summary text".to_string(),
        };

        service.maybe_compact(&mut messages, &llm).await.unwrap();

        // Should be: system, summary, msg4, msg5
        assert_eq!(messages.len(), 4);
        assert_eq!(messages[0].role, MessageRole::System);
        assert_eq!(messages[0].content, "You are helpful");
        assert!(
            messages[1].content.contains("Summary text"),
            "Expected summary message, got: {}",
            messages[1].content
        );
        assert_eq!(messages[2].role, MessageRole::Assistant);
        assert!(messages[2].content.starts_with("msg4"));
        assert_eq!(messages[3].role, MessageRole::User);
        assert!(messages[3].content.starts_with("msg5"));
    }

    #[tokio::test]
    async fn test_compact_preserves_system_prompt_position() {
        let config = CompactionServiceConfig {
            token_limit: 5,
            compaction_model: "kimi-latest".to_string(),
            history_retention_tokens: 2,
        };
        let service = CompactionService::with_config(config);

        let mut messages = vec![
            Message::system("Original system prompt"),
            Message::user("old user message"),
            Message::assistant("old assistant message"),
            Message::user("recent"),
        ];

        let llm = MockLlm {
            response: "Compact summary".to_string(),
        };

        service.maybe_compact(&mut messages, &llm).await.unwrap();

        // First message must still be the original system prompt.
        assert_eq!(messages[0].role, MessageRole::System);
        assert_eq!(messages[0].content, "Original system prompt");

        // Second message should be the summary placed directly after it.
        assert_eq!(messages[1].role, MessageRole::System);
        assert!(
            messages[1].content.contains("Compact summary"),
            "Expected summary after system prompt, got: {}",
            messages[1].content
        );
    }

    #[tokio::test]
    async fn test_compact_graceful_on_llm_error() {
        let config = CompactionServiceConfig {
            token_limit: 5,
            compaction_model: "kimi-latest".to_string(),
            history_retention_tokens: 2,
        };
        let service = CompactionService::with_config(config);

        let original = vec![
            Message::system("System"),
            Message::user("old"),
            Message::user("recent"),
        ];
        let mut messages = original.clone();

        service
            .maybe_compact(&mut messages, &FailingMockLlm)
            .await
            .unwrap();

        // Messages should remain unchanged on LLM error.
        assert_eq!(messages.len(), original.len());
        for (a, b) in messages.iter().zip(original.iter()) {
            assert_eq!(a.role, b.role);
            assert_eq!(a.content, b.content);
        }
    }

    #[tokio::test]
    async fn test_no_compaction_when_disabled() {
        // "disabled" = token limit is high enough that compaction is not needed
        let config = CompactionServiceConfig {
            token_limit: 1000,
            compaction_model: "kimi-latest".to_string(),
            history_retention_tokens: 100,
        };
        let service = CompactionService::with_config(config);

        let original = vec![
            Message::system("System"),
            Message::user("Hello"),
            Message::assistant("Hi!"),
        ];
        let mut messages = original.clone();

        let llm = MockLlm {
            response: "Should not be used".to_string(),
        };

        service.maybe_compact(&mut messages, &llm).await.unwrap();

        // Should remain untouched because token count is below limit.
        assert_eq!(messages.len(), original.len());
        for (a, b) in messages.iter().zip(original.iter()) {
            assert_eq!(a.role, b.role);
            assert_eq!(a.content, b.content);
        }
    }

    #[tokio::test]
    async fn test_compact_with_no_system_prompt() {
        let config = CompactionServiceConfig {
            token_limit: 5,
            compaction_model: "kimi-latest".to_string(),
            history_retention_tokens: 2,
        };
        let service = CompactionService::with_config(config);

        let mut messages = vec![
            Message::user("old user"),
            Message::assistant("old assistant"),
            Message::user("recent"),
        ];

        let llm = MockLlm {
            response: "No system summary".to_string(),
        };

        service.maybe_compact(&mut messages, &llm).await.unwrap();

        // No system prompt existed, so summary becomes the first message.
        assert_eq!(messages[0].role, MessageRole::System);
        assert!(messages[0].content.contains("No system summary"));
        // Recent message preserved.
        assert_eq!(messages[1].content, "recent");
    }

    #[tokio::test]
    async fn test_tier1_compact_truncates_old_assistant_text() {
        let config = CompactionServiceConfig {
            token_limit: 10,
            compaction_model: "kimi-latest".to_string(),
            history_retention_tokens: 4,
        };
        let service = CompactionService::with_config(config);

        let long_text = "a".repeat(200);
        let mut messages = vec![
            Message::system("You are helpful"),
            Message::user("short"),
            Message::assistant(long_text.clone()),
            Message::user("recent"),
        ];

        service.tier1_compact(&mut messages);

        assert!(
            messages[2].content.contains("[...truncated"),
            "Expected truncation marker, got: {}",
            messages[2].content
        );
        assert_eq!(
            messages[2].content.len(),
            120 + " [...truncated, 200 total chars]".len()
        );
        // Recent message untouched
        assert_eq!(messages[3].content, "recent");
    }

    #[tokio::test]
    async fn test_tier1_compact_preserves_tool_calls() {
        let config = CompactionServiceConfig {
            token_limit: 10,
            compaction_model: "kimi-latest".to_string(),
            history_retention_tokens: 4,
        };
        let service = CompactionService::with_config(config);

        let long_text = "b".repeat(200);
        let mut messages = vec![
            Message::system("You are helpful"),
            Message::user("short"),
            Message {
                role: MessageRole::Assistant,
                content: long_text.clone(),
                tool_calls: Some(vec![]),
                tool_call_id: None,
            },
            Message::user("recent"),
        ];

        service.tier1_compact(&mut messages);

        // Assistant message with tool_calls should NOT be truncated
        assert_eq!(messages[2].content, long_text);
    }

    #[tokio::test]
    async fn test_tier1_compact_skips_when_disabled() {
        let config = CompactionServiceConfig {
            token_limit: 10,
            compaction_model: "kimi-latest".to_string(),
            history_retention_tokens: 4,
        };
        let service = CompactionService::with_config(config).with_tier1(false);

        let long_text = "c".repeat(200);
        let mut messages = vec![
            Message::system("You are helpful"),
            Message::user("short"),
            Message::assistant(long_text.clone()),
            Message::user("recent"),
        ];

        // Tier-1 disabled, so maybe_compact goes straight to LLM summarization.
        let llm = MockLlm {
            response: "Summary".to_string(),
        };
        service.maybe_compact(&mut messages, &llm).await.unwrap();

        // The old assistant text should have been summarized away, not truncated.
        assert!(
            !messages.iter().any(|m| m.content.contains("[...truncated")),
            "Tier-1 should be skipped when disabled"
        );
    }

    #[tokio::test]
    async fn test_llm_called_with_correct_prompt() {
        struct RecordingMockLlm {
            call_count: AtomicUsize,
        }

        #[async_trait]
        impl LlmProvider for RecordingMockLlm {
            async fn complete(
                &self,
                messages: &[Message],
                _tools: &Value,
            ) -> Result<crate::agent::LlmResponse, AgentError> {
                self.call_count.fetch_add(1, Ordering::SeqCst);
                // Verify prompt structure
                assert_eq!(messages.len(), 2);
                assert_eq!(messages[0].role, MessageRole::System);
                assert_eq!(messages[1].role, MessageRole::User);
                assert!(
                    messages[1]
                        .content
                        .contains("Summarize the following conversation"),
                    "Expected summarization prompt"
                );
                assert!(
                    messages[1].content.contains("old user message"),
                    "Prompt should contain old message content"
                );
                Ok(crate::agent::LlmResponse {
                    content: "Recorded summary".to_string(),
                    tool_calls: vec![],
                    is_complete: true,
                })
            }

            fn stream(
                &self,
                _messages: &[Message],
                _tools: &Value,
            ) -> Result<
                tokio::sync::mpsc::Receiver<Result<crate::llm::StreamDelta, AgentError>>,
                AgentError,
            > {
                let (tx, rx) = tokio::sync::mpsc::channel(1);
                tokio::spawn(async move {
                    let _ = tx
                        .send(Ok(crate::llm::StreamDelta {
                            content: Some("chunk".to_string()),
                            tool_calls: vec![],
                        }))
                        .await;
                });
                Ok(rx)
            }

            fn set_prompt_cache_key(&mut self, _key: &str) {}
        }

        let config = CompactionServiceConfig {
            token_limit: 5,
            compaction_model: "kimi-latest".to_string(),
            history_retention_tokens: 2,
        };
        let service = CompactionService::with_config(config);

        let mut messages = vec![
            Message::system("System"),
            Message::user("old user message"),
            Message::assistant("old assistant message"),
            Message::user("recent"),
        ];

        let llm = RecordingMockLlm {
            call_count: AtomicUsize::new(0),
        };

        service.maybe_compact(&mut messages, &llm).await.unwrap();

        assert_eq!(llm.call_count.load(Ordering::SeqCst), 1);
        assert!(messages[1].content.contains("Recorded summary"));
    }
}
