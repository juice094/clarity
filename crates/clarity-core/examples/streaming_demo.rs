//! 流式响应演示
//!
//! 本示例使用 MockLlm 验证 Agent::run_streaming() 的流式行为。
//! 真实环境中，只需将 MockLlm 替换为 KimiLlm / OpenAiCompatibleLlm。

use clarity_core::agent::{Agent, AgentConfig, MockLlm};
use clarity_core::ToolRegistry;
use std::sync::{Arc, Mutex};

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

    let streamed = Arc::new(Mutex::new(Vec::new()));
    let streamed_clone = streamed.clone();

    let response = agent
        .run_streaming(query, move |chunk| {
            streamed_clone.lock().unwrap().push(chunk.to_string());
            print!("{}", chunk);
            let _ = std::io::Write::flush(&mut std::io::stdout());
        })
        .await?;

    println!("\n\n✅ Final response matches streamed text: {}", response == streamed.lock().unwrap().join(""));
    Ok(())
}
