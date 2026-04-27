//! Shared UI types + Pretext cold-path prepare() for Message.
//!
//! ARCHITECTURE CONSTRAINT (Pretext-aligned):
//!   - `Message::prepare()` is the ONLY cold-path entry for markdown parsing.
//!   - `RenderBlock` / `InlineSpan` are the intermediate representation.
//!   - When adding new block types, update `estimate_height()` in main.rs.
//!
//! See `crates/clarity-egui/ARCHITECTURE.md` §1.1, §2.2.

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
}

#[derive(Clone)]
pub struct Session {
    pub id: String,
    pub title: String,
    pub messages: Vec<Message>,
    pub updated_at: u64,
}

#[derive(Clone)]
pub struct Message {
    pub role: Role,
    pub content: String,
    #[allow(dead_code)]
    pub timestamp: Instant,
    /// Pretext-style prepared blocks — parsed once when content changes.
    pub parsed: Vec<RenderBlock>,
    /// Cached bubble height from last render (for virtual list estimation).
    pub cached_height: Option<f32>,
}

impl Message {
    /// Cold path: parse markdown into cached blocks.
    pub fn prepare(&mut self) {
        self.parsed = crate::ui::markdown::parse_markdown(&self.content);
        // Invalidate height cache when content changes.
        self.cached_height = None;
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

#[derive(Clone)]
pub enum ToolCallStatus {
    Running,
    Done,
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
