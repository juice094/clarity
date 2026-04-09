use clarity_core::mcp::{McpClient, McpClientBuilder, McpServerConfig};

#[tokio::test]
async fn debug_stdio_connect() {
    let mut config = McpServerConfig::stdio("fs", "npx");
    if let clarity_core::mcp::McpTransport::Stdio { args, .. } = &mut config.transport {
        args.push("-y".to_string());
        args.push("@modelcontextprotocol/server-filesystem".to_string());
        args.push(".".to_string());
    }
    let mut client = McpClientBuilder::from_config(config);
    let result = client.connect().await;
    eprintln!("connect result: {:?}", result);
    if result.is_ok() {
        let tools = client.list_tools().await;
        eprintln!("list_tools result: {:?}", tools);
    }
}
