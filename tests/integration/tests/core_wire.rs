mod common;

use clarity_core::agent::{Agent, AgentConfig, ToolCall};
use clarity_core::registry::ToolRegistry;
use clarity_integration_tests::mock_consumer::MockConsumer;
use clarity_wire::WireMessage;
use common::{text_response, tool_call_response, SequentialMockLlm};
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
        msgs.iter().any(|m| matches!(m, WireMessage::TurnEnd)),
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

    // First response: ask the agent to execute bash.
    let first = tool_call_response(
        "Let me run a command for you.",
        vec![ToolCall {
            id: "call_001".to_string(),
            call_type: "function".to_string(),
            function: clarity_core::agent::FunctionCall {
                name: "bash".to_string(),
                arguments: r#"{"command":"echo integration-test"}"#.to_string(),
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

    // Verify StepBegin for bash
    assert!(
        msgs.iter()
            .any(|m| matches!(m, WireMessage::StepBegin { tool_name } if tool_name == "bash")),
        "Expected StepBegin for bash in {:?}",
        msgs
    );

    // Verify ToolCall
    assert!(
        msgs.iter()
            .any(|m| matches!(m, WireMessage::ToolCall { name, .. } if name == "bash")),
        "Expected ToolCall for bash in {:?}",
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
        msgs.iter().any(|m| matches!(m, WireMessage::TurnEnd)),
        "Expected TurnEnd in {:?}",
        msgs
    );
}
