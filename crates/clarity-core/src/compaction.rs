//! Context compaction module for managing conversation history size
//!
//! This module provides functionality to compact conversation history when
//! the context window approaches its limit, helping to maintain efficient
//! token usage while preserving important context.
//!
//! # Example
//!
//! ```rust,no_run
//! use clarity_core::compaction::{CompactionConfig, SimpleCompaction, estimate_text_tokens};
//! use clarity_llm::api::{Message, MessageRole};
//!
//! // Estimate token count for text
//! let tokens = estimate_text_tokens("Hello, world!");
//!
//! // Check if compaction is needed
//! let config = CompactionConfig::default();
//! let should_compact = config.should_compact(7000, 8192);
//! ```

use crate::error::AgentError;
use clarity_llm::api::{LlmProvider, Message, MessageRole};
use async_trait::async_trait;
use std::sync::OnceLock;

/// Default trigger ratio for compaction (80% of max tokens)
pub const DEFAULT_TRIGGER_RATIO: f64 = 0.8;

/// Default reserved tokens (buffer for new messages)
pub const DEFAULT_RESERVED_TOKENS: usize = 2000;

/// Default number of messages to preserve during compaction
pub const DEFAULT_MAX_PRESERVE_MESSAGES: usize = 2;

/// Compaction prompt template
const COMPACTION_PROMPT: &str = r#"Please summarize and compact the above conversation messages into a concise summary that captures:

1. Key decisions made and their rationale
2. Important context established during the conversation
3. Current state of any ongoing tasks or discussions
4. Any critical information needed to continue effectively

The summary should be detailed enough to maintain context continuity but concise to save tokens. Focus on preserving actionable information and technical details over conversational pleasantries."#;

/// System prompt for compaction
const COMPACTION_SYSTEM_PROMPT: &str = "You are a helpful assistant that compacts conversation context. Your task is to create a concise but comprehensive summary of the provided conversation history.";

/// Lazily-initialized tiktoken tokenizer (cl100k_base — used by GPT-4, GPT-3.5, GPT-4o).
static TOKENIZER: OnceLock<Option<tiktoken_rs::CoreBPE>> = OnceLock::new();

fn get_tokenizer() -> Option<&'static tiktoken_rs::CoreBPE> {
    TOKENIZER
        .get_or_init(|| tiktoken_rs::cl100k_base().ok())
        .as_ref()
}

/// Estimate the number of tokens in a text string.
///
/// Uses `tiktoken-rs` (`cl100k_base`) for exact token counting when available.
/// Falls back to a weighted byte heuristic if the tokenizer fails to initialize.
///
/// Weighted heuristic (fallback):
/// - ASCII text: ~4 bytes per token
/// - Non-ASCII text (CJK, emoji, etc.): ~2 bytes per token in UTF-8
///
/// # Arguments
///
/// * `text` - The text to estimate tokens for
///
/// # Returns
///
/// Estimated token count
///
/// # Examples
///
/// ```
/// use clarity_core::compaction::estimate_text_tokens;
///
/// let tokens = estimate_text_tokens("Hello, world!");
/// // Exact count via tiktoken (cl100k_base): 4 tokens
/// assert!(tokens >= 3);
///
/// let tokens = estimate_text_tokens("你好世界");
/// // Exact count via tiktoken: typically 4-6 tokens for CJK
/// assert!(tokens >= 4);
/// ```
pub fn estimate_text_tokens(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    // Prefer exact tokenizer when available
    if let Some(bpe) = get_tokenizer() {
        return bpe.encode_with_special_tokens(text).len();
    }
    // Fallback to weighted heuristic
    let ascii_bytes = text.bytes().filter(|b| b.is_ascii()).count();
    let non_ascii_bytes = text.len() - ascii_bytes;
    ascii_bytes.div_ceil(4) + non_ascii_bytes.div_ceil(2)
}

/// Estimate tokens for a slice of messages
///
/// # Arguments
///
/// * `messages` - The messages to estimate tokens for
///
/// # Returns
///
/// Estimated total token count for all messages
pub fn estimate_message_tokens(messages: &[Message]) -> usize {
    messages
        .iter()
        .map(|m| estimate_text_tokens(&m.content))
        .sum()
}

/// Configuration for context compaction behavior
///
/// This struct defines when and how compaction should be triggered,
/// as well as how many messages to preserve.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompactionConfig {
    /// Ratio of max tokens that triggers compaction (default: 0.8)
    pub trigger_ratio: f64,
    /// Reserved tokens buffer for new messages (default: 2000)
    pub reserved_tokens: usize,
    /// Maximum number of recent user/assistant messages to preserve (default: 2)
    pub max_preserve_messages: usize,
}

impl Default for CompactionConfig {
    fn default() -> Self {
        Self {
            trigger_ratio: DEFAULT_TRIGGER_RATIO,
            reserved_tokens: DEFAULT_RESERVED_TOKENS,
            max_preserve_messages: DEFAULT_MAX_PRESERVE_MESSAGES,
        }
    }
}

impl CompactionConfig {
    /// Create a new compaction config with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the trigger ratio
    pub fn with_trigger_ratio(mut self, ratio: f64) -> Self {
        self.trigger_ratio = ratio.clamp(0.0, 1.0);
        self
    }

    /// Set the reserved tokens
    pub fn with_reserved_tokens(mut self, tokens: usize) -> Self {
        self.reserved_tokens = tokens;
        self
    }

    /// Set the max preserve messages
    pub fn with_max_preserve_messages(mut self, count: usize) -> Self {
        self.max_preserve_messages = count;
        self
    }

    /// Determine whether auto-compaction should be triggered.
    ///
    /// Returns true when either condition is met (whichever fires first):
    /// - Ratio-based: `current_tokens >= max_tokens * trigger_ratio`
    /// - Reserved-based: `current_tokens + reserved_tokens >= max_tokens`
    ///
    /// # Arguments
    ///
    /// * `current_tokens` - Current token count
    /// * `max_tokens` - Maximum allowed tokens
    ///
    /// # Returns
    ///
    /// `true` if compaction should be triggered, `false` otherwise
    ///
    /// # Examples
    ///
    /// ```
    /// use clarity_core::compaction::CompactionConfig;
    ///
    /// let config = CompactionConfig::default();
    ///
    /// // Trigger by ratio (8000 >= 10000 * 0.8)
    /// assert!(config.should_compact(8000, 10000));
    ///
    /// // Trigger by reserved (8500 + 2000 >= 10000)
    /// assert!(config.should_compact(8500, 10000));
    ///
    /// // No trigger
    /// assert!(!config.should_compact(5000, 10000));
    /// ```
    pub fn should_compact(&self, current_tokens: usize, max_tokens: usize) -> bool {
        if max_tokens == 0 {
            return false;
        }

        let trigger_threshold = (max_tokens as f64 * self.trigger_ratio) as usize;

        // Dual condition trigger:
        // 1. Ratio-based: current >= max * trigger_ratio
        // 2. Reserved-based: current + reserved >= max
        current_tokens >= trigger_threshold
            || current_tokens.saturating_add(self.reserved_tokens) >= max_tokens
    }
}

/// Result of a compaction operation
#[derive(Debug, Clone)]
pub struct CompactionResult {
    /// The compacted messages (summary + preserved)
    pub messages: Vec<Message>,
    /// Whether compaction was actually performed
    pub was_compacted: bool,
    /// Estimated token count after compaction
    pub estimated_tokens: usize,
}

/// Split messages for compaction into (to_compact, to_preserve)
///
/// This function separates messages into two groups:
/// - `to_compact`: Messages that will be summarized
/// - `to_preserve`: Recent user/assistant messages to keep as-is
///
/// The function counts from the end backwards to preserve the most recent
/// `max_preserve` user/assistant message pairs.
///
/// # Arguments
///
/// * `messages` - The full message history
/// * `max_preserve` - Maximum number of user/assistant messages to preserve
///
/// # Returns
///
/// A tuple of (messages_to_compact, messages_to_preserve)
///
/// # Examples
///
/// ```
/// use clarity_core::compaction::split_messages_for_compaction;
/// use clarity_core::agent::{Message, MessageRole};
///
/// let messages = vec![
///     Message::user("Hello"),
///     Message::assistant("Hi there!"),
///     Message::user("How are you?"),
///     Message::assistant("I'm doing well!"),
/// ];
///
/// let (to_compact, to_preserve) = split_messages_for_compaction(&messages, 2);
/// // to_compact will contain first 2 messages
/// // to_preserve will contain last 2 messages
/// ```
pub fn split_messages_for_compaction(
    messages: &[Message],
    max_preserve: usize,
) -> (Vec<Message>, Vec<Message>) {
    if messages.is_empty() || max_preserve == 0 {
        return (messages.to_vec(), Vec::new());
    }

    // Count backwards from the end to find preservation point
    let mut preserve_start_index = messages.len();
    let mut preserved_count = 0;

    for (index, msg) in messages.iter().enumerate().rev() {
        if matches!(msg.role, MessageRole::User | MessageRole::Assistant) {
            preserved_count += 1;
            if preserved_count == max_preserve {
                preserve_start_index = index;
                break;
            }
        }
    }

    // If we didn't find enough messages to preserve, don't compact anything
    if preserved_count < max_preserve {
        return (messages.to_vec(), Vec::new());
    }

    let to_compact = messages[..preserve_start_index].to_vec();
    let to_preserve = messages[preserve_start_index..].to_vec();

    // If nothing to compact, return all as preserved
    if to_compact.is_empty() {
        return (Vec::new(), messages.to_vec());
    }

    (to_compact, to_preserve)
}

/// Trait for compaction strategies
#[async_trait]
pub trait Compaction: Send + Sync {
    /// Compact a sequence of messages into a new sequence
    ///
    /// # Arguments
    ///
    /// * `messages` - The messages to compact
    /// * `llm` - The LLM provider to use for generating summaries
    ///
    /// # Returns
    ///
    /// A result containing the compacted messages
    async fn compact(
        &self,
        messages: &[Message],
        llm: &dyn LlmProvider,
    ) -> Result<CompactionResult, AgentError>;
}

/// Simple compaction implementation
///
/// This implementation:
/// 1. Splits messages into (to_compact, to_preserve)
/// 2. Sends to_compact messages to LLM for summarization
/// 3. Returns a summary message + preserved messages
#[derive(Debug, Clone)]
pub struct SimpleCompaction {
    /// Maximum number of messages to preserve
    max_preserved_messages: usize,
    /// Custom compaction instruction (optional)
    custom_instruction: Option<String>,
}

impl Default for SimpleCompaction {
    fn default() -> Self {
        Self {
            max_preserved_messages: DEFAULT_MAX_PRESERVE_MESSAGES,
            custom_instruction: None,
        }
    }
}

impl SimpleCompaction {
    /// Create a new SimpleCompaction with default settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with custom max preserved messages
    pub fn with_max_preserved(max: usize) -> Self {
        Self {
            max_preserved_messages: max,
            custom_instruction: None,
        }
    }

    /// Set custom instruction for compaction
    pub fn with_custom_instruction(mut self, instruction: impl Into<String>) -> Self {
        self.custom_instruction = Some(instruction.into());
        self
    }

    /// Prepare messages for compaction
    ///
    /// Returns (compact_message, to_preserve) where:
    /// - compact_message: A user message containing all messages to be compacted
    /// - to_preserve: The recent messages that should be kept as-is
    fn prepare(&self, messages: &[Message]) -> (Option<Message>, Vec<Message>) {
        if messages.is_empty() || self.max_preserved_messages == 0 {
            return (None, messages.to_vec());
        }

        let (to_compact, to_preserve) =
            split_messages_for_compaction(messages, self.max_preserved_messages);

        // If nothing to compact or not enough to preserve, skip compaction
        if to_compact.is_empty() {
            return (None, to_preserve);
        }

        // Create the compaction input message
        let mut compact_content = String::new();
        compact_content.push_str("Please summarize the following conversation history:\n\n");

        for (i, msg) in to_compact.iter().enumerate() {
            let role_label = match msg.role {
                MessageRole::System => "System",
                MessageRole::User => "User",
                MessageRole::Assistant => "Assistant",
                MessageRole::Tool => "Tool",
            };
            compact_content.push_str(&format!(
                "## Message {}\nRole: {}\nContent:\n{}\n\n",
                i + 1,
                role_label,
                msg.content
            ));
        }

        // Add the main compaction prompt
        compact_content.push_str(COMPACTION_PROMPT);

        // Add custom instruction if provided
        if let Some(ref custom) = self.custom_instruction {
            compact_content.push_str(&format!(
                "\n\n**User's Custom Compaction Instruction:**\n\
                 The user has specifically requested the following focus during compaction. \
                 You MUST prioritize this instruction above the default compression priorities:\n{}",
                custom
            ));
        }

        let compact_message = Message::user(compact_content);
        (Some(compact_message), to_preserve)
    }
}

#[async_trait]
impl Compaction for SimpleCompaction {
    async fn compact(
        &self,
        messages: &[Message],
        llm: &dyn LlmProvider,
    ) -> Result<CompactionResult, AgentError> {
        let (compact_message, to_preserve) = self.prepare(messages);

        // If no compaction needed, return original messages
        let compact_msg = match compact_message {
            Some(msg) => msg,
            None => {
                let estimated = estimate_message_tokens(&to_preserve);
                return Ok(CompactionResult {
                    messages: to_preserve,
                    was_compacted: false,
                    estimated_tokens: estimated,
                });
            }
        };

        // Create a system message for the compaction context
        let system_message = Message::system(COMPACTION_SYSTEM_PROMPT);

        // Call LLM to generate summary
        let compaction_messages = vec![system_message, compact_msg];
        let tools = serde_json::json!({ "functions": [] });

        let response = llm.complete(&compaction_messages, &tools).await?;

        // Create the summary message
        let summary_content = format!(
            "Previous context has been compacted. Here is the compaction output:\n\n{}",
            response.content
        );
        let summary_message = Message::user(summary_content);

        // Build final message list: summary + preserved
        let mut final_messages = vec![summary_message];
        final_messages.extend(to_preserve);

        let estimated = estimate_message_tokens(&final_messages);

        Ok(CompactionResult {
            messages: final_messages,
            was_compacted: true,
            estimated_tokens: estimated,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_tokens() {
        // Empty string
        assert_eq!(estimate_text_tokens(""), 0);

        // Short English text — tiktoken (cl100k_base) exact counts
        assert_eq!(estimate_text_tokens("Hello"), 1);
        assert_eq!(estimate_text_tokens("Hello, world!"), 4);

        // Longer text
        let long_text = "This is a longer piece of text that should give us more tokens.";
        assert_eq!(estimate_text_tokens(long_text), 14);

        // CJK text — each character typically 1-2 tokens in cl100k
        let cjk_text = "你好世界";
        let cjk_tokens = estimate_text_tokens(cjk_text);
        assert!((4..=8).contains(&cjk_tokens), "CJK tokens = {}", cjk_tokens);

        // Verify tiktoken is active: exact counts should be lower than old heuristic
        assert!(estimate_text_tokens("Hello") < 2); // heuristic would give 2
    }

    #[test]
    fn test_should_compact_trigger_ratio() {
        let config = CompactionConfig::default();

        // Exactly at trigger threshold (0.8 * 10000 = 8000)
        assert!(config.should_compact(8000, 10000));

        // Above trigger threshold
        assert!(config.should_compact(8500, 10000));

        // Below trigger threshold
        assert!(!config.should_compact(7000, 10000));

        // Custom trigger ratio
        let strict_config = CompactionConfig::new().with_trigger_ratio(0.5);
        assert!(strict_config.should_compact(5000, 10000));
        assert!(!strict_config.should_compact(4000, 10000));
    }

    #[test]
    fn test_should_compact_reserved() {
        let config = CompactionConfig::default();

        // Trigger by reserved: 8500 + 2000 >= 10000
        assert!(config.should_compact(8500, 10000));

        // Just below reserved threshold
        assert!(!config.should_compact(7990, 10000));

        // Custom reserved tokens
        let high_reserved = CompactionConfig::new().with_reserved_tokens(5000);
        // 6000 + 5000 >= 10000
        assert!(high_reserved.should_compact(6000, 10000));
        // 4000 + 5000 < 10000
        assert!(!high_reserved.should_compact(4000, 10000));
    }

    #[test]
    fn test_split_messages() {
        // Create test messages
        let messages = vec![
            Message::system("You are a helpful assistant"),
            Message::user("Hello"),
            Message::assistant("Hi there!"),
            Message::user("How are you?"),
            Message::assistant("I'm doing well!"),
            Message::user("What's the weather?"),
            Message::assistant("It's sunny today!"),
        ];

        // Preserve last 2 user/assistant pairs (4 messages)
        let (to_compact, to_preserve) = split_messages_for_compaction(&messages, 4);

        // System message + first 2 user/assistant exchanges = 3 messages to compact
        assert_eq!(to_compact.len(), 3);
        assert_eq!(to_compact[0].role, MessageRole::System);

        // Last 2 user/assistant exchanges = 4 messages to preserve
        assert_eq!(to_preserve.len(), 4);
        assert_eq!(to_preserve[0].role, MessageRole::User);
        assert_eq!(to_preserve[0].content, "How are you?");

        // Test edge case: max_preserve = 0
        let (to_compact, to_preserve) = split_messages_for_compaction(&messages, 0);
        assert_eq!(to_compact.len(), 7);
        assert!(to_preserve.is_empty());

        // Test edge case: not enough messages
        let few_messages = vec![Message::user("Hello"), Message::assistant("Hi!")];
        let (to_compact, to_preserve) = split_messages_for_compaction(&few_messages, 4);
        // Should return all as compact since not enough to preserve
        assert_eq!(to_compact.len(), 2);
        assert!(to_preserve.is_empty());

        // Test empty messages
        let empty: Vec<Message> = vec![];
        let (to_compact, to_preserve) = split_messages_for_compaction(&empty, 2);
        assert!(to_compact.is_empty());
        assert!(to_preserve.is_empty());
    }

    #[test]
    fn test_split_messages_with_tools() {
        // Messages with tool calls
        let messages = vec![
            Message::user("Read a file"),
            Message {
                role: MessageRole::Assistant,
                content: "I'll read that file".to_string(),
                tool_calls: Some(vec![]),
                tool_call_id: None,
            },
            Message::tool("call_1", "File contents here"),
            Message::user("Thanks!"),
            Message::assistant("You're welcome!"),
        ];

        // Tool messages should not be counted for preservation
        let (to_compact, to_preserve) = split_messages_for_compaction(&messages, 2);

        // Should preserve "Thanks!" and "You're welcome!"
        assert_eq!(to_preserve.len(), 2);
        assert_eq!(to_preserve[0].content, "Thanks!");
        assert_eq!(to_preserve[1].content, "You're welcome!");

        // Everything else should be compacted
        assert_eq!(to_compact.len(), 3);
    }

    #[test]
    fn test_compaction_config_builder() {
        let config = CompactionConfig::new()
            .with_trigger_ratio(0.9)
            .with_reserved_tokens(1000)
            .with_max_preserve_messages(4);

        assert_eq!(config.trigger_ratio, 0.9);
        assert_eq!(config.reserved_tokens, 1000);
        assert_eq!(config.max_preserve_messages, 4);

        // Test ratio clamping
        let clamped = CompactionConfig::new().with_trigger_ratio(1.5);
        assert_eq!(clamped.trigger_ratio, 1.0);

        let clamped_neg = CompactionConfig::new().with_trigger_ratio(-0.5);
        assert_eq!(clamped_neg.trigger_ratio, 0.0);
    }

    #[test]
    fn test_estimate_message_tokens() {
        let messages = vec![
            Message::user("Hello"),          // 5 chars -> 2 tokens
            Message::assistant("Hi there!"), // 10 chars -> 3 tokens
            Message::user("How are you?"),   // 12 chars -> 3 tokens
        ];

        let total = estimate_message_tokens(&messages);
        assert_eq!(total, 8); // 2 + 3 + 3
    }

    #[test]
    fn test_simple_compaction_prepare() {
        let compaction = SimpleCompaction::new();

        let messages = vec![
            Message::system("System prompt"),
            Message::user("Hello"),
            Message::assistant("Hi!"),
            Message::user("How are you?"),
            Message::assistant("Good!"),
        ];

        let (compact_msg, to_preserve) = compaction.prepare(&messages);

        assert!(compact_msg.is_some());
        assert_eq!(to_preserve.len(), 2); // Last user/assistant pair

        let compact = compact_msg.unwrap();
        assert!(compact.content.contains("Please summarize"));
        assert!(compact.content.contains("Message 1"));
        assert!(compact.content.contains("System"));
        assert!(compact.content.contains("User"));
    }

    #[test]
    fn test_simple_compaction_prepare_no_compaction_needed() {
        let compaction = SimpleCompaction::new();

        // Too few messages
        let few_messages = vec![Message::user("Hello"), Message::assistant("Hi!")];

        let (compact_msg, to_preserve) = compaction.prepare(&few_messages);

        // Should return None for compact_msg since we can't preserve 2 and compact anything
        assert!(compact_msg.is_none());
        assert_eq!(to_preserve.len(), 2);
    }

    #[test]
    fn test_simple_compaction_with_custom_instruction() {
        let compaction = SimpleCompaction::with_max_preserved(2)
            .with_custom_instruction("Focus on technical details");

        let messages = vec![
            Message::user("Hello"),
            Message::assistant("Hi!"),
            Message::user("How are you?"),
            Message::assistant("Good!"),
        ];

        let (compact_msg, _) = compaction.prepare(&messages);
        assert!(compact_msg.is_some());
        assert!(compact_msg
            .unwrap()
            .content
            .contains("Focus on technical details"));
    }

    #[test]
    fn test_compaction_result() {
        let messages = vec![Message::user("Test")];
        let result = CompactionResult {
            messages: messages.clone(),
            was_compacted: true,
            estimated_tokens: 10,
        };

        assert!(result.was_compacted);
        assert_eq!(result.estimated_tokens, 10);
        assert_eq!(result.messages.len(), 1);
    }
}
