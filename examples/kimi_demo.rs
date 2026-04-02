//! Kimi Code / Kimi API 使用示例
//!
//! ## 快速开始
//!
//! ### 方式一：Kimi Code（会员权益，每周 1024 次免费）
//! ```powershell
//! $env:ANTHROPIC_BASE_URL="https://api.kimi.com/coding/"
//! $env:ANTHROPIC_AUTH_TOKEN="sk-kimi-your-key"
//! cargo run --example kimi_demo
//! ```
//!
//! ### 方式二：Kimi API（开放平台，按量付费）
//! ```powershell
//! $env:KIMI_API_KEY="sk-your-moonshot-key"
//! $env:KIMI_BASE_URL="https://api.moonshot.cn/v1"
//! cargo run --example kimi_demo
//! ```
//!
//! ### 方式三：Ollama 本地模型
//! ```powershell
//! # 确保 ollama 已安装并运行
//! # ollama run llama3.2
//! cargo run --example ollama_demo
//! ```

use clarity_core::{Agent, AgentConfig, KimiLlm, ToolRegistry};
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    tracing_subscriber::fmt::init();
    
    println!("🚀 Project Clarity - LLM Demo\n");
    
    // 检测使用的 LLM 配置
    let (provider_name, is_kimi_code) = if let Ok(url) = std::env::var("ANTHROPIC_BASE_URL") {
        if url.contains("kimi.com/coding") {
            ("Kimi Code (会员权益)", true)
        } else {
            ("Kimi API / 其他", false)
        }
    } else if let Ok(url) = std::env::var("KIMI_BASE_URL") {
        if url.contains("moonshot.cn") {
            ("Kimi API (开放平台)", false)
        } else {
            ("Kimi / 其他", false)
        }
    } else {
        ("默认 (Moonshot)", false)
    };
    
    // 从环境变量创建 Kimi LLM
    let llm = match KimiLlm::from_env() {
        Ok(llm) => {
            println!("✅ {} 已配置", provider_name);
            if is_kimi_code {
                println!("   额度: 每周 1024 次免费调用");
                println!("   限制: 仅限白名单客户端");
            }
            llm
        }
        Err(e) => {
            eprintln!("❌ LLM 配置失败: {}", e);
            eprintln!("\n请设置以下环境变量之一：\n");
            eprintln!("【Kimi Code - 会员权益】");
            eprintln!("  $env:ANTHROPIC_BASE_URL=\"https://api.kimi.com/coding/\"");
            eprintln!("  $env:ANTHROPIC_AUTH_TOKEN=\"sk-kimi-your-key\"");
            eprintln!("\n【Kimi API - 开放平台】");
            eprintln!("  $env:KIMI_API_KEY=\"sk-your-moonshot-key\"");
            eprintln!("  $env:KIMI_BASE_URL=\"https://api.moonshot.cn/v1\"");
            return Ok(());
        }
    };
    
    // 创建工具注册表
    let registry = ToolRegistry::with_builtin_tools();
    println!("✅ 已加载 {} 个工具\n", registry.len());
    
    // 配置 Agent
    let config = AgentConfig::new()
        .with_max_iterations(5)
        .with_read_only(true);  // 安全模式
    
    // 创建 Agent
    let agent = Agent::with_config(registry, config)
        .with_llm(Arc::new(llm));
    
    // 运行对话
    let query = "你好！请介绍一下你自己。";
    println!("👤 User: {}", query);
    println!("🤖 Assistant: 正在思考...\n");
    
    match agent.run(query).await {
        Ok(response) => {
            println!("{}\n", response);
        }
        Err(e) => {
            eprintln!("❌ 错误: {}\n", e);
            if e.to_string().contains("403") {
                eprintln!("提示: 如果使用 Kimi Code，请确保:");
                eprintln!("  1. 端点是 https://api.kimi.com/coding/ (注意末尾斜杠)");
                eprintln!("  2. Key 是 Kimi Code 会员权益的 sk-kimi-... 格式");
                eprintln!("  3. 不是直接 HTTP 调用（需要模拟 Claude Code 客户端）");
            }
        }
    }
    
    Ok(())
}
