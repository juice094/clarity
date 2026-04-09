//! Fact extraction from text
//!
//! Provides two modes of fact extraction:
//! 1. **LLM-based extraction**: Uses language models to extract structured facts
//! 2. **Rule-based extraction**: Pattern matching for common fact types (no LLM required)
//!
//! ## Rule-based Extraction
//!
//! The rule-based extractor can identify:
//! - User preferences ("I like...", "I prefer...")
//! - Facts about the user ("I am...", "I work at...")
//! - Decisions and choices ("Let's use...", "I choose...")
//! - Code snippets and file paths
//! - Dates and times

use crate::types::{MemoryError, MetaFact, Result};
use async_trait::async_trait;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;
use tracing::{debug, error, info, instrument};

// ============================================================================
// LLM Client Trait
// ============================================================================

/// LLM client trait for fact extraction
#[async_trait]
pub trait LlmClient: Send + Sync {
    /// Send a prompt to the LLM and get the response text
    async fn complete(&self, prompt: &str, model: &str) -> Result<String>;
}

// ============================================================================
// LLM-based Fact Extractor
// ============================================================================

/// Default prompt for fact extraction
const DEFAULT_EXTRACTION_PROMPT: &str = r#"You are a memory extraction system. Analyze the following text and extract factual information about the user, their preferences, relationships, goals, or important context.

For each fact you extract, provide:
1. The fact itself (concise, third-person statement)
2. Relevant tags (array of strings like ["preference", "person", "goal", "tech"])
3. Time reference if mentioned (ISO date or relative time like "2024-01-15" or "tomorrow")

Respond ONLY with a JSON array in this exact format:
[
  {"fact": "User prefers Rust over Python for systems programming", "tags": ["preference", "tech", "rust"], "time": null},
  {"fact": "User has a meeting with Alice tomorrow", "tags": ["schedule", "person"], "time": "tomorrow"}
]

If no facts can be extracted, return an empty array: []

Text to analyze:
"""
{input}
"""

JSON output:"#;

/// Extracts meta-facts from text using an LLM
#[derive(Clone)]
pub struct FactExtractor {
    llm_client: Arc<dyn LlmClient>,
    model: String,
    prompt_template: String,
}

impl fmt::Debug for FactExtractor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FactExtractor")
            .field("model", &self.model)
            .field("prompt_template", &self.prompt_template)
            .field("llm_client", &"<dyn LlmClient>")
            .finish()
    }
}

impl FactExtractor {
    /// Create a new FactExtractor with the default prompt
    pub fn new(llm_client: Arc<dyn LlmClient>, model: impl Into<String>) -> Self {
        Self {
            llm_client,
            model: model.into(),
            prompt_template: DEFAULT_EXTRACTION_PROMPT.to_string(),
        }
    }

    /// Create a new FactExtractor with a custom prompt template
    ///
    /// The template should contain `{input}` placeholder for the text to analyze.
    pub fn with_prompt(
        llm_client: Arc<dyn LlmClient>,
        model: impl Into<String>,
        prompt_template: impl Into<String>,
    ) -> Self {
        Self {
            llm_client,
            model: model.into(),
            prompt_template: prompt_template.into(),
        }
    }

    /// Extract meta-facts from a text summary
    #[instrument(skip(self, summary))]
    pub async fn extract_facts(&self, summary: &str) -> Result<Vec<MetaFact>> {
        if summary.trim().is_empty() {
            return Ok(Vec::new());
        }

        let prompt = self.prompt_template.replace("{input}", summary);

        debug!("Sending extraction prompt to LLM");

        let response = self.llm_client.complete(&prompt, &self.model).await?;

        // Try to extract JSON from the response
        // The LLM might wrap it in markdown code blocks
        let json_str = Self::extract_json(&response);

        match serde_json::from_str::<Vec<ExtractedFact>>(&json_str) {
            Ok(facts) => {
                let meta_facts: Vec<MetaFact> = facts
                    .into_iter()
                    .filter(|f| !f.fact.trim().is_empty())
                    .map(|f| MetaFact {
                        fact: f.fact.trim().to_string(),
                        tags: f.tags,
                        time: f.time.filter(|t| !t.trim().is_empty()),
                    })
                    .collect();

                info!("Extracted {} facts from summary", meta_facts.len());
                Ok(meta_facts)
            }
            Err(e) => {
                error!("Failed to parse extraction response: {}", e);
                error!("Raw response: {}", response);
                Err(MemoryError::Serialization(e))
            }
        }
    }

    /// Extract JSON array from a potentially markdown-wrapped response
    fn extract_json(response: &str) -> String {
        let trimmed = response.trim();

        // Check for markdown code blocks
        if let Some(start) = trimmed.find("```json") {
            if let Some(end) = trimmed[start + 7..].find("```") {
                return trimmed[start + 7..start + 7 + end].trim().to_string();
            }
        }

        if let Some(start) = trimmed.find("```") {
            if let Some(end) = trimmed[start + 3..].find("```") {
                return trimmed[start + 3..start + 3 + end].trim().to_string();
            }
        }

        // Look for array brackets
        if let Some(start) = trimmed.find('[') {
            if let Some(end) = trimmed.rfind(']') {
                if end > start {
                    return trimmed[start..=end].to_string();
                }
            }
        }

        // Return as-is if no markers found
        trimmed.to_string()
    }

    /// Extract facts from a conversation summary and save them
    ///
    /// This is a convenience method that combines extraction with storage.
    pub async fn extract_and_save(
        &self,
        summary: &str,
        store: &crate::store::MemoryStore,
        session_id: Option<&str>,
    ) -> Result<Vec<i64>> {
        let facts = self.extract_facts(summary).await?;
        let mut ids = Vec::new();

        for fact in facts {
            let id = store
                .save_fact(&fact.fact, &fact.tags, fact.time.as_deref(), session_id)
                .await?;
            ids.push(id);
        }

        info!("Saved {} extracted facts to store", ids.len());
        Ok(ids)
    }
}

/// Internal structure for parsing LLM response
#[derive(Debug, Deserialize, Serialize)]
struct ExtractedFact {
    fact: String,
    tags: Vec<String>,
    time: Option<String>,
}

// ============================================================================
// Rule-based Fact Extractor
// ============================================================================

/// Extracts facts using pattern matching without requiring an LLM
///
/// This extractor uses regular expressions to identify common patterns
/// in user messages that indicate facts, preferences, and decisions.
#[derive(Debug, Clone)]
pub struct RuleBasedExtractor {
    patterns: Vec<ExtractionPattern>,
}

/// A pattern for extracting facts
#[derive(Debug, Clone)]
struct ExtractionPattern {
    /// Regex pattern to match
    regex: Regex,
    /// Tags to apply to extracted facts
    tags: Vec<String>,
    /// Template for constructing the fact string
    template: FactTemplate,
}

/// Template for constructing fact strings
#[derive(Debug, Clone)]
enum FactTemplate {
    /// Use the first capture group as the fact
    Capture(usize),
    /// Use a static prefix + capture group
    Prefixed(String, usize),
    /// Custom formatter
    #[allow(dead_code)]
    Custom(fn(&regex::Captures) -> Option<String>),
}

impl RuleBasedExtractor {
    /// Create a new RuleBasedExtractor with default patterns
    pub fn new() -> Self {
        let patterns = vec![
            // User preferences
            ExtractionPattern {
                regex: Regex::new(r"(?i)(?:i|user)\s+(?:really\s+)?(?:like|love|enjoy|prefer)\s+(.+)").unwrap(),
                tags: vec!["preference".to_string()],
                template: FactTemplate::Prefixed("User likes ".to_string(), 1),
            },
            // User dislikes
            ExtractionPattern {
                regex: Regex::new(r"(?i)(?:i|user)\s+(?:dislike|hate|don't like|do not like)\s+(.+)").unwrap(),
                tags: vec!["preference".to_string(), "dislike".to_string()],
                template: FactTemplate::Prefixed("User dislikes ".to_string(), 1),
            },
            // User is/has/works
            ExtractionPattern {
                regex: Regex::new(r"(?i)(?:i|user)\s+(?:am|is)\s+(?:a\s+)?(.+)").unwrap(),
                tags: vec!["identity".to_string()],
                template: FactTemplate::Prefixed("User is ".to_string(), 1),
            },
            // User works at
            ExtractionPattern {
                regex: Regex::new(r"(?i)(?:i|user)\s+(?:work\s+(?:at|for)|am\s+(?:employed\s+by|at))\s+(.+)").unwrap(),
                tags: vec!["work".to_string(), "employment".to_string()],
                template: FactTemplate::Prefixed("User works at ".to_string(), 1),
            },
            // User has (possessions, attributes)
            ExtractionPattern {
                regex: Regex::new(r"(?i)(?:i|user)\s+have\s+(?:a\s+)?(.+)").unwrap(),
                tags: vec!["possession".to_string()],
                template: FactTemplate::Prefixed("User has ".to_string(), 1),
            },
            // Decisions/choices
            ExtractionPattern {
                regex: Regex::new(r"(?i)(?:let's|we should|i will|i'll)\s+(.+)").unwrap(),
                tags: vec!["decision".to_string()],
                template: FactTemplate::Prefixed("Decision: ".to_string(), 1),
            },
            // Goals/plans
            ExtractionPattern {
                regex: Regex::new(r"(?i)(?:i|user)\s+(?:want|plan|need)\s+to\s+(.+)").unwrap(),
                tags: vec!["goal".to_string()],
                template: FactTemplate::Prefixed("User wants to ".to_string(), 1),
            },
            // Important dates
            ExtractionPattern {
                regex: Regex::new(r"(?i)(?:on|at)\s+(\w+day|today|tomorrow|yesterday|\d{1,2}(?:st|nd|rd|th)?\s+(?:of\s+)?\w+|\d{4}-\d{2}-\d{2})").unwrap(),
                tags: vec!["time".to_string()],
                template: FactTemplate::Capture(1),
            },
        ];

        Self { patterns }
    }

    /// Add a custom pattern
    pub fn with_pattern(
        mut self,
        regex: Regex,
        tags: Vec<String>,
        template: impl Into<String>,
    ) -> Self {
        self.patterns.push(ExtractionPattern {
            regex,
            tags,
            template: FactTemplate::Prefixed(template.into(), 1),
        });
        self
    }

    /// Extract facts from text using pattern matching
    pub fn extract(&self, text: &str) -> Vec<MetaFact> {
        let mut facts = Vec::new();
        let lines: Vec<&str> = text.lines().collect();

        for line in &lines {
            for pattern in &self.patterns {
                if let Some(captures) = pattern.regex.captures(line) {
                    if let Some(fact_text) = self.apply_template(&pattern.template, &captures) {
                        let fact = MetaFact {
                            fact: fact_text,
                            tags: pattern.tags.clone(),
                            time: self.extract_time(line),
                        };
                        facts.push(fact);
                    }
                }
            }
        }

        // Also extract code snippets and file paths
        facts.extend(self.extract_code_snippets(text));
        facts.extend(self.extract_file_paths(text));

        facts
    }

    /// Apply a template to regex captures
    fn apply_template(&self, template: &FactTemplate, captures: &regex::Captures) -> Option<String> {
        match template {
            FactTemplate::Capture(group) => captures.get(*group).map(|m| m.as_str().trim().to_string()),
            FactTemplate::Prefixed(prefix, group) => {
                captures.get(*group).map(|m| format!("{}{}", prefix, m.as_str().trim()))
            }
            FactTemplate::Custom(f) => f(captures),
        }
    }

    /// Try to extract time references
    fn extract_time(&self, text: &str) -> Option<String> {
        // Simple patterns for time extraction
        let time_patterns = [
            (Regex::new(r"\b(today|tomorrow|yesterday)\b").unwrap(), 1),
            (Regex::new(r"\b(\d{4}-\d{2}-\d{2})\b").unwrap(), 1),
            (Regex::new(r"\b(\w+day)\b").unwrap(), 1),
        ];

        for (pattern, group) in &time_patterns {
            if let Some(captures) = pattern.captures(text) {
                return captures.get(*group).map(|m| m.as_str().to_string());
            }
        }

        None
    }

    /// Extract code snippets
    fn extract_code_snippets(&self, text: &str) -> Vec<MetaFact> {
        let mut facts = Vec::new();

        // Match code blocks
        let code_block_regex = Regex::new(r"```(\w+)?\n(.*?)```").unwrap();
        for captures in code_block_regex.captures_iter(text) {
            let language = captures.get(1).map(|m| m.as_str()).unwrap_or("code");
            let code = captures.get(2).map(|m| m.as_str()).unwrap_or("");

            // Extract first line or summarize
            let summary = code.lines().next().unwrap_or("code snippet");
            let truncated = if summary.len() > 50 {
                format!("{}...", &summary[..50])
            } else {
                summary.to_string()
            };

            facts.push(MetaFact {
                fact: format!("Code snippet ({}): {}", language, truncated),
                tags: vec!["code".to_string(), language.to_string()],
                time: None,
            });
        }

        // Match inline code
        let inline_code_regex = Regex::new(r"`([^`]+)`").unwrap();
        let mut inline_codes = Vec::new();
        for captures in inline_code_regex.captures_iter(text) {
            if let Some(code) = captures.get(1) {
                let code_str = code.as_str();
                if code_str.len() > 10 && !inline_codes.contains(&code_str.to_string()) {
                    inline_codes.push(code_str.to_string());
                    if inline_codes.len() >= 3 {
                        // Limit to avoid spam
                        break;
                    }
                }
            }
        }

        if !inline_codes.is_empty() {
            facts.push(MetaFact {
                fact: format!("Inline code references: {}", inline_codes.join(", ")),
                tags: vec!["code".to_string()],
                time: None,
            });
        }

        facts
    }

    /// Extract file paths
    fn extract_file_paths(&self, text: &str) -> Vec<MetaFact> {
        let mut facts = Vec::new();
        let mut paths = Vec::new();

        // Match file paths
        let path_regex = Regex::new(
            r"(?:[\w-]+/)+[\w-]+\.(rs|py|js|ts|json|toml|yaml|yml|md|txt|go|java|cpp|c|h|hpp)"
        ).unwrap();

        for captures in path_regex.captures_iter(text) {
            if let Some(path) = captures.get(0) {
                let path_str = path.as_str();
                if !paths.contains(&path_str.to_string()) {
                    paths.push(path_str.to_string());
                }
            }
        }

        if !paths.is_empty() {
            facts.push(MetaFact {
                fact: format!("Referenced files: {}", paths.join(", ")),
                tags: vec!["file".to_string(), "reference".to_string()],
                time: None,
            });
        }

        facts
    }

    /// Extract conversation summary
    pub fn summarize(&self, messages: &[(String, String)]) -> String {
        let mut summary_parts = Vec::new();

        for (role, content) in messages {
            // Extract key points from each message
            let facts = self.extract(content);
            for fact in facts {
                summary_parts.push(format!("[{}] {}", role, fact.fact));
            }
        }

        summary_parts.join("\n")
    }
}

impl Default for RuleBasedExtractor {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Mock LLM Client for Testing
// ============================================================================

/// A mock LLM client for testing
#[derive(Debug, Default)]
pub struct MockLlmClient {
    response: Option<String>,
}

impl MockLlmClient {
    pub fn new(response: impl Into<String>) -> Self {
        Self {
            response: Some(response.into()),
        }
    }

    pub fn with_facts(facts: Vec<MetaFact>) -> Self {
        let extracted: Vec<ExtractedFact> = facts
            .into_iter()
            .map(|f| ExtractedFact {
                fact: f.fact,
                tags: f.tags,
                time: f.time,
            })
            .collect();

        Self {
            response: Some(serde_json::to_string(&extracted).unwrap()),
        }
    }
}

#[async_trait]
impl LlmClient for MockLlmClient {
    async fn complete(&self, _prompt: &str, _model: &str) -> Result<String> {
        self.response.clone().ok_or_else(|| {
            MemoryError::LlmClient("No response configured".to_string())
        })
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_extract_facts() {
        let json_response = r#"[
            {"fact": "User likes Rust programming", "tags": ["preference", "tech"], "time": null},
            {"fact": "User has a dog named Max", "tags": ["pet", "personal"], "time": "2024-01-15"}
        ]"#;

        let client = Arc::new(MockLlmClient::new(json_response));
        let extractor = FactExtractor::new(client, "gpt-4");

        let facts = extractor
            .extract_facts("Some conversation summary")
            .await
            .unwrap();

        assert_eq!(facts.len(), 2);
        assert_eq!(facts[0].fact, "User likes Rust programming");
        assert_eq!(facts[0].tags, vec!["preference", "tech"]);
        assert_eq!(facts[0].time, None);

        assert_eq!(facts[1].fact, "User has a dog named Max");
        assert_eq!(facts[1].time, Some("2024-01-15".to_string()));
    }

    #[tokio::test]
    async fn test_extract_from_markdown() {
        let markdown_response = r#"Here are the facts:

```json
[
    {"fact": "User enjoys hiking", "tags": ["hobby"], "time": null}
]
```

Hope that helps!"#;

        let client = Arc::new(MockLlmClient::new(markdown_response));
        let extractor = FactExtractor::new(client, "gpt-4");

        let facts = extractor.extract_facts("test").await.unwrap();

        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].fact, "User enjoys hiking");
    }

    #[tokio::test]
    async fn test_extract_empty_response() {
        let client = Arc::new(MockLlmClient::new("[]"));
        let extractor = FactExtractor::new(client, "gpt-4");

        let facts = extractor.extract_facts("test").await.unwrap();
        assert!(facts.is_empty());
    }

    #[tokio::test]
    async fn test_extract_empty_input() {
        let client = Arc::new(MockLlmClient::new("should not be called"));
        let extractor = FactExtractor::new(client, "gpt-4");

        let facts = extractor.extract_facts("").await.unwrap();
        assert!(facts.is_empty());

        let facts = extractor.extract_facts("   ").await.unwrap();
        assert!(facts.is_empty());
    }

    #[tokio::test]
    async fn test_extract_with_mock_facts() {
        let meta_facts = vec![
            MetaFact {
                fact: "User is a software engineer".to_string(),
                tags: vec!["profession".to_string()],
                time: None,
            },
            MetaFact {
                fact: "User lives in Tokyo".to_string(),
                tags: vec!["location".to_string()],
                time: Some("2024".to_string()),
            },
        ];

        let client = Arc::new(MockLlmClient::with_facts(meta_facts));
        let extractor = FactExtractor::new(client, "gpt-4");

        let facts = extractor.extract_facts("test").await.unwrap();

        assert_eq!(facts.len(), 2);
        assert_eq!(facts[0].fact, "User is a software engineer");
        assert_eq!(facts[1].fact, "User lives in Tokyo");
    }

    #[test]
    fn test_extract_json_various_formats() {
        // Plain JSON
        let plain = r#"[{"fact": "test"}]"#;
        assert_eq!(FactExtractor::extract_json(plain), plain);

        // Markdown wrapped
        let markdown = "```json\n[{\"fact\": \"test\"}]\n```";
        assert_eq!(
            FactExtractor::extract_json(markdown),
            "[{\"fact\": \"test\"}]"
        );

        // With extra text
        let extra = "Here is the result:\n[{\"fact\": \"test\"}]\nDone!";
        assert_eq!(FactExtractor::extract_json(extra), "[{\"fact\": \"test\"}]");

        // With markdown and extra
        let complex = "Result:\n```\n[{\"fact\": \"test\"}]\n```\nThat's it!";
        assert_eq!(FactExtractor::extract_json(complex), "[{\"fact\": \"test\"}]");
    }

    // Rule-based extractor tests
    #[test]
    fn test_rule_based_preferences() {
        let extractor = RuleBasedExtractor::new();

        let text = "I really like Rust programming and I enjoy hiking on weekends";
        let facts = extractor.extract(text);

        assert!(!facts.is_empty());
        assert!(facts.iter().any(|f| f.fact.contains("likes Rust")));
        assert!(facts.iter().any(|f| f.tags.contains(&"preference".to_string())));
    }

    #[test]
    fn test_rule_based_identity() {
        let extractor = RuleBasedExtractor::new();

        let text = "I am a software engineer and I work at Google";
        let facts = extractor.extract(text);

        assert!(facts.iter().any(|f| f.fact.contains("software engineer")));
        assert!(facts.iter().any(|f| f.fact.contains("works at Google")));
    }

    #[test]
    fn test_rule_based_goals() {
        let extractor = RuleBasedExtractor::new();

        let text = "I want to learn machine learning and I plan to build an app";
        let facts = extractor.extract(text);

        assert!(facts.iter().any(|f| f.fact.contains("wants to")));
    }

    #[test]
    fn test_rule_based_code_snippets() {
        let extractor = RuleBasedExtractor::new();

        let text = r#"
Here's some code:
```rust
fn main() {
    println!("Hello, world!");
}
```
You can also use `Vec::new()` to create a vector.
        "#;

        let facts = extractor.extract(text);

        assert!(facts.iter().any(|f| f.tags.contains(&"code".to_string())));
    }

    #[test]
    fn test_rule_based_file_paths() {
        let extractor = RuleBasedExtractor::new();

        let text = "Check the file at src/main.rs and also look at Cargo.toml";
        let facts = extractor.extract(text);

        assert!(facts.iter().any(|f| f.tags.contains(&"file".to_string())));
    }

    #[test]
    fn test_rule_based_summarize() {
        let extractor = RuleBasedExtractor::new();

        let messages = vec![
            ("user".to_string(), "I like Rust programming".to_string()),
            ("assistant".to_string(), "That's great!".to_string()),
            ("user".to_string(), "I am a software engineer".to_string()),
        ];

        let summary = extractor.summarize(&messages);
        assert!(!summary.is_empty());
        assert!(summary.contains("likes Rust"));
        assert!(summary.contains("software engineer"));
    }

    #[test]
    fn test_rule_based_empty() {
        let extractor = RuleBasedExtractor::new();

        let facts = extractor.extract("Hello, how are you?");
        assert!(facts.is_empty());
    }

    #[test]
    fn test_rule_based_time_extraction() {
        let extractor = RuleBasedExtractor::new();

        let text = "I have a meeting tomorrow and another one on Monday";
        let facts = extractor.extract(text);

        // Should extract some facts with time references
        assert!(!facts.is_empty());
    }
}
