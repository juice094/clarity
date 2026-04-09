use clarity_core::mcp::config::{McpConfig, McpServerEntry};
use clarity_core::mcp::McpManager;
use clarity_core::tools::{Tool, ToolContext};
use std::collections::HashMap;

#[tokio::test]
#[ignore = "Requires network, Node.js/npx, and uvx"]
async fn test_real_mcp_servers() {
    let temp_dir = tempfile::tempdir().unwrap();
    let repo_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let mut config = McpConfig::default();
    config.servers.insert(
        "filesystem".to_string(),
        McpServerEntry {
            command: "npx".to_string(),
            args: vec![
                "-y".to_string(),
                "@modelcontextprotocol/server-filesystem".to_string(),
                temp_dir.path().to_string_lossy().to_string(),
            ],
            env: HashMap::new(),
            disabled: false,
        },
    );
    config.servers.insert(
        "git".to_string(),
        McpServerEntry {
            command: "uvx".to_string(),
            args: vec![
                "mcp-server-git".to_string(),
                "--repository".to_string(),
                repo_path.to_string_lossy().to_string(),
            ],
            env: HashMap::new(),
            disabled: false,
        },
    );
    config.servers.insert(
        "brave-search".to_string(),
        McpServerEntry {
            command: "npx".to_string(),
            args: vec![
                "-y".to_string(),
                "@modelcontextprotocol/server-brave-search".to_string(),
            ],
            env: {
                let mut env = HashMap::new();
                env.insert("BRAVE_API_KEY".to_string(), "dummy-key".to_string());
                env
            },
            disabled: false,
        },
    );

    let manager = tokio::time::timeout(
        std::time::Duration::from_secs(120),
        McpManager::from_config(&config),
    )
    .await
    .expect("Timed out waiting for MCP servers to start");

    let tools = manager.tools();
    assert!(
        !tools.is_empty(),
        "Expected at least some tools from real servers"
    );

    // Filesystem tool test
    let fs_tool = tools.iter().find(|t| t.name() == "read_file");
    if let Some(tool) = fs_tool {
        let test_file = temp_dir.path().join("test.txt");
        std::fs::write(&test_file, "hello mcp").unwrap();
        let args = serde_json::json!({
            "path": test_file.to_string_lossy().to_string()
        });
        let result = tool.execute(args, ToolContext::new()).await;
        assert!(result.is_ok(), "read_file failed: {:?}", result);
        let text = result.unwrap().as_str().unwrap_or("").to_string();
        assert!(
            text.contains("hello mcp"),
            "Expected file content, got: {}",
            text
        );
    } else {
        let list_tool = tools
            .iter()
            .find(|t| t.name() == "list_directory")
            .expect("No filesystem tool found");
        let args = serde_json::json!({
            "path": temp_dir.path().to_string_lossy().to_string()
        });
        let result = list_tool.execute(args, ToolContext::new()).await;
        assert!(result.is_ok(), "list_directory failed: {:?}", result);
    }

    // Git tool test
    if let Some(tool) = tools.iter().find(|t| t.name() == "git_status") {
        let args = serde_json::json!({
            "repo_path": repo_path.to_string_lossy().to_string()
        });
        let result = tool.execute(args, ToolContext::new()).await;
        assert!(result.is_ok(), "git_status failed: {:?}", result);
    }

    // Brave search tool test (dummy key -> expect error or empty)
    if let Some(tool) = tools.iter().find(|t| t.name() == "brave_web_search") {
        let args = serde_json::json!({ "query": "rust programming" });
        let result = tool.execute(args, ToolContext::new()).await;
        match result {
            Ok(_) => {}
            Err(e) => {
                let msg = e.to_string().to_lowercase();
                assert!(
                    msg.contains("unauthorized")
                        || msg.contains("api key")
                        || msg.contains("error")
                        || msg.contains("unavailable"),
                    "Expected meaningful error for brave search with dummy key, got: {}",
                    e
                );
            }
        }
    }
}
