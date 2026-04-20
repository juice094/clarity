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
    // 结果应该是 Err，因为工具不存在，但审批流程已经测试到了
    assert!(
        result.is_err(),
        "Expected error because mock_tool is not registered"
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
