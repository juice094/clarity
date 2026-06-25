//! DeepSeek 设备登录 Provider 端到端测试
//!
//! 需要真实 token 或手机号密码。运行方式：
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
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

#[tokio::test]
#[ignore]
async fn test_e2e_tool_call_powershell_list_dir() {
    let provider = provider_from_env()
        .expect("set DEEPSEEK_DEVICE_TOKEN or DEEPSEEK_DEVICE_MOBILE+DEEPSEEK_DEVICE_PASSWORD");

    let tools = serde_json::json!([
        {
            "type": "function",
            "function": {
                "name": "powershell",
                "description": "Run a PowerShell command on Windows.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": "The PowerShell command to run"
                        }
                    },
                    "required": ["command"]
                }
            }
        }
    ]);

    let prompt = "List the files in the current directory using the powershell tool. Output exactly one XML <tool> block and then wait for the result.";
    let first_messages = vec![clarity_llm::Message::user(prompt)];
    let response = provider
        .complete(&first_messages, &tools)
        .await
        .expect("first complete should succeed");

    println!("first response content:\n{}", response.content);
    assert!(
        !response.content.is_empty(),
        "first response should not be empty"
    );
    assert!(
        !response.content.contains("FINISHED"),
        "response must not contain stream terminator sentinel"
    );

    // Parse the XML tool call produced by prompt-guided generation.
    let tool_re = regex::Regex::new(r#"(?s)<tool\s+name=["']([^"']+)["'][^>]*>(.*?)</tool>"#)
        .expect("tool regex should compile");
    let caps = tool_re
        .captures(&response.content)
        .expect("expected a <tool> block in the response");
    let tool_name = caps.get(1).unwrap().as_str();
    let tool_body = caps.get(2).unwrap().as_str();
    assert_eq!(tool_name, "powershell", "expected powershell tool call");

    // The model may emit parameters in several XML forms. Accept the same
    // variants that the production XML tool parser supports.
    let command = if let Some(caps) =
        regex::Regex::new(r#"<arg\s+key=["']command["']\s*>(.*?)</arg>"#)
            .unwrap()
            .captures(tool_body)
    {
        caps.get(1).unwrap().as_str().trim()
    } else if let Some(caps) = regex::Regex::new(r#"<command>(.*?)</command>"#)
        .unwrap()
        .captures(tool_body)
    {
        caps.get(1).unwrap().as_str().trim()
    } else if let Some(caps) = regex::Regex::new(r#"command\s*=\s*["']([^"']+)["']"#)
        .unwrap()
        .captures(&response.content)
    {
        caps.get(1).unwrap().as_str().trim()
    } else {
        // Raw text directly inside <tool> (no arg wrapper tags).
        // The model emitted the command as the sole content of the <tool> block.
        let raw = tool_body.trim();
        assert!(!raw.is_empty(), "raw tool body should not be empty");
        raw
    };
    println!("parsed command: {}", command);

    // Basic safety gate: only run directory-listing-style commands and reject
    // obviously destructive cmdlets.
    let lower = command.to_lowercase();
    let allowed_prefix = lower.starts_with("get-childitem")
        || lower.starts_with("dir")
        || lower.starts_with("ls")
        || lower.starts_with("write-output")
        || lower.starts_with("echo");
    assert!(
        allowed_prefix,
        "command does not start with a known safe cmdlet: {command}"
    );
    let dangerous = [
        "remove-item",
        "rm ",
        "del ",
        "erase",
        "format-volume",
        "clear-content",
        "set-content",
        "out-file",
        "invoke-expression",
        "iex",
        "invoke-webrequest",
        "start-process",
        "reg delete",
    ];
    assert!(
        !dangerous.iter().any(|d| lower.contains(d)),
        "refusing to run potentially destructive command: {command}"
    );

    let output = tokio::process::Command::new("powershell")
        .arg("-Command")
        .arg(command)
        .current_dir(".")
        .output()
        .await
        .expect("powershell should spawn");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    println!("powershell stdout:\n{}", stdout);
    println!("powershell stderr:\n{}", stderr);
    assert!(output.status.success(), "powershell command failed");

    // Second turn: feed the tool result back and ask the model to summarize.
    let result_prompt = format!(
        "Here is the result of the powershell command:\n{}\n\nSummarize what files were found.",
        stdout
    );
    let final_response = provider
        .complete(&[clarity_llm::Message::user(result_prompt)], &tools)
        .await
        .expect("second complete should succeed");

    println!("final response content:\n{}", final_response.content);
    assert!(
        !final_response.content.is_empty(),
        "final response should not be empty"
    );
    assert!(
        !final_response.content.contains("FINISHED"),
        "final response must not contain stream terminator sentinel"
    );
}
