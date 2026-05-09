//! Turn-based memory extraction — forked agent extracts structured notes after each turn.

use crate::registry::ToolRegistry;
use crate::subagents::{CapabilityToken, RunSpec, SubagentManager};
use clarity_memory::SessionNotes;
use std::path::PathBuf;
use std::sync::Arc;

/// Lightweight subagent that extracts structured notes from a conversation turn.
#[derive(Clone)]
pub struct TurnMemoryExtractor {
    llm: Arc<dyn clarity_llm::api::LlmProvider>,
    working_dir: PathBuf,
}

impl TurnMemoryExtractor {
    pub fn new(
        llm: Arc<dyn clarity_llm::api::LlmProvider>,
        working_dir: impl Into<PathBuf>,
    ) -> Self {
        Self {
            llm,
            working_dir: working_dir.into(),
        }
    }

    /// Extract structured notes from a single conversation turn.
    /// Spawns a lightweight read-only subagent to analyze the transcript.
    pub async fn extract(&self, turn_transcript: &str) -> anyhow::Result<SessionNotes> {
        let spec = RunSpec::new(
            "Extract structured notes from conversation turn",
            format!(
                "Analyze the following conversation turn and extract structured notes. \
                 Respond with a JSON object containing exactly these keys: \
                 current_state (string), errors (array of strings), \
                 learnings (array of strings), key_results (array of strings).\n\n{}",
                turn_transcript
            ),
        )
        .with_type("memory-extractor")
        .with_capability_token(CapabilityToken::read_only())
        .without_git_context();

        let mut manager = SubagentManager::new(
            ToolRegistry::with_builtin_tools(),
            &self.working_dir,
            self.working_dir.join(".clarity").join("subagents"),
        )
        .with_llm(self.llm.clone());

        let result = manager.run(spec, None).await?;

        let notes: SessionNotes = serde_json::from_str(&result.summary)
            .unwrap_or_else(|_| SessionNotes::new("extracted"));

        Ok(notes)
    }
}
