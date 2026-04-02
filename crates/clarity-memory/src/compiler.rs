//! Four-level memory compilation pipeline (OpenHanako-style)

use crate::extractor::{FactExtractor, LlmClient};
use crate::store::MemoryStore;
use crate::types::{CompileStatus, CompileConfig, Message, Result};
use crate::session_store::SessionStore;
use chrono::{Duration, Utc};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, error, info, instrument, warn};

/// Four-level memory compiler following OpenHanako's design
/// 
/// Level 1: Today - Recent conversation summaries
/// Level 2: Week - Aggregated daily summaries
/// Level 3: Long-term - Compressed historical context
/// Level 4: Facts - Structured fact database (via extractor)
pub struct MemoryCompiler {
    store: MemoryStore,
    session_store: SessionStore,
    llm_client: Arc<dyn LlmClient>,
    config: CompileConfig,
    fingerprints: HashMap<String, String>, // path -> fingerprint
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
    /// 
    /// Aggregates recent messages from all sessions into a daily summary.
    #[instrument(skip(self, output_path))]
    pub async fn compile_today(&self, output_path: &Path) -> Result<CompileStatus> {
        info!("Starting Level 1 compilation (today)");

        // Get messages from last 24 hours
        let since = Utc::now() - Duration::days(1);
        let sessions = self.session_store.read_all_sessions(Some(since))?;

        if sessions.is_empty() {
            return Ok(CompileStatus::Skipped {
                fingerprint: "empty".to_string(),
            });
        }

        // Build compilation input
        let mut input = String::new();
        input.push_str("# Conversations from the last 24 hours\n\n");
        
        for (session_id, messages) in &sessions {
            input.push_str(&format!("## Session: {}\n\n", session_id));
            for msg in messages {
                input.push_str(&format!("**{}**: {}\n\n", msg.role, msg.content));
            }
        }

        // Check if changed
        let (changed, fingerprint) = self.has_changed(output_path, &input);
        if !changed {
            debug!("No changes detected, skipping today compilation");
            return Ok(CompileStatus::Skipped { fingerprint });
        }

        // Generate summary using LLM
        let summary = self.summarize(&input, self.config.max_tokens_today).await?;

        // Write output
        self.write_compiled(output_path, "Today", &summary)?;

        Ok(CompileStatus::Success { fingerprint })
    }

    /// Level 2: Compile week summaries
    /// 
    /// Aggregates daily summaries into a weekly summary.
    #[instrument(skip(self, output_path))]
    pub async fn compile_week(&self, output_path: &Path) -> Result<CompileStatus> {
        info!("Starting Level 2 compilation (week)");

        // Get messages from last 7 days (excluding today which is handled separately)
        let since = Utc::now() - Duration::days(7);
        let until = Utc::now() - Duration::days(1);
        let sessions = self.session_store.read_all_sessions(Some(since))?;

        // Filter messages to the week range
        let mut weekly_messages: Vec<(String, Vec<Message>)> = Vec::new();
        for (session_id, messages) in sessions {
            let filtered: Vec<Message> = messages.into_iter()
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

        // Build compilation input
        let mut input = String::new();
        input.push_str("# Conversations from the last week\n\n");
        
        for (session_id, messages) in &weekly_messages {
            input.push_str(&format!("## Session: {}\n\n", session_id));
            for msg in messages {
                input.push_str(&format!("**{}**: {}\n\n", msg.role, &msg.content[..100.min(msg.content.len())]));
            }
        }

        // Check if changed
        let (changed, fingerprint) = self.has_changed(output_path, &input);
        if !changed {
            debug!("No changes detected, skipping week compilation");
            return Ok(CompileStatus::Skipped { fingerprint });
        }

        // Generate summary
        let summary = self.summarize(&input, self.config.max_tokens_week).await?;

        // Write output
        self.write_compiled(output_path, "Week", &summary)?;

        Ok(CompileStatus::Success { fingerprint })
    }

    /// Level 3: Compile long-term memory
    /// 
    /// Takes week summaries and creates a compressed long-term summary.
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
        
        // Check if changed
        let (changed, fingerprint) = self.has_changed(output_path, &week_content);
        if !changed {
            debug!("No changes detected, skipping long-term compilation");
            return Ok(CompileStatus::Skipped { fingerprint });
        }

        // Generate compressed long-term summary
        let prompt = format!(
            r#"Create a highly compressed, long-term memory summary from the following weekly summaries.
Focus on persistent facts, relationships, and important context that should be retained indefinitely.

Be extremely concise. Extract only the most important, timeless information.

Weekly summaries:
{}

Long-term memory summary:"#,
            week_content
        );

        let summary = self.llm_client.complete(&prompt, &self.config.compile_model).await?;

        // Write output
        self.write_compiled(output_path, "Long-term", &summary)?;

        Ok(CompileStatus::Success { fingerprint })
    }

    /// Level 4: Compile structured facts
    /// 
    /// Uses LLM extraction to build structured fact database.
    #[instrument(skip(self, output_path))]
    pub async fn compile_facts(&self, output_path: &Path) -> Result<CompileStatus> {
        info!("Starting Level 4 compilation (facts)");

        // Get recent conversations for fact extraction
        let since = Utc::now() - Duration::days(7);
        let sessions = self.session_store.read_all_sessions(Some(since))?;

        if sessions.is_empty() {
            return Ok(CompileStatus::Skipped {
                fingerprint: "empty".to_string(),
            });
        }

        // Create fact extractor
        let extractor = FactExtractor::new(
            self.llm_client.clone(),
            self.config.extractor_model.clone(),
        );

        // Extract facts from each session
        let mut all_facts = Vec::new();
        for (session_id, messages) in &sessions {
            let conversation: String = messages.iter()
                .map(|m| format!("{}: {}", m.role, m.content))
                .collect::<Vec<_>>()
                .join("\n");

            match extractor.extract_facts(&conversation).await {
                Ok(facts) => {
                    for fact in facts {
                        // Save to database
                        let _ = self.store.save_fact(
                            &fact.fact,
                            &fact.tags,
                            fact.time.as_deref(),
                            Some(session_id),
                        )?;
                        all_facts.push(fact);
                    }
                }
                Err(e) => {
                    warn!("Failed to extract facts from session {}: {}", session_id, e);
                }
            }
        }

        if all_facts.is_empty() {
            return Ok(CompileStatus::Skipped {
                fingerprint: "no_facts".to_string(),
            });
        }

        // Generate output content
        let mut content = String::new();
        content.push_str("# Extracted Facts\n\n");
        
        for fact in &all_facts {
            content.push_str(&format!("- **{}**\n", fact.fact));
            content.push_str(&format!("  - Tags: {}\n", fact.tags.join(", ")));
            if let Some(time) = &fact.time {
                content.push_str(&format!("  - Time: {}\n", time));
            }
            content.push('\n');
        }

        // Check if changed
        let (changed, fingerprint) = self.has_changed(output_path, &content);
        if !changed {
            debug!("No changes detected, skipping facts compilation");
            return Ok(CompileStatus::Skipped { fingerprint });
        }

        // Write output
        fs::write(output_path, content)?;

        Ok(CompileStatus::Success { fingerprint })
    }

    /// Assemble the final memory.md file
    /// 
    /// Combines all four levels into a single memory file with sections.
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
        
        // Header
        output.push_str("# Memory\n\n");
        output.push_str(&format!("*Generated at {}*\n\n", Utc::now().to_rfc3339()));

        // Section 1: Facts (highest priority)
        output.push_str("## 1. Key Facts\n\n");
        if facts_path.exists() {
            let content = fs::read_to_string(facts_path)?;
            // Strip the header if present
            let clean = content.strip_prefix("# Extracted Facts\n\n")
                .or_else(|| content.strip_prefix("# Extracted Facts\n"))
                .unwrap_or(&content);
            output.push_str(clean);
        } else {
            output.push_str("_No facts compiled yet_\n\n");
        }

        // Section 2: Today
        output.push_str("## 2. Today\n\n");
        if today_path.exists() {
            let content = fs::read_to_string(today_path)?;
            let clean = content.strip_prefix("# Today\n\n")
                .or_else(|| content.strip_prefix("# Today\n"))
                .unwrap_or(&content);
            output.push_str(clean);
        } else {
            output.push_str("_No recent conversations_\n\n");
        }

        // Section 3: This Week
        output.push_str("## 3. This Week\n\n");
        if week_path.exists() {
            let content = fs::read_to_string(week_path)?;
            let clean = content.strip_prefix("# Week\n\n")
                .or_else(|| content.strip_prefix("# Week\n"))
                .unwrap_or(&content);
            output.push_str(clean);
        } else {
            output.push_str("_No weekly summary_\n\n");
        }

        // Section 4: Long-term
        output.push_str("## 4. Long-term\n\n");
        if longterm_path.exists() {
            let content = fs::read_to_string(longterm_path)?;
            let clean = content.strip_prefix("# Long-term\n\n")
                .or_else(|| content.strip_prefix("# Long-term\n"))
                .unwrap_or(&content);
            output.push_str(clean);
        } else {
            output.push_str("_No long-term memory_\n\n");
        }

        // Ensure output directory exists
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(output_path, output)?;
        info!("Memory assembled at {:?}", output_path);

        Ok(())
    }

    /// Run the full compilation pipeline
    #[instrument(skip(self))]
    pub async fn compile_all(&mut self, output_dir: &Path) -> Result<HashMap<String, CompileStatus>> {
        let facts_path = output_dir.join("facts.md");
        let today_path = output_dir.join("today.md");
        let week_path = output_dir.join("week.md");
        let longterm_path = output_dir.join("longterm.md");
        let memory_path = output_dir.join("memory.md");

        let mut results = HashMap::new();

        // Level 1: Today
        match self.compile_today(&today_path).await {
            Ok(status) => {
                if let Some(fp) = status.fingerprint() {
                    self.update_fingerprint(&today_path, fp.to_string());
                }
                results.insert("today".to_string(), status);
            }
            Err(e) => {
                results.insert("today".to_string(), CompileStatus::Failed { error: e.to_string() });
            }
        }

        // Level 2: Week
        match self.compile_week(&week_path).await {
            Ok(status) => {
                if let Some(fp) = status.fingerprint() {
                    self.update_fingerprint(&week_path, fp.to_string());
                }
                results.insert("week".to_string(), status);
            }
            Err(e) => {
                results.insert("week".to_string(), CompileStatus::Failed { error: e.to_string() });
            }
        }

        // Level 3: Long-term
        match self.compile_longterm(&week_path, &longterm_path).await {
            Ok(status) => {
                if let Some(fp) = status.fingerprint() {
                    self.update_fingerprint(&longterm_path, fp.to_string());
                }
                results.insert("longterm".to_string(), status);
            }
            Err(e) => {
                results.insert("longterm".to_string(), CompileStatus::Failed { error: e.to_string() });
            }
        }

        // Level 4: Facts
        match self.compile_facts(&facts_path).await {
            Ok(status) => {
                if let Some(fp) = status.fingerprint() {
                    self.update_fingerprint(&facts_path, fp.to_string());
                }
                results.insert("facts".to_string(), status);
            }
            Err(e) => {
                results.insert("facts".to_string(), CompileStatus::Failed { error: e.to_string() });
            }
        }

        // Assemble final memory.md
        if let Err(e) = self.assemble(&facts_path, &today_path, &week_path, &longterm_path, &memory_path) {
            error!("Failed to assemble memory: {}", e);
        }

        // Save fingerprints
        let fp_path = output_dir.join(".fingerprints.json");
        if let Err(e) = self.save_fingerprints(&fp_path) {
            warn!("Failed to save fingerprints: {}", e);
        }

        Ok(results)
    }

    /// Generate a summary using the LLM
    async fn summarize(&self, input: &str, max_tokens: usize) -> Result<String> {
        let prompt = format!(
            r#"Summarize the following conversation content concisely. 
Focus on key topics discussed, important information shared, and action items.
Limit your response to approximately {} tokens.

Content to summarize:
{}

Summary:"#,
            max_tokens / 4, // Rough approximation: 1 token ~= 4 chars
            input
        );

        self.llm_client.complete(&prompt, &self.config.compile_model).await
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
        assert_eq!(fp1.len(), 64); // SHA256 hex is 64 chars
    }

    #[tokio::test]
    async fn test_compile_today_empty() {
        let (temp, compiler) = create_test_compiler();
        let output_path = temp.path().join("test_today.md");

        let result = compiler.compile_today(&output_path).await.unwrap();
        assert!(result.is_skipped());
    }

    #[tokio::test]
    async fn test_assemble() {
        let (temp, compiler) = create_test_compiler();
        
        // Create test input files
        let facts_path = temp.path().join("facts.md");
        let today_path = temp.path().join("today.md");
        let week_path = temp.path().join("week.md");
        let longterm_path = temp.path().join("longterm.md");
        let output_path = temp.path().join("memory.md");

        fs::write(&facts_path, "# Extracted Facts\n\n- User likes Rust\n").unwrap();
        fs::write(&today_path, "# Today\n\nDiscussed programming.\n").unwrap();
        fs::write(&week_path, "# Week\n\nWeekly summary here.\n").unwrap();
        fs::write(&longterm_path, "# Long-term\n\nLong term facts.\n").unwrap();

        compiler.assemble(&facts_path, &today_path, &week_path, &longterm_path, &output_path).unwrap();

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
        
        // Add some fingerprints
        compiler.fingerprints.insert("test".to_string(), "abc123".to_string());
        
        // Save
        compiler.save_fingerprints(&fp_path).unwrap();
        
        // Load into new compiler
        let store = MemoryStore::new_in_memory().unwrap();
        let session_store = SessionStore::new(temp.path().join("sessions2")).unwrap();
        let client = Arc::new(MockLlmClient::new("test"));
        let mut compiler2 = MemoryCompiler::new(store, session_store, client, CompileConfig::default());
        
        compiler2.load_fingerprints(&fp_path).unwrap();
        
        assert_eq!(compiler2.fingerprints.get("test"), Some(&"abc123".to_string()));
    }
}
