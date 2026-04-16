use clarity_core::mcp::enhanced::{McpClient, McpClientBuilder};

#[tokio::test]
async fn test_stdio_debug_git_no_timeout() {
    let repo_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut client = McpClientBuilder::stdio("git", "uvx")
        .arg("mcp-server-git")
        .arg("--repository")
        .arg(repo_path.to_string_lossy().to_string())
        .build();

    let start = std::time::Instant::now();
    match client.connect().await {
        Ok(()) => println!("GIT Connected in {:?}", start.elapsed()),
        Err(e) => println!("GIT Connect error after {:?}: {:?}", start.elapsed(), e),
    }

    let start = std::time::Instant::now();
    match client.list_tools().await {
        Ok(tools) => println!("GIT Listed {} tools in {:?}", tools.len(), start.elapsed()),
        Err(e) => println!("GIT List tools error after {:?}: {:?}", start.elapsed(), e),
    }
}
