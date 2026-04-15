//! DeepSeek Provider Demo
//!
//! This example demonstrates how to use the DeepSeek LLM provider.
//!
//! ## Setup
//!
//! Set your DeepSeek API key:
//! ```powershell
//! $env:DEEPSEEK_API_KEY="sk-your-key"
//! cargo run --example deepseek_demo -p clarity-core
//! ```

use clarity_core::agent::{LlmProvider, Message};
use clarity_core::llm::deepseek::{DeepSeekProvider, models};
use serde_json::json;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║              DeepSeek Provider Demo                          ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    // Check for API key
    let api_key = match std::env::var("DEEPSEEK_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            println!("❌ DEEPSEEK_API_KEY not set");
            println!();
            println!("To run this demo, set your DeepSeek API key:");
            println!("  Powershell: $env:DEEPSEEK_API_KEY=\"sk-your-key\"");
            println!("  Bash:       export DEEPSEEK_API_KEY=\"sk-your-key\"");
            println!();
            println!("Get your API key from: https://platform.deepseek.com/");
            return Ok(());
        }
    };

    // =================================================================
    // Demo 1: DeepSeek Chat (V3)
    // =================================================================
    println!("▶️  Demo 1: DeepSeek Chat (V3)");
    println!("   Model: {}", models::DEEPSEEK_CHAT);
    println!();

    let provider = DeepSeekProvider::chat(&api_key);

    let messages = vec![
        Message::system("You are a helpful assistant."),
        Message::user("What is Rust programming language? Keep it brief."),
    ];

    match provider.complete(&messages, &json!([])).await {
        Ok(response) => {
            println!("   ✅ Response:");
            println!("   {}", response.content);
        }
        Err(e) => {
            println!("   ❌ Error: {}", e);
        }
    }
    println!();

    // =================================================================
    // Demo 2: DeepSeek Reasoner (R1)
    // =================================================================
    println!("▶️  Demo 2: DeepSeek Reasoner (R1)");
    println!("   Model: {}", models::DEEPSEEK_REASONER);
    println!();

    let provider = DeepSeekProvider::reasoner(&api_key);

    let messages = vec![
        Message::system("You are a helpful assistant."),
        Message::user("Solve this step by step: What is 23 + 47?"),
    ];

    match provider.complete(&messages, &json!([])).await {
        Ok(response) => {
            println!("   ✅ Response:");
            println!("   {}", response.content);
        }
        Err(e) => {
            println!("   ❌ Error: {}", e);
        }
    }
    println!();

    // =================================================================
    // Demo 3: Streaming
    // =================================================================
    println!("▶️  Demo 3: Streaming Response");
    println!();

    let provider = DeepSeekProvider::chat(&api_key);

    let messages = vec![
        Message::system("You are a helpful assistant."),
        Message::user("Count from 1 to 5."),
    ];

    println!("   Streaming: ");
    match provider.stream(&messages, &json!([])) {
        Ok(mut rx) => {
            while let Some(result) = rx.recv().await {
                match result {
                    Ok(chunk) => {
                        if let Some(text) = chunk.content {
                            print!("{}", text);
                        }
                    }
                    Err(e) => {
                        eprintln!("\n   ❌ Stream error: {}", e);
                        break;
                    }
                }
            }
            println!();
            println!("   ✅ Stream complete");
        }
        Err(e) => {
            println!("   ❌ Error: {}", e);
        }
    }
    println!();

    // =================================================================
    // Summary
    // =================================================================
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║                    Demo Complete!                            ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
    println!("Summary:");
    println!("  • DeepSeek Chat (V3): General purpose model");
    println!("  • DeepSeek Reasoner (R1): Advanced reasoning");
    println!("  • Streaming: Real-time token generation");
    println!();
    println!("Models available:");
    println!("  • deepseek-chat: General conversation");
    println!("  • deepseek-reasoner: Complex reasoning tasks");
    println!("  • deepseek-coder: Code generation");

    Ok(())
}
