#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    missing_docs,
    unsafe_code
)]
//! MCP contract E2E tests — success/error detection logic.
//!
//! These tests verify that `clarity-core` correctly detects application-level
//! errors in JSON payloads returned by MCP servers (specifically devbase).
//!
//! Scenarios:
//!   A. `{"success": true, ...}`  → Ok
//!   B. `{"success": false, ...}` → Err(ExecutionFailed)
//!   C. No `success` field         → Ok
//!   D. `"error"` field present    → Err(ExecutionFailed)

use clarity_contract::ToolError;
use clarity_core::mcp::{ToolCallResult, ToolContent, process_mcp_tool_result};
use serde_json::Value;

// ---------------------------------------------------------------------------
// Scenario A: MCP Server returns `"success": true` + normal data
// ---------------------------------------------------------------------------
#[test]
fn scenario_a_success_true_returns_ok() {
    let result = ToolCallResult {
        content: vec![ToolContent::Text {
            text: r#"{"success": true, "data": "ok"}"#.to_string(),
        }],
        is_error: false,
    };

    let outcome = process_mcp_tool_result(result);
    assert!(
        outcome.is_ok(),
        "Expected Ok for success=true payload, got {:?}",
        outcome
    );
    assert_eq!(
        outcome.unwrap(),
        Value::String(r#"{"success": true, "data": "ok"}"#.to_string())
    );
}

// ---------------------------------------------------------------------------
// Scenario B: MCP Server returns `"success": false` + `"error": "..."`
// ---------------------------------------------------------------------------
#[test]
fn scenario_b_success_false_returns_execution_failed() {
    let result = ToolCallResult {
        content: vec![ToolContent::Text {
            text: r#"{"success": false, "error": "something failed"}"#.to_string(),
        }],
        is_error: false,
    };

    let outcome = process_mcp_tool_result(result);
    assert!(
        outcome.is_err(),
        "Expected Err for success=false payload, got {:?}",
        outcome
    );
    match outcome.unwrap_err() {
        ToolError::ExecutionFailed(msg) => {
            assert!(msg.contains("something failed"));
        }
        other => panic!("Expected ExecutionFailed, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// Scenario C: MCP Server returns JSON without `"success"` field
// ---------------------------------------------------------------------------
#[test]
fn scenario_c_no_success_field_returns_ok() {
    let result = ToolCallResult {
        content: vec![ToolContent::Text {
            text: r#"{"status": "fresh", "repo": "test"}"#.to_string(),
        }],
        is_error: false,
    };

    let outcome = process_mcp_tool_result(result);
    assert!(
        outcome.is_ok(),
        "Expected Ok for payload without success field, got {:?}",
        outcome
    );
    assert_eq!(
        outcome.unwrap(),
        Value::String(r#"{"status": "fresh", "repo": "test"}"#.to_string())
    );
}

// ---------------------------------------------------------------------------
// Scenario D: MCP Server returns JSON with `"error"` field but no `"success"`
// ---------------------------------------------------------------------------
#[test]
fn scenario_d_error_field_without_success_returns_execution_failed() {
    let result = ToolCallResult {
        content: vec![ToolContent::Text {
            text: r#"{"error": "internal", "detail": "db connection lost"}"#.to_string(),
        }],
        is_error: false,
    };

    let outcome = process_mcp_tool_result(result);
    assert!(
        outcome.is_err(),
        "Expected Err for payload with error field, got {:?}",
        outcome
    );
    match outcome.unwrap_err() {
        ToolError::ExecutionFailed(msg) => {
            assert!(msg.contains("internal"));
        }
        other => panic!("Expected ExecutionFailed, got {:?}", other),
    }
}
