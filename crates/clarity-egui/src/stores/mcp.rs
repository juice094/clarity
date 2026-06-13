//! MCP Store
//!
//! MCP server config panel

use std::time::Instant;

/// Holds mcp UI state.
pub struct McpStore {
    pub mcp_config: Option<clarity_core::mcp::config::McpConfig>,
    pub mcp_changed: bool,
    /// Names of currently connected MCP tools (for hot-reload unregister).
    pub connected_tools: Vec<String>,
    /// Last poll time for MCP config file watcher.
    pub last_mcp_poll: Instant,
    /// Last known mtime of mcp.json.
    pub last_mcp_mtime: Option<std::time::SystemTime>,
}
