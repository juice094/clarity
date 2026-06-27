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
use std::collections::HashMap;
use std::time::Instant;

// ============================================================================
// Shared UI Types — extracted from main.rs for modularity
// ============================================================================

/// ui event variants.
#[derive(Debug, Clone)]
pub enum UiEvent {
    Chunk {
        session_id: String,
        text: String,
    },
    ToolStart {
        session_id: String,
        id: String,
        name: String,
        arguments: serde_json::Value,
    },
    ToolResult {
        session_id: String,
        id: String,
        result: String,
    },
    StepBegin {
        session_id: String,
        tool_name: String,
    },
    CompactionBegin {
        session_id: String,
    },
    CompactionEnd {
        session_id: String,
    },
    /// Transient draft/thinking indicator shown while the model is preparing a response.
    DraftProgress {
        session_id: String,
        text: String,
    },
    /// Clear the transient draft indicator (the model is about to emit real content).
    DraftClear {
        session_id: String,
    },
    /// Optional reasoning/thinking content (e.g. <think> blocks from reasoning models).
    DraftContent {
        session_id: String,
        text: String,
    },
    /// A reasoning/thinking chunk that should be persisted as a collapsible block
    /// in the current assistant message.
    ReasoningChunk {
        session_id: String,
        text: String,
    },
    /// A new agent turn has begun. Carries the user input for confirmation / telemetry.
    TurnStart {
        session_id: String,
        user_input: String,
    },
    /// The current agent turn has ended. Replaces the legacy `Done` event over time.
    TurnEnd {
        session_id: String,
    },
    /// Backend status text update (e.g. "Executing tools...", "Compacting context...").
    StatusUpdate {
        session_id: String,
        message: String,
    },
    /// Delta update for the frontend view state.
    ///
    /// Only fields that changed are present; absent fields must be ignored.
    /// Initially only `turn` is authoritative from the backend.
    ViewStateUpdate {
        session_id: String,
        turn: Option<clarity_core::ui::TurnState>,
    },
    /// A thread/session became active (e.g. user switched or backend promoted one).
    ThreadActive {
        thread_id: String,
        #[allow(dead_code)]
        title: Option<String>,
    },
    /// The list of recent threads/sessions has been refreshed.
    ThreadList {
        threads: Vec<Session>,
    },
    /// A new thread/session was created.
    ThreadCreated {
        session: Session,
    },
    /// Thread/session metadata was updated (title, archive state, etc.).
    ThreadUpdated {
        thread_id: String,
        title: Option<String>,
        archived: Option<bool>,
    },
    /// A thread/session was deleted.
    #[allow(dead_code)]
    ThreadDeleted {
        thread_id: String,
    },
    /// Session-level metadata update from the backend (e.g. provider state blobs).
    SessionMeta {
        session_id: String,
        provider_state: HashMap<String, String>,
    },
    Done {
        session_id: String,
    },
    Error {
        session_id: String,
        message: String,
    },
    Fallback {
        fallback: bool,
        reason: String,
    },
    TaskList(Vec<TaskInfo>),
    Usage {
        session_id: String,
        prompt_tokens: u32,
        completion_tokens: u32,
        total_tokens: u32,
    },
    /// SubAgent parallel batch status update from Gateway polling.
    SubAgentBatch(String, serde_json::Value),
    PlanReady(Plan),
    PlanStepBegin {
        session_id: String,
        step_id: String,
        #[allow(dead_code)]
        tool_name: String,
    },
    PlanStepEnd {
        session_id: String,
        step_id: String,
        success: bool,
    },
    /// A plan step was skipped by user request.
    #[allow(dead_code)]
    PlanSkip {
        step_id: String,
    },
    /// Retry a failed plan step.
    #[allow(dead_code)]
    PlanRetry {
        step_id: String,
    },
    /// Notification that a plan step has been skipped (from wire).
    PlanStepSkipped {
        session_id: String,
        step_id: String,
    },
    /// Cron task list refreshed from backend store.
    CronList(Vec<clarity_core::background::cron::CronTask>),
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
    #[allow(dead_code)]
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
    /// Single subagent stage update (e.g. "Planning", "Executing").
    SubagentStage {
        agent_id: String,
        name: String,
    },
    /// Single subagent output chunk.
    SubagentOutput {
        agent_id: String,
        text: String,
    },
    /// Single subagent status change (Pending → Running → Completed/Failed).
    SubagentStatus {
        agent_id: String,
        agent_type: String,
        status: String,
    },
    /// Single subagent finished (success or failure).
    SubagentComplete {
        agent_id: String,
        success: bool,
    },
    /// Single subagent budget progress update.
    SubagentProgress {
        agent_id: String,
        steps: usize,
        max_steps: usize,
    },
    /// Gateway health check result.
    GatewayHealth(GatewayStatus),
    /// Snapshot restore operation completed (async callback).
    SnapshotRestored {
        id: usize,
        success: bool,
        error: Option<String>,
    },
    /// Task result loaded from background store (async callback).
    #[allow(dead_code)]
    TaskResultLoaded {
        task_id: String,
        result: clarity_core::background::TaskResult,
    },
    /// Direct shell execution (!cmd) completed.
    ShellResult {
        session_id: String,
        command: String,
        output: String,
        exit_code: i32,
    },
    /// OKF knowledge bundle finished loading.
    KnowledgeLoaded {
        /// Requested bundle path.
        path: String,
        /// Loaded bundle on success, error message on failure.
        result: Result<clarity_core::okf::OkfBundle, String>,
    },
}

/// Transient draft indicator state for the current agent turn.
///
/// This is intentionally separate from `Message` so the UI team can decide
/// later whether to render it inside the agent bubble, above the composer,
/// or in a side panel — without touching event/handler code.
#[derive(Clone, Debug, Default)]
pub enum DraftStatus {
    /// No draft indicator is currently shown.
    #[default]
    None,
    /// Model is still preparing a response. `text` is a short label such as
    /// "thinking...", "analyzing...", or "searching...".
    Progress { text: String },
    /// Optional reasoning content emitted by the model before the final answer.
    /// The UI may choose to show this inline, collapsed, or not at all.
    Content { text: String },
}

/// Progress summary for a parallel batch of subagents.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct SubAgentProgress {
    pub batch_id: String,
    pub total: usize,
    pub completed: usize,
    pub failed: usize,
    pub status: String,
    pub last_poll: std::time::Instant,
}

/// Live progress for a single subagent invoked via /coder or /explore.
#[derive(Clone, Debug)]
pub struct SingleSubagentProgress {
    pub agent_type: String,
    pub status: String,
    pub stages: Vec<String>,
    pub output_lines: Vec<String>,
    pub started_at: std::time::Instant,
    pub completed_at: Option<std::time::Instant>,
    /// Budget progress: steps taken.
    pub steps: usize,
    /// Budget progress: maximum allowed steps.
    pub max_steps: usize,
}

/// Live tracker for an executing plan.
#[derive(Clone, Debug)]
pub struct PlanExecutionTracker {
    pub title: String,
    pub steps: Vec<PlanStepTracker>,
}

/// Holds plan step tracker state.
#[derive(Clone, Debug)]
pub struct PlanStepTracker {
    pub id: String,
    pub description: String,
    pub status: PlanStepStatus,
}

/// Lifecycle status variants for plan step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PlanStepStatus {
    Pending,
    Running,
    Success,
    Failed,
    Skipped,
}

/// Code-change statistics for a completed session.
///
/// Computed after the agent finishes executing tools that modify files.
/// Displayed as a `+N -M` badge in the session history list, matching the
/// convention used by Claude Code Web and other AI code editors.
#[derive(Clone, Default)]
#[allow(dead_code)] // files_changed/computed_at reserved for future diff-stat badges
pub struct DiffStats {
    /// Number of distinct files touched by file_write / file_edit tools.
    pub files_changed: usize,
    /// Total lines added across all patches in this session.
    pub lines_added: usize,
    /// Total lines removed across all patches in this session.
    pub lines_removed: usize,
    /// Timestamp (ms) when these stats were last computed.  Used to detect
    /// whether a re-computation is needed after a session is updated.
    pub computed_at: u64,
}

/// Holds session state.
#[derive(Clone)]
pub struct Session {
    pub id: String,
    pub title: String,
    pub category: String,
    /// Optional project binding.
    pub project_id: Option<String>,
    /// Conversation context that drives the Bot bar and right rail.
    pub context: SessionContext,
    /// Lifecycle category.
    pub lifecycle: SessionLifecycle,
    /// Whether the session is archived.
    pub archived: bool,
    pub messages: Vec<Message>,
    pub updated_at: u64,
    /// Cached heights for aggregated agent turns.
    pub turn_heights: Vec<Option<f32>>,
    /// Opaque provider-side state blobs, keyed by provider id.
    /// Clarity does not interpret the contents; stateful providers use this to
    /// resume server-side sessions across app restarts.
    pub provider_state: HashMap<String, String>,
    /// Runtime-only flag: true while this session is waiting for a streamed
    /// response. Never persisted; used to keep per-session turn state.
    pub in_flight: bool,
    /// Diff stats computed after the last completed turn.
    pub diff_stats: Option<DiffStats>,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("id", &self.id)
            .field("title", &self.title)
            .field("category", &self.category)
            .field("project_id", &self.project_id)
            .field("archived", &self.archived)
            .field("messages", &self.messages.len())
            .field("updated_at", &self.updated_at)
            .finish_non_exhaustive()
    }
}

/// content block variants.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ContentBlock {
    Text {
        text: String,
    },
    Code {
        language: String,
        code: String,
    },
    ToolResult {
        name: String,
        args: Option<String>,
        output: String,
        truncated: bool,
    },
    ToolCall {
        #[serde(default)]
        id: String,
        name: String,
        args: String,
    },
    #[allow(dead_code)]
    Think {
        steps: Vec<String>,
    },
    Plan {
        title: String,
        steps: Vec<String>,
    },
    FilePreview {
        path: String,
        content: String,
    },
}

/// Holds message state.
#[derive(Clone, Debug)]
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
    /// S6 Phase-2C line-atoms representation (parallel to `parsed`).
    /// Populated by `prepare()` via `markdown_to_lines`.
    pub lines: Vec<clarity_core::ui::RenderLine>,
}

impl Message {
    /// Cold path: sync content from blocks (if any), then parse markdown into cached blocks.
    pub fn prepare(&mut self) {
        if !self.blocks.is_empty() {
            self.content = Self::blocks_to_markdown(&self.blocks);
        }
        self.parsed = crate::ui::markdown::parse_markdown(&self.content);
        self.lines = clarity_core::ui::markdown_to_lines(&self.content);
        // Map default AgentMessage → UserMessage for user-authored content.
        if self.role == Role::User {
            for line in &mut self.lines {
                if let clarity_core::ui::RenderLine::Text { role, .. } = line {
                    if *role == clarity_core::ui::LineRole::AgentMessage {
                        *role = clarity_core::ui::LineRole::UserMessage;
                    }
                }
            }
        }
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

/// role variants.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Role {
    User,
    Agent,
}

/// Lifecycle status variants for agent.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum AgentStatus {
    Online,
    Busy,
    Unconfigured,
    Offline,
}

/// Lifecycle status variants for gateway.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum GatewayStatus {
    Online,
    Offline,
    Checking,
}

// ============================================================================
// Session / Project context (S6 Phase D)
// ============================================================================

/// Device affinity for a Claw session.
///
/// Determines which remote device a Claw session is bound to.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DeviceAffinity {
    /// Any online device may handle the session.
    #[default]
    AnyOnline,
    /// The session is pinned to a specific device id.
    Specific(String),
}

/// Conversation context that drives the Bot bar and right rail panels.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum SessionContext {
    /// Plain conversation without project or device binding.
    #[default]
    Chat,
    /// Work-bound conversation.
    Work {
        /// Workspace / project identifier.
        workspace_id: Option<String>,
        /// Whether the workspace has a local compute environment.
        has_workspace: bool,
    },
    /// Claw remote-device conversation.
    Claw {
        /// Claw role used for this session (e.g. "operator").
        role: String,
        /// Session key used for Gateway history / send routing.
        session_key: String,
        /// Device affinity: which device should handle the session.
        affinity: DeviceAffinity,
    },
}

impl serde::Serialize for SessionContext {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            SessionContext::Chat => {
                serde_json::json!({"Chat": serde_json::Value::Null}).serialize(serializer)
            }
            SessionContext::Work {
                workspace_id,
                has_workspace,
            } => serde_json::json!({
                "Work": {
                    "workspace_id": workspace_id,
                    "has_workspace": has_workspace,
                }
            })
            .serialize(serializer),
            SessionContext::Claw {
                role,
                session_key,
                affinity,
            } => serde_json::json!({
                "Claw": {
                    "role": role,
                    "session_key": session_key,
                    "affinity": affinity,
                }
            })
            .serialize(serializer),
        }
    }
}

impl<'de> serde::Deserialize<'de> for SessionContext {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;
        let value = serde_json::Value::deserialize(deserializer)?;
        let obj = value
            .as_object()
            .ok_or_else(|| D::Error::custom("expected object"))?;

        // Plain chat context.
        if obj.contains_key("Chat") {
            return Ok(SessionContext::Chat);
        }

        // Work context.
        if let Some(work) = obj.get("Work") {
            let workspace_id = work
                .get("workspace_id")
                .and_then(|v| v.as_str())
                .map(String::from);
            let has_workspace = work
                .get("has_workspace")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            return Ok(SessionContext::Work {
                workspace_id,
                has_workspace,
            });
        }

        // Legacy "Project" variant → Work.
        if let Some(project) = obj.get("Project") {
            let workspace_id = project
                .get("project_id")
                .and_then(|v| v.as_str())
                .map(String::from);
            let has_workspace = project
                .get("has_workspace")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            return Ok(SessionContext::Work {
                workspace_id,
                has_workspace,
            });
        }

        // Claw with backward-compatible defaults for missing role / session_key / affinity.
        if let Some(claw) = obj.get("Claw") {
            let role = claw
                .get("role")
                .and_then(|v| v.as_str())
                .map(String::from)
                .unwrap_or_else(|| "operator".to_string());
            let device_id = claw
                .get("device_id")
                .and_then(|v| v.as_str())
                .map(String::from);
            let session_key = claw
                .get("session_key")
                .and_then(|v| v.as_str())
                .map(String::from)
                .or_else(|| device_id.clone())
                .unwrap_or_default();
            let affinity = if let Some(aff) = claw.get("affinity") {
                serde_json::from_value(aff.clone()).map_err(D::Error::custom)?
            } else {
                device_id.map(DeviceAffinity::Specific).unwrap_or_default()
            };
            return Ok(SessionContext::Claw {
                role,
                session_key,
                affinity,
            });
        }

        Err(D::Error::custom(format!(
            "unknown SessionContext variant: {value}"
        )))
    }
}

/// Session lifecycle category.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[allow(dead_code)]
pub enum SessionLifecycle {
    /// Temporary problem-oriented chat.
    #[default]
    Temporary,
    /// Bound to a project lifecycle.
    ProjectBound,
    /// Long-lived, bound to the user (e.g. Claw device).
    UserBound,
}

/// Project descriptor.
#[derive(Clone, Debug)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub archived: bool,
    /// Whether the project has a local workspace (API + compute tools available).
    pub has_workspace: bool,
}

/// Holds tool call info state.
#[derive(Clone)]
pub struct ToolCallInfo {
    pub id: String,
    pub name: String,
    pub status: ToolCallStatus,
    pub result: Option<String>,
}

/// Lifecycle status variants for tool call.
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

/// Holds attachment state.
#[derive(Clone, Debug)]
pub struct Attachment {
    pub path: std::path::PathBuf,
    pub name: String,
}

/// A context item collected via `#` quick-add in the input field.
/// Displayed as a chip/tag and injected as system context before the user
/// message is sent to the LLM.
#[derive(Clone, Debug)]
#[allow(dead_code)] // Reserved: context picker widget wiring in progress
pub struct ContextItem {
    pub source: ContextSource,
    /// Short display label (e.g. "main.rs:42-58").
    pub display: String,
    /// Actual content injected into the prompt.
    pub payload: String,
}

/// Origin of a context item collected via `#` quick-add.
#[derive(Clone, Debug)]
#[allow(dead_code)] // Reserved: context picker widget wiring in progress
pub enum ContextSource {
    /// A file (optionally with line range).
    File {
        path: String,
        start_line: Option<usize>,
        end_line: Option<usize>,
    },
    /// A code symbol (function, class, etc.) identified by LSP.
    Code { symbol: String, file: String },
    /// All content from a directory.
    Folder { path: String },
    /// Terminal command output.
    Terminal { command: String },
    /// Web page content.
    Web { url: String },
    // === Extension points (reserved for future backend features) ===
    /// Documentation reference. Reserved.
    Documentation { url: String },
    /// Semantic codebase search result. Reserved.
    Codebase { query: String },
    /// Git diff against a base branch. Reserved for GitHub integration.
    GitDiff { base_branch: String },
}

/// Holds toast state.
#[derive(Clone, Debug)]
pub struct Toast {
    pub message: String,
    pub level: ToastLevel,
    pub created_at: std::time::Instant,
}

/// toast level variants.
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

/// inline span variants.
#[derive(Clone, Debug)]
pub enum InlineSpan {
    Text(String),
    Bold(String),
    Code(String),
    Link { text: String, url: String },
}

/// render block variants.
#[derive(Clone, Debug)]
pub enum RenderBlock {
    Paragraph(Vec<InlineSpan>),
    Heading(u8, Vec<InlineSpan>),
    CodeBlock {
        lang: String,
        code: String,
    },
    ListItem(Vec<InlineSpan>),
    Blockquote(Vec<InlineSpan>),
    HorizontalRule,
    /// Markdown table: headers + rows of cells.
    Table {
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
    },
}

// ============================================================================
// Preview — reused glass-card preview in chat area for files and web pages
// ============================================================================

/// preview item variants.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum PreviewItem {
    File {
        name: String,
        content: String,
        path: String,
    },
    WebPage {
        title: String,
        url: String,
        content: String,
    },
}

/// Holds web tab state.
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

    #[test]
    fn session_context_default_is_chat() {
        let ctx = SessionContext::default();
        assert_eq!(ctx, SessionContext::Chat);
    }

    #[test]
    fn session_lifecycle_default_is_temporary() {
        assert_eq!(SessionLifecycle::default(), SessionLifecycle::Temporary);
    }

    #[test]
    fn project_has_expected_fields() {
        let p = Project {
            id: "p-1".into(),
            name: "ui refactor".into(),
            archived: false,
            has_workspace: true,
        };
        assert_eq!(p.name, "ui refactor");
        assert!(p.has_workspace);
        assert!(!p.archived);
    }

    #[test]
    fn session_context_work_roundtrips() {
        let ctx = SessionContext::Work {
            workspace_id: Some("ws-1".into()),
            has_workspace: true,
        };
        let json = serde_json::to_string(&ctx).unwrap();
        let restored: SessionContext = serde_json::from_str(&json).unwrap();
        assert_eq!(ctx, restored);
    }

    #[test]
    fn session_context_claw_roundtrips() {
        let ctx = SessionContext::Claw {
            role: "operator".into(),
            session_key: "sess-key-1".into(),
            affinity: DeviceAffinity::Specific("dev-1".into()),
        };
        let json = serde_json::to_string(&ctx).unwrap();
        let restored: SessionContext = serde_json::from_str(&json).unwrap();
        assert_eq!(ctx, restored);
    }

    #[test]
    fn session_context_legacy_project_deserializes_to_work() {
        let json = r#"{"Project":{"project_id":"p-legacy","has_workspace":false}}"#;
        let restored: SessionContext = serde_json::from_str(json).unwrap();
        assert_eq!(
            restored,
            SessionContext::Work {
                workspace_id: Some("p-legacy".into()),
                has_workspace: false,
            }
        );
    }

    #[test]
    fn session_context_legacy_claw_defaults_fields() {
        let json = r#"{"Claw":{"device_id":"d-legacy"}}"#;
        let restored: SessionContext = serde_json::from_str(json).unwrap();
        assert_eq!(
            restored,
            SessionContext::Claw {
                role: "operator".into(),
                session_key: "d-legacy".into(),
                affinity: DeviceAffinity::Specific("d-legacy".into()),
            }
        );
    }

    #[test]
    fn session_context_chat_roundtrips() {
        let ctx = SessionContext::Chat;
        let json = serde_json::to_string(&ctx).unwrap();
        let restored: SessionContext = serde_json::from_str(&json).unwrap();
        assert_eq!(ctx, restored);
    }
}
