#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    missing_docs,
    unsafe_code
)]
//! 流式响应演示
//!
//! 本示例使用 MockLlm 验证 Agent::run_streaming() 的流式行为。
//! 真实环境中，只需将 MockLlm 替换为 KimiLlm / OpenAiCompatibleLlm。

use clarity_core::ToolRegistry;
use clarity_core::agent::{Agent, AgentConfig, MockLlm};
use clarity_wire::{Wire, WireMessage};
use std::sync::Arc;
use tokio::time::{Duration, timeout};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    println!("🚀 Project Clarity - Streaming Demo (MockLlm)\n");

    let registry = ToolRegistry::with_builtin_tools();
    let config = AgentConfig::new().with_max_iterations(3);

    let agent = Agent::with_config(registry, config).with_llm(Arc::new(MockLlm));

    let query = "Hello, can you introduce yourself?";
    println!("👤 User: {}", query);
    println!("🤖 Assistant: ",);

    let wire = Wire::new();
    let mut ui_side = wire.ui_side(false);

    let agent = agent.with_wire(Arc::new(wire));

    let response_handle = tokio::spawn({
        let agent = agent.clone();
        let query = query.to_string();
        async move { agent.run_streaming(&query).await }
    });

    let mut streamed = Vec::new();
    loop {
        match timeout(Duration::from_millis(500), ui_side.recv()).await {
            Ok(Some(WireMessage::ContentPart { text, .. })) => {
                streamed.push(text.clone());
                print!("{}", text);
                let _ = std::io::Write::flush(&mut std::io::stdout());
            }
            Ok(Some(_)) => continue,
            _ => break,
        }
    }

    let response = response_handle.await??;

    println!(
        "\n\n✅ Final response matches streamed text: {}",
        response == streamed.join("")
    );
    Ok(())
}
