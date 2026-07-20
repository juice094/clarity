use super::*;
use std::collections::HashMap;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn no_proxy_client() -> reqwest::Client {
    reqwest::Client::builder()
        .no_proxy()
        .build()
        .expect("test client build should not fail")
}

// =============================================================================
// Tests
// =============================================================================

#[test]
fn test_stdio_config() {
    let config = McpServerConfig::stdio("test", "npx");
    assert!(matches!(&config.transport, McpTransport::Stdio { command, .. } if command == "npx"));
}

#[test]
fn test_http_config() {
    let config = McpServerConfig::http("api", "https://api.example.com/mcp")
        .with_header("Authorization", "Bearer token");

    if let McpTransport::Http { url, headers, .. } = &config.transport {
        assert_eq!(url, "https://api.example.com/mcp");
        assert_eq!(
            headers.get("Authorization"),
            Some(&"Bearer token".to_string())
        );
    } else {
        panic!("Expected HTTP transport");
    }
}

#[test]
fn test_mcp_registry() {
    let mut registry = McpRegistry::new();
    let client = McpClientBuilder::stdio("test", "echo").build();
    registry.register("test", client);

    assert_eq!(registry.list(), vec!["test"]);
    assert!(registry.get("test").is_some());
    assert!(registry.get("missing").is_none());
}

#[test]
fn test_mcp_client_instance() {
    let instance = McpClientBuilder::stdio("test", "echo").build();
    assert!(matches!(instance, McpClientInstance::Stdio(_)));
}

#[test]
fn test_validate_command_bare_name_allowed() {
    assert!(validate_mcp_command_with_allowlist("npx", None).is_ok());
    assert!(validate_mcp_command_with_allowlist("node", None).is_ok());
    assert!(validate_mcp_command_with_allowlist("uvx", None).is_ok());
}

#[test]
fn test_validate_command_rejects_metacharacters() {
    assert!(validate_mcp_command_with_allowlist("bash; rm -rf /", None).is_err());
    assert!(validate_mcp_command_with_allowlist("node | curl", None).is_err());
    assert!(validate_mcp_command_with_allowlist("npx && evil", None).is_err());
    assert!(validate_mcp_command_with_allowlist("`whoami`", None).is_err());
    assert!(validate_mcp_command_with_allowlist("$(id)", None).is_err());
}

#[test]
fn test_validate_command_rejects_relative_paths() {
    assert!(validate_mcp_command_with_allowlist("../evil.exe", None).is_err());
    assert!(validate_mcp_command_with_allowlist("./script.sh", None).is_err());
    assert!(validate_mcp_command_with_allowlist("subdir/binary", None).is_err());
}

#[test]
fn test_validate_command_allowlist_override() {
    let allowlist = "/usr/bin/npx,/opt/bin";
    // Allowed because it matches exactly
    assert!(validate_mcp_command_with_allowlist("/usr/bin/npx", Some(allowlist)).is_ok());
    // Allowed because it starts with /opt/bin/
    assert!(validate_mcp_command_with_allowlist("/opt/bin/tool", Some(allowlist)).is_ok());
    // Blocked
    assert!(validate_mcp_command_with_allowlist("/usr/bin/node", Some(allowlist)).is_err());
    assert!(validate_mcp_command_with_allowlist("npx", Some(allowlist)).is_err());
}

#[test]
fn test_resource_types_deserialize() {
    let json = serde_json::json!({
        "resources": [
            {
                "uri": "file:///tmp/test.txt",
                "name": "test.txt",
                "mimeType": "text/plain",
                "description": "A test file"
            }
        ]
    });
    let result: ListResourcesResult = serde_json::from_value(json).unwrap();
    assert_eq!(result.resources.len(), 1);
    assert_eq!(result.resources[0].uri, "file:///tmp/test.txt");
    assert_eq!(result.resources[0].name.as_ref().unwrap(), "test.txt");
}

#[test]
fn test_read_resource_result_deserialize() {
    let json = serde_json::json!({
        "contents": [
            {
                "uri": "file:///tmp/test.txt",
                "mimeType": "text/plain",
                "text": "Hello, world!"
            }
        ]
    });
    let result: ReadResourceResult = serde_json::from_value(json).unwrap();
    assert_eq!(result.contents.len(), 1);
    match &result.contents[0] {
        ResourceContents::Text(t) => {
            assert_eq!(t.text, "Hello, world!");
        }
        _ => panic!("Expected text resource"),
    }
}

#[test]
fn test_prompt_types_deserialize() {
    let json = serde_json::json!({
        "prompts": [
            {
                "name": "code-review",
                "description": "Review code changes",
                "arguments": [
                    {
                        "name": "pr_number",
                        "description": "The PR number",
                        "required": true
                    }
                ]
            }
        ]
    });
    let result: ListPromptsResult = serde_json::from_value(json).unwrap();
    assert_eq!(result.prompts.len(), 1);
    assert_eq!(result.prompts[0].name, "code-review");
    let args = result.prompts[0].arguments.as_ref().unwrap();
    assert_eq!(args[0].name, "pr_number");
    assert_eq!(args[0].required, Some(true));
}

#[test]
fn test_get_prompt_result_deserialize() {
    let json = serde_json::json!({
        "description": "Code review prompt",
        "messages": [
            {
                "role": "user",
                "content": {
                    "type": "text",
                    "text": "Please review this code."
                }
            }
        ]
    });
    let result: GetPromptResult = serde_json::from_value(json).unwrap();
    assert_eq!(result.description.as_ref().unwrap(), "Code review prompt");
    assert_eq!(result.messages.len(), 1);
    assert!(matches!(result.messages[0].role, PromptMessageRole::User));
    assert!(matches!(
        result.messages[0].content,
        PromptContent::Text { .. }
    ));
}

#[tokio::test]
async fn test_sse_connect_timeout_without_endpoint() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = vec![0u8; 1024];
        let _ = socket.read(&mut buf).await;
        // Send SSE stream but NEVER send endpoint event
        let headers = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n\r\n";
        socket.write_all(headers.as_bytes()).await.unwrap();
        // Keep connection open long enough for timeout
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    let config = McpServerConfig {
        name: "test".to_string(),
        transport: McpTransport::Sse {
            url: format!("http://127.0.0.1:{}/sse", port),
            headers: HashMap::new(),
            timeout_seconds: 1,
            reconnect_delay_ms: 5000,
        },
        oauth: None,
    };
    let mut client = SseMcpClient::with_client(config, no_proxy_client());

    let result = client.connect().await;
    assert!(
        result.is_err(),
        "Expected timeout when endpoint is not sent"
    );
    assert!(matches!(result.unwrap_err(), McpError::RequestTimeout));
}

#[tokio::test]
async fn test_sse_full_handshake() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let base = format!("http://127.0.0.1:{}", port);

    tokio::spawn(async move {
        // SSE connection
        let (mut sse_socket, _) = listener.accept().await.unwrap();
        let mut buf = vec![0u8; 1024];
        let n = sse_socket.read(&mut buf).await.unwrap();
        assert!(String::from_utf8_lossy(&buf[..n]).contains("GET /sse"));

        sse_socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n\r\n")
            .await
            .unwrap();
        sse_socket
            .write_all(b"event: endpoint\r\ndata: /messages?sid=abc\r\n\r\n")
            .await
            .unwrap();

        // POST for initialize (id=1)
        let (mut post1, _) = listener.accept().await.unwrap();
        let mut buf1 = vec![0u8; 4096];
        let n1 = post1.read(&mut buf1).await.unwrap();
        let req1 = String::from_utf8_lossy(&buf1[..n1]);
        assert!(req1.contains("POST /messages?sid=abc"));
        assert!(req1.contains("initialize"));
        post1
            .write_all(b"HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\n\r\n")
            .await
            .unwrap();
        drop(post1);

        // Send initialize response via SSE
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        let init_resp = r#"data: {"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2024-11-05","serverInfo":{"name":"test","version":"1.0"}}}"#.to_string() + "\r\n\r\n";
        sse_socket.write_all(init_resp.as_bytes()).await.unwrap();

        // POST for notifications/initialized (id=2) — fire-and-forget
        let (mut post2, _) = listener.accept().await.unwrap();
        let mut buf2 = vec![0u8; 4096];
        let n2 = post2.read(&mut buf2).await.unwrap();
        let req2 = String::from_utf8_lossy(&buf2[..n2]);
        assert!(req2.contains("notifications/initialized"));
        post2
            .write_all(b"HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\n\r\n")
            .await
            .unwrap();
        drop(post2);
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    let config = McpServerConfig {
        name: "test".to_string(),
        transport: McpTransport::Sse {
            url: format!("{}/sse", base),
            headers: HashMap::new(),
            timeout_seconds: 10,
            reconnect_delay_ms: 5000,
        },
        oauth: None,
    };
    let mut client = SseMcpClient::with_client(config, no_proxy_client());

    let result = client.connect().await;
    assert!(result.is_ok(), "connect failed: {:?}", result.err());

    client.disconnect().await.unwrap();
}
