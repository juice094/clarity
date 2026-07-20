//! DeepSeek Device password-login test — gets a fresh token via mobile + password.
//!
//! Usage:
//!   $env:DS_MOBILE="13800138000"
//!   $env:DS_PASSWORD="your-password"
//!   cargo run -p clarity-core --example test_device_login

use clarity_contract::LlmProvider;
use clarity_llm::{
    DeepSeekDeviceConfig, DeepSeekDeviceCredentials, DeepSeekDeviceOptions, DeepSeekDeviceProvider,
};

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

    println!("=== DeepSeek Device Password Login Test ===\n");
    println!(
        "Mobile: {}****{}",
        &mobile[..3],
        &mobile[mobile.len() - 3..]
    );

    // Build provider with password credentials — this will auto-login on first use
    let provider = DeepSeekDeviceProvider::new(DeepSeekDeviceConfig {
        base_url: "https://chat.deepseek.com".to_string(),
        client_version: "2.1.8".to_string(),
        device_id: format!("clarity-login-test-{}", std::process::id()),
        credentials: DeepSeekDeviceCredentials::Password {
            mobile: mobile.clone(),
            password: password.clone(),
        },
        options: DeepSeekDeviceOptions::from_model_id("deepseek-chat"),
    });

    // Test 1: Just send a simple completion — this triggers auto-login internally
    println!("\n--- Test: complete() (auto-login on first call) ---");
    let messages = vec![clarity_contract::Message {
        role: clarity_contract::MessageRole::User,
        content: "你好，请回复OK即可。".to_string(),
        tool_calls: None,
        tool_call_id: None,
    }];
    let tools = serde_json::json!([]);

    match provider.complete(&messages, &tools).await {
        Ok(response) => {
            println!("✅ SUCCESS: {} chars", response.content.len());
            println!(
                "Content: {}",
                &response.content[..response.content.len().min(200)]
            );
        }
        Err(e) => {
            eprintln!("❌ FAILED: {:?}", e);
            std::process::exit(1);
        }
    }

    println!("\n=== Test Complete ===");
}
