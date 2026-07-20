//! Full pipeline integration test: DeepSeek API → Agent → Wire events.
//!
//! Validates the complete path from LLM through agent loop to wire messages,
//! simulating what Clarity egui does in production (without the UI layer).
//!
//! Usage:
//!   $env:DEEPSEEK_API_KEY="sk-..."
//!   cargo run -p clarity-core --example test_full_pipeline
//!
//! Validates 5 phases:
//!   1. Provider construction (DeepSeek via LlmFactory)
//!   2. complete() smoke test
//!   3. stream() test with wire events
//!   4. Tool-calling test
//!   5. Full Agent.run_streaming() with wire capture

use clarity_contract::{Message, MessageRole};
use clarity_core::agent::Agent;
use clarity_core::agent::config::AgentConfig;
use clarity_core::registry::ToolRegistry;
use clarity_wire::WireMessage;
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() {
    let api_key = std::env::var("DEEPSEEK_API_KEY").unwrap_or_else(|_| {
        eprintln!("ERROR: DEEPSEEK_API_KEY not set");
        std::process::exit(1);
    });

    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║   Clarity Full Pipeline Integration Test                ║");
    println!("║   DeepSeek API → Agent → Wire → (simulated UI)          ║");
    println!("╚══════════════════════════════════════════════════════════╝\n");

    // ══════════════════════════════════════════════════════════════════════
    // Phase 1: Build provider via LlmFactory
    // ══════════════════════════════════════════════════════════════════════
    println!("── Phase 1: Build DeepSeek provider (LlmFactory) ──");
    let provider =
        clarity_llm::LlmFactory::create_with_key_arc("deepseek", &api_key, "deepseek-chat")
            .expect("Failed to build deepseek provider");

    let caps = provider.capabilities();
    println!(
        "  Protocol: OpenAI Chat | Model: deepseek-chat | Tools: {} | Vision: {}",
        caps.native_tool_calling, caps.vision
    );
    assert!(
        caps.native_tool_calling,
        "DeepSeek must support native tool calling"
    );

    // ══════════════════════════════════════════════════════════════════════
    // Phase 2: complete() smoke test
    // ══════════════════════════════════════════════════════════════════════
    println!("\n── Phase 2: complete() smoke test ──");
    let messages = vec![Message {
        role: MessageRole::User,
        content: "用一句话介绍 Rust 语言，30字以内。".to_string(),
        tool_calls: None,
        tool_call_id: None,
    }];
    let tools = serde_json::json!([]);

    let response = provider
        .complete(&messages, &tools)
        .await
        .expect("complete() failed");
    println!(
        "  ✅ {} chars | \"{}\"",
        response.content.len(),
        &response.content[..response.content.len().min(80)]
    );
    assert!(!response.content.is_empty(), "Response must not be empty");

    // ══════════════════════════════════════════════════════════════════════
    // Phase 3: stream() test
    // ══════════════════════════════════════════════════════════════════════
    println!("\n── Phase 3: stream() test ──");
    let stream_messages = vec![Message {
        role: MessageRole::User,
        content: "回复一个JSON: {\"status\":\"ok\"}".to_string(),
        tool_calls: None,
        tool_call_id: None,
    }];

    let mut rx = provider
        .stream(&stream_messages, &tools)
        .expect("stream() setup failed");
    let mut streamed = String::new();
    let mut chunk_count = 0u32;
    let mut had_error = false;
    while let Some(result) = rx.recv().await {
        match result {
            Ok(delta) => {
                chunk_count += 1;
                if let Some(c) = delta.content {
                    streamed.push_str(&c);
                }
            }
            Err(e) => {
                eprintln!("  Stream chunk error: {:?}", e);
                had_error = true;
                break;
            }
        }
    }
    println!(
        "  ✅ {} chunks | {} chars | {}",
        chunk_count,
        streamed.len(),
        if had_error { "ERROR" } else { "OK" }
    );
    assert!(!had_error, "Stream must complete without error");
    assert!(!streamed.is_empty(), "Streamed content must not be empty");
    assert!(
        streamed.contains("status") || streamed.contains("ok"),
        "Streamed content should contain the JSON response. Got: {}",
        &streamed[..streamed.len().min(100)]
    );

    // ══════════════════════════════════════════════════════════════════════
    // Phase 4: Tool-calling test
    // ══════════════════════════════════════════════════════════════════════
    println!("\n── Phase 4: Tool calling test ──");
    let tool_schema = serde_json::json!([{
        "type": "function",
        "function": {
            "name": "get_weather",
            "description": "Get current weather for a city",
            "parameters": {
                "type": "object",
                "properties": {
                    "city": { "type": "string", "description": "City name" }
                },
                "required": ["city"]
            }
        }
    }]);

    let tool_messages = vec![Message {
        role: MessageRole::User,
        content: "北京现在天气怎么样？请调用 get_weather 工具查询。".to_string(),
        tool_calls: None,
        tool_call_id: None,
    }];

    let tool_response = provider
        .complete(&tool_messages, &tool_schema)
        .await
        .expect("Tool calling complete() failed");
    println!(
        "  Content: {} chars | Tool calls: {}",
        tool_response.content.len(),
        tool_response.tool_calls.len()
    );
    if tool_response.tool_calls.is_empty() {
        println!("  ⚠ No tool calls returned — model chose text-only response");
        println!(
            "  Content preview: {}",
            &tool_response.content[..tool_response.content.len().min(120)]
        );
    } else {
        for tc in &tool_response.tool_calls {
            println!(
                "  ✅ Tool call: {} | id: {} | args: {}",
                tc.function.name,
                tc.id,
                &tc.function.arguments[..tc.function.arguments.len().min(80)]
            );
        }
        assert!(
            tool_response
                .tool_calls
                .iter()
                .any(|tc| tc.function.name == "get_weather"),
            "Expected get_weather tool call"
        );
    }

    // ══════════════════════════════════════════════════════════════════════
    // Phase 5: Full Agent.run_streaming() with Wire capture
    // ══════════════════════════════════════════════════════════════════════
    println!("\n── Phase 5: Agent.run_streaming() + Wire capture ──");

    let temp_dir =
        std::env::temp_dir().join(format!("clarity-pipeline-test-{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let config = AgentConfig::new()
        .with_max_iterations(3)
        .with_working_dir(&temp_dir)
        .with_read_only(true)
        .with_system_prompt("You are a helpful assistant. Keep responses short and direct.")
        .with_user_id("test-user")
        .with_session_id("test-session");

    let wire = Arc::new(clarity_wire::Wire::new());
    let mut wire_ui = wire.ui_side(false);

    // Build a fresh provider for the agent (separate from Phase 1-4 provider
    // so the stream/complete usage above doesn't interfere).
    let agent_provider =
        clarity_llm::LlmFactory::create_with_key_arc("deepseek", &api_key, "deepseek-chat")
            .expect("Failed to build agent provider");

    let agent = Agent::with_config(ToolRegistry::with_builtin_tools(), config)
        .with_llm(agent_provider)
        .with_wire(wire.clone());

    println!("  Agent built: max_iterations=3, read_only=true, tools=builtin");

    // Spawn wire drainer
    let (wire_done_tx, mut wire_done_rx) = tokio::sync::mpsc::channel::<()>(1);
    let wire_events = Arc::new(std::sync::Mutex::new(Vec::new()));
    let wire_events_clone = wire_events.clone();

    tokio::spawn(async move {
        while let Some(msg) = wire_ui.recv().await {
            let variant = match &msg {
                WireMessage::TurnBegin { .. } => "TurnBegin",
                WireMessage::TurnEnd { .. } => "TurnEnd",
                WireMessage::ContentPart { .. } => "ContentPart",
                WireMessage::ReasoningPart { .. } => "ReasoningPart",
                WireMessage::DraftEvent { .. } => "DraftEvent",
                WireMessage::ToolCall { .. } => "ToolCall",
                WireMessage::ToolCallProgress { .. } => "ToolCallProgress",
                WireMessage::ToolResult { .. } => "ToolResult",
                WireMessage::Usage { .. } => "Usage",
                WireMessage::CompactionBegin { .. } => "CompactionBegin",
                WireMessage::CompactionEnd { .. } => "CompactionEnd",
                WireMessage::StatusUpdate { .. } => "StatusUpdate",
                WireMessage::ViewStateUpdate { .. } => "ViewStateUpdate",
                _ => "Other",
            };
            wire_events_clone.lock().unwrap().push(variant.to_string());
        }
        let _ = wire_done_tx.send(()).await;
    });

    let query = "用一句话回复：Clarity是一个什么样的项目？";
    println!("  Sending: \"{}\"", query);
    let start = std::time::Instant::now();

    let result = tokio::time::timeout(Duration::from_secs(60), agent.run_streaming(query))
        .await
        .expect("run_streaming timed out");

    // Drop agent and wait for wire events to drain
    drop(agent);
    let _ = tokio::time::timeout(Duration::from_secs(2), wire_done_rx.recv()).await;

    let elapsed = start.elapsed();
    let events: Vec<String> = {
        let guard = wire_events.lock().unwrap();
        guard.clone()
    };

    match result {
        Ok(final_response) => {
            println!("  ✅ SUCCESS in {} ms", elapsed.as_millis());
            println!("  Response: {} chars", final_response.len());
            println!(
                "  Content: \"{}\"",
                &final_response[..final_response.len().min(120)]
            );
        }
        Err(e) => {
            eprintln!("  ❌ FAILED: {:?}", e);
            std::process::exit(1);
        }
    }

    println!("\n  Wire events ({} total):", events.len());
    for ev in &events {
        print!("  → {}", ev);
        if ev == "ContentPart" {
            print!(" 📝");
        }
        if ev == "TurnBegin" {
            print!(" 🚀");
        }
        if ev == "TurnEnd" {
            print!(" 🏁");
        }
        if ev == "Usage" {
            print!(" 📊");
        }
        if ev == "DraftEvent" {
            print!(" ✏");
        }
        if ev == "ToolCall" {
            print!(" 🔧");
        }
        println!();
    }

    // Verify essential wire events
    assert!(
        events.contains(&"TurnBegin".to_string()),
        "Must have TurnBegin event"
    );
    assert!(
        events.contains(&"ContentPart".to_string()),
        "Must have ContentPart event (streaming response)"
    );
    assert!(
        events.contains(&"TurnEnd".to_string()),
        "Must have TurnEnd event"
    );

    // Cleanup
    let _ = std::fs::remove_dir_all(&temp_dir);

    println!("\n╔══════════════════════════════════════════════════════════╗");
    println!("║   ✅ All 5 phases passed                                ║");
    println!("║   Complete → Stream → Tools → Agent → Wire             ║");
    println!("╚══════════════════════════════════════════════════════════╝");
}
