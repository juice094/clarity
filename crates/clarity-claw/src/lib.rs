#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        missing_docs,
        unsafe_code
    )
)]
//! clarity-claw —— 联邦运行时协调器
//!
//! Claw is the federation runtime for Project Clarity.
//! It coordinates multiple federal nodes (core, memory, egui, gateway)
//! via a central Coordinator and capability registry.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────┐
//! │  Coordinator │  ← 联邦消息路由 + 能力注册表
//! └──────┬──────┘
//!        │
//!   ┌────┴────┬────────┬────────┐
//!   ▼         ▼        ▼        ▼
//! Core    Memory   egui    Gateway
//! Node    Node     Node     Node
//! ```

use serde::Deserialize;
use std::time::Duration;

// ------------------------------------------------------------------
// Federation modules
// ------------------------------------------------------------------

pub mod coordinator;
pub mod nodes;
pub mod runtime;
pub mod tray;

// ------------------------------------------------------------------
// Legacy tray helpers (retained for backward compatibility)
// ------------------------------------------------------------------

/// Default Gateway address.
pub const GATEWAY_URL: &str = "http://127.0.0.1:18790";

/// Gateway polling interval in seconds.
pub const POLL_INTERVAL_SECS: u64 = 5;

/// Minimal task info deserialized from Gateway `/v1/tasks`.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct TaskSummary {
    #[serde(rename = "task_id")]
    /// Unique task identifier.
    pub task_id: String,
    /// Human-readable task name.
    pub name: String,
    /// Current task status (e.g., "Running", "Completed").
    pub status: String,
}

/// Gateway task list payload.
#[derive(Clone, Debug, Deserialize)]
pub struct TaskListPayload {
    /// Tasks returned by the Gateway.
    pub tasks: Vec<TaskSummary>,
}

// ------------------------------------------------------------------
// Pure logic helpers
// ------------------------------------------------------------------

/// Resolve the Gateway URL from the environment.
pub fn resolve_gateway_url() -> String {
    std::env::var("CLARITY_GATEWAY_URL").unwrap_or_else(|_| GATEWAY_URL.to_string())
}

/// Format the tray tooltip from task counts.
pub fn format_tooltip(running: usize, pending: usize, total: usize) -> String {
    if total == 0 {
        "Clarity Claw — idle (no tasks)".to_string()
    } else {
        format!(
            "Clarity Claw — {} running, {} pending ({} total)",
            running, pending, total
        )
    }
}

/// Classify a task status into a notification summary.
pub fn classify_task_status(status: &str) -> (&'static str, Option<notify_rust::Urgency>) {
    match status {
        "Completed" => ("✅ Task completed", None),
        "Failed" => ("❌ Task failed", Some(notify_rust::Urgency::Critical)),
        "Cancelled" => ("🚫 Task cancelled", None),
        _ => ("Task finished", None),
    }
}

// ------------------------------------------------------------------
// HTTP helpers for Gateway interaction
// ------------------------------------------------------------------

/// Send a quick chat message to the Gateway (non-streaming).
pub async fn quick_chat(gateway_url: &str, input: &str) -> anyhow::Result<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    let payload = serde_json::json!({
        "model": "default",
        "messages": [{"role": "user", "content": input}],
        "stream": false
    });

    let url = format!("{}/v1/chat/completions", gateway_url);
    let resp = client
        .post(&url)
        .json(&payload)
        .send()
        .await?
        .error_for_status()?;

    let body: serde_json::Value = resp.json().await?;
    let reply = body["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("(no response)");

    Ok(reply.to_string())
}

/// Create a background task via Gateway.
pub async fn create_remote_task(
    gateway_url: &str,
    name: &str,
    prompt: &str,
) -> anyhow::Result<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

    let payload = serde_json::json!({
        "name": name,
        "prompt": prompt,
        "max_iterations": 10
    });

    let url = format!("{}/v1/tasks", gateway_url);
    let resp = client
        .post(&url)
        .json(&payload)
        .send()
        .await?
        .error_for_status()?;

    let body: serde_json::Value = resp.json().await?;
    let task_id = body["task_id"].as_str().unwrap_or("unknown").to_string();
    Ok(task_id)
}

/// Cancel a background task via Gateway.
pub async fn cancel_remote_task(gateway_url: &str, task_id: &str) -> anyhow::Result<()> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

    let url = format!("{}/v1/tasks/{}", gateway_url, task_id);
    client.delete(&url).send().await?.error_for_status()?;
    Ok(())
}

// ------------------------------------------------------------------
// Tests
// ------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn set_env(key: &str, value: &str) {
        // SAFETY: test-only helper; env vars are manipulated in single-threaded test context.
        unsafe { std::env::set_var(key, value) };
    }

    fn remove_env(key: &str) {
        // SAFETY: test-only helper; env vars are manipulated in single-threaded test context.
        unsafe { std::env::remove_var(key) };
    }

    #[test]
    fn test_resolve_gateway_url() {
        // Default (no env var)
        remove_env("CLARITY_GATEWAY_URL");
        assert_eq!(resolve_gateway_url(), GATEWAY_URL);

        // From env var
        set_env("CLARITY_GATEWAY_URL", "http://custom:8080");
        assert_eq!(resolve_gateway_url(), "http://custom:8080");
        remove_env("CLARITY_GATEWAY_URL");
    }

    #[test]
    fn test_format_tooltip_idle() {
        assert_eq!(format_tooltip(0, 0, 0), "Clarity Claw — idle (no tasks)");
    }

    #[test]
    fn test_format_tooltip_with_tasks() {
        assert_eq!(
            format_tooltip(2, 1, 3),
            "Clarity Claw — 2 running, 1 pending (3 total)"
        );
    }

    #[test]
    fn test_classify_task_status() {
        assert_eq!(
            classify_task_status("Completed"),
            ("✅ Task completed", None)
        );
        assert_eq!(
            classify_task_status("Failed"),
            ("❌ Task failed", Some(notify_rust::Urgency::Critical))
        );
        assert_eq!(
            classify_task_status("Cancelled"),
            ("🚫 Task cancelled", None)
        );
        assert_eq!(classify_task_status("Unknown"), ("Task finished", None));
    }

    #[test]
    fn test_task_summary_deserialization() {
        let json = r#"{"task_id":"abc-123","name":"test task","status":"Running"}"#;
        let summary: TaskSummary = serde_json::from_str(json).unwrap();
        assert_eq!(summary.task_id, "abc-123");
        assert_eq!(summary.name, "test task");
        assert_eq!(summary.status, "Running");
    }
}
