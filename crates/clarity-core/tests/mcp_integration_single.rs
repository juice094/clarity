use clarity_core::mcp::config::{McpConfig, McpServerEntry};
use clarity_core::mcp::McpManager;
use std::collections::HashMap;

#[tokio::test]
#[ignore = "Requires Node.js/npx"]
async fn test_filesystem_only() {
    let temp_dir = tempfile::tempdir().unwrap();

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

    let manager = tokio::time::timeout(
        std::time::Duration::from_secs(120),
        McpManager::from_config(&config),
    )
    .await
    .expect("Timed out");

    let tools = manager.tools();
    eprintln!("Tools: {:?}", tools.iter().map(|t| t.name()).collect::<Vec<_>>());
    assert!(!tools.is_empty(), "Expected filesystem tools");
}
