#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    missing_docs,
    unsafe_code
)]
mod common;

use clarity_core::agent::{Agent, AgentConfig, ToolCall};
use clarity_core::registry::ToolRegistry;
use clarity_integration_tests::mock_consumer::MockConsumer;
use clarity_wire::WireMessage;
use common::{SequentialMockLlm, text_response, tool_call_response};
use std::sync::Arc;

/// Scenario A — Core -> Wire basic flow.
/// Spawn an Agent with the built-in MockLlm, run a simple prompt, and verify
/// the wire receives TurnBegin, ContentPart, and TurnEnd.
#[tokio::test]
async fn test_core_wire_basic_flow() {
    let wire = clarity_wire::Wire::new();
    let consumer = MockConsumer::subscribe(&wire);

    let registry = ToolRegistry::with_builtin_tools();
    let config = AgentConfig::new().with_max_iterations(2);

    let agent = Agent::with_config(registry, config)
        .with_llm(Arc::new(clarity_core::agent::MockLlm))
        .with_wire(Arc::new(wire));

    let result = agent.run("Hello, world!").await;
    assert!(result.is_ok(), "Agent run failed: {:?}", result);

    // Give the broadcast a moment to propagate.
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    let msgs = consumer.messages().await;

    // Verify sequence contains the expected lifecycle messages.
    assert!(
        msgs.iter()
            .any(|m| matches!(m, WireMessage::TurnBegin { .. })),
        "Expected TurnBegin in {:?}",
        msgs
    );
    consumer
        .assert_received_content("This is a mock response")
        .await;
    assert!(
        msgs.iter()
            .any(|m| matches!(m, WireMessage::TurnEnd { .. })),
        "Expected TurnEnd in {:?}",
        msgs
    );
}

/// Scenario B — Core -> Wire with a tool call.
/// The mock LLM first asks to run `bash` and then replies with final text.
/// We verify that ToolCall and ToolResult messages are emitted on the wire.
#[tokio::test]
async fn test_core_wire_tool_flow() {
    let wire = clarity_wire::Wire::new();
    let consumer = MockConsumer::subscribe(&wire);

    let registry = ToolRegistry::with_builtin_tools();
    let config = AgentConfig::new()
        .with_max_iterations(3)
        .with_read_only(false);

    // Use a platform-appropriate shell tool for the integration test.
    #[cfg(target_os = "windows")]
    let (tool_name, command_arg) = (
        "powershell",
        r#"{"command":"Write-Output integration-test"}"#,
    );
    #[cfg(not(target_os = "windows"))]
    let (tool_name, command_arg) = ("bash", r#"{"command":"echo integration-test"}"#);

    // First response: ask the agent to execute a shell command.
    let first = tool_call_response(
        "Let me run a command for you.",
        vec![ToolCall {
            id: "call_001".to_string(),
            call_type: "function".to_string(),
            function: clarity_core::agent::FunctionCall {
                name: tool_name.to_string(),
                arguments: command_arg.to_string(),
            },
        }],
    );
    // Second response: plain text after tool execution.
    let second = text_response("The command returned 'integration-test'.");

    let mock_llm = SequentialMockLlm::new(vec![first, second]);

    let agent = Agent::with_config(registry, config)
        .with_llm(Arc::new(mock_llm))
        .with_wire(Arc::new(wire));

    let result = agent.run("Run a bash command for me").await;
    assert!(result.is_ok(), "Agent run failed: {:?}", result);

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    let msgs = consumer.messages().await;

    // Verify TurnBegin
    assert!(
        msgs.iter()
            .any(|m| matches!(m, WireMessage::TurnBegin { .. })),
        "Expected TurnBegin in {:?}",
        msgs
    );

    // Verify StepBegin for the shell tool
    assert!(
        msgs.iter()
            .any(|m| matches!(m, WireMessage::StepBegin { tool_name: n, .. } if n == tool_name)),
        "Expected StepBegin for {} in {:?}",
        tool_name,
        msgs
    );

    // Verify ToolCall
    assert!(
        msgs.iter()
            .any(|m| matches!(m, WireMessage::ToolCall { name, .. } if name == tool_name)),
        "Expected ToolCall for {} in {:?}",
        tool_name,
        msgs
    );

    // Verify ToolResult
    assert!(
        msgs.iter().any(|m| matches!(m, WireMessage::ToolResult { result, .. } if result.contains("integration-test"))),
        "Expected ToolResult containing 'integration-test' in {:?}",
        msgs
    );

    // Verify final content
    consumer
        .assert_received_content("The command returned 'integration-test'.")
        .await;

    // Verify TurnEnd
    assert!(
        msgs.iter()
            .any(|m| matches!(m, WireMessage::TurnEnd { .. })),
        "Expected TurnEnd in {:?}",
        msgs
    );
}
