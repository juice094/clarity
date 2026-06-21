//! 回归测试：deepseek-device 多轮上下文 + provider state 持久化
//!
//! 模拟 clarity 重启场景：
//! 1. 第一轮和第二轮在同 provider 实例上完成，验证上下文连续。
//! 2. 捕获 provider state。
//! 3. 新建 provider 实例并恢复 state，验证第三轮仍能记住之前的信息。
//!
//! 运行：
//!   cargo run -p clarity-llm --example deepseek_device_persistence_test
use clarity_contract::{LlmProvider, Message};
use clarity_llm::DeepSeekDeviceProvider;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let token = env::var("DEEPSEEK_DEVICE_TOKEN")
        .map_err(|_| "DEEPSEEK_DEVICE_TOKEN not set in environment")?;
    let tools = serde_json::json!([]);

    println!("=== Provider instance A ===");
    let provider_a = DeepSeekDeviceProvider::with_token(token.clone());

    println!("--- Turn 1: 我叫张三 ---");
    let resp1 = provider_a
        .complete(&[Message::user("我叫张三")], &tools)
        .await?;
    println!("Response: {}", resp1.content);

    println!("--- Turn 2: 我叫什么名字 ---");
    let resp2 = provider_a
        .complete(&[Message::user("我叫什么名字")], &tools)
        .await?;
    println!("Response: {}", resp2.content);
    let context_ok_in_same_instance = resp2.content.contains("张三");

    println!("--- Capturing provider state ---");
    let state_blob = provider_a
        .capture_provider_state()
        .ok_or("provider did not return state after two turns")?;
    println!("Captured state: {}", state_blob);

    println!("\n=== Provider instance B (simulated restart) ===");
    let provider_b = DeepSeekDeviceProvider::with_token(token);
    provider_b.restore_provider_state(&state_blob);

    println!("--- Turn 3: 还记得我叫什么吗 ---");
    let resp3 = provider_b
        .complete(&[Message::user("还记得我叫什么吗")], &tools)
        .await?;
    println!("Response: {}", resp3.content);
    let context_ok_after_restore = resp3.content.contains("张三");

    println!("\n=== Result ===");
    if context_ok_in_same_instance && context_ok_after_restore {
        println!("✅ 同一实例内上下文连续，且 provider state 恢复后仍能记住名字。");
        Ok(())
    } else {
        eprintln!(
            "❌ 回归失败：same_instance={} after_restore={}",
            context_ok_in_same_instance, context_ok_after_restore
        );
        Err("persistence regression".into())
    }
}
