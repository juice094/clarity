//! DeepSeek 设备登录 Provider 端到端测试
//!
//! 需要真实 token 或手机号密码。运行方式：
#![allow(clippy::expect_used)]
//!
//! ```bash
//! # token 模式
//! DEEPSEEK_DEVICE_TOKEN="你的 MMKV token" \
//!   cargo test -p clarity-llm --test deepseek_device_e2e -- --ignored --nocapture
//!
//! # 手机号密码模式
//! DEEPSEEK_DEVICE_MOBILE="136****" DEEPSEEK_DEVICE_PASSWORD="***" \
//!   cargo test -p clarity-llm --test deepseek_device_e2e -- --ignored --nocapture
//! ```

use clarity_llm::{DeepSeekDeviceProvider, LlmProvider};

fn provider_from_env() -> Option<DeepSeekDeviceProvider> {
    if let Ok(token) = std::env::var("DEEPSEEK_DEVICE_TOKEN") {
        return Some(DeepSeekDeviceProvider::with_token(token));
    }
    if let (Ok(mobile), Ok(password)) = (
        std::env::var("DEEPSEEK_DEVICE_MOBILE"),
        std::env::var("DEEPSEEK_DEVICE_PASSWORD"),
    ) {
        return Some(DeepSeekDeviceProvider::with_password(mobile, password));
    }
    None
}

#[tokio::test]
#[ignore]
async fn test_e2e_complete_hello() {
    let provider = provider_from_env()
        .expect("set DEEPSEEK_DEVICE_TOKEN or DEEPSEEK_DEVICE_MOBILE+DEEPSEEK_DEVICE_PASSWORD");

    let response = provider
        .complete(
            &[clarity_llm::Message::user("hello")],
            &serde_json::Value::Null,
        )
        .await
        .expect("complete should succeed");

    println!("response: {:?}", response);
    assert!(!response.content.is_empty(), "response should not be empty");
    assert!(
        !response.content.contains("FINISHED"),
        "response must not contain stream terminator sentinel"
    );
}

#[tokio::test]
#[ignore]
async fn test_e2e_stream_hello() {
    let provider = provider_from_env()
        .expect("set DEEPSEEK_DEVICE_TOKEN or DEEPSEEK_DEVICE_MOBILE+DEEPSEEK_DEVICE_PASSWORD");

    let mut stream = provider
        .stream(
            &[clarity_llm::Message::user("hello")],
            &serde_json::Value::Null,
        )
        .expect("stream should start");

    let mut received = String::new();
    while let Some(delta) = stream.recv().await {
        let delta = delta.expect("stream delta should be ok");
        if let Some(text) = delta.content {
            received.push_str(&text);
            print!("{}", text);
        }
    }
    println!();
    assert!(!received.is_empty(), "streamed content should not be empty");
    assert!(
        !received.contains("FINISHED"),
        "streamed content must not contain stream terminator sentinel"
    );
}
