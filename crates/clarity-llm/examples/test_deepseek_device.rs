//! Standalone DeepSeek Device provider connectivity test.
//!
//! Usage:
//!   cargo run -p clarity-llm --example test_deepseek_device
//!
//! Requires: `DEEPSEEK_DEVICE_TOKEN` environment variable.

use clarity_contract::{LlmProvider, Message, MessageRole};
use clarity_llm::{
    DeepSeekDeviceConfig, DeepSeekDeviceCredentials, DeepSeekDeviceOptions, DeepSeekDeviceProvider,
};

#[tokio::main]
async fn main() {
    let token = std::env::var("DEEPSEEK_DEVICE_TOKEN").unwrap_or_else(|_| {
        eprintln!("ERROR: DEEPSEEK_DEVICE_TOKEN not set");
        std::process::exit(1);
    });

    println!("=== DeepSeek Device Provider Connectivity Test ===\n");
    println!("Token length: {} chars", token.len());
    println!("Token prefix: {}...", &token[..token.len().min(8)]);

    let provider = DeepSeekDeviceProvider::new(DeepSeekDeviceConfig {
        base_url: "https://chat.deepseek.com".to_string(),
        client_version: "2.1.8".to_string(),
        device_id: format!("clarity-test-{}", std::process::id()),
        credentials: DeepSeekDeviceCredentials::Token(token),
        options: DeepSeekDeviceOptions::from_model_id("deepseek-chat"),
    });

    println!("\n--- Capabilities ---");
    let caps = provider.capabilities();
    println!("  native_tool_calling:       {}", caps.native_tool_calling);
    println!(
        "  prompt_guided_tool_calling: {}",
        caps.prompt_guided_tool_calling
    );
    println!("  vision:                     {}", caps.vision);
    println!();

    // Test 1: Simple completion (non-streaming)
    println!("--- Test 1: complete() ---");
    let messages = vec![Message {
        role: MessageRole::User,
        content: "你好，请用一句话介绍你自己。".to_string(),
        tool_calls: None,
        tool_call_id: None,
    }];
    let tools = serde_json::json!([]);

    match provider.complete(&messages, &tools).await {
        Ok(response) => {
            println!(
                "SUCCESS: complete() returned {} chars",
                response.content.len()
            );
            println!(
                "Content: {}",
                &response.content[..response.content.len().min(200)]
            );
            println!("Tool calls: {}", response.tool_calls.len());
        }
        Err(e) => {
            eprintln!("FAILED: complete() error: {:?}", e);
        }
    }

    // Reset session state between tests
    provider.reset_session_state();

    // Test 2: Streaming
    println!("\n--- Test 2: stream() ---");
    let messages2 = vec![Message {
        role: MessageRole::User,
        content: "请回复'OK'即可。".to_string(),
        tool_calls: None,
        tool_call_id: None,
    }];

    // stream() is NOT async — it returns Result<Receiver, AgentError> directly.
    match provider.stream(&messages2, &tools) {
        Ok(mut rx) => {
            println!("Stream channel opened, reading chunks...");
            let mut total_content = String::new();
            let mut chunk_count = 0u32;
            let mut has_error = false;
            while let Some(chunk_result) = rx.recv().await {
                match chunk_result {
                    Ok(delta) => {
                        chunk_count += 1;
                        if let Some(reasoning) = &delta.reasoning_content {
                            print!("[R:{}]", reasoning.len());
                        }
                        if let Some(content) = &delta.content {
                            total_content.push_str(&content);
                            print!(".");
                        }
                    }
                    Err(e) => {
                        eprintln!("\nStream error at chunk {}: {:?}", chunk_count, e);
                        has_error = true;
                        break;
                    }
                }
            }
            println!();
            println!(
                "stream() complete — {} chunks, {} total chars, error={}",
                chunk_count,
                total_content.len(),
                has_error
            );
            if !total_content.is_empty() {
                println!(
                    "Content: {}",
                    &total_content[..total_content.len().min(200)]
                );
            }
        }
        Err(e) => {
            eprintln!("FAILED: stream() setup error: {:?}", e);
        }
    }

    println!("\n=== Test Complete ===");
}
