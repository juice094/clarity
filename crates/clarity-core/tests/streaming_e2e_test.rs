#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    missing_docs,
    unsafe_code
)]
//! 端到端流式响应集成测试
//!
//! 启动一个本地 mock HTTP server，模拟 OpenAI SSE 流式响应，
//! 验证 OpenAiCompatibleLlm + Agent::run_streaming 的完整网络链路。

use clarity_core::agent::AgentConfig;
use clarity_core::{Agent, ToolRegistry};
use clarity_llm::OpenAiCompatibleLlm;
use clarity_wire::{Wire, WireMessage};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio::time::{Duration, timeout};

async fn run_mock_server(port: u16, mut shutdown: oneshot::Receiver<()>) -> Vec<String> {
    let listener = TcpListener::bind(("127.0.0.1", port)).await.unwrap();
    let mut requests = Vec::new();

    loop {
        tokio::select! {
            Ok((mut stream, _)) = listener.accept() => {
                let mut buf = vec![0u8; 8192];
                let n = stream.read(&mut buf).await.unwrap();
                let req = String::from_utf8_lossy(&buf[..n]);
                requests.push(req.to_string());

                let is_stream = req.contains(r#""stream":true"#);

                if is_stream {
                    let body = "data: {\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}\n\n\
                               data: {\"choices\":[{\"delta\":{\"content\":\" world\"}}]}\n\n\
                               data: [DONE]\n\n";
                    let response = format!(
                        "HTTP/1.1 200 OK\r\n\
                         Content-Type: text/event-stream\r\n\
                         Transfer-Encoding: chunked\r\n\r\n\
                         {:x}\r\n{}\r\n0\r\n\r\n",
                        body.len(),
                        body
                    );
                    stream.write_all(response.as_bytes()).await.unwrap();
                } else {
                    let body = r#"{"choices":[{"message":{"content":"","role":"assistant"},"finish_reason":"stop","index":0}],"model":"mock"}"#;
                    let response = format!(
                        "HTTP/1.1 200 OK\r\n\
                         Content-Type: application/json\r\n\
                         Content-Length: {}\r\n\r\n\
                         {}",
                        body.len(),
                        body
                    );
                    stream.write_all(response.as_bytes()).await.unwrap();
                }
            }
            _ = &mut shutdown => break,
        }
    }

    requests
}

#[tokio::test]
async fn test_openai_streaming_e2e() {
    let port = 19876u16;
    let (tx, rx) = oneshot::channel();

    let server_handle = tokio::spawn(run_mock_server(port, rx));

    // 构造 OpenAI 兼容 LLM，指向本地 mock
    let llm = OpenAiCompatibleLlm::new(
        "fake-key",
        format!("http://127.0.0.1:{}", port),
        "mock-model",
    );
    let wire = Wire::new();
    let mut ui_side = wire.ui_side(false);

    let agent = Agent::with_config(ToolRegistry::new(), AgentConfig::default())
        .with_llm(Arc::new(llm))
        .with_wire(Arc::new(wire));

    let result = agent.run_streaming("test query").await;

    // 允许 server 优雅退出
    let _ = tx.send(());
    let reqs = server_handle.await.unwrap();

    assert!(result.is_ok(), "run_streaming failed: {:?}", result);
    assert_eq!(result.unwrap(), "Hello world");

    // Collect ContentPart chunks emitted through the wire.
    let mut received = Vec::new();
    loop {
        match timeout(Duration::from_millis(500), ui_side.recv()).await {
            Ok(Some(WireMessage::ContentPart { text, .. })) => {
                assert!(!text.is_empty(), "chunk should not be empty");
                received.push(text);
            }
            Ok(Some(_)) => continue,
            _ => break,
        }
    }

    assert!(!received.is_empty(), "should receive at least one chunk");
    assert_eq!(received.join(""), "Hello world");
    // Verify chunks appear in order without gaps or overlaps
    let mut pos = 0usize;
    for chunk in &received {
        assert!(
            "Hello world"[pos..].starts_with(chunk),
            "chunk {:?} doesn't match at offset {}",
            chunk,
            pos
        );
        pos += chunk.len();
    }
    assert_eq!(pos, "Hello world".len());

    // 验证收到了 complete 和 stream 两个请求
    let complete_count = reqs
        .iter()
        .filter(|r| !r.contains(r#""stream":true"#))
        .count();
    let stream_count = reqs
        .iter()
        .filter(|r| r.contains(r#""stream":true"#))
        .count();
    assert_eq!(stream_count, 1, "expected 1 stream request");
    // When streaming succeeds, run_streaming should not fall back to complete().
    assert_eq!(
        complete_count, 0,
        "expected 0 complete request when stream succeeds"
    );
}
