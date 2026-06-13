#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    missing_docs,
    unsafe_code
)]
//! MCP Development Server Example
//!
//! Exposes Clarity workspace build/test/lint tools as an MCP server over stdio.
//!
//! ## Build
//! ```bash
//! cargo build --example mcp_dev_stdio_server -p clarity-mcp
//! ```
//!
//! ## Run (for Claude Code MCP registration)
//! ```bash
//! claude mcp add clarity-dev -- \
//!   .\target\debug\examples\mcp_dev_stdio_server.exe
//! ```

use clarity_mcp::{McpServer, McpTool, StdioMcpServer, ToolCallResult, ToolContent};
use serde_json::Value;
use std::process::Stdio;
use tokio::process::Command;
use tokio::time::{Duration, timeout};

struct DevMcpServer {
    workspace_root: std::path::PathBuf,
}

impl DevMcpServer {
    fn new() -> Self {
        let workspace_root =
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        Self { workspace_root }
    }

    async fn run_cargo(&self, args: &[&str]) -> Result<ToolCallResult, clarity_mcp::McpError> {
        let mut cmd = Command::new("cargo");
        cmd.args(args)
            .current_dir(&self.workspace_root)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let output = timeout(Duration::from_secs(300), cmd.output())
            .await
            .map_err(|_| {
                clarity_mcp::McpError::RequestFailed("cargo command timed out after 300s".into())
            })?
            .map_err(|e| {
                clarity_mcp::McpError::RequestFailed(format!("failed to spawn cargo: {}", e))
            })?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let text = format!(
            "Exit code: {}\n\n--- stdout ---\n{}\n\n--- stderr ---\n{}",
            output.status.code().unwrap_or(-1),
            stdout,
            stderr
        );

        Ok(ToolCallResult {
            content: vec![ToolContent::Text { text }],
            is_error: !output.status.success(),
        })
    }
}

#[async_trait::async_trait]
impl McpServer for DevMcpServer {
    fn name(&self) -> &str {
        "clarity-dev"
    }

    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    async fn list_tools(&self) -> Vec<McpTool> {
        vec![
            McpTool {
                name: "cargo_check".into(),
                description: Some("Run cargo check --workspace".into()),
                input_schema: serde_json::json!({ "type": "object", "properties": {} }),
            },
            McpTool {
                name: "cargo_test".into(),
                description: Some("Run cargo test --workspace --lib".into()),
                input_schema: serde_json::json!({ "type": "object", "properties": {} }),
            },
            McpTool {
                name: "cargo_clippy".into(),
                description: Some(
                    "Run cargo clippy --workspace --lib --bins --tests -- -D warnings".into(),
                ),
                input_schema: serde_json::json!({ "type": "object", "properties": {} }),
            },
            McpTool {
                name: "cargo_fmt_check".into(),
                description: Some("Run cargo fmt --all -- --check".into()),
                input_schema: serde_json::json!({ "type": "object", "properties": {} }),
            },
        ]
    }

    async fn call_tool(
        &self,
        name: &str,
        _args: Value,
    ) -> Result<ToolCallResult, clarity_mcp::McpError> {
        match name {
            "cargo_check" => self.run_cargo(&["check", "--workspace"]).await,
            "cargo_test" => self.run_cargo(&["test", "--workspace", "--lib"]).await,
            "cargo_clippy" => {
                self.run_cargo(&[
                    "clippy",
                    "--workspace",
                    "--lib",
                    "--bins",
                    "--tests",
                    "--",
                    "-D",
                    "warnings",
                ])
                .await
            }
            "cargo_fmt_check" => self.run_cargo(&["fmt", "--all", "--", "--check"]).await,
            _ => Err(clarity_mcp::McpError::RequestFailed(format!(
                "Unknown tool: {}",
                name
            ))),
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();

    let server = DevMcpServer::new();
    tracing::info!(
        "MCP Dev Server '{}' v{} starting on stdio (cwd: {:?})",
        server.name(),
        server.version(),
        server.workspace_root
    );

    StdioMcpServer::run(server).await;
    Ok(())
}
