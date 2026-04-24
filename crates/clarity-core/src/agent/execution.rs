//! Tool execution, approval flow, and context compaction.

use super::Agent;
use crate::approval::{ApprovalMode, ApprovalResponse, ApprovalRuntime, ApprovalSource};
use crate::compaction::estimate_message_tokens;
use crate::error::{AgentError, ToolError};
use crate::llm::api::Message;
use crate::tools::ToolContext;
use crate::types::ToolCall;
use serde_json::Value;
use std::sync::Arc;
use tracing::info;

impl Agent {
    /// Detect whether a tool call targets a sensitive file or path.
    fn detect_sensitive_access(&self, tool_name: &str, args: &Value) -> Option<String> {
        use crate::tools::file::is_sensitive_file;
        match tool_name {
            "file_read" | "file_write" | "file_edit" => {
                if let Some(path_str) = args.get("path").and_then(|v| v.as_str()) {
                    let path = std::path::PathBuf::from(path_str);
                    let path = if path.is_absolute() {
                        path
                    } else {
                        self.config.working_dir.join(path)
                    };
                    if is_sensitive_file(&path) {
                        return Some(path.display().to_string());
                    }
                }
            }
            "bash" | "powershell" => {
                if let Some(cmd) = args.get("command").and_then(|v| v.as_str()) {
                    for token in cmd.split_whitespace() {
                        let trimmed = token.trim_matches(|c| c == '"' || c == '\'');
                        if !trimmed.is_empty() {
                            let path = std::path::Path::new(trimmed);
                            if is_sensitive_file(path) {
                                return Some(trimmed.to_string());
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        None
    }

    /// Wait for user approval of a tool call, with timeout handling.
    async fn wait_for_tool_approval(
        &self,
        runtime: &Arc<dyn ApprovalRuntime>,
        tool_call: &ToolCall,
        description: Option<String>,
    ) -> Result<(), ToolError> {
        let turn_id = uuid::Uuid::new_v4().to_string();
        let request_id = runtime
            .create_request(
                tool_call,
                ApprovalSource::ForegroundTurn { turn_id },
                description,
            )
            .await
            .map_err(|e| ToolError::execution_failed(format!("Approval error: {}", e)))?;

        let approval_result = tokio::time::timeout(
            tokio::time::Duration::from_secs(300),
            runtime.wait_for_response(&request_id),
        )
        .await;

        match approval_result {
            Ok(Ok(ApprovalResponse::Approve)) => Ok(()),
            Ok(Ok(ApprovalResponse::Reject)) => Err(ToolError::execution_failed(
                "Tool call rejected by user".to_string(),
            )),
            Ok(Ok(ApprovalResponse::ApproveForSession)) => {
                runtime
                    .resolve(&request_id, ApprovalResponse::ApproveForSession)
                    .await
                    .map_err(|e| ToolError::execution_failed(format!("Approval error: {}", e)))?;
                Ok(())
            }
            Ok(Err(e)) => Err(ToolError::execution_failed(format!(
                "Approval error: {}",
                e
            ))),
            Err(_) => Err(ToolError::execution_failed(
                "Approval timeout after 300 seconds".to_string(),
            )),
        }
    }

    /// Execute a single tool call
    pub(crate) async fn execute_tool_call(&self, tool_call: &ToolCall) -> Result<Value, ToolError> {
        let name = &tool_call.function.name;
        let args: Value = serde_json::from_str(&tool_call.function.arguments)
            .map_err(|e| ToolError::invalid_params(format!("Invalid JSON: {}", e)))?;

        info!("Executing tool '{}' with args: {:?}", name, args);

        let sensitive_path = self.detect_sensitive_access(name, &args);

        // 检查工具是否强制审批
        let tool_requires_approval = self
            .registry
            .get(name)
            .ok()
            .flatten()
            .map(|t| t.requires_approval())
            .unwrap_or(false);

        // 如果配置了审批运行时，先请求审批
        if let Some(ref runtime) = self.approval_runtime {
            let mut description = sensitive_path
                .as_ref()
                .map(|p| format!("Sensitive file access: {}", p));

            if tool_requires_approval {
                description = Some(description.unwrap_or_else(|| {
                    "This tool directly controls the computer desktop and requires explicit approval.".to_string()
                }));
            }

            let tool_call_for_approval = if sensitive_path.is_some() || tool_requires_approval {
                let mut tc = tool_call.clone();
                let mut approval_args = args.clone();
                if tool_requires_approval {
                    approval_args["_requires_approval_warning"] =
                        serde_json::json!("This tool directly controls the computer desktop.");
                }
                if sensitive_path.is_some() {
                    approval_args["_sensitive_file_warning"] =
                        serde_json::json!("This operation accesses a sensitive file");
                }
                tc.function.arguments = approval_args.to_string();
                tc
            } else {
                tool_call.clone()
            };

            match self.approval_mode {
                ApprovalMode::Interactive => {
                    self.wait_for_tool_approval(runtime, &tool_call_for_approval, description)
                        .await?;
                }
                ApprovalMode::Yolo => {
                    if tool_requires_approval {
                        self.wait_for_tool_approval(runtime, &tool_call_for_approval, description)
                            .await?;
                    }
                    // 否则跳过审批（原有 Yolo 行为）
                }
                ApprovalMode::Plan => {
                    // Plan mode's approval is handled at the run() level:
                    // run() bypasses the ReAct loop and uses plan-driven execution
                    // (generate plan → execute steps). If we reach here inside
                    // run_sync_loop, auto-approve to avoid blocking on per-tool
                    // approvals after the plan has already been vetted.
                }
            }
        }

        let ctx = ToolContext::new()
            .with_working_dir(&self.config.working_dir)
            .with_read_only(self.config.read_only)
            .with_timeout(self.config.tool_timeout_secs)
            .with_approval_mode(self.approval_mode)
            .with_capability_token(self.config.capability_token.clone());

        self.registry.execute(name, args, ctx).await
    }

    /// Execute a tool directly (bypassing the LLM loop)
    ///
    /// Useful for programmatic tool execution
    pub async fn execute_tool(&self, name: &str, args: Value) -> Result<Value, ToolError> {
        let ctx = ToolContext::new()
            .with_working_dir(&self.config.working_dir)
            .with_read_only(self.config.read_only)
            .with_timeout(self.config.tool_timeout_secs)
            .with_approval_mode(self.approval_mode)
            .with_capability_token(self.config.capability_token.clone());

        self.registry.execute(name, args, ctx).await
    }

    /// 检查是否需要压缩
    pub(crate) async fn should_compact(&self, messages: &[Message]) -> bool {
        let token_count = estimate_message_tokens(messages);
        self.compaction_config
            .should_compact(token_count, self.max_context_tokens)
    }

    /// 执行压缩
    pub(crate) async fn compact_messages(
        &self,
        messages: &[Message],
    ) -> Result<Vec<Message>, AgentError> {
        use crate::compaction::{Compaction, SimpleCompaction};

        let compactor = SimpleCompaction::new();

        // 调用 LLM 压缩 (如果配置了 LLM)
        let llm_opt = self.llm();
        if let Some(ref llm) = llm_opt {
            let result = compactor.compact(messages, llm.as_ref()).await?;

            // 构建压缩后的消息列表
            let mut new_messages = vec![Message::system(format!(
                "Previous context compacted: {} messages summarized",
                messages.len() - result.messages.len() + 1
            ))];
            new_messages.extend(result.messages);

            Ok(new_messages)
        } else {
            Ok(messages.to_vec())
        }
    }
}
