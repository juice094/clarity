//! 临时验证脚本：deepseek-device 多轮上下文连续性
//!
//! 运行方式（需先在 PowerShell 里设置 $env:DEEPSEEK_DEVICE_TOKEN）：
//!   cargo run -p clarity-llm --example deepseek_device_context_test
use clarity_contract::{LlmProvider, Message};
use clarity_llm::DeepSeekDeviceProvider;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let token = env::var("DEEPSEEK_DEVICE_TOKEN")
        .map_err(|_| "DEEPSEEK_DEVICE_TOKEN not set in environment")?;

    let provider = DeepSeekDeviceProvider::with_token(token);
    let tools = serde_json::json!([]);

    println!("--- Turn 1: 我叫张三 ---");
    let resp1 = provider
        .complete(&[Message::user("我叫张三")], &tools)
        .await?;
    println!("Response: {}", resp1.content);

    println!("--- Turn 2: 我叫什么名字 ---");
    let resp2 = provider
        .complete(&[Message::user("我叫什么名字")], &tools)
        .await?;
    println!("Response: {}", resp2.content);

    if resp2.content.contains("张三") {
        println!("✅ 上下文连续：模型记住了名字。");
    } else {
        println!("❌ 上下文断裂：模型没有说出张三。");
    }

    Ok(())
}
