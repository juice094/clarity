//! Rollout persistence types.
//!
//! Rollouts are append-only JSONL event logs that form the durable replay history
//! of a thread. The vocabulary is inspired by the OpenAI Codex `codex_protocol`
//! and `codex_rollout` crates (Apache-2.0); the types are original to Clarity.
//! See `crates/clarity-contract/NOTICES.md` for attribution.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::thread::{SessionId, ThreadId};

/// Classification of the runtime entry point that created a session.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SessionSource {
    /// Source is not known or not recorded.
    #[default]
    Unknown,
    /// Plain CLI invocation.
    Cli,
    /// Terminal UI.
    Tui,
    /// Web / app server.
    AppServer,
    /// Language-server protocol integration.
    Lsp,
    /// Automated test.
    Test,
}

/// Classification of how a thread came to exist.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ThreadSource {
    /// Origin is not known or not recorded.
    #[default]
    Unknown,
    /// Created from scratch.
    New,
    /// Resumed from a previous rollout.
    Resumed,
    /// Forked from another thread.
    Forked,
}

/// Session-level metadata written as the first line of every rollout file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionMeta {
    /// Persistent thread identifier.
    pub id: ThreadId,
    /// Thread this session was forked from, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub forked_from_id: Option<ThreadId>,
    /// Parent thread in a sub-agent spawn graph, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_thread_id: Option<ThreadId>,
    /// ISO-8601 timestamp of session creation.
    pub timestamp: String,
    /// Working directory at session creation.
    pub cwd: PathBuf,
    /// Process or user that originated the session.
    pub originator: String,
    /// Version of the Clarity binary that created the session.
    pub cli_version: String,
    /// Runtime source of the session.
    #[serde(default)]
    pub source: SessionSource,
    /// Optional analytics source classification for this thread.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_source: Option<ThreadSource>,
    /// Optional nickname assigned to a spawned sub-agent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_nickname: Option<String>,
    /// Optional role assigned to a spawned sub-agent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_role: Option<String>,
    /// Optional canonical agent path assigned to a spawned sub-agent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_path: Option<String>,
    /// Model provider identifier used for the session.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_provider: Option<String>,
    /// Base instructions object for the session, stored as opaque JSON.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_instructions: Option<serde_json::Value>,
    /// Dynamic tool specifications registered for the session.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic_tools: Option<Vec<serde_json::Value>>,
    /// Memory mode string, if configured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_mode: Option<String>,
    /// Multi-agent protocol version, if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub multi_agent_version: Option<String>,
    /// User identifier for attribution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    /// Team identifier for team-scoped sessions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team_id: Option<String>,
    /// Organization identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub org_id: Option<String>,
}

impl Default for SessionMeta {
    fn default() -> Self {
        Self {
            id: ThreadId::default(),
            forked_from_id: None,
            parent_thread_id: None,
            timestamp: String::new(),
            cwd: PathBuf::new(),
            originator: String::new(),
            cli_version: String::new(),
            source: SessionSource::default(),
            thread_source: None,
            agent_nickname: None,
            agent_role: None,
            agent_path: None,
            model_provider: None,
            base_instructions: None,
            dynamic_tools: None,
            memory_mode: None,
            multi_agent_version: None,
            user_id: None,
            team_id: None,
            org_id: None,
        }
    }
}

/// Session metadata plus optional git context, flattened for JSONL serialization.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionMetaLine {
    /// Core session metadata.
    #[serde(flatten)]
    pub meta: SessionMeta,
    /// Optional git repository context at session creation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git: Option<GitInfo>,
}

/// Git repository context recorded at session creation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GitInfo {
    /// Current commit hash, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_hash: Option<String>,
    /// Current branch name, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// Repository remote URL, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository_url: Option<String>,
}

/// A minimal, Clarity-specific subset of response items that can be persisted in
/// rollouts. The full Codex `ResponseItem` enum contains many more variants;
/// this subset captures the items required for replay and metadata extraction.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
#[allow(clippy::large_enum_variant)]
pub enum RolloutResponseItem {
    /// A plain text message.
    Message {
        /// Message role.
        role: String,
        /// Text content.
        content: String,
    },
    /// A function/tool call requested by the assistant.
    FunctionCall {
        /// Tool name.
        name: String,
        /// JSON-encoded arguments.
        arguments: String,
    },
    /// The result of a function/tool call.
    FunctionCallOutput {
        /// Tool call identifier.
        call_id: String,
        /// Result content.
        output: String,
    },
    /// A reasoning block produced by a reasoning model.
    Reasoning {
        /// Raw reasoning content.
        content: String,
    },
    /// A compaction marker.
    Compaction,
    /// A context-compaction marker.
    ContextCompaction,
    /// Any other response item, preserved as opaque JSON.
    Other(serde_json::Value),
}

/// A minimal subset of durable event messages.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
#[allow(clippy::large_enum_variant)]
pub enum RolloutEventMsg {
    /// User-provided input.
    UserMessage(String),
    /// Assistant-generated message.
    AgentMessage(String),
    /// Assistant reasoning content.
    AgentReasoning(String),
    /// Token count update.
    TokenCount(u64),
    /// A turn started.
    TurnStarted {
        /// Turn identifier.
        turn_id: Option<String>,
    },
    /// A turn completed.
    TurnComplete {
        /// Turn identifier.
        turn_id: Option<String>,
    },
    /// Thread goal was updated.
    ThreadGoalUpdated(String),
    /// Context was compacted.
    ContextCompacted {
        /// Turn identifier at the compaction boundary.
        turn_id: Option<String>,
    },
    /// A turn was aborted.
    TurnAborted {
        /// Turn identifier.
        turn_id: Option<String>,
        /// Abort reason.
        reason: Option<String>,
    },
    /// Turn lifecycle event for state-machine replay.
    Lifecycle {
        /// Event that drove the transition.
        event: crate::lifecycle::RunEvent,
        /// State after applying the event.
        state: crate::lifecycle::RunState,
    },
    /// Sub-agent activity notice.
    SubAgentActivity(serde_json::Value),
    /// Error event.
    Error(String),
    /// Any other event, preserved as opaque JSON.
    Other(serde_json::Value),
}

/// A single atom in a rollout file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
#[allow(clippy::large_enum_variant)]
pub enum RolloutItem {
    /// Session header.
    SessionMeta(SessionMetaLine),
    /// A model response item.
    ResponseItem(RolloutResponseItem),
    /// Durable inter-agent communication metadata.
    InterAgentCommunication(serde_json::Value),
    /// A compaction marker.
    Compacted(CompactedItem),
    /// Per-turn context snapshot.
    TurnContext(TurnContextItem),
    /// An event message.
    EventMsg(RolloutEventMsg),
}

/// A compaction marker inserted into a rollout when history is compressed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CompactedItem {
    /// Summary message that replaces the compressed prefix.
    pub message: String,
    /// Optional replacement history that re-establishes full context.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replacement_history: Option<Vec<RolloutResponseItem>>,
    /// Optional window identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_id: Option<u64>,
}

/// Per-turn context persisted to rollouts so that resume/fork can recover the
/// durable baseline.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TurnContextItem {
    /// Turn identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    /// Working directory for the turn.
    pub cwd: PathBuf,
    /// Model identifier used for the turn.
    pub model: String,
    /// Optional approval policy name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_policy: Option<String>,
    /// Optional sandbox policy name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox_policy: Option<String>,
    /// Optional permission profile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_profile: Option<serde_json::Value>,
    /// Optional configuration hash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comp_hash: Option<String>,
    /// Optional multi-agent version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multi_agent_version: Option<String>,
}

/// A JSONL line envelope: timestamp plus a rollout item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RolloutLine {
    /// ISO-8601 timestamp of the line.
    pub timestamp: String,
    /// Monotonic sequence number within the thread.
    ///
    /// ponytail: optional for backward compatibility with pre-Phase-5 rollout files;
    /// new writes always populate it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seq: Option<u64>,
    /// Device that produced this line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
    /// Logical clock of the originating device.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin_clock: Option<u64>,
    /// The rollout item payload.
    #[serde(flatten)]
    pub item: RolloutItem,
}

impl Default for RolloutLine {
    fn default() -> Self {
        Self {
            timestamp: String::new(),
            seq: None,
            device_id: None,
            origin_clock: None,
            // ponytail: placeholder used only for struct-update syntax in tests.
            item: RolloutItem::EventMsg(RolloutEventMsg::Other(serde_json::Value::Null)),
        }
    }
}

/// Parameters for creating a new thread rollout.
#[derive(Debug, Clone)]
pub struct CreateRolloutParams {
    /// Thread identifier.
    pub thread_id: ThreadId,
    /// Session identifier (same UUID as root thread by default).
    pub session_id: SessionId,
    /// Thread this rollout was forked from.
    pub forked_from_id: Option<ThreadId>,
    /// Parent thread in a sub-agent graph.
    pub parent_thread_id: Option<ThreadId>,
    /// Runtime source.
    pub source: SessionSource,
    /// Thread origin classification.
    pub thread_source: Option<ThreadSource>,
    /// Working directory.
    pub cwd: PathBuf,
    /// Process or user that originated the session.
    pub originator: String,
    /// Version of the Clarity binary that created the session.
    pub cli_version: String,
    /// Base instructions, if any.
    pub base_instructions: Option<serde_json::Value>,
    /// Dynamic tools, if any.
    pub dynamic_tools: Vec<serde_json::Value>,
    /// Model provider identifier.
    pub model_provider: Option<String>,
    /// Multi-agent protocol version, if applicable.
    pub multi_agent_version: Option<String>,
    /// User identifier for attribution.
    pub user_id: Option<String>,
    /// Team identifier for team-scoped sessions.
    pub team_id: Option<String>,
    /// Organization identifier.
    pub org_id: Option<String>,
    /// If true, do not write an initial `SessionMeta` line when creating the
    /// rollout file. Used when the caller will supply the initial items, such
    /// as during a fork.
    pub skip_initial_meta: bool,
}

/// Parameters for resuming an existing thread rollout.
#[derive(Debug, Clone)]
pub struct ResumeRolloutParams {
    /// Path to the existing rollout file.
    pub path: PathBuf,
}
