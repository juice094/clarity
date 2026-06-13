#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    missing_docs,
    unsafe_code
)]
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

use clarity_core::ToolRegistry;
use clarity_core::mcp::{McpClient, McpClientBuilder, McpManager, McpToolAdapter};

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
        println!(
            "  cargo run --example mcp_demo -- npx -y @modelcontextprotocol/server-filesystem ."
        );
        println!();
        println!("Example (custom mock server):");
        println!("  cargo run --example mcp_demo -- python mock_mcp_server.py");
        return Ok(());
    };

    // ------------------------------------------------------------------
    // 1. Connect to an MCP server via stdio
    // ------------------------------------------------------------------
    println!("▶️  Connecting to MCP server: {} {:?}", cmd, mcp_args);
    let mut builder = McpClientBuilder::stdio("demo", &cmd);
    for arg in &mcp_args {
        builder = builder.arg(arg);
    }
    let mut client = builder.build();
    if let Err(e) = client.connect().await {
        eprintln!("❌ Failed to connect to MCP server: {}", e);
        eprintln!("Make sure the command '{}' is available in PATH.", cmd);
        return Ok(());
    }
    println!("✅ Connected to MCP server\n");

    // ------------------------------------------------------------------
    // 2. Discover tools from the MCP server
    // ------------------------------------------------------------------
    let tools = client.list_tools().await?;
    println!("🔧 Discovered {} MCP tool(s):", tools.len());
    for tool in &tools {
        println!(
            "  - {}: {}",
            tool.name,
            tool.description.as_deref().unwrap_or("")
        );
    }
    println!();

    // ------------------------------------------------------------------
    // 3. Wrap each MCP tool as a Clarity Tool and register them
    // ------------------------------------------------------------------
    let registry = ToolRegistry::new();
    let client = std::sync::Arc::new(tokio::sync::Mutex::new(client));
    for tool in tools {
        let adapter = McpToolAdapter::new(client.clone(), tool);
        registry.register(adapter)?;
    }
    println!(
        "✅ Registered {} tool(s) into ToolRegistry\n",
        registry.len()?
    );

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
    let mut manager = McpManager::new();
    manager.connect_stdio("filesystem", &cmd, &mcp_args).await?;
    println!("✅ Added client to McpManager");

    let all_tools = manager.tools();
    println!("📊 McpManager reports {} total tool(s)\n", all_tools.len());

    // ------------------------------------------------------------------
    // 6. Clean up
    // ------------------------------------------------------------------
    println!("👋 Disconnected from MCP server");

    Ok(())
}
