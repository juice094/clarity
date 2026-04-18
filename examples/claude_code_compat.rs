//! Claude Code 配置兼容示例
//!
//! 本示例展示如何使用与 Claude Code 完全相同的环境变量配置
//!
//! ## 快速开始（与 Claude Code 相同配置）
//!
//! ### Kimi Code（推荐，每周 1024 次免费）
//! ```bash
//! # 与 Claude Code 完全一致的环境变量
//! export ANTHROPIC_BASE_URL="https://api.kimi.com/coding/"
//! export ANTHROPIC_AUTH_TOKEN="sk-kimi-your-key"
//!
//! # 可选：指定模型
//! export ANTHROPIC_MODEL="kimi-for-coding"
//!
//! cargo run --example claude_code_compat
//! ```
//!
//! ### 验证配置
//! ```bash
//! # 先检查环境变量
//! echo $ANTHROPIC_BASE_URL
//! echo $ANTHROPIC_AUTH_TOKEN
//!
//! # 运行示例
//! cargo run --example claude_code_compat
//! ```
//!
//! ## 故障排除
//!
//! ### 403 Forbidden 错误
//! Kimi Code API 会检查客户端身份。如果返回 403：
//! 1. 确认 Key 是 Kimi Code 会员权益的 Key（sk-kimi-...）
//! 2. 确认端点正确：https://api.kimi.com/coding/（注意末尾斜杠）
//! 3. 尝试在请求头中添加更多 Claude Code 特有的头信息
//!
//! ### 模型名称错误
//! Kimi Code 可能使用 `kimi-for-coding` 而不是 `kimi-k2-0711`

use clarity_core::{Agent, AgentConfig, KimiLlm, ToolRegistry, LlmFactory};
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    tracing_subscriber::fmt::init();
    
    println!("🚀 Project Clarity - Claude Code 配置兼容示例\n");
    
    // 显示当前环境变量（调试用）
    println!("环境变量检测:");
    if let Ok(url) = std::env::var("ANTHROPIC_BASE_URL") {
        println!("  ✅ ANTHROPIC_BASE_URL={}", url);
    } else {
        println!("  ❌ ANTHROPIC_BASE_URL 未设置");
    }
    
    if let Ok(token) = std::env::var("ANTHROPIC_AUTH_TOKEN") {
        let masked = if token.len() > 10 {
            format!("{}...{}", &token[..10], &token[token.len()-4..])
        } else {
            "***".into()
        };
        println!("  ✅ ANTHROPIC_AUTH_TOKEN={}", masked);
    } else {
        println!("  ❌ ANTHROPIC_AUTH_TOKEN 未设置");
    }
    
    if let Ok(model) = std::env::var("ANTHROPIC_MODEL") {
        println!("  ✅ ANTHROPIC_MODEL={}", model);
    } else {
        println!("  ℹ️  ANTHROPIC_MODEL 未设置，使用默认");
    }
    println!();
    
    // 使用 LlmFactory 自动检测
    let llm = match LlmFactory::auto().await {
        Ok(llm) => llm,
        Err(e) => {
            eprintln!("❌ 配置错误: {}", e);
            eprintln!("\n请设置以下环境变量:");
            eprintln!("  export ANTHROPIC_BASE_URL=\"https://api.kimi.com/coding/\"");
            eprintln!("  export ANTHROPIC_AUTH_TOKEN=\"sk-kimi-your-key\"");
            return Ok(());
        }
    };
    
    // 创建工具注册表
    let registry = ToolRegistry::with_builtin_tools();
    println!("✅ 已加载 {} 个工具\n", registry.len());
    
    // 配置 Agent
    let config = AgentConfig::new()
        .with_max_iterations(3)
        .with_read_only(true);
    
    // 创建 Agent
    let agent = Agent::with_config(registry, config)
        .with_llm(llm);
    
    // 测试对话
    println!("开始测试对话...\n");
    
    let queries = vec![
        "你好！请简短介绍一下你自己。",
        "你能帮我查看当前目录有哪些文件吗？",
    ];
    
    for query in queries {
        println!("👤 User: {}", query);
        println!("🤖 Assistant: 思考中...");
        
        match agent.run(query).await {
            Ok(response) => {
                println!("{}\n", response);
            }
            Err(e) => {
                eprintln!("❌ 错误: {}", e);
                if e.to_string().contains("403") {
                    eprintln!("\n💡 提示: Kimi Code 可能需要特定的请求头。");
                    eprintln!("   请确认:");
                    eprintln!("   1. Key 是有效的 Kimi Code 会员权益 Key");
                    eprintln!("   2. 端点正确: https://api.kimi.com/coding/");
                    eprintln!("   3. 模型名称: kimi-for-coding");
                }
                break;
            }
        }
    }
    
    Ok(())
}
