//! Shell execution tools: Bash and PowerShell

use async_trait::async_trait;
use serde_json::json;
use serde_json::Value;
use std::process::Stdio;
use tokio::process::Command;
use tokio::time::{timeout, Duration};
use tracing::{debug, error, warn};

use crate::approval::ApprovalMode;
use crate::error::ToolError;
use crate::tools::file::is_sensitive_file;
use crate::tools::helpers;
use crate::tools::{Tool, ToolContext, ToolResult};

/// Best-effort scan of a shell command string for references to sensitive files.
fn detect_sensitive_in_command(command: &str) -> Option<String> {
    for token in command.split_whitespace() {
        let trimmed = token.trim_matches(|c| c == '"' || c == '\'');
        if !trimmed.is_empty() {
            let path = std::path::Path::new(trimmed);
            if is_sensitive_file(path) {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

/// Result of a shell command execution
#[derive(Debug)]
pub struct ShellResult {
    /// Exit code (0 for success)
    pub exit_code: i32,
    /// Standard output
    pub stdout: String,
    /// Standard error
    pub stderr: String,
    /// Whether the command timed out
    pub timed_out: bool,
}

/// Tool for executing Bash commands (Linux/macOS/WSL)
pub struct BashTool;

impl BashTool {
    /// Create a new BashTool instance
    pub fn new() -> Self {
        Self
    }

    /// Execute a bash command
    async fn execute_bash(
        &self,
        command: &str,
        working_dir: &std::path::Path,
        env: &std::collections::HashMap<String, String>,
        timeout_secs: u64,
    ) -> ToolResult<ShellResult> {
        debug!("Executing bash command: {}", command);

        let mut cmd = Command::new("bash");
        cmd.arg("-c")
            .arg(command)
            .current_dir(working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Set environment variables
        for (key, value) in env {
            cmd.env(key, value);
        }

        let result = timeout(Duration::from_secs(timeout_secs), cmd.output()).await;

        match result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let exit_code = output.status.code().unwrap_or(-1);

                Ok(ShellResult {
                    exit_code,
                    stdout,
                    stderr,
                    timed_out: false,
                })
            }
            Ok(Err(e)) => {
                error!("Failed to execute bash: {}", e);
                Err(ToolError::execution_failed(format!(
                    "Failed to execute: {}",
                    e
                )))
            }
            Err(_) => {
                warn!("Bash command timed out after {} seconds", timeout_secs);
                Err(ToolError::Timeout(timeout_secs))
            }
        }
    }
}

impl Default for BashTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

    fn description(&self) -> &str {
        "Execute a bash shell command. Returns exit code, stdout, and stderr. \
         Commands run in the current working directory with access to environment variables."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The bash command to execute"
                },
                "timeout": {
                    "type": "integer",
                    "description": "Timeout in seconds (default: 30, max: 300)",
                    "minimum": 1,
                    "maximum": 300
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, args: Value, ctx: ToolContext) -> ToolResult<Value> {
        let command = helpers::required_str(&args, "command")?;
        let timeout_secs = args
            .get("timeout")
            .and_then(|v| v.as_u64())
            .map(|v| v.min(300))
            .unwrap_or(ctx.timeout_secs);

        let sensitive = detect_sensitive_in_command(command);

        let result = self
            .execute_bash(command, &ctx.working_dir, &ctx.env, timeout_secs)
            .await?;

        let mut value = json!({
            "exit_code": result.exit_code,
            "stdout": result.stdout,
            "stderr": result.stderr,
            "timed_out": result.timed_out,
            "success": result.exit_code == 0
        });

        if let Some(ref path) = sensitive {
            if ctx.approval_mode == ApprovalMode::Yolo {
                tracing::warn!("Sensitive file access in bash command (YOLO): {}", path);
                if let Some(obj) = value.as_object_mut() {
                    obj.insert(
                        "sensitive_file_warning".to_string(),
                        json!(format!("Command references sensitive file: {}", path)),
                    );
                }
            }
        }

        Ok(value)
    }
}

/// Tool for executing PowerShell commands (Windows)
pub struct PowerShellTool;

impl PowerShellTool {
    /// Create a new PowerShellTool instance
    pub fn new() -> Self {
        Self
    }

    /// Execute a PowerShell command
    async fn execute_powershell(
        &self,
        command: &str,
        working_dir: &std::path::Path,
        env: &std::collections::HashMap<String, String>,
        timeout_secs: u64,
    ) -> ToolResult<ShellResult> {
        debug!("Executing PowerShell command: {}", command);

        let mut cmd = Command::new("powershell.exe");
        cmd.arg("-NoProfile")
            .arg("-ExecutionPolicy")
            .arg("Bypass")
            .arg("-Command")
            .arg(command)
            .current_dir(working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Set environment variables
        for (key, value) in env {
            cmd.env(key, value);
        }

        let result = timeout(Duration::from_secs(timeout_secs), cmd.output()).await;

        match result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let exit_code = output.status.code().unwrap_or(-1);

                Ok(ShellResult {
                    exit_code,
                    stdout,
                    stderr,
                    timed_out: false,
                })
            }
            Ok(Err(e)) => {
                error!("Failed to execute PowerShell: {}", e);
                Err(ToolError::execution_failed(format!(
                    "Failed to execute: {}",
                    e
                )))
            }
            Err(_) => {
                warn!(
                    "PowerShell command timed out after {} seconds",
                    timeout_secs
                );
                Err(ToolError::Timeout(timeout_secs))
            }
        }
    }
}

impl Default for PowerShellTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for PowerShellTool {
    fn name(&self) -> &str {
        "powershell"
    }

    fn description(&self) -> &str {
        "Execute a PowerShell command. Returns exit code, stdout, and stderr. \
         Commands run in the current working directory with access to environment variables."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The PowerShell command to execute"
                },
                "timeout": {
                    "type": "integer",
                    "description": "Timeout in seconds (default: 30, max: 300)",
                    "minimum": 1,
                    "maximum": 300
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, args: Value, ctx: ToolContext) -> ToolResult<Value> {
        let command = helpers::required_str(&args, "command")?;
        let timeout_secs = args
            .get("timeout")
            .and_then(|v| v.as_u64())
            .map(|v| v.min(300))
            .unwrap_or(ctx.timeout_secs);

        let sensitive = detect_sensitive_in_command(command);

        let result = self
            .execute_powershell(command, &ctx.working_dir, &ctx.env, timeout_secs)
            .await?;

        let mut value = json!({
            "exit_code": result.exit_code,
            "stdout": result.stdout,
            "stderr": result.stderr,
            "timed_out": result.timed_out,
            "success": result.exit_code == 0
        });

        if let Some(ref path) = sensitive {
            if ctx.approval_mode == ApprovalMode::Yolo {
                tracing::warn!(
                    "Sensitive file access in PowerShell command (YOLO): {}",
                    path
                );
                if let Some(obj) = value.as_object_mut() {
                    obj.insert(
                        "sensitive_file_warning".to_string(),
                        json!(format!("Command references sensitive file: {}", path)),
                    );
                }
            }
        }

        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    #[cfg_attr(target_os = "windows", ignore = "bash not available on Windows")]
    async fn test_bash_echo() {
        let tool = BashTool::new();
        let ctx = ToolContext::new();

        let args = json!({"command": "echo 'Hello World'"});
        let result = tool.execute(args, ctx).await.unwrap();

        assert_eq!(result["exit_code"], 0);
        assert!(result["stdout"].as_str().unwrap().contains("Hello World"));
        assert!(result["success"].as_bool().unwrap());
    }

    #[tokio::test]
    #[cfg_attr(target_os = "windows", ignore = "bash not available on Windows")]
    async fn test_bash_with_working_dir() {
        let temp_dir = TempDir::new().unwrap();
        let tool = BashTool::new();
        let ctx = ToolContext::new().with_working_dir(temp_dir.path());

        let args = json!({"command": "pwd"});
        let result = tool.execute(args, ctx).await.unwrap();

        assert_eq!(result["exit_code"], 0);
    }

    #[tokio::test]
    #[cfg(target_os = "windows")]
    async fn test_powershell_echo() {
        let tool = PowerShellTool::new();
        let ctx = ToolContext::new();

        let args = json!({"command": "Write-Output 'Hello World'"});
        let result = tool.execute(args, ctx).await.unwrap();

        assert_eq!(result["exit_code"], 0);
        assert!(result["stdout"].as_str().unwrap().contains("Hello World"));
        assert!(result["success"].as_bool().unwrap());
    }

    #[tokio::test]
    #[cfg(target_os = "windows")]
    async fn test_powershell_sensitive_file_yolo_warning() {
        use tokio::fs;
        let temp_dir = TempDir::new().unwrap();
        let tool = PowerShellTool::new();
        let ctx = ToolContext::new()
            .with_working_dir(temp_dir.path())
            .with_approval_mode(ApprovalMode::Yolo);

        let sensitive_path = temp_dir.path().join(".env");
        fs::write(&sensitive_path, "secret").await.unwrap();

        let args = json!({"command": format!("Get-Content '{}'", sensitive_path.display())});
        let result = tool.execute(args, ctx).await.unwrap();
        assert!(result["sensitive_file_warning"].as_str().is_some());
    }
}
