//! LLM-based meta-fact extraction

use crate::types::{MemoryError, MetaFact, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;
use tracing::{debug, error, info, instrument};

/// LLM client trait for fact extraction
#[async_trait]
pub trait LlmClient: Send + Sync {
    /// Send a prompt to the LLM and get the response text
    async fn complete(&self, prompt: &str, model: &str) -> Result<String>;
}

/// Default prompt for fact extraction
const DEFAULT_EXTRACTION_PROMPT: &str = r#"You are a memory extraction system. Analyze the following text and extract factual information about the user, their preferences, relationships, goals, or important context.

For each fact you extract, provide:
1. The fact itself (concise, third-person statement)
2. Relevant tags (array of strings like ["preference", "person", "goal", "tech"])
3. Time reference if mentioned (ISO date or relative time like "2024-01-15" or "yesterday")

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
                let meta_facts: Vec<MetaFact> = facts.into_iter()
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
            let id = store.save_fact(&fact.fact, &fact.tags, fact.time.as_deref(), session_id)?;
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
        let extracted: Vec<ExtractedFact> = facts.into_iter()
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

        let facts = extractor.extract_facts("Some conversation summary").await.unwrap();
        
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
        assert_eq!(FactExtractor::extract_json(markdown), "[{\"fact\": \"test\"}]");

        // With extra text
        let extra = "Here is the result:\n[{\"fact\": \"test\"}]\nDone!";
        assert_eq!(FactExtractor::extract_json(extra), "[{\"fact\": \"test\"}]");

        // With markdown and extra
        let complex = "Result:\n```\n[{\"fact\": \"test\"}]\n```\nThat's it!";
        assert_eq!(FactExtractor::extract_json(complex), "[{\"fact\": \"test\"}]");
    }
}
