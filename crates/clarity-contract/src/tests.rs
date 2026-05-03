use crate::*;
use serde_json;

// ============================================================================
// ToolCall & FunctionCall
// ============================================================================

#[test]
fn tool_call_roundtrip() {
    let original = ToolCall {
        id: "call_123".to_string(),
        call_type: "function".to_string(),
        function: FunctionCall {
            name: "read_file".to_string(),
            arguments: r#"{"path": "/tmp/test.txt"}"#.to_string(),
        },
    };

    let json = serde_json::to_string(&original).unwrap();
    let deserialized: ToolCall = serde_json::from_str(&json).unwrap();

    assert_eq!(original.id, deserialized.id);
    assert_eq!(original.call_type, deserialized.call_type);
    assert_eq!(original.function.name, deserialized.function.name);
    assert_eq!(original.function.arguments, deserialized.function.arguments);
}

#[test]
fn function_call_roundtrip() {
    let original = FunctionCall {
        name: "write_file".to_string(),
        arguments: "{}".to_string(),
    };

    let json = serde_json::to_string(&original).unwrap();
    let deserialized: FunctionCall = serde_json::from_str(&json).unwrap();

    assert_eq!(original.name, deserialized.name);
    assert_eq!(original.arguments, deserialized.arguments);
}

// ============================================================================
// MessageRole
// ============================================================================

#[test]
fn message_role_serialization_is_lowercase() {
    assert_eq!(
        serde_json::to_string(&MessageRole::System).unwrap(),
        "\"system\""
    );
    assert_eq!(
        serde_json::to_string(&MessageRole::User).unwrap(),
        "\"user\""
    );
    assert_eq!(
        serde_json::to_string(&MessageRole::Assistant).unwrap(),
        "\"assistant\""
    );
    assert_eq!(
        serde_json::to_string(&MessageRole::Tool).unwrap(),
        "\"tool\""
    );
}

#[test]
fn message_role_deserialization_lowercase() {
    assert_eq!(
        serde_json::from_str::<MessageRole>("\"system\"").unwrap(),
        MessageRole::System
    );
    assert_eq!(
        serde_json::from_str::<MessageRole>("\"user\"").unwrap(),
        MessageRole::User
    );
    assert_eq!(
        serde_json::from_str::<MessageRole>("\"assistant\"").unwrap(),
        MessageRole::Assistant
    );
    assert_eq!(
        serde_json::from_str::<MessageRole>("\"tool\"").unwrap(),
        MessageRole::Tool
    );
}

#[test]
fn message_role_partial_eq() {
    assert_eq!(MessageRole::System, MessageRole::System);
    assert_ne!(MessageRole::System, MessageRole::User);
}

// ============================================================================
// Message
// ============================================================================

#[test]
fn message_constructors() {
    let sys = Message::system("sys prompt");
    assert_eq!(sys.role, MessageRole::System);
    assert_eq!(sys.content, "sys prompt");
    assert!(sys.tool_calls.is_none());
    assert!(sys.tool_call_id.is_none());

    let user = Message::user("hello");
    assert_eq!(user.role, MessageRole::User);
    assert_eq!(user.content, "hello");
    assert!(user.tool_calls.is_none());
    assert!(user.tool_call_id.is_none());

    let assistant = Message::assistant("world");
    assert_eq!(assistant.role, MessageRole::Assistant);
    assert_eq!(assistant.content, "world");
    assert!(assistant.tool_calls.is_none());
    assert!(assistant.tool_call_id.is_none());

    let tool = Message::tool("id_1", "result");
    assert_eq!(tool.role, MessageRole::Tool);
    assert_eq!(tool.content, "result");
    assert_eq!(tool.tool_call_id, Some("id_1".to_string()));
    assert!(tool.tool_calls.is_none());
}

#[test]
fn message_roundtrip_without_optional_fields() {
    let original = Message::user("hello");
    let json = serde_json::to_string(&original).unwrap();
    // tool_calls and tool_call_id should be skipped when None
    assert!(!json.contains("tool_calls"));
    assert!(!json.contains("tool_call_id"));

    let deserialized: Message = serde_json::from_str(&json).unwrap();
    assert_eq!(original.role, deserialized.role);
    assert_eq!(original.content, deserialized.content);
    assert_eq!(original.tool_calls, deserialized.tool_calls);
    assert_eq!(original.tool_call_id, deserialized.tool_call_id);
}

#[test]
fn message_roundtrip_with_tool_calls() {
    let original = Message {
        role: MessageRole::Assistant,
        content: "calling tool".to_string(),
        tool_calls: Some(vec![ToolCall {
            id: "call_1".to_string(),
            call_type: "function".to_string(),
            function: FunctionCall {
                name: "bash".to_string(),
                arguments: "{}".to_string(),
            },
        }]),
        tool_call_id: None,
    };

    let json = serde_json::to_string(&original).unwrap();
    assert!(json.contains("tool_calls"));
    assert!(!json.contains("tool_call_id"));

    let deserialized: Message = serde_json::from_str(&json).unwrap();
    assert_eq!(original.role, deserialized.role);
    assert_eq!(original.content, deserialized.content);
    assert_eq!(original.tool_calls.as_ref().unwrap()[0].id, deserialized.tool_calls.as_ref().unwrap()[0].id);
}

#[test]
fn message_roundtrip_with_tool_call_id() {
    let original = Message::tool("tc_1", "output");
    let json = serde_json::to_string(&original).unwrap();
    assert!(!json.contains("tool_calls"));
    assert!(json.contains("tool_call_id"));

    let deserialized: Message = serde_json::from_str(&json).unwrap();
    assert_eq!(original.role, deserialized.role);
    assert_eq!(original.tool_call_id, deserialized.tool_call_id);
}

// ============================================================================
// StreamDelta
// ============================================================================

#[test]
fn stream_delta_default_is_empty() {
    let delta = StreamDelta::default();
    assert!(delta.content.is_none());
    assert!(delta.tool_calls.is_empty());
    assert!(delta.is_empty());
}

#[test]
fn stream_delta_is_empty_logic() {
    let mut delta = StreamDelta::default();
    assert!(delta.is_empty());

    delta.content = Some("hi".to_string());
    assert!(!delta.is_empty());

    delta.content = None;
    delta.tool_calls.push(ToolCall {
        id: "c1".to_string(),
        call_type: "function".to_string(),
        function: FunctionCall {
            name: "n".to_string(),
            arguments: "{}".to_string(),
        },
    });
    assert!(!delta.is_empty());
}

// ============================================================================
// Error types (error.rs)
// ============================================================================

#[test]
fn tool_error_display() {
    assert_eq!(
        format!("{}", ToolError::InvalidParameters("bad".to_string())),
        "Invalid parameters: bad"
    );
    assert_eq!(
        format!("{}", ToolError::ExecutionFailed("oops".to_string())),
        "Execution failed: oops"
    );
    assert_eq!(
        format!("{}", ToolError::NotFound("tool_x".to_string())),
        "Tool not found: tool_x"
    );
    assert_eq!(
        format!("{}", ToolError::IoError("disk full".to_string())),
        "I/O error: disk full"
    );
    assert_eq!(
        format!("{}", ToolError::Timeout(30)),
        "Execution timeout after 30 seconds"
    );
    assert_eq!(
        format!("{}", ToolError::PermissionDenied("no".to_string())),
        "Permission denied: no"
    );
    assert_eq!(
        format!("{}", ToolError::Unavailable("missing".to_string())),
        "Tool unavailable: missing"
    );
}

#[test]
fn tool_error_constructors() {
    assert!(matches!(ToolError::invalid_params("x"), ToolError::InvalidParameters(_)));
    assert!(matches!(ToolError::execution_failed("x"), ToolError::ExecutionFailed(_)));
    assert!(matches!(ToolError::not_found("x"), ToolError::NotFound(_)));
}

#[test]
fn tool_error_is_recoverable() {
    assert!(ToolError::IoError("e".to_string()).is_recoverable());
    assert!(ToolError::Timeout(1).is_recoverable());
    assert!(ToolError::Unavailable("u".to_string()).is_recoverable());
    assert!(!ToolError::InvalidParameters("p".to_string()).is_recoverable());
    assert!(!ToolError::PermissionDenied("d".to_string()).is_recoverable());
}

#[test]
fn tool_error_sanitize_paths() {
    let err = ToolError::ExecutionFailed("C:\\Users\\Alice\\file.txt".to_string());
    let sanitized = err.sanitize_paths();
    let text = format!("{}", sanitized);
    assert!(!text.contains("C:\\Users\\Alice"));
}

#[test]
fn agent_error_display() {
    assert_eq!(
        format!("{}", AgentError::Tool(ToolError::NotFound("t".to_string()))),
        "Tool error: Tool not found: t"
    );
    assert_eq!(
        format!("{}", AgentError::Registry("bad".to_string())),
        "Registry error: bad"
    );
    assert_eq!(
        format!("{}", AgentError::DuplicateTool("dup".to_string())),
        "Duplicate tool: dup"
    );
    assert_eq!(
        format!("{}", AgentError::ToolExecutionFailed("t".to_string(), "e".to_string())),
        "Tool 't' execution failed: e"
    );
    assert_eq!(
        format!("{}", AgentError::Llm("down".to_string())),
        "LLM error: down"
    );
    assert_eq!(
        format!("{}", AgentError::MaxIterationsExceeded(5)),
        "Maximum iterations (5) exceeded"
    );
    assert_eq!(
        format!("{}", AgentError::MaxIterationsReached),
        "Maximum iterations reached"
    );
    assert_eq!(
        format!("{}", AgentError::ContextOverflow),
        "Context size exceeded maximum"
    );
    assert_eq!(
        format!("{}", AgentError::InvalidResponse("bad json".to_string())),
        "Invalid LLM response: bad json"
    );
    assert_eq!(
        format!("{}", AgentError::Cancelled),
        "Operation cancelled"
    );
    assert_eq!(
        format!("{}", AgentError::Unconfigured),
        "Agent is not configured with an LLM provider"
    );
    assert_eq!(
        format!("{}", AgentError::AlreadyRunning),
        "Agent is already running a turn"
    );
    assert_eq!(
        format!("{}", AgentError::Stalled),
        "Agent is in a stalled state; call reset() first"
    );
    assert_eq!(
        format!("{}", AgentError::Federation("net".to_string())),
        "Federation error: net"
    );
    assert_eq!(
        format!("{}", AgentError::FlowExecution("flow".to_string())),
        "Flow execution error: flow"
    );
}

#[test]
fn agent_error_from_tool_error() {
    let te = ToolError::Timeout(10);
    let ae: AgentError = te.into();
    assert!(matches!(ae, AgentError::Tool(ToolError::Timeout(10))));
}

#[test]
fn agent_error_is_recoverable() {
    assert!(AgentError::Llm("e".to_string()).is_recoverable());
    assert!(AgentError::Federation("e".to_string()).is_recoverable());
    assert!(!AgentError::Cancelled.is_recoverable());
    assert!(!AgentError::ContextOverflow.is_recoverable());
}

#[test]
fn contract_error_is_agent_error_alias() {
    let ce: ContractError = AgentError::Llm("test".to_string());
    assert!(matches!(ce, AgentError::Llm(_)));
}

// ============================================================================
// Federation types (federation.rs)
// ============================================================================

#[test]
fn capability_serialization_roundtrip() {
    let cap = Capability::LlmInference {
        models: vec!["gpt-4".to_string()],
    };
    let json = serde_json::to_string(&cap).unwrap();
    let deserialized: Capability = serde_json::from_str(&json).unwrap();
    match deserialized {
        Capability::LlmInference { models } => assert_eq!(models, vec!["gpt-4"]),
        _ => panic!("wrong variant"),
    }
}

#[test]
fn capability_variants_serialize_with_snake_case_tag() {
    let cap = Capability::VectorSearch { dims: 768 };
    let json = serde_json::to_string(&cap).unwrap();
    assert!(json.contains("\"type\":\"vector_search\""));
}

#[test]
fn tool_spec_roundtrip() {
    let spec = ToolSpec {
        name: "bash".to_string(),
        description: "run shell".to_string(),
        parameters: serde_json::json!({"type": "object"}),
        requires_approval: true,
    };
    let json = serde_json::to_string(&spec).unwrap();
    let deserialized: ToolSpec = serde_json::from_str(&json).unwrap();
    assert_eq!(spec.name, deserialized.name);
    assert_eq!(spec.description, deserialized.description);
    assert_eq!(spec.parameters, deserialized.parameters);
    assert_eq!(spec.requires_approval, deserialized.requires_approval);
}

#[test]
fn fact_roundtrip() {
    let fact = Fact {
        id: 42,
        fact: "sky is blue".to_string(),
        tags: vec!["nature".to_string()],
        time: Some("2024-01-01".to_string()),
        session_id: None,
    };
    let json = serde_json::to_string(&fact).unwrap();
    let deserialized: Fact = serde_json::from_str(&json).unwrap();
    assert_eq!(fact.id, deserialized.id);
    assert_eq!(fact.fact, deserialized.fact);
    assert_eq!(fact.tags, deserialized.tags);
    assert_eq!(fact.time, deserialized.time);
    assert_eq!(fact.session_id, deserialized.session_id);
}

#[test]
fn node_status_roundtrip() {
    assert_eq!(
        serde_json::from_str::<NodeStatus>("\"Healthy\"").unwrap(),
        NodeStatus::Healthy
    );
    assert_eq!(
        serde_json::from_str::<NodeStatus>("\"Degraded\"").unwrap(),
        NodeStatus::Degraded
    );
    assert_eq!(
        serde_json::from_str::<NodeStatus>("\"Offline\"").unwrap(),
        NodeStatus::Offline
    );
}

#[test]
fn task_spec_roundtrip() {
    let task = TaskSpec {
        task_id: "t1".to_string(),
        name: "task".to_string(),
        prompt: "do it".to_string(),
        max_iterations: 10,
        target_capability: Some("llm".to_string()),
    };
    let json = serde_json::to_string(&task).unwrap();
    let deserialized: TaskSpec = serde_json::from_str(&json).unwrap();
    assert_eq!(task.task_id, deserialized.task_id);
    assert_eq!(task.name, deserialized.name);
    assert_eq!(task.prompt, deserialized.prompt);
    assert_eq!(task.max_iterations, deserialized.max_iterations);
    assert_eq!(task.target_capability, deserialized.target_capability);
}

#[test]
fn federation_message_roundtrip() {
    let msg = FederationMessage::Heartbeat {
        node_id: "core".to_string(),
        status: NodeStatus::Healthy,
    };
    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains("\"type\":\"heartbeat\""));
    let deserialized: FederationMessage = serde_json::from_str(&json).unwrap();
    match deserialized {
        FederationMessage::Heartbeat { node_id, status } => {
            assert_eq!(node_id, "core");
            assert_eq!(status, NodeStatus::Healthy);
        }
        _ => panic!("wrong variant"),
    }
}

#[test]
fn federation_message_llm_request_contains_messages() {
    let msg = FederationMessage::LlmRequest {
        messages: vec![Message::user("hi")],
        tools: serde_json::json!({}),
        sender: "egui".to_string(),
    };
    let json = serde_json::to_string(&msg).unwrap();
    let deserialized: FederationMessage = serde_json::from_str(&json).unwrap();
    match deserialized {
        FederationMessage::LlmRequest { messages, sender, .. } => {
            assert_eq!(sender, "egui");
            assert_eq!(messages.len(), 1);
            assert_eq!(messages[0].role, MessageRole::User);
        }
        _ => panic!("wrong variant"),
    }
}

#[test]
fn federation_response_into_json() {
    assert_eq!(FederationResponse::Ack.into_json().unwrap(), serde_json::Value::Null);
    assert_eq!(
        FederationResponse::Json(serde_json::json!({"a": 1})).into_json().unwrap(),
        serde_json::json!({"a": 1})
    );
    assert_eq!(
        FederationResponse::Text("123".to_string()).into_json().unwrap(),
        serde_json::json!(123)
    );
    assert!(FederationResponse::Error(AgentError::Cancelled).into_json().is_err());
}

#[test]
fn federation_response_into_text() {
    assert_eq!(FederationResponse::Ack.into_text().unwrap(), "");
    assert_eq!(
        FederationResponse::Text("hello".to_string()).into_text().unwrap(),
        "hello"
    );
    assert_eq!(
        FederationResponse::Json(serde_json::json!({"a": 1})).into_text().unwrap(),
        r#"{"a":1}"#
    );
    assert!(FederationResponse::Error(AgentError::Cancelled).into_text().is_err());
}

// ============================================================================
// LLM types (llm.rs)
// ============================================================================

#[test]
fn llm_response_construction() {
    let resp = LlmResponse {
        content: "hello".to_string(),
        tool_calls: vec![ToolCall {
            id: "c1".to_string(),
            call_type: "function".to_string(),
            function: FunctionCall {
                name: "bash".to_string(),
                arguments: "{}".to_string(),
            },
        }],
        is_complete: true,
    };
    assert_eq!(resp.content, "hello");
    assert_eq!(resp.tool_calls.len(), 1);
    assert!(resp.is_complete);
}

// ============================================================================
// Tool types (tool.rs)
// ============================================================================

#[test]
fn approval_mode_default_and_eq() {
    assert_eq!(ApprovalMode::default(), ApprovalMode::Interactive);
    assert_eq!(ApprovalMode::Interactive, ApprovalMode::Interactive);
    assert_ne!(ApprovalMode::Interactive, ApprovalMode::Yolo);
}

#[test]
fn tool_context_default() {
    let ctx = ToolContext::default();
    assert!(ctx.working_dir.as_os_str().len() > 0);
    assert!(ctx.timeout_secs > 0);
    assert_eq!(ctx.max_output_size, 1024 * 1024);
    assert!(!ctx.read_only);
    assert_eq!(ctx.approval_mode, ApprovalMode::Interactive);
    assert!(ctx.capability_token.is_none());
}

#[test]
fn tool_context_builder() {
    let ctx = ToolContext::new()
        .with_working_dir("/tmp")
        .with_timeout(120)
        .with_read_only(true)
        .with_env("KEY", "VALUE")
        .with_approval_mode(ApprovalMode::Yolo)
        .with_capability_token(None);

    assert_eq!(ctx.working_dir, std::path::PathBuf::from("/tmp"));
    assert_eq!(ctx.timeout_secs, 120);
    assert!(ctx.read_only);
    assert_eq!(ctx.env.get("KEY"), Some(&"VALUE".to_string()));
    assert_eq!(ctx.approval_mode, ApprovalMode::Yolo);
}

// ============================================================================
// Capability token (capability.rs)
// ============================================================================

#[test]
fn capability_token_new() {
    let token = CapabilityToken::new(vec!["bash".to_string()]);
    assert_eq!(token.allowed_tools, vec!["bash"]);
    assert!(token.sandbox_dir.is_none());
    assert!(!token.read_only);
    assert!(token.max_iterations.is_none());
}

#[test]
fn capability_token_read_only() {
    let token = CapabilityToken::read_only();
    assert!(token.read_only);
    assert!(token.allowed_tools.contains(&"file_read".to_string()));
}

#[test]
fn capability_token_verify_whitelist() {
    let token = CapabilityToken::new(vec!["bash".to_string()]);
    assert!(token.verify("bash", std::path::Path::new("/")).is_ok());
    assert!(token.verify("file_read", std::path::Path::new("/")).is_err());
}

#[test]
fn capability_token_verify_read_only() {
    let token = CapabilityToken::read_only();
    assert!(token.verify("file_read", std::path::Path::new("/")).is_ok());

    // file_write is not in the read-only whitelist, so it hits ToolNotAllowed first.
    // To test read-only blocking, we need a write tool that IS in the whitelist.
    let token = CapabilityToken::new(vec!["bash".to_string()]).with_read_only(true);
    let err = token.verify("bash", std::path::Path::new("/")).unwrap_err();
    assert!(format!("{}", err).contains("read-only"));
}

#[test]
fn capability_token_builder() {
    let token = CapabilityToken::new(vec![])
        .with_sandbox_dir("/tmp")
        .with_read_only(true)
        .with_max_iterations(5)
        .allow_tool("bash");
    assert_eq!(token.max_iterations, Some(5));
    assert!(token.read_only);
    assert_eq!(token.allowed_tools.len(), 1);
}

#[test]
fn token_error_display() {
    assert_eq!(
        format!("{}", TokenError::ToolNotAllowed("x".to_string())),
        "Tool 'x' is not allowed by capability token"
    );
    assert_eq!(
        format!("{}", TokenError::ReadOnlyViolation("y".to_string())),
        "Tool 'y' is blocked in read-only mode"
    );
}

// ============================================================================
// Utility functions
// ============================================================================

#[test]
fn sanitize_path_str_replaces_home() {
    // We can only test that it runs without panicking and produces output.
    let result = sanitize_path_str("some text");
    assert_eq!(result, "some text");
}

#[test]
fn sanitize_path_str_replaces_windows_absolute() {
    let result = sanitize_path_str("C:\\Users\\Alice\\file.txt");
    assert!(result.contains("<absolute-path>"));
}
