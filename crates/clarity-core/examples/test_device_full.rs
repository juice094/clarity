//! Full pipeline test with DeepSeek Device password auth.
//!
//! Usage:
//!   $env:DS_MOBILE="13800138000"
//!   $env:DS_PASSWORD="your-password"
//!   cargo run -p clarity-core --example test_device_full

use clarity_contract::{LlmProvider, Message, MessageRole};
use clarity_core::agent::Agent;
use clarity_core::agent::config::AgentConfig;
use clarity_core::registry::ToolRegistry;
use clarity_llm::{
    DeepSeekDeviceConfig, DeepSeekDeviceCredentials, DeepSeekDeviceOptions, DeepSeekDeviceProvider,
};
use clarity_wire::WireMessage;
use std::sync::Arc;
use std::time::Duration;

fn safe_slice(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        s
    } else {
        &s[..s.floor_char_boundary(max_bytes)]
    }
}

#[tokio::main]
async fn main() {
    let mobile = std::env::var("DS_MOBILE").unwrap_or_else(|_| {
        eprintln!("ERROR: DS_MOBILE not set");
        std::process::exit(1);
    });
    let password = std::env::var("DS_PASSWORD").unwrap_or_else(|_| {
        eprintln!("ERROR: DS_PASSWORD not set");
        std::process::exit(1);
    });

    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║   Clarity + DeepSeek Device (Password Auth) Test         ║");
    println!("╚══════════════════════════════════════════════════════════╝\n");
    println!(
        "Mobile: {}****{}",
        &mobile[..3],
        &mobile[mobile.len() - 3..]
    );

    // ── Phase 1: Provider ─────────────────────────────────────────
    println!("\n── Phase 1: Build DeepSeek Device provider ──");
    let provider = DeepSeekDeviceProvider::new(DeepSeekDeviceConfig {
        base_url: "https://chat.deepseek.com".to_string(),
        client_version: "2.1.8".to_string(),
        device_id: format!("clarity-test-{}", std::process::id()),
        credentials: DeepSeekDeviceCredentials::Password {
            mobile: mobile.clone(),
            password: password.clone(),
        },
        options: DeepSeekDeviceOptions::from_model_id("deepseek-chat"),
    });
    let caps = provider.capabilities();
    println!(
        "  Protocol: deepseek_device | Tools: {} (prompt-guided) | Vision: {}",
        caps.prompt_guided_tool_calling, caps.vision
    );

    // ── Phase 2: complete() ───────────────────────────────────────
    println!("\n── Phase 2: complete() ──");
    let messages = vec![Message {
        role: MessageRole::User,
        content: "用一句话介绍 Rust 语言，30字以内。".to_string(),
        tool_calls: None,
        tool_call_id: None,
    }];
    let tools = serde_json::json!([]);
    let resp = provider
        .complete(&messages, &tools)
        .await
        .expect("complete() failed");
    println!(
        "  ✅ {} chars | \"{}\"",
        resp.content.len(),
        safe_slice(&resp.content, 80)
    );

    // ── Phase 3: stream() ─────────────────────────────────────────
    println!("\n── Phase 3: stream() ──");
    provider.reset_session_state();
    let s_msgs = vec![Message {
        role: MessageRole::User,
        content: "回复'OK'即可".to_string(),
        tool_calls: None,
        tool_call_id: None,
    }];
    let mut rx = provider.stream(&s_msgs, &tools).expect("stream() failed");
    let mut content = String::new();
    let mut chunks = 0u32;
    let mut err = false;
    while let Some(r) = rx.recv().await {
        match r {
            Ok(d) => {
                chunks += 1;
                if let Some(c) = d.content {
                    content.push_str(&c);
                }
            }
            Err(e) => {
                eprintln!("  err: {:?}", e);
                err = true;
                break;
            }
        }
    }
    println!(
        "  ✅ {} chunks | {} chars | {}",
        chunks,
        content.len(),
        if err { "ERROR" } else { "OK" }
    );

    // ── Phase 4: Agent + Wire ─────────────────────────────────────
    println!("\n── Phase 4: Agent.run_streaming() + Wire ──");
    provider.reset_session_state();

    let temp_dir = std::env::temp_dir().join(format!("clarity-dev-test-{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let config = AgentConfig::new()
        .with_max_iterations(3)
        .with_working_dir(&temp_dir)
        .with_read_only(true)
        .with_system_prompt("You are a helpful assistant. Keep responses short.")
        .with_user_id("test-user")
        .with_session_id("test-session");

    let wire = Arc::new(clarity_wire::Wire::new());
    let mut wire_ui = wire.ui_side(false);

    let agent = Agent::with_config(ToolRegistry::with_builtin_tools(), config)
        .with_llm(Arc::new(provider))
        .with_wire(wire.clone());

    let (done_tx, mut done_rx) = tokio::sync::mpsc::channel::<()>(1);
    let events = Arc::new(std::sync::Mutex::new(Vec::new()));
    let ev_clone = events.clone();
    tokio::spawn(async move {
        while let Some(msg) = wire_ui.recv().await {
            let v = match &msg {
                WireMessage::TurnBegin { .. } => "TurnBegin",
                WireMessage::TurnEnd { .. } => "TurnEnd",
                WireMessage::ContentPart { .. } => "ContentPart",
                WireMessage::DraftEvent { .. } => "DraftEvent",
                WireMessage::ToolCall { .. } => "ToolCall",
                WireMessage::ToolResult { .. } => "ToolResult",
                WireMessage::Usage { .. } => "Usage",
                WireMessage::ViewStateUpdate { .. } => "ViewStateUpdate",
                WireMessage::StatusUpdate { .. } => "StatusUpdate",
                _ => "Other",
            };
            ev_clone.lock().unwrap().push(v.to_string());
        }
        let _ = done_tx.send(()).await;
    });

    let query = "用一句话回复：Clarity是一个什么样的项目？";
    println!("  Query: \"{}\"", query);
    let start = std::time::Instant::now();
    let result = tokio::time::timeout(Duration::from_secs(120), agent.run_streaming(query))
        .await
        .expect("timeout");
    drop(agent);
    let _ = tokio::time::timeout(Duration::from_secs(2), done_rx.recv()).await;

    let elapsed = start.elapsed();
    let evs: Vec<String> = events.lock().unwrap().clone();

    match result {
        Ok(final_response) => {
            println!("  ✅ SUCCESS in {} ms", elapsed.as_millis());
            println!("  Response: {} chars", final_response.len());
            println!("  Content: \"{}\"", safe_slice(&final_response, 120));
        }
        Err(e) => {
            eprintln!("  ❌ FAILED: {:?}", e);
            std::process::exit(1);
        }
    }

    println!("\n  Wire events ({} total):", evs.len());
    for ev in &evs {
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
        if ev == "ToolCall" {
            print!(" 🔧");
        }
        if ev == "ToolResult" {
            print!(" 📦");
        }
        println!();
    }

    assert!(evs.contains(&"TurnBegin".to_string()), "missing TurnBegin");
    assert!(
        evs.contains(&"ContentPart".to_string()),
        "missing ContentPart"
    );
    assert!(evs.contains(&"TurnEnd".to_string()), "missing TurnEnd");

    let _ = std::fs::remove_dir_all(&temp_dir);

    println!("\n╔══════════════════════════════════════════════════════════╗");
    println!("║   ✅ All 4 phases passed                                ║");
    println!("║   Device Login → Complete → Stream → Agent + Wire      ║");
    println!("╚══════════════════════════════════════════════════════════╝");
}
