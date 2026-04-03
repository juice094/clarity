//! MCP (Model Context Protocol) Integration Demo
//!
//! This example demonstrates how to connect to an external MCP server
//! via stdio, discover its tools, and register them in the Clarity
//! ToolRegistry so they can be used by the Agent.
//!
//! ## Prerequisites
//!
//! Install an MCP server, for example the filesystem server:
//! ```bash
//! npm install -g @modelcontextprotocol/server-filesystem
//! ```
//!
//! Or use npx (no install required):
//! ```powershell
//! cargo run --example mcp_demo -- "npx" "-y" "@modelcontextprotocol/server-filesystem" "."
//! ```
//!
//! ## Run
//! ```powershell
//! cargo run --example mcp_demo -- <command> [args...]
//! ```

use clarity_core::mcp::{McpClient, McpManager, McpToolAdapter};
use clarity_core::ToolRegistry;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    println!("🔌 MCP Integration Demo\n");

    // Parse command-line arguments for the MCP server command
    let args: Vec<String> = std::env::args().collect();
    let (cmd, mcp_args) = if args.len() > 1 {
        let cmd = &args[1];
        let mcp_args = args[2..].to_vec();
        (cmd.clone(), mcp_args)
    } else {
        println!("Usage: cargo run --example mcp_demo -- <command> [args...]");
        println!();
        println!("Example (filesystem server with npx):");
        println!("  cargo run --example mcp_demo -- npx -y @modelcontextprotocol/server-filesystem .");
        println!();
        println!("Example (custom mock server):");
        println!("  cargo run --example mcp_demo -- python mock_mcp_server.py");
        return Ok(());
    };

    // ------------------------------------------------------------------
    // 1. Connect to an MCP server via stdio
    // ------------------------------------------------------------------
    println!("▶️  Connecting to MCP server: {} {:?}", cmd, mcp_args);
    let client = match McpClient::connect_stdio(&cmd, &mcp_args).await {
        Ok(client) => client,
        Err(e) => {
            eprintln!("❌ Failed to connect to MCP server: {}", e);
            eprintln!("Make sure the command '{}' is available in PATH.", cmd);
            return Ok(());
        }
    };
    println!("✅ Connected to MCP server\n");

    // ------------------------------------------------------------------
    // 2. Discover tools from the MCP server
    // ------------------------------------------------------------------
    let tools = client.list_tools().await?;
    println!("🔧 Discovered {} MCP tool(s):", tools.len());
    for tool in &tools {
        println!("  - {}: {}", tool.name, tool.description);
    }
    println!();

    // ------------------------------------------------------------------
    // 3. Wrap each MCP tool as a Clarity Tool and register them
    // ------------------------------------------------------------------
    let registry = ToolRegistry::new();
    for tool in tools {
        let adapter = McpToolAdapter::new(client.clone(), tool);
        registry.register(adapter)?;
    }
    println!("✅ Registered {} tool(s) into ToolRegistry\n", registry.len()?);

    // ------------------------------------------------------------------
    // 4. Show tool schemas (as they would appear to an LLM)
    // ------------------------------------------------------------------
    let schemas = registry.get_tool_schemas()?;
    println!("📋 Tool schemas for LLM:");
    println!("{}", serde_json::to_string_pretty(&schemas)?);
    println!();

    // ------------------------------------------------------------------
    // 5. Alternative: using McpManager for multiple connections
    // ------------------------------------------------------------------
    let manager = McpManager::new();
    manager.add_client("filesystem", client).await?;
    println!("✅ Added client to McpManager");

    let all_tools = manager.get_all_tools().await;
    println!("📊 McpManager reports {} total tool(s)\n", all_tools.len());

    // ------------------------------------------------------------------
    // 6. Clean up
    // ------------------------------------------------------------------
    manager.disconnect_all().await?;
    println!("👋 Disconnected from MCP server");

    Ok(())
}
