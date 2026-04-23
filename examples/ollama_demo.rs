//! Ollama 本地模型使用示例
//!
//! ## 前提条件
//!
//! 1. 安装 Ollama: https://ollama.com/download
//! 2. 拉取模型: `ollama run llama3.2`
//! 3. 确保 Ollama 服务在运行 (默认 http://localhost:11434)
//!
//! ## 运行
//!
//! ```bash
//! cargo run --example ollama_demo
//! ```

use clarity_core::{Agent, AgentConfig, OllamaProvider, ToolRegistry};
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    tracing_subscriber::fmt::init();
    
    println!("🚀 Project Clarity - Ollama Local Demo\n");
    
    // 检查 Ollama 是否可访问
    match reqwest::get("http://localhost:11434").await {
        Ok(_) => println!("✅ Ollama 服务已连接"),
        Err(_) => {
            eprintln!("❌ 无法连接到 Ollama");
            eprintln!("\n请确保:");
            eprintln!("  1. Ollama 已安装: https://ollama.com/download");
            eprintln!("  2. 已拉取模型: ollama run llama3.2");
            eprintln!("  3. Ollama 服务正在运行");
            return Ok(());
        }
    }
    
    // 创建 Ollama LLM（使用原生 /api/chat 接口）
    let llm = OllamaProvider::new("http://localhost:11434", "llama3.2");
    println!("✅ 使用模型: llama3.2\n");
    
    // 创建工具注册表（本地模型可能不支持工具调用，使用空注册表）
    let registry = ToolRegistry::new();
    
    // 配置 Agent
    let config = AgentConfig::new()
        .with_max_iterations(3)  // 本地模型较慢，减少迭代
        .with_read_only(true);
    
    // 创建 Agent
    let agent = Agent::with_config(registry, config)
        .with_llm(Arc::new(llm));
    
    // 运行对话
    let query = "你好！请介绍一下你自己。";
    println!("👤 User: {}", query);
    println!("🤖 Assistant: 正在思考（本地模型可能需要一些时间）...\n");
    
    match agent.run(query).await {
        Ok(response) => {
            println!("{}\n", response);
        }
        Err(e) => {
            eprintln!("❌ 错误: {}\n", e);
        }
    }
    
    Ok(())
}
