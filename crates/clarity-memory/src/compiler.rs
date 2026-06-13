//! Four-level memory compilation pipeline (OpenHanako-style) with enhancements
//!
//! This module provides:
//! - Four-level memory compilation (today, week, long-term, facts)
//! - Memory deduplication and merging
//! - Hierarchical memory organization
//! - Forgetting mechanism based on importance and time

use crate::extractor::{FactExtractor, LlmClient, RuleBasedExtractor};
use crate::session_store::SessionStore;
use crate::store::MemoryStore;
use crate::types::{CompileConfig, CompileStatus, Fact, Message, Result};
use chrono::{Duration, Utc};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, error, info, instrument, warn};

/// Importance score for facts (0.0 - 1.0)
pub type ImportanceScore = f32;

/// Enhanced four-level memory compiler with deduplication and forgetting
pub struct MemoryCompiler {
    store: MemoryStore,
    session_store: SessionStore,
    llm_client: Arc<dyn LlmClient>,
    config: CompileConfig,
    fingerprints: HashMap<String, String>,
    rule_extractor: RuleBasedExtractor,
}

/// Memory merge configuration
#[derive(Debug, Clone)]
pub struct MergeConfig {
    /// Cosine similarity threshold above which memories are merged
    pub similarity_threshold: f32,
    /// Minimum importance score for a memory to be promoted to long-term
    pub min_long_term_importance: f32,
    /// Number of days after which low-importance memories may be forgotten
    pub forget_after_days: i64,
}

impl Default for MergeConfig {
    fn default() -> Self {
        Self {
            similarity_threshold: 0.85,
            min_long_term_importance: 0.3,
            forget_after_days: 90,
        }
    }
}

impl MemoryCompiler {
    /// Create a new MemoryCompiler
    pub fn new(
        store: MemoryStore,
        session_store: SessionStore,
        llm_client: Arc<dyn LlmClient>,
        config: CompileConfig,
    ) -> Self {
        Self {
            store,
            session_store,
            llm_client,
            config,
            fingerprints: HashMap::new(),
            rule_extractor: RuleBasedExtractor::new(),
        }
    }

    /// Load fingerprints from a file
    pub fn load_fingerprints(&mut self, path: &Path) -> Result<()> {
        if path.exists() {
            let content = fs::read_to_string(path)?;
            self.fingerprints = serde_json::from_str(&content)?;
            debug!("Loaded {} fingerprints", self.fingerprints.len());
        }
        Ok(())
    }

    /// Save fingerprints to a file
    pub fn save_fingerprints(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(&self.fingerprints)?;
        fs::write(path, content)?;
        Ok(())
    }

    /// Calculate fingerprint of content
    fn calculate_fingerprint(content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Check if content has changed
    fn has_changed(&self, path: &Path, content: &str) -> (bool, String) {
        let path_str = path.to_string_lossy().to_string();
        let new_fp = Self::calculate_fingerprint(content);

        match self.fingerprints.get(&path_str) {
            Some(old_fp) if old_fp == &new_fp => (false, new_fp),
            _ => (true, new_fp),
        }
    }

    /// Update fingerprint after successful write
    fn update_fingerprint(&mut self, path: &Path, fingerprint: String) {
        let path_str = path.to_string_lossy().to_string();
        self.fingerprints.insert(path_str, fingerprint);
    }

    /// Level 1: Compile today's conversations into summaries
    #[instrument(skip(self, output_path))]
    pub async fn compile_today(&self, output_path: &Path) -> Result<CompileStatus> {
        info!("Starting Level 1 compilation (today)");

        let since = Utc::now() - Duration::days(1);
        let sessions = self.session_store.read_all_sessions(Some(since))?;

        if sessions.is_empty() {
            return Ok(CompileStatus::Skipped {
                fingerprint: "empty".to_string(),
            });
        }

        let mut input = String::new();
        input.push_str("# Conversations from the last 24 hours\n\n");

        for (session_id, messages) in &sessions {
            input.push_str(&format!("## Session: {}\n\n", session_id));
            for msg in messages {
                input.push_str(&format!("**{}**: {}\n\n", msg.role, msg.content));
            }
        }

        let (changed, fingerprint) = self.has_changed(output_path, &input);
        if !changed {
            debug!("No changes detected, skipping today compilation");
            return Ok(CompileStatus::Skipped { fingerprint });
        }

        let summary = self.summarize(&input, self.config.max_tokens_today).await?;
        self.write_compiled(output_path, "Today", &summary)?;

        Ok(CompileStatus::Success { fingerprint })
    }

    /// Level 2: Compile week summaries
    #[instrument(skip(self, output_path))]
    pub async fn compile_week(&self, output_path: &Path) -> Result<CompileStatus> {
        info!("Starting Level 2 compilation (week)");

        let since = Utc::now() - Duration::days(7);
        let until = Utc::now() - Duration::days(1);
        let sessions = self.session_store.read_all_sessions(Some(since))?;

        let mut weekly_messages: Vec<(String, Vec<Message>)> = Vec::new();
        for (session_id, messages) in sessions {
            let filtered: Vec<Message> = messages
                .into_iter()
                .filter(|m| m.timestamp >= since && m.timestamp < until)
                .collect();
            if !filtered.is_empty() {
                weekly_messages.push((session_id, filtered));
            }
        }

        if weekly_messages.is_empty() {
            return Ok(CompileStatus::Skipped {
                fingerprint: "empty".to_string(),
            });
        }

        let mut input = String::new();
        input.push_str("# Conversations from the last week\n\n");

        for (session_id, messages) in &weekly_messages {
            input.push_str(&format!("## Session: {}\n\n", session_id));
            for msg in messages {
                let truncated = if msg.content.len() > 100 {
                    format!("{}...", &msg.content[..100])
                } else {
                    msg.content.clone()
                };
                input.push_str(&format!("**{}**: {}\n\n", msg.role, truncated));
            }
        }

        let (changed, fingerprint) = self.has_changed(output_path, &input);
        if !changed {
            debug!("No changes detected, skipping week compilation");
            return Ok(CompileStatus::Skipped { fingerprint });
        }

        let summary = self.summarize(&input, self.config.max_tokens_week).await?;
        self.write_compiled(output_path, "Week", &summary)?;

        Ok(CompileStatus::Success { fingerprint })
    }

    /// Level 3: Compile long-term memory
    #[instrument(skip(self, week_path, output_path))]
    pub async fn compile_longterm(
        &self,
        week_path: &Path,
        output_path: &Path,
    ) -> Result<CompileStatus> {
        info!("Starting Level 3 compilation (long-term)");

        if !week_path.exists() {
            return Ok(CompileStatus::Skipped {
                fingerprint: "no_week_data".to_string(),
            });
        }

        let week_content = fs::read_to_string(week_path)?;

        let (changed, fingerprint) = self.has_changed(output_path, &week_content);
        if !changed {
            debug!("No changes detected, skipping long-term compilation");
            return Ok(CompileStatus::Skipped { fingerprint });
        }

        let prompt = format!(
            r#"Create a highly compressed, long-term memory summary from the following weekly summaries.
Focus on persistent facts, relationships, and important context that should be retained indefinitely.

Be extremely concise. Extract only the most important, timeless information.

Weekly summaries:
{}

Long-term memory summary:"#,
            week_content
        );

        let summary = self
            .llm_client
            .complete(&prompt, &self.config.compile_model)
            .await?;
        self.write_compiled(output_path, "Long-term", &summary)?;

        Ok(CompileStatus::Success { fingerprint })
    }

    /// Level 4: Compile structured facts
    #[instrument(skip(self, output_path))]
    pub async fn compile_facts(&self, output_path: &Path) -> Result<CompileStatus> {
        info!("Starting Level 4 compilation (facts)");

        let since = Utc::now() - Duration::days(7);
        let sessions = self.session_store.read_all_sessions(Some(since))?;

        if sessions.is_empty() {
            return Ok(CompileStatus::Skipped {
                fingerprint: "empty".to_string(),
            });
        }

        let extractor =
            FactExtractor::new(self.llm_client.clone(), self.config.extractor_model.clone());

        let mut all_facts = Vec::new();
        for (session_id, messages) in &sessions {
            let conversation: String = messages
                .iter()
                .map(|m| format!("{}: {}", m.role, m.content))
                .collect::<Vec<_>>()
                .join("\n");

            match extractor.extract_facts(&conversation).await {
                Ok(facts) => {
                    for fact in &facts {
                        let _ = self
                            .store
                            .save_fact(
                                &fact.fact,
                                &fact.tags,
                                fact.time.as_deref(),
                                Some(session_id),
                            )
                            .await;
                    }
                    all_facts.extend(facts);
                }
                Err(e) => {
                    warn!("Failed to extract facts from session {}: {}", session_id, e);
                }
            }

            let rule_facts = self.rule_extractor.extract(&conversation);
            for fact in &rule_facts {
                let _ = self
                    .store
                    .save_fact(
                        &fact.fact,
                        &fact.tags,
                        fact.time.as_deref(),
                        Some(session_id),
                    )
                    .await;
            }
            all_facts.extend(rule_facts);
        }

        if all_facts.is_empty() {
            return Ok(CompileStatus::Skipped {
                fingerprint: "no_facts".to_string(),
            });
        }

        let unique_facts = self.deduplicate_facts(&all_facts);

        let mut content = String::new();
        content.push_str("# Extracted Facts\n\n");

        for fact in &unique_facts {
            content.push_str(&format!("- **{}**\n", fact.fact));
            content.push_str(&format!("  - Tags: {}\n", fact.tags.join(", ")));
            if let Some(time) = &fact.time {
                content.push_str(&format!("  - Time: {}\n", time));
            }
            content.push('\n');
        }

        let (changed, fingerprint) = self.has_changed(output_path, &content);
        if !changed {
            debug!("No changes detected, skipping facts compilation");
            return Ok(CompileStatus::Skipped { fingerprint });
        }

        fs::write(output_path, content)?;

        // Also export as flashcards for spaced-repetition review
        let flashcards_path = output_path.with_file_name("flashcards.json");
        if let Err(e) =
            crate::flashcards::export_facts_to_flashcards(&unique_facts, &flashcards_path)
        {
            warn!("Failed to export flashcards: {}", e);
        }

        info!("Compiled {} unique facts", unique_facts.len());
        Ok(CompileStatus::Success { fingerprint })
    }

    /// Deduplicate facts based on content similarity
    fn deduplicate_facts(&self, facts: &[crate::types::MetaFact]) -> Vec<crate::types::MetaFact> {
        let mut unique = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();

        for fact in facts {
            let normalized = fact.fact.to_lowercase().trim().to_string();
            if !seen.contains(&normalized) {
                seen.insert(normalized);
                unique.push(fact.clone());
            }
        }

        unique
    }

    /// Merge similar memories based on similarity threshold
    pub fn merge_memories(&self, memories: &[String], threshold: f32) -> Vec<String> {
        if memories.len() < 2 {
            return memories.to_vec();
        }

        use crate::embedding::TfidfVectorizer;

        let mut vectorizer = TfidfVectorizer::new();
        vectorizer.fit(memories);

        let mut merged = Vec::new();
        let mut used: HashSet<usize> = HashSet::new();

        for (i, mem) in memories.iter().enumerate() {
            if used.contains(&i) {
                continue;
            }

            let query_vec = vectorizer.transform(mem);
            let mut similar_indices = vec![i];

            for (j, other) in memories.iter().enumerate() {
                if i != j && !used.contains(&j) {
                    let other_vec = vectorizer.transform(other);
                    let similarity = query_vec.cosine_similarity(&other_vec);

                    if similarity >= threshold {
                        similar_indices.push(j);
                        used.insert(j);
                    }
                }
            }

            if similar_indices.len() > 1 {
                let best = similar_indices
                    .iter()
                    .map(|&idx| &memories[idx])
                    .max_by_key(|m| m.len())
                    .cloned()
                    .unwrap_or_else(|| mem.clone());
                merged.push(best);
            } else {
                merged.push(mem.clone());
            }

            used.insert(i);
        }

        merged
    }

    /// Apply forgetting mechanism to remove old, low-importance memories
    pub async fn forget_old_memories(
        &self,
        older_than_days: i64,
        importance_threshold: f32,
    ) -> Result<usize> {
        let cutoff = Utc::now() - Duration::days(older_than_days);
        let old_facts = self.store.get_facts_since(cutoff).await?;

        let mut removed = 0;
        for fact in old_facts {
            let importance = self.calculate_importance(&fact);

            if importance < importance_threshold {
                self.store.delete_fact(fact.id).await?;
                removed += 1;
            }
        }

        info!("Forgot {} old memories", removed);
        Ok(removed)
    }

    /// Calculate importance score for a fact
    fn calculate_importance(&self, fact: &Fact) -> ImportanceScore {
        let mut score = 0.5;

        let important_tags: HashSet<&str> = ["preference", "goal", "identity", "work", "important"]
            .iter()
            .cloned()
            .collect();

        for tag in &fact.tags {
            if important_tags.contains(tag.as_str()) {
                score += 0.2;
            }
        }

        let age_days = (Utc::now() - fact.created_at).num_days() as f32;
        let age_penalty = (age_days / 365.0) * 0.1;
        score -= age_penalty;

        if fact.fact.len() > 50 {
            score += 0.05;
        }

        score.clamp(0.0, 1.0)
    }

    /// Assemble the final memory.md file
    #[instrument(skip(self))]
    pub fn assemble(
        &self,
        facts_path: &Path,
        today_path: &Path,
        week_path: &Path,
        longterm_path: &Path,
        output_path: &Path,
    ) -> Result<()> {
        info!("Assembling final memory.md");

        let mut output = String::new();

        output.push_str("# Memory\n\n");
        output.push_str(&format!("*Generated at {}*\n\n", Utc::now().to_rfc3339()));

        output.push_str("## 1. Key Facts\n\n");
        if facts_path.exists() {
            let content = fs::read_to_string(facts_path)?;
            let clean = content
                .strip_prefix("# Extracted Facts\n\n")
                .or_else(|| content.strip_prefix("# Extracted Facts\n"))
                .unwrap_or(&content);
            output.push_str(clean);
        } else {
            output.push_str("_No facts compiled yet_\n\n");
        }

        output.push_str("## 2. Today\n\n");
        if today_path.exists() {
            let content = fs::read_to_string(today_path)?;
            let clean = content
                .strip_prefix("# Today\n\n")
                .or_else(|| content.strip_prefix("# Today\n"))
                .unwrap_or(&content);
            output.push_str(clean);
        } else {
            output.push_str("_No recent conversations_\n\n");
        }

        output.push_str("## 3. This Week\n\n");
        if week_path.exists() {
            let content = fs::read_to_string(week_path)?;
            let clean = content
                .strip_prefix("# Week\n\n")
                .or_else(|| content.strip_prefix("# Week\n"))
                .unwrap_or(&content);
            output.push_str(clean);
        } else {
            output.push_str("_No weekly summary_\n\n");
        }

        output.push_str("## 4. Long-term\n\n");
        if longterm_path.exists() {
            let content = fs::read_to_string(longterm_path)?;
            let clean = content
                .strip_prefix("# Long-term\n\n")
                .or_else(|| content.strip_prefix("# Long-term\n"))
                .unwrap_or(&content);
            output.push_str(clean);
        } else {
            output.push_str("_No long-term memory_\n\n");
        }

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(output_path, output)?;
        info!("Memory assembled at {:?}", output_path);

        Ok(())
    }

    /// Run the full compilation pipeline
    #[instrument(skip(self))]
    pub async fn compile_all(
        &mut self,
        output_dir: &Path,
    ) -> Result<HashMap<String, CompileStatus>> {
        let facts_path = output_dir.join("facts.md");
        let today_path = output_dir.join("today.md");
        let week_path = output_dir.join("week.md");
        let longterm_path = output_dir.join("longterm.md");
        let memory_path = output_dir.join("memory.md");

        let mut results = HashMap::new();

        match self.compile_today(&today_path).await {
            Ok(status) => {
                if let Some(fp) = status.fingerprint() {
                    self.update_fingerprint(&today_path, fp.to_string());
                }
                results.insert("today".to_string(), status);
            }
            Err(e) => {
                results.insert(
                    "today".to_string(),
                    CompileStatus::Failed {
                        error: e.to_string(),
                    },
                );
            }
        }

        match self.compile_week(&week_path).await {
            Ok(status) => {
                if let Some(fp) = status.fingerprint() {
                    self.update_fingerprint(&week_path, fp.to_string());
                }
                results.insert("week".to_string(), status);
            }
            Err(e) => {
                results.insert(
                    "week".to_string(),
                    CompileStatus::Failed {
                        error: e.to_string(),
                    },
                );
            }
        }

        match self.compile_longterm(&week_path, &longterm_path).await {
            Ok(status) => {
                if let Some(fp) = status.fingerprint() {
                    self.update_fingerprint(&longterm_path, fp.to_string());
                }
                results.insert("longterm".to_string(), status);
            }
            Err(e) => {
                results.insert(
                    "longterm".to_string(),
                    CompileStatus::Failed {
                        error: e.to_string(),
                    },
                );
            }
        }

        match self.compile_facts(&facts_path).await {
            Ok(status) => {
                if let Some(fp) = status.fingerprint() {
                    self.update_fingerprint(&facts_path, fp.to_string());
                }
                results.insert("facts".to_string(), status);
            }
            Err(e) => {
                results.insert(
                    "facts".to_string(),
                    CompileStatus::Failed {
                        error: e.to_string(),
                    },
                );
            }
        }

        if let Err(e) = self.assemble(
            &facts_path,
            &today_path,
            &week_path,
            &longterm_path,
            &memory_path,
        ) {
            error!("Failed to assemble memory: {}", e);
        }

        let fp_path = output_dir.join(".fingerprints.json");
        if let Err(e) = self.save_fingerprints(&fp_path) {
            warn!("Failed to save fingerprints: {}", e);
        }

        Ok(results)
    }

    /// Generate a summary using the LLM
    async fn summarize(&self, input: &str, max_tokens: usize) -> Result<String> {
        let prompt = format!(
            "Summarize the following conversation content concisely.\n\
            Focus on key topics discussed, important information shared, and action items.\n\
            Limit your response to approximately {} tokens.\n\n\
            Content to summarize:\n{}\n\n\
            Summary:",
            max_tokens / 4,
            input
        );

        self.llm_client
            .complete(&prompt, &self.config.compile_model)
            .await
    }

    /// Write compiled content with header
    fn write_compiled(&self, path: &Path, level: &str, content: &str) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let output = format!("# {}\n\n{}", level, content);
        fs::write(path, output)?;

        debug!("Wrote {} compilation to {:?}", level, path);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extractor::MockLlmClient;
    use tempfile::TempDir;

    fn create_test_compiler() -> (TempDir, MemoryCompiler) {
        let temp_dir = TempDir::new().unwrap();

        let store = MemoryStore::new_in_memory().unwrap();
        let session_store = SessionStore::new(temp_dir.path().join("sessions")).unwrap();
        let client = Arc::new(MockLlmClient::new("Test summary output"));
        let config = CompileConfig::default();

        let compiler = MemoryCompiler::new(store, session_store, client, config);

        (temp_dir, compiler)
    }

    #[test]
    fn test_fingerprint_calculation() {
        let fp1 = MemoryCompiler::calculate_fingerprint("test content");
        let fp2 = MemoryCompiler::calculate_fingerprint("test content");
        let fp3 = MemoryCompiler::calculate_fingerprint("different content");

        assert_eq!(fp1, fp2);
        assert_ne!(fp1, fp3);
        assert_eq!(fp1.len(), 64);
    }

    #[tokio::test]
    async fn test_compile_today_empty() {
        let (temp, compiler) = create_test_compiler();
        let output_path = temp.path().join("test_today.md");

        let result = compiler.compile_today(&output_path).await.unwrap();
        assert!(result.is_skipped());
    }

    #[test]
    fn test_assemble() {
        let (temp, compiler) = create_test_compiler();

        let facts_path = temp.path().join("facts.md");
        let today_path = temp.path().join("today.md");
        let week_path = temp.path().join("week.md");
        let longterm_path = temp.path().join("longterm.md");
        let output_path = temp.path().join("memory.md");

        fs::write(&facts_path, "# Extracted Facts\n\n- User likes Rust\n").unwrap();
        fs::write(&today_path, "# Today\n\nDiscussed programming.\n").unwrap();
        fs::write(&week_path, "# Week\n\nWeekly summary here.\n").unwrap();
        fs::write(&longterm_path, "# Long-term\n\nLong term facts.\n").unwrap();

        compiler
            .assemble(
                &facts_path,
                &today_path,
                &week_path,
                &longterm_path,
                &output_path,
            )
            .unwrap();

        let output = fs::read_to_string(&output_path).unwrap();

        assert!(output.contains("# Memory"));
        assert!(output.contains("## 1. Key Facts"));
        assert!(output.contains("## 2. Today"));
        assert!(output.contains("## 3. This Week"));
        assert!(output.contains("## 4. Long-term"));
        assert!(output.contains("User likes Rust"));
    }

    #[test]
    fn test_fingerprints_persist() {
        let (temp, mut compiler) = create_test_compiler();

        let fp_path = temp.path().join("fingerprints.json");

        compiler
            .fingerprints
            .insert("test".to_string(), "abc123".to_string());

        compiler.save_fingerprints(&fp_path).unwrap();

        let store = MemoryStore::new_in_memory().unwrap();
        let session_store = SessionStore::new(temp.path().join("sessions2")).unwrap();
        let client = Arc::new(MockLlmClient::new("test"));
        let mut compiler2 =
            MemoryCompiler::new(store, session_store, client, CompileConfig::default());

        compiler2.load_fingerprints(&fp_path).unwrap();

        assert_eq!(
            compiler2.fingerprints.get("test"),
            Some(&"abc123".to_string())
        );
    }

    #[test]
    fn test_merge_memories() {
        let (_temp, compiler) = create_test_compiler();

        let memories = vec![
            "User likes Rust programming".to_string(),
            "User likes Rust for systems programming".to_string(),
            "User enjoys Python scripting".to_string(),
            "User prefers JavaScript for web".to_string(),
        ];

        let merged = compiler.merge_memories(&memories, 0.7);

        assert!(merged.len() < memories.len());
    }

    #[test]
    fn test_calculate_importance() {
        let (_temp, compiler) = create_test_compiler();

        let important_fact = Fact {
            id: 1,
            fact: "User is CEO of company".to_string(),
            tags: vec!["identity".to_string(), "work".to_string()],
            time: None,
            session_id: None,
            created_at: Utc::now(),
        };

        let unimportant_fact = Fact {
            id: 2,
            fact: "User said hello".to_string(),
            tags: vec!["greeting".to_string()],
            time: None,
            session_id: None,
            created_at: Utc::now() - Duration::days(400),
        };

        let important_score = compiler.calculate_importance(&important_fact);
        let unimportant_score = compiler.calculate_importance(&unimportant_fact);

        assert!(important_score > unimportant_score);
        assert!(important_score > 0.5);
    }

    #[test]
    fn test_deduplicate_facts() {
        let (_temp, compiler) = create_test_compiler();

        let facts = vec![
            crate::types::MetaFact {
                fact: "User likes Rust".to_string(),
                tags: vec!["tech".to_string()],
                time: None,
            },
            crate::types::MetaFact {
                fact: "user likes rust".to_string(),
                tags: vec!["tech".to_string()],
                time: None,
            },
            crate::types::MetaFact {
                fact: "User likes Python".to_string(),
                tags: vec!["tech".to_string()],
                time: None,
            },
        ];

        let unique = compiler.deduplicate_facts(&facts);
        assert_eq!(unique.len(), 2);
    }
}
