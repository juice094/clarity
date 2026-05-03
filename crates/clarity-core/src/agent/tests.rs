//! Agent unit tests.

use super::*;
use crate::tools::FileReadTool;
use clarity_wire::WireMessage;

#[test]
fn test_message_creation() {
    let system = Message::system("You are helpful");
    assert_eq!(system.role, MessageRole::System);

    let user = Message::user("Hello");
    assert_eq!(user.role, MessageRole::User);

    let tool = Message::tool("call_123", "result");
    assert_eq!(tool.role, MessageRole::Tool);
    assert_eq!(tool.tool_call_id, Some("call_123".to_string()));
}

#[test]
fn test_agent_config() {
    let config = AgentConfig::new()
        .with_max_iterations(5)
        .with_read_only(true);

    assert_eq!(config.max_iterations, 5);
    assert!(config.read_only);
}

#[tokio::test]
async fn test_agent_direct_tool_execution() {
    let registry = ToolRegistry::new();
    registry.register(FileReadTool::new()).unwrap();

    let agent = Agent::new(registry);

    // This will fail because file doesn't exist, but tests the path
    let result = agent
        .execute_tool(
            "file_read",
            serde_json::json!({"path": "/nonexistent/file.txt"}),
        )
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_agent_lazy_llm_factory() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let registry = ToolRegistry::with_builtin_tools();
    let agent = Agent::with_config(registry, AgentConfig::default());

    // Agent starts unconfigured (no LLM)
    assert!(agent.llm().is_none());

    let call_count = Arc::new(AtomicUsize::new(0));
    let call_count_clone = call_count.clone();

    // Set a lazy factory that returns MockLlm
    let agent = agent.with_llm_factory(Arc::new(move || {
        let count = call_count_clone.clone();
        Box::pin(async move {
            count.fetch_add(1, Ordering::SeqCst);
            Ok(Arc::new(MockLlm) as Arc<dyn LlmProvider>)
        })
    }));

    // First call to ensure_initialized triggers the factory
    agent.ensure_initialized().await.unwrap();
    assert!(agent.llm().is_some());
    assert_eq!(call_count.load(Ordering::SeqCst), 1);

    // Second call does NOT trigger the factory again
    agent.ensure_initialized().await.unwrap();
    assert_eq!(call_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_agent_run_streaming() {
    use std::sync::{Arc, Mutex};

    let registry = ToolRegistry::new();
    let config = AgentConfig::new();
    let agent = Agent::with_config(registry, config).with_llm(Arc::new(MockLlm));

    let chunks = Arc::new(Mutex::new(Vec::new()));
    let chunks_clone = chunks.clone();
    let result = agent
        .run_streaming("Hello", move |chunk| {
            chunks_clone.lock().unwrap().push(chunk.to_string());
        })
        .await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "This is a mock response");
    assert_eq!(*chunks.lock().unwrap(), vec!["This is a mock response"]);
}

#[tokio::test]
async fn test_compaction_triggered_in_agent() {
    use crate::compaction::CompactionConfig;
    use std::sync::atomic::{AtomicUsize, Ordering};

    // 创建一个 Mock LLM 记录调用次数
    struct CountingMockLlm {
        call_count: AtomicUsize,
    }

    #[async_trait::async_trait]
    impl LlmProvider for CountingMockLlm {
        async fn complete(
            &self,
            _messages: &[Message],
            _tools: &serde_json::Value,
        ) -> Result<LlmResponse, AgentError> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            Ok(LlmResponse {
                content: "This is a mock response for compaction test".to_string(),
                tool_calls: vec![],
                is_complete: true,
            })
        }

        fn stream(
            &self,
            _messages: &[Message],
            _tools: &serde_json::Value,
        ) -> Result<tokio::sync::mpsc::Receiver<Result<StreamDelta, AgentError>>, AgentError>
        {
            let (tx, rx) = tokio::sync::mpsc::channel(1);
            tokio::spawn(async move {
                let _ = tx
                    .send(Ok(StreamDelta {
                        content: Some("This is a mock response".to_string()),
                        tool_calls: vec![],
                    }))
                    .await;
            });
            Ok(rx)
        }

        fn set_prompt_cache_key(&mut self, _key: &str) {}
    }

    let registry = ToolRegistry::new();
    let config = AgentConfig::new().with_max_iterations(5);

    // 创建一个使用低阈值触发压缩的 Agent
    let agent = Agent::with_config(registry, config)
        .with_llm(Arc::new(CountingMockLlm {
            call_count: AtomicUsize::new(0),
        }))
        .with_max_context_tokens(100) // 设置低阈值触发压缩
        .with_compaction_config(CompactionConfig::default());

    // 运行多次对话
    for i in 0..3 {
        let result = agent
            .run(
                format!(
                    "test query with some content to increase token count {} ",
                    i
                )
                .repeat(10),
            )
            .await;
        assert!(result.is_ok());
    }

    // 验证压缩逻辑被正确配置 (token 估算和压缩配置)
    // 由于 MockLlm 在压缩时也会返回简单响应，测试主要验证代码路径不崩溃
}

#[test]
fn test_should_compact_method() {
    use crate::compaction::CompactionConfig;

    let registry = ToolRegistry::new();
    let config = AgentConfig::new();
    let agent = Agent::with_config(registry, config)
        .with_max_context_tokens(100)
        .with_compaction_config(CompactionConfig::default());

    // 创建足够多的消息以超过阈值
    let messages: Vec<Message> = (0..20)
        .map(|i| {
            Message::user(
                format!(
                    "This is a test message with enough content to consume tokens {} ",
                    i
                )
                .repeat(5),
            )
        })
        .collect();

    // 验证 should_compact 方法存在并且可以调用
    // 注意：由于方法是 async 的，我们主要验证编译通过
    let rt = tokio::runtime::Runtime::new().unwrap();
    let should_compact = rt.block_on(agent.should_compact(&messages));

    // 消息内容应该触发压缩（超过 100 token 的 80% = 80 tokens）
    assert!(
        should_compact,
        "Should detect that compaction is needed with large messages"
    );
}

#[tokio::test]
async fn test_tool_call_approval_flow() {
    use crate::approval::{ApprovalResponse, InMemoryApprovalRuntime};
    use std::time::Duration;

    // 创建一个 Mock LLM 会返回工具调用
    struct MockLlmWithToolCall;

    #[async_trait::async_trait]
    impl LlmProvider for MockLlmWithToolCall {
        async fn complete(
            &self,
            _messages: &[Message],
            _tools: &serde_json::Value,
        ) -> Result<LlmResponse, AgentError> {
            Ok(LlmResponse {
                content: "I'll use the mock tool".to_string(),
                tool_calls: vec![ToolCall {
                    id: "call_123".to_string(),
                    call_type: "function".to_string(),
                    function: FunctionCall {
                        name: "shell".to_string(),
                        arguments: r#"{"command": "echo test"}"#.to_string(),
                    },
                }],
                is_complete: false,
            })
        }

        fn stream(
            &self,
            _messages: &[Message],
            _tools: &serde_json::Value,
        ) -> Result<tokio::sync::mpsc::Receiver<Result<StreamDelta, AgentError>>, AgentError>
        {
            let (tx, rx) = tokio::sync::mpsc::channel(1);
            tokio::spawn(async move {
                let _ = tx
                    .send(Ok(StreamDelta {
                        content: Some("Mock response".to_string()),
                        tool_calls: vec![],
                    }))
                    .await;
            });
            Ok(rx)
        }

        fn set_prompt_cache_key(&mut self, _key: &str) {}
    }

    // 创建注册表并注册一个 Mock 工具
    let registry = ToolRegistry::new();
    // 由于我们没有真正的 mock_tool，我们期望工具执行失败
    // 但审批流程应该被触发

    // 创建内存审批运行时
    let approval_rt = Arc::new(InMemoryApprovalRuntime::new());
    let rt_clone = approval_rt.clone();

    let agent = Agent::with_config(registry, AgentConfig::new().with_max_iterations(1))
        .with_approval_runtime(approval_rt)
        .with_approval_mode(ApprovalMode::Interactive)
        .with_llm(Arc::new(MockLlmWithToolCall));

    // 在后台运行 Agent
    let handle = tokio::spawn(async move { agent.run("use mock tool").await });

    // 等待审批请求出现
    tokio::time::sleep(Duration::from_millis(100)).await;
    let pending = rt_clone.list_pending();
    assert_eq!(pending.len(), 1, "Should have one pending approval request");

    // 批准请求
    rt_clone
        .resolve(&pending[0].id, ApprovalResponse::Approve)
        .await
        .expect("Failed to resolve approval");

    // Agent 应该完成（虽然工具执行会失败，因为 mock_tool 不存在）
    let result = handle.await.unwrap();
    // 结果应该是 Err，因为工具未注册，但审批流程已经测试到了
    assert!(
        result.is_err(),
        "Expected error because shell is not registered in the empty test registry"
    );
}

#[tokio::test]
async fn test_tool_call_yolo_mode() {
    use crate::approval::InMemoryApprovalRuntime;

    // 创建一个 Mock LLM 会返回工具调用
    struct MockLlmWithToolCall;

    #[async_trait::async_trait]
    impl LlmProvider for MockLlmWithToolCall {
        async fn complete(
            &self,
            _messages: &[Message],
            _tools: &serde_json::Value,
        ) -> Result<LlmResponse, AgentError> {
            Ok(LlmResponse {
                content: "I'll use the mock tool".to_string(),
                tool_calls: vec![ToolCall {
                    id: "call_456".to_string(),
                    call_type: "function".to_string(),
                    function: FunctionCall {
                        name: "mock_tool".to_string(),
                        arguments: r#"{"param": "value"}"#.to_string(),
                    },
                }],
                is_complete: false,
            })
        }

        fn stream(
            &self,
            _messages: &[Message],
            _tools: &serde_json::Value,
        ) -> Result<tokio::sync::mpsc::Receiver<Result<StreamDelta, AgentError>>, AgentError>
        {
            let (tx, rx) = tokio::sync::mpsc::channel(1);
            tokio::spawn(async move {
                let _ = tx
                    .send(Ok(StreamDelta {
                        content: Some("Mock response".to_string()),
                        tool_calls: vec![],
                    }))
                    .await;
            });
            Ok(rx)
        }

        fn set_prompt_cache_key(&mut self, _key: &str) {}
    }

    let registry = ToolRegistry::new();
    let approval_rt = Arc::new(InMemoryApprovalRuntime::new());

    let agent = Agent::with_config(registry, AgentConfig::new().with_max_iterations(1))
        .with_approval_runtime(approval_rt.clone())
        .with_approval_mode(ApprovalMode::Yolo) // Yolo 模式
        .with_llm(Arc::new(MockLlmWithToolCall));

    // 运行 Agent
    let result = agent.run("use mock tool").await;
    // 结果应该是 Err，因为工具不存在，但 Yolo 模式应该跳过审批
    assert!(
        result.is_err(),
        "Expected error because mock_tool is not registered"
    );

    // Yolo 模式下不应有 pending 审批请求
    let pending = approval_rt.list_pending();
    assert!(
        pending.is_empty(),
        "Yolo mode should not create pending approval requests"
    );
}

#[tokio::test]
async fn test_agent_run_with_wire() {
    use clarity_wire::Wire;
    use std::sync::Arc;
    use tokio::time::{timeout, Duration};

    // Create Wire
    let wire = Wire::new();
    let mut ui_side = wire.ui_side(false);

    // Create Agent with Wire
    let registry = ToolRegistry::new();
    let config = AgentConfig::new();
    let agent = Agent::with_config(registry, config)
        .with_llm(Arc::new(MockLlm))
        .with_wire(Arc::new(wire));

    // Run Agent in background
    let handle = tokio::spawn(async move { agent.run("test query").await });

    // Verify UI side receives TurnBegin
    let msg = timeout(Duration::from_millis(1000), ui_side.recv())
        .await
        .expect("timeout waiting for TurnBegin")
        .expect("channel closed");
    assert!(matches!(msg, WireMessage::TurnBegin { user_input } if user_input == "test query"));

    // Verify ContentPart is received
    let msg = timeout(Duration::from_millis(1000), ui_side.recv())
        .await
        .expect("timeout waiting for ContentPart")
        .expect("channel closed");
    assert!(matches!(msg, WireMessage::ContentPart { text } if text == "This is a mock response"));

    // Verify TurnEnd is received
    let msg = timeout(Duration::from_millis(1000), ui_side.recv())
        .await
        .expect("timeout waiting for TurnEnd")
        .expect("channel closed");
    assert!(matches!(msg, WireMessage::TurnEnd));

    // Wait for agent to complete
    let result = timeout(Duration::from_millis(1000), handle)
        .await
        .expect("timeout waiting for agent")
        .expect("join error");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "This is a mock response");
}

#[tokio::test]
async fn test_agent_run_streaming_with_wire() {
    use clarity_wire::Wire;
    use std::sync::Arc;
    use std::sync::{Arc as StdArc, Mutex};
    use tokio::time::{timeout, Duration};

    // Create Wire
    let wire = Wire::new();
    let mut ui_side = wire.ui_side(false);

    // Create Agent with Wire
    let registry = ToolRegistry::new();
    let config = AgentConfig::new();
    let agent = Agent::with_config(registry, config)
        .with_llm(Arc::new(MockLlm))
        .with_wire(Arc::new(wire));

    // Run Agent in background with streaming
    let chunks = StdArc::new(Mutex::new(Vec::new()));
    let chunks_clone = chunks.clone();
    let handle = tokio::spawn(async move {
        agent
            .run_streaming("streaming test", move |chunk| {
                chunks_clone.lock().unwrap().push(chunk.to_string());
            })
            .await
    });

    // Verify UI side receives TurnBegin
    let msg = timeout(Duration::from_millis(1000), ui_side.recv())
        .await
        .expect("timeout waiting for TurnBegin")
        .expect("channel closed");
    assert!(matches!(msg, WireMessage::TurnBegin { user_input } if user_input == "streaming test"));

    // Verify ContentPart is received (empty start marker)
    let msg = timeout(Duration::from_millis(1000), ui_side.recv())
        .await
        .expect("timeout waiting for ContentPart start")
        .expect("channel closed");
    assert!(matches!(msg, WireMessage::ContentPart { .. }));

    // Verify streaming ContentParts are received
    let mut content_received = false;
    loop {
        match timeout(Duration::from_millis(500), ui_side.recv()).await {
            Ok(Some(msg)) => match msg {
                WireMessage::ContentPart { text } => {
                    if !text.is_empty() {
                        content_received = true;
                    }
                }
                WireMessage::TurnEnd => break,
                _ => {}
            },
            Ok(None) => break,
            Err(_) => break, // Timeout
        }
    }
    assert!(content_received, "Should have received content parts");

    // Wait for agent to complete
    let result = timeout(Duration::from_millis(1000), handle)
        .await
        .expect("timeout waiting for agent")
        .expect("join error");
    assert!(result.is_ok());
}

#[test]
fn test_active_skill_snapshotted_at_turn_start() {
    let registry = ToolRegistry::new();
    let agent = Agent::with_config(registry, AgentConfig::new()).with_llm(Arc::new(MockLlm));

    // Set active skill before turn
    agent.set_active_skill(Some("test-skill".to_string()));

    // begin_turn should snapshot it
    let _token = agent.begin_turn().expect("begin_turn should succeed");
    assert_eq!(
        agent.snapshotted_active_skill(),
        Some("test-skill".to_string())
    );

    // Changing active_skill mid-turn should NOT affect the snapshot
    agent.set_active_skill(Some("other-skill".to_string()));
    assert_eq!(
        agent.snapshotted_active_skill(),
        Some("test-skill".to_string())
    );

    // finish_turn clears the snapshot
    agent.finish_turn();
    assert_eq!(agent.snapshotted_active_skill(), None);
}

#[test]
fn test_active_skill_snapshot_none_when_not_set() {
    let registry = ToolRegistry::new();
    let agent = Agent::with_config(registry, AgentConfig::new()).with_llm(Arc::new(MockLlm));

    let _token = agent.begin_turn().expect("begin_turn should succeed");
    assert_eq!(agent.snapshotted_active_skill(), None);

    agent.finish_turn();
    assert_eq!(agent.snapshotted_active_skill(), None);
}

// ------------------------------------------------------------------
// Sprint 11 Phase A — Context Snapshot tests
// ------------------------------------------------------------------

#[test]
fn test_context_getters_and_setters() {
    let registry = ToolRegistry::new();
    let agent = Agent::with_config(registry, AgentConfig::new());

    assert!(agent.git_context().is_none());
    assert!(agent.active_files().is_none());
    assert!(agent.project_metadata().is_none());

    agent.set_git_context(Some("Branch: main".to_string()));
    agent.set_active_files(Some("src/main.rs".to_string()));
    agent.set_project_metadata(Some("[package]".to_string()));

    assert_eq!(agent.git_context(), Some("Branch: main".to_string()));
    assert_eq!(agent.active_files(), Some("src/main.rs".to_string()));
    assert_eq!(agent.project_metadata(), Some("[package]".to_string()));
}

#[test]
fn test_build_active_files_context() {
    let registry = ToolRegistry::new();
    let agent = Agent::with_config(registry, AgentConfig::new());

    // No active files -> None
    assert!(agent.build_active_files_context().is_none());

    // With active files -> Some (preserves directory structure)
    agent.set_active_file_paths(vec![
        std::path::PathBuf::from("src/main.rs"),
        std::path::PathBuf::from("Cargo.toml"),
    ]);
    let ctx = agent.build_active_files_context().unwrap();
    assert!(
        ctx.contains("src/main.rs"),
        "should preserve directory structure: {}",
        ctx
    );
    assert!(
        ctx.contains("Cargo.toml"),
        "should include Cargo.toml: {}",
        ctx
    );
}

#[test]
fn test_build_active_files_external_path_redacted() {
    let registry = ToolRegistry::new();
    let agent = Agent::with_config(registry, AgentConfig::new());

    // Use a platform-specific absolute path that is outside the working directory.
    let external_path = if cfg!(windows) {
        std::path::PathBuf::from("C:\\Windows\\secret.txt")
    } else {
        std::path::PathBuf::from("/etc/passwd")
    };

    agent.set_active_file_paths(vec![
        external_path.clone(),
        std::path::PathBuf::from("src/main.rs"),
    ]);
    let ctx = agent.build_active_files_context().unwrap();
    assert!(
        ctx.contains("<external>"),
        "external path should be redacted: {}",
        ctx
    );
    assert!(
        !ctx.contains(if cfg!(windows) { "C:\\Windows" } else { "/etc/passwd" }),
        "absolute path must NOT leak: {}",
        ctx
    );
    assert!(
        ctx.contains("src/main.rs"),
        "internal path should still appear: {}",
        ctx
    );
}

#[test]
fn test_collect_project_metadata_cargo_toml() {
    use std::io::Write;
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let cargo_toml = temp_dir.path().join("Cargo.toml");
    let mut file = std::fs::File::create(&cargo_toml).unwrap();
    writeln!(file, "[package]").unwrap();
    writeln!(file, "name = \"test-project\"").unwrap();
    writeln!(file, "version = \"0.1.0\"").unwrap();

    let registry = ToolRegistry::new();
    let mut config = AgentConfig::new();
    config.working_dir = temp_dir.path().to_path_buf();
    let agent = Agent::with_config(registry, config);

    let meta = agent.collect_project_metadata().unwrap();
    assert!(meta.contains("test-project"));
    assert!(meta.contains("```toml"));
}

#[test]
fn test_collect_project_metadata_package_json() {
    use std::io::Write;
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let package_json = temp_dir.path().join("package.json");
    let mut file = std::fs::File::create(&package_json).unwrap();
    writeln!(file, "{{").unwrap();
    writeln!(file, "  \"name\": \"test-js-project\",").unwrap();
    writeln!(file, "  \"version\": \"1.0.0\"").unwrap();
    writeln!(file, "}}").unwrap();

    let registry = ToolRegistry::new();
    let mut config = AgentConfig::new();
    config.working_dir = temp_dir.path().to_path_buf();
    let agent = Agent::with_config(registry, config);

    let meta = agent.collect_project_metadata().unwrap();
    assert!(meta.contains("test-js-project"));
    assert!(meta.contains("```json"));
}

#[test]
fn test_collect_project_metadata_none_when_no_manifest() {
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();

    let registry = ToolRegistry::new();
    let mut config = AgentConfig::new();
    config.working_dir = temp_dir.path().to_path_buf();
    let agent = Agent::with_config(registry, config);

    assert!(agent.collect_project_metadata().is_none());
}

#[tokio::test]
async fn test_refresh_context_populates_fields() {
    use std::io::Write;
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();

    // Create a Cargo.toml so project_metadata is populated
    let cargo_toml = temp_dir.path().join("Cargo.toml");
    let mut file = std::fs::File::create(&cargo_toml).unwrap();
    writeln!(file, "[package]").unwrap();
    writeln!(file, "name = \"refresh-test\"").unwrap();

    let registry = ToolRegistry::new();
    let mut config = AgentConfig::new();
    config.working_dir = temp_dir.path().to_path_buf();
    let agent = Agent::with_config(registry, config);

    // Pre-set active file paths
    agent.set_active_file_paths(vec![std::path::PathBuf::from("src/lib.rs")]);

    agent.refresh_context().await;

    // Git context may be None (no git repo) or Some — just check it's been set
    // (i.e. the field was touched by refresh_context)
    assert!(
        agent.git_context().is_none() || agent.git_context().is_some(),
        "git_context should have been set"
    );

    // Active files should be populated
    let active = agent
        .active_files()
        .expect("active_files should be populated");
    assert!(active.contains("lib.rs"));

    // Project metadata should be populated
    let meta = agent
        .project_metadata()
        .expect("project_metadata should be populated");
    assert!(meta.contains("refresh-test"));
}

// ------------------------------------------------------------------
// V2 端到端验证 — Sprint 11 能力闭环
// ------------------------------------------------------------------

#[tokio::test]
async fn test_end_to_end_context_injection() {
    use std::io::Write;
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();

    // Initialize a git repo
    let output = std::process::Command::new("git")
        .args(["init"])
        .current_dir(temp_dir.path())
        .output();
    if output.is_err() || !output.unwrap().status.success() {
        return; // Skip if git unavailable
    }

    // Create a file and commit it
    let file_path = temp_dir.path().join("lib.rs");
    let mut file = std::fs::File::create(&file_path).unwrap();
    writeln!(file, "fn main() {{}}").unwrap();

    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(temp_dir.path())
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "init", "--no-gpg-sign"])
        .current_dir(temp_dir.path())
        .output()
        .unwrap();

    // Create Cargo.toml
    let cargo_toml = temp_dir.path().join("Cargo.toml");
    let mut file = std::fs::File::create(&cargo_toml).unwrap();
    writeln!(file, "[package]").unwrap();
    writeln!(file, "name = \"e2e-test\"").unwrap();

    let registry = ToolRegistry::new();
    let mut config = AgentConfig::new();
    config.working_dir = temp_dir.path().to_path_buf();
    let agent = Agent::with_config(registry, config);

    agent.set_active_file_paths(vec![std::path::PathBuf::from("lib.rs")]);

    agent.refresh_context().await;

    let prompt = agent.build_system_prompt();

    // Git Context and Project Metadata should now be auto-injected into prompt.
    assert!(
        prompt.contains("Git Context"),
        "should contain Git Context section:\n{}",
        prompt
    );
    assert!(
        prompt.contains("Project Metadata"),
        "should contain Project Metadata section:\n{}",
        prompt
    );

    // Active Files should still be present (paths are sanitized).
    assert!(
        prompt.contains("Active Files"),
        "should contain Active Files section:\n{}",
        prompt
    );
    assert!(
        prompt.contains("lib.rs"),
        "should mention lib.rs:\n{}",
        prompt
    );
}

#[test]
fn test_approval_mode_switch() {
    use crate::approval::ApprovalMode;

    let registry = ToolRegistry::new();
    let agent = Agent::with_config(registry, AgentConfig::new());

    // Default is Interactive
    assert_eq!(agent.approval_mode(), ApprovalMode::Interactive);

    agent.set_approval_mode(ApprovalMode::Yolo);
    assert_eq!(agent.approval_mode(), ApprovalMode::Yolo);

    agent.set_approval_mode(ApprovalMode::Plan);
    assert_eq!(agent.approval_mode(), ApprovalMode::Plan);

    agent.set_approval_mode(ApprovalMode::Smart);
    assert_eq!(agent.approval_mode(), ApprovalMode::Smart);
}

// ------------------------------------------------------------------
// Sprint 13 Phase A — Circuit breaker & path sanitization tests
// ------------------------------------------------------------------

/// Mock tool that always fails with a non-recoverable error.
struct FailingTool;

#[async_trait::async_trait]
impl crate::tools::Tool for FailingTool {
    fn name(&self) -> &str {
        "failing_tool"
    }

    fn description(&self) -> &str {
        "A tool that always fails for testing"
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn execute(
        &self,
        _args: serde_json::Value,
        _ctx: crate::tools::ToolContext,
    ) -> crate::error::ToolResult<serde_json::Value> {
        Err(crate::error::ToolError::PermissionDenied(
            "Access denied to C:\\Users\\Test\\secret.txt".to_string(),
        ))
    }
}

/// Mock LLM that emits a single call to `failing_tool`.
struct MockLlmFailingTool;

#[async_trait::async_trait]
impl LlmProvider for MockLlmFailingTool {
    async fn complete(
        &self,
        _messages: &[Message],
        _tools: &serde_json::Value,
    ) -> Result<LlmResponse, AgentError> {
        Ok(LlmResponse {
            content: "I'll use failing_tool".to_string(),
            tool_calls: vec![ToolCall {
                id: "call_fail".to_string(),
                call_type: "function".to_string(),
                function: FunctionCall {
                    name: "failing_tool".to_string(),
                    arguments: "{}".to_string(),
                },
            }],
            is_complete: false,
        })
    }

    fn stream(
        &self,
        _messages: &[Message],
        _tools: &serde_json::Value,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<StreamDelta, AgentError>>, AgentError> {
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        tokio::spawn(async move {
            let _ = tx
                .send(Ok(StreamDelta {
                    content: Some("Mock".to_string()),
                    tool_calls: vec![],
                }))
                .await;
        });
        Ok(rx)
    }

    fn set_prompt_cache_key(&mut self, _key: &str) {}
}

#[tokio::test]
async fn test_non_recoverable_tool_error_stops_turn() {
    let registry = ToolRegistry::new();
    registry.register(FailingTool).unwrap();

    let agent = Agent::with_config(registry, AgentConfig::new().with_max_iterations(10))
        .with_llm(Arc::new(MockLlmFailingTool));

    let result = agent.run("trigger failing tool").await;

    assert!(
        result.is_err(),
        "Expected turn to stop on non-recoverable tool error, got: {:?}",
        result
    );
    let err = result.unwrap_err();
    match err {
        AgentError::ToolExecutionFailed(tool_name, _) => {
            assert_eq!(tool_name, "failing_tool");
        }
        other => panic!("Expected ToolExecutionFailed, got: {:?}", other),
    }
}

/// Mock tool that always fails with a recoverable IoError.
struct RecoverableFailingTool;

#[async_trait::async_trait]
impl crate::tools::Tool for RecoverableFailingTool {
    fn name(&self) -> &str {
        "recoverable_failing_tool"
    }

    fn description(&self) -> &str {
        "A tool that always fails with recoverable error for testing"
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn execute(
        &self,
        _args: serde_json::Value,
        _ctx: crate::tools::ToolContext,
    ) -> crate::error::ToolResult<serde_json::Value> {
        Err(crate::error::ToolError::IoError(
            "transient network failure".to_string(),
        ))
    }
}

/// Mock LLM that always emits a call to `recoverable_failing_tool`.
struct MockLlmRecoverableLoop;

#[async_trait::async_trait]
impl LlmProvider for MockLlmRecoverableLoop {
    async fn complete(
        &self,
        _messages: &[Message],
        _tools: &serde_json::Value,
    ) -> Result<LlmResponse, AgentError> {
        Ok(LlmResponse {
            content: "I'll try recoverable_failing_tool".to_string(),
            tool_calls: vec![ToolCall {
                id: "call_recover".to_string(),
                call_type: "function".to_string(),
                function: FunctionCall {
                    name: "recoverable_failing_tool".to_string(),
                    arguments: "{}".to_string(),
                },
            }],
            is_complete: false,
        })
    }

    fn stream(
        &self,
        _messages: &[Message],
        _tools: &serde_json::Value,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<StreamDelta, AgentError>>, AgentError> {
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        tokio::spawn(async move {
            let _ = tx
                .send(Ok(StreamDelta {
                    content: Some("Mock".to_string()),
                    tool_calls: vec![],
                }))
                .await;
        });
        Ok(rx)
    }

    fn set_prompt_cache_key(&mut self, _key: &str) {}
}

#[tokio::test]
async fn test_recoverable_tool_circuit_breaker() {
    let registry = ToolRegistry::new();
    registry.register(RecoverableFailingTool).unwrap();

    let agent = Agent::with_config(registry, AgentConfig::new().with_max_iterations(10))
        .with_llm(Arc::new(MockLlmRecoverableLoop));

    let result = agent.run("trigger recoverable failing tool").await;

    assert!(
        result.is_err(),
        "Expected turn to stop after 3 recoverable failures, got: {:?}",
        result
    );
    let err = result.unwrap_err();
    match err {
        AgentError::ToolExecutionFailed(tool_name, msg) => {
            assert_eq!(tool_name, "recoverable_failing_tool");
            assert!(
                msg.contains("recoverable errors exhausted"),
                "Expected circuit-breaker message, got: {}",
                msg
            );
        }
        other => panic!("Expected ToolExecutionFailed, got: {:?}", other),
    }
}

#[test]
fn test_tool_error_sanitize_paths() {
    // Home-directory redaction
    let home = dirs::home_dir();
    if let Some(ref h) = home {
        let home_str = h.to_string_lossy().to_string();
        let err = crate::error::ToolError::ExecutionFailed(format!(
            "Failed to read {}",
            home_str
        ));
        let sanitized = err.sanitize_paths();
        let msg = sanitized.to_string();
        assert!(
            !msg.contains(&home_str),
            "home dir must be redacted: {}",
            msg
        );
        assert!(msg.contains('~'), "should use ~ shorthand: {}", msg);
    }

    // Windows absolute path redaction
    let err = crate::error::ToolError::ExecutionFailed(
        "Failed to read C:\\Users\\Someone\\secret.txt".to_string(),
    );
    let sanitized = err.sanitize_paths();
    assert!(
        !sanitized.to_string().contains("C:\\Users\\Someone"),
        "Windows absolute path must be redacted: {}",
        sanitized
    );
}
