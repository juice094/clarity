//! Shared UI types + Pretext cold-path prepare() for Message.
//!
//! ARCHITECTURE CONSTRAINT (Pretext-aligned):
//!   - `Message::prepare()` is the ONLY cold-path entry for markdown parsing.
//!   - `RenderBlock` / `InlineSpan` are the intermediate representation.
//!   - When adding new block types, update `estimate_height()` in main.rs.
//!
//! See `crates/clarity-egui/ARCHITECTURE.md` §1.1, §2.2.

use clarity_core::agent::Plan;
use clarity_core::background::TaskInfo;
use std::time::Instant;

// ============================================================================
// Shared UI Types — extracted from main.rs for modularity
// ============================================================================

#[derive(Debug, Clone)]
pub enum UiEvent {
    Chunk(String),
    ToolStart {
        id: String,
        name: String,
        arguments: serde_json::Value,
    },
    ToolResult {
        id: String,
        result: String,
    },
    StepBegin {
        tool_name: String,
    },
    CompactionBegin,
    CompactionEnd,
    Done,
    Error(String),
    Fallback {
        fallback: bool,
        reason: String,
    },
    TaskList(Vec<TaskInfo>),
    Usage {
        prompt_tokens: u32,
        completion_tokens: u32,
        total_tokens: u32,
    },
    /// SubAgent parallel batch status update from Gateway polling.
    SubAgentBatch(String, serde_json::Value),
    PlanReady(Plan),
    PlanStepBegin {
        step_id: String,
        #[allow(dead_code)]
        tool_name: String,
    },
    PlanStepEnd {
        step_id: String,
        success: bool,
    },
    /// Provider test connection result (async callback from Provider panel).
    ProviderTestResult {
        provider_id: String,
        success: bool,
        error: Option<String>,
    },
    /// Provider model list fetched from API (async callback from Provider panel).
    ProviderModelList {
        provider_id: String,
        models: Vec<String>,
    },
    /// Async web page fetch completed — payload delivered to chat preview area.
    WebPageFetched {
        title: String,
        url: String,
        content: String,
    },
    /// Resolve an approval request asynchronously (moved off the UI thread).
    ResolveApproval {
        req_id: String,
        response: clarity_core::approval::ApprovalResponse,
    },
    /// MCP tools reloaded after config save.
    McpReloaded {
        success: bool,
        tools: Vec<String>,
        message: String,
    },
    /// OAuth login flow result (Kimi Code or any future OAuth provider).
    KimiCodeLoginResult {
        success: bool,
        message: String,
        /// Provider id to switch to on success (e.g. "kimi_code").
        provider_id: String,
    },
    /// Kimi Code OAuth login intermediate state update.
    KimiCodeLoginStateUpdate {
        state: String,
        user_code: Option<String>,
        url: Option<String>,
        error: Option<String>,
    },
}

/// Progress summary for a parallel batch of subagents.
#[derive(Clone, Debug)]
pub struct SubAgentProgress {
    pub batch_id: String,
    pub total: usize,
    pub completed: usize,
    pub failed: usize,
    pub status: String,
    pub elapsed_ms: u64,
    pub agent_statuses: Vec<AgentStatusEntry>,
    pub last_poll: std::time::Instant,
}

#[derive(Clone, Debug)]
pub struct AgentStatusEntry {
    pub agent_id: String,
    pub status: String,
    pub summary: Option<String>,
}

/// Live tracker for an executing plan.
#[derive(Clone, Debug)]
pub struct PlanExecutionTracker {
    pub title: String,
    pub steps: Vec<PlanStepTracker>,
}

#[derive(Clone, Debug)]
pub struct PlanStepTracker {
    pub id: String,
    pub description: String,
    pub tool_name: String,
    pub status: PlanStepStatus,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PlanStepStatus {
    Pending,
    Running,
    Success,
    Failed,
}

#[derive(Clone)]
pub struct Session {
    pub id: String,
    pub title: String,
    pub category: String,
    pub messages: Vec<Message>,
    pub updated_at: u64,
    /// Cached heights for aggregated agent turns (used when agent_turn_style is enabled).
    pub turn_heights: Vec<Option<f32>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ContentBlock {
    Text { text: String },
    Code { language: String, code: String },
    ToolResult {
        name: String,
        args: Option<String>,
        output: String,
        truncated: bool,
    },
    ToolCall {
        name: String,
        args: String,
    },
    Think { steps: Vec<String> },
    Plan { title: String, steps: Vec<String> },
    FilePreview { path: String, content: String },
}

#[derive(Clone)]
pub struct Message {
    pub role: Role,
    pub content: String,
    /// Pretext structured content blocks.
    pub blocks: Vec<ContentBlock>,
    #[allow(dead_code)]
    pub timestamp: Instant,
    /// Pretext-style prepared blocks — parsed once when content changes.
    pub parsed: Vec<RenderBlock>,
    /// Cached bubble height from last render (for virtual list estimation).
    pub cached_height: Option<f32>,
    /// Semantic flag: true for system/error messages that need distinct styling.
    pub is_error: bool,
}

impl Message {
    /// Cold path: sync content from blocks (if any), then parse markdown into cached blocks.
    pub fn prepare(&mut self) {
        if !self.blocks.is_empty() {
            self.content = Self::blocks_to_markdown(&self.blocks);
        }
        self.parsed = crate::ui::markdown::parse_markdown(&self.content);
        // Invalidate height cache when content changes.
        self.cached_height = None;
    }

    /// Serialize structured blocks back into markdown for fallback / height estimation.
    fn blocks_to_markdown(blocks: &[ContentBlock]) -> String {
        let mut out = String::new();
        for block in blocks {
            match block {
                ContentBlock::Text { text } => {
                    out.push_str(text);
                }
                ContentBlock::Code { language, code } => {
                    out.push_str(&format!("\n```{language}\n{code}\n```\n"));
                }
                ContentBlock::ToolResult { name, output, .. } => {
                    out.push_str(&format!("\n🔧 **{name}**\n```json\n{output}\n```\n"));
                }
                ContentBlock::ToolCall { .. } => {
                    // Intentionally not rendered in markdown fallback.
                }
                ContentBlock::Think { steps } => {
                    out.push_str(&format!("\n💭 Thinking ({} steps)\n", steps.len()));
                }
                ContentBlock::Plan { title, steps } => {
                    out.push_str(&format!("\n📋 **{title}**\n"));
                    for step in steps {
                        out.push_str(&format!("- {step}\n"));
                    }
                }
                ContentBlock::FilePreview { path, content } => {
                    out.push_str(&format!("\n📄 **{path}**\n```\n{content}\n```\n"));
                }
            }
        }
        out
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Role {
    User,
    Agent,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum AgentStatus {
    Online,
    Busy,
    Unconfigured,
    Offline,
}

#[derive(Clone)]
pub struct ToolCallInfo {
    pub id: String,
    pub name: String,
    pub status: ToolCallStatus,
    pub result: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ToolCallStatus {
    Running,
    Success,
    Error,
    Warning,
}

impl ToolCallInfo {
    /// UI-layer heuristic: infer status from result text when the raw status
    /// is already terminal (non-Running).
    pub fn inferred_status(&self) -> ToolCallStatus {
        match self.status {
            ToolCallStatus::Running => ToolCallStatus::Running,
            _ => {
                if let Some(ref result) = self.result {
                    let lower = result.to_lowercase();
                    if lower.contains("panic")
                        || lower.contains("unreachable")
                        || lower.contains("fatal")
                    {
                        return ToolCallStatus::Error;
                    }
                    if lower.contains("error")
                        || lower.contains("failed")
                        || lower.contains("fail")
                        || lower.contains("exception")
                    {
                        return ToolCallStatus::Warning;
                    }
                    ToolCallStatus::Success
                } else {
                    self.status
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct Attachment {
    pub path: std::path::PathBuf,
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct Toast {
    pub message: String,
    pub level: ToastLevel,
    pub created_at: std::time::Instant,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ToastLevel {
    Info,
    Warn,
    Error,
}

// ============================================================================
// Parsed Markdown — Pretext-style two-stage separation
// prepare(): parse text into blocks once when content changes
// layout():  iterate blocks and issue egui commands per frame
// ============================================================================

#[derive(Clone, Debug)]
pub enum InlineSpan {
    Text(String),
    Bold(String),
    Code(String),
    Link { text: String, url: String },
}

#[derive(Clone, Debug)]
pub enum RenderBlock {
    Paragraph(Vec<InlineSpan>),
    Heading(u8, Vec<InlineSpan>),
    CodeBlock { lang: String, code: String },
    ListItem(Vec<InlineSpan>),
    Blockquote(Vec<InlineSpan>),
    HorizontalRule,
}

// ============================================================================
// Preview — reused glass-card preview in chat area for files and web pages
// ============================================================================

#[derive(Clone, Debug)]
pub enum PreviewItem {
    File { name: String, content: String, path: String },
    WebPage { title: String, url: String, content: String },
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct WebTab {
    pub title: String,
    pub url: String,
}

// ============================================================================
// Unit tests for ToolCallStatus inference
// ============================================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_status_inference_success() {
        let tc = ToolCallInfo {
            id: "1".into(),
            name: "read_file".into(),
            status: ToolCallStatus::Success,
            result: Some("file content".into()),
        };
        assert_eq!(tc.inferred_status(), ToolCallStatus::Success);
    }

    #[test]
    fn test_tool_status_inference_error_panic() {
        let tc = ToolCallInfo {
            id: "2".into(),
            name: "exec".into(),
            status: ToolCallStatus::Success,
            result: Some("thread panicked at src/main.rs:42".into()),
        };
        assert_eq!(tc.inferred_status(), ToolCallStatus::Error);
    }

    #[test]
    fn test_tool_status_inference_warning_error() {
        let tc = ToolCallInfo {
            id: "3".into(),
            name: "search".into(),
            status: ToolCallStatus::Success,
            result: Some("Command failed with exit code 1".into()),
        };
        assert_eq!(tc.inferred_status(), ToolCallStatus::Warning);
    }

    #[test]
    fn test_tool_status_inference_running_passthrough() {
        let tc = ToolCallInfo {
            id: "4".into(),
            name: "run".into(),
            status: ToolCallStatus::Running,
            result: Some("panic in progress".into()),
        };
        assert_eq!(tc.inferred_status(), ToolCallStatus::Running);
    }

    #[test]
    fn test_tool_status_inference_no_result() {
        let tc = ToolCallInfo {
            id: "5".into(),
            name: "noop".into(),
            status: ToolCallStatus::Warning,
            result: None,
        };
        assert_eq!(tc.inferred_status(), ToolCallStatus::Warning);
    }
}
