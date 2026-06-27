//! Tool trait and execution context for the Clarity contract layer.
//!
//! These types define the interface between the Agent and tool implementations.
//! They are designed to be implementation-agnostic and shared across all
//! crates in the workspace.

use crate::{CapabilityToken, ToolError};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

/// Approval mode for tool execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ApprovalMode {
    /// Interactive mode - wait for user confirmation
    #[default]
    Interactive,
    /// Smart mode - remember approvals per tool
    Smart,
    /// Plan mode - approve a batch of tools at once
    Plan,
    /// Yolo mode - no approval required
    Yolo,
}

/// Context passed to tools during execution.
///
/// Contains information about the current execution environment,
/// working directory, and shared resources.
#[derive(Debug, Clone)]
pub struct ToolContext {
    /// Current working directory for the operation
    pub working_dir: PathBuf,
    /// Environment variables
    pub env: HashMap<String, String>,
    /// Request timeout in seconds
    pub timeout_secs: u64,
    /// Maximum output size (bytes)
    pub max_output_size: usize,
    /// Whether the operation is read-only
    pub read_only: bool,
    /// Current approval mode
    pub approval_mode: ApprovalMode,
    /// Optional capability token for permission isolation
    pub capability_token: Option<CapabilityToken>,
}

impl ToolContext {
    /// Create a new tool context with default settings.
    pub fn new() -> Self {
        Self {
            working_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            env: std::env::vars().collect(),
            timeout_secs: 60,
            max_output_size: 1024 * 1024, // 1MB
            read_only: false,
            approval_mode: ApprovalMode::Interactive,
            capability_token: None,
        }
    }

    /// Set the working directory.
    pub fn with_working_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.working_dir = path.into();
        self
    }

    /// Set the timeout.
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    /// Set read-only mode.
    pub fn with_read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    /// Add an environment variable.
    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    /// Set the approval mode.
    pub fn with_approval_mode(mut self, mode: ApprovalMode) -> Self {
        self.approval_mode = mode;
        self
    }

    /// Set the capability token.
    pub fn with_capability_token(mut self, token: Option<CapabilityToken>) -> Self {
        self.capability_token = token;
        self
    }
}

impl Default for ToolContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Tool trait — implement this to add new capabilities to the agent.
///
/// This trait lives in the contract layer so that downstream crates
/// can define tools without depending on `clarity-core`.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Tool name — must be unique within a registry.
    fn name(&self) -> &str;

    /// Human-readable description for LLM.
    fn description(&self) -> &str;

    /// JSON Schema for tool parameters.
    ///
    /// This should follow the JSON Schema specification and describe
    /// all parameters the tool accepts.
    fn parameters(&self) -> Value;

    /// Execute the tool with the given arguments.
    ///
    /// # Arguments
    ///
    /// * `args` - JSON Value containing the tool parameters
    /// * `ctx` - Execution context (working dir, env vars, etc.)
    ///
    /// # Returns
    ///
    /// The result of the execution as a JSON Value.
    async fn execute(&self, args: Value, ctx: ToolContext) -> Result<Value, ToolError>;

    /// Whether this tool requires explicit user approval regardless of global approval mode.
    ///
    /// Tools that directly interact with the OS GUI should return `true`.
    /// The default is `false`.
    fn requires_approval(&self) -> bool {
        false
    }

    /// Runtime readiness check — returns `None` if the tool is ready to execute,
    /// or `Some(reason)` if a required dependency is missing.
    ///
    /// Called by `ToolRegistry::self_check()` after registration.
    /// The default implementation always returns `None` (always ready).
    fn check_readiness(&self) -> Option<String> {
        None
    }

    /// Maximum characters of output to inject into the LLM context.
    ///
    /// Returns `None` to use the global [`AgentConfig::max_tool_result_chars`]
    /// default (30,000). Tools that produce known-large outputs (e.g. `file_read`
    /// on large files) should return a tighter limit.
    ///
    /// Wire-level delivery to frontends is **never** affected — only the
    /// version the LLM sees is limited.
    fn max_output_chars(&self) -> Option<usize> {
        None
    }
}

/// Type-erased tool wrapper for storage in collections.
pub type BoxedTool = Box<dyn Tool>;

/// Shared tool reference (for concurrency).
pub type SharedTool = Arc<dyn Tool>;

/// Helper trait to convert tools to shared references.
pub trait IntoSharedTool: Tool + Sized
where
    Self: 'static,
{
    /// Wrap this tool in an `Arc<dyn Tool>`.
    fn into_shared(self) -> SharedTool {
        Arc::new(self)
    }
}

impl<T: Tool + Sized + 'static> IntoSharedTool for T {}

// ============================================================================
// ToolRegistry trait (uplifted from clarity-core to break subagents↔core cycle)
// ============================================================================

use crate::error::{AgentError, ToolResult};

/// Abstract interface for tool registries.
///
/// Implemented by `clarity_core::registry::ToolRegistry` and by lightweight
/// filtered views (e.g. `FilteredToolRegistry` in `clarity-subagents`).
#[async_trait]
pub trait ToolRegistry: Send + Sync {
    /// Look up a tool by name.
    fn get(&self, name: &str) -> Result<Option<SharedTool>, AgentError>;

    /// Execute a tool with the given arguments and context.
    async fn execute(
        &self,
        name: &str,
        args: serde_json::Value,
        ctx: ToolContext,
    ) -> ToolResult<serde_json::Value>;

    /// List all registered tool names.
    fn list(&self) -> Result<Vec<String>, AgentError>;

    /// Check whether a tool name is registered.
    fn contains(&self, name: &str) -> Result<bool, AgentError>;

    /// Number of registered tools.
    fn len(&self) -> Result<usize, AgentError>;

    /// Convenience: `len() == 0`.
    fn is_empty(&self) -> Result<bool, AgentError> {
        Ok(self.len()? == 0)
    }
}
