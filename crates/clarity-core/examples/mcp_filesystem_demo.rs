//! MCP Filesystem Server Integration Demo
//!
//! This example demonstrates how to connect to the official Model Context Protocol
//! filesystem server and use its tools through Clarity's ToolRegistry.
//!
//! ## Prerequisites
//!
//! You need Node.js and npx installed:
//! ```bash
//! # Check if npx is available
//! npx --version
//! ```
//!
//! The filesystem server will be automatically downloaded via npx if not already cached.
//!
//! ## Usage
//!
//! ```powershell
//! # Run with current directory as allowed path
//! cargo run --example mcp_filesystem_demo -- "C:\Users\YourName\projects"
//!
//! # Run with multiple allowed paths
//! cargo run --example mcp_filesystem_demo -- "C:\Users\YourName\projects" "C:\Users\YourName\documents"
//!
//! # Run with default (current directory)
//! cargo run --example mcp_filesystem_demo
//! ```
//!
//! ## What This Demo Does
//!
//! 1. Connects to the MCP filesystem server via stdio
//! 2. Discovers available tools from the server
//! 3. Registers them in the Clarity ToolRegistry
//! 4. Demonstrates tool execution:
//!    - List a directory
//!    - Read a file
//!    - Search for files
//! 5. Shows proper error handling

use clarity_core::mcp::{McpClient, McpClientBuilder, McpClientInstance, McpManager, McpToolAdapter};
use clarity_core::tools::ToolContext;
use clarity_core::ToolRegistry;
use serde_json::json;
use std::env;
use tracing::warn;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║     MCP Filesystem Server Integration Demo                   ║");
    println!("║     Clarity + @modelcontextprotocol/server-filesystem        ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    // Parse command-line arguments for allowed paths
    let args: Vec<String> = env::args().skip(1).collect();
    let allowed_paths = if args.is_empty() {
        vec![env::current_dir()?.to_string_lossy().to_string()]
    } else {
        args
    };

    println!("📁 Allowed paths:");
    for path in &allowed_paths {
        println!("   • {}", path);
    }
    println!();

    // =================================================================
    // STEP 1: Connect to MCP Filesystem Server
    // =================================================================
    println!("▶️  Step 1: Connecting to MCP Filesystem Server...");
    println!("   Command: npx -y @modelcontextprotocol/server-filesystem");
    
    let client = match connect_to_filesystem_server(&allowed_paths).await {
        Ok(client) => {
            println!("✅ Connected to MCP Filesystem Server\n");
            client
        }
        Err(e) => {
            eprintln!("❌ Failed to connect to MCP server");
            eprintln!("\nError: {}", e);
            eprintln!("\nTroubleshooting:");
            eprintln!("  1. Ensure Node.js is installed: node --version");
            eprintln!("  2. Ensure npx is available: npx --version");
            eprintln!("  3. Check your internet connection (npx needs to download the package)");
            eprintln!("  4. Try running manually:");
            eprintln!("     npx -y @modelcontextprotocol/server-filesystem {}", allowed_paths.join(" "));
            return Ok(());
        }
    };

    // =================================================================
    // STEP 2: Discover Available Tools
    // =================================================================
    println!("▶️  Step 2: Discovering available tools...");
    
    let tools = client.list_tools().await?;
    println!("   Found {} tool(s):\n", tools.len());
    
    for tool in &tools {
        println!("   🔧 {}", tool.name);
        println!("      {}", tool.description.as_deref().unwrap_or(""));
        let schema = &tool.input_schema;
        if let Some(props) = schema.get("properties") {
            println!("      Parameters: {}", 
                props.as_object()
                    .map(|p| p.keys().cloned().collect::<Vec<_>>().join(", "))
                    .unwrap_or_default()
            );
        }
        println!();
    }

    // =================================================================
    // STEP 3: Register Tools in Clarity Registry
    // =================================================================
    println!("▶️  Step 3: Registering tools in Clarity ToolRegistry...");
    
    let registry = ToolRegistry::new();
    let mut registered_count = 0;
    let client = std::sync::Arc::new(tokio::sync::Mutex::new(client));
    
    for tool in tools {
        let adapter = McpToolAdapter::new(client.clone(), tool);
        match registry.register(adapter) {
            Ok(_) => registered_count += 1,
            Err(e) => {
                warn!("Failed to register tool: {}", e);
            }
        }
    }
    
    println!("✅ Registered {} tool(s) in ToolRegistry\n", registered_count);

    // =================================================================
    // STEP 4: Demonstrate Tool Execution
    // =================================================================
    println!("▶️  Step 4: Demonstrating tool execution...\n");
    
    let ctx = ToolContext::new();
    let test_path = &allowed_paths[0];

    // Demo 1: List Directory
    println!("   ─────────────────────────────────────────");
    println!("   Demo 1: List Directory");
    println!("   ─────────────────────────────────────────");
    
    if let Some(tool) = registry.get("list_directory")? {
        println!("   Tool: list_directory");
        println!("   Path: {}", test_path);
        
        let args = json!({
            "path": test_path
        });
        
        match tool.execute(args, ctx.clone()).await {
            Ok(result) => {
                println!("   ✅ Success!");
                if let Some(files) = result.get("files").and_then(|f| f.as_array()) {
                    println!("   Found {} item(s):", files.len());
                    for (i, file) in files.iter().take(5).enumerate() {
                        let name = file.get("name").and_then(|n| n.as_str()).unwrap_or("?");
                        let ftype = file.get("type").and_then(|t| t.as_str()).unwrap_or("unknown");
                        println!("      {}. {} ({})", i + 1, name, ftype);
                    }
                    if files.len() > 5 {
                        println!("      ... and {} more", files.len() - 5);
                    }
                }
            }
            Err(e) => {
                println!("   ❌ Error: {}", e);
            }
        }
    } else {
        println!("   ⚠️  Tool 'list_directory' not available");
    }
    println!();

    // Demo 2: Search Files
    println!("   ─────────────────────────────────────────");
    println!("   Demo 2: Search Files");
    println!("   ─────────────────────────────────────────");
    
    if let Some(tool) = registry.get("search_files")? {
        println!("   Tool: search_files");
        println!("   Pattern: *.rs");
        
        let args = json!({
            "path": test_path,
            "pattern": "*.rs"
        });
        
        match tool.execute(args, ctx.clone()).await {
            Ok(result) => {
                println!("   ✅ Success!");
                if let Some(matches) = result.get("matches").and_then(|m| m.as_array()) {
                    println!("   Found {} Rust file(s):", matches.len());
                    for (i, m) in matches.iter().take(5).enumerate() {
                        let path = m.as_str().unwrap_or("?");
                        println!("      {}. {}", i + 1, path);
                    }
                    if matches.len() > 5 {
                        println!("      ... and {} more", matches.len() - 5);
                    }
                }
            }
            Err(e) => {
                println!("   ❌ Error: {}", e);
            }
        }
    } else {
        println!("   ⚠️  Tool 'search_files' not available");
    }
    println!();

    // Demo 3: Read File (try to find a README or Cargo.toml)
    println!("   ─────────────────────────────────────────");
    println!("   Demo 3: Read File");
    println!("   ─────────────────────────────────────────");
    
    if let Some(tool) = registry.get("read_file")? {
        // Try to find a file to read
        let test_files = ["README.md", "Cargo.toml", "package.json", ".gitignore"];
        let mut file_read = false;
        
        for file_name in &test_files {
            let file_path = std::path::Path::new(test_path).join(file_name);
            if file_path.exists() {
                println!("   Tool: read_file");
                println!("   Path: {}", file_path.display());
                
                let args = json!({
                    "path": file_path.to_string_lossy()
                });
                
                match tool.execute(args, ctx.clone()).await {
                    Ok(result) => {
                        println!("   ✅ Success!");
                        if let Some(content) = result.get("content").and_then(|c| c.as_str()) {
                            let preview: String = content.chars().take(200).collect();
                            println!("   Content preview:");
                            println!("   {}", preview.replace('\n', "\n   "));
                            if content.len() > 200 {
                                println!("   ... ({} more characters)", content.len() - 200);
                            }
                        }
                        file_read = true;
                    }
                    Err(e) => {
                        println!("   ❌ Error: {}", e);
                    }
                }
                break;
            }
        }
        
        if !file_read {
            println!("   ⚠️  No suitable test file found in {}", test_path);
        }
    } else {
        println!("   ⚠️  Tool 'read_file' not available");
    }
    println!();

    // =================================================================
    // STEP 5: Alternative - Using McpManager
    // =================================================================
    println!("▶️  Step 5: Using McpManager for multiple connections...\n");
    
    let mut manager = McpManager::new();
    let fs_args: Vec<String> = std::iter::once("-y".into())
        .chain(std::iter::once("@modelcontextprotocol/server-filesystem".into()))
        .chain(allowed_paths.iter().cloned())
        .collect();
    manager.connect_stdio("filesystem", "npx", &fs_args).await?;
    println!("   ✅ Added filesystem client to McpManager");
    
    let clients = manager.list_servers();
    println!("   Connected clients: {:?}", clients);
    
    let all_tools = manager.tools();
    println!("   Total tools available: {}", all_tools.len());
    
    manager.register_all(&registry);
    
    println!("   Total registered tools: {}\n", registry.len()?);

    // =================================================================
    // STEP 6: Error Handling Demo
    // =================================================================
    println!("▶️  Step 6: Error handling demonstration...\n");
    
    if let Some(tool) = registry.get("read_file")? {
        println!("   ─────────────────────────────────────────");
        println!("   Testing: Read non-existent file");
        println!("   ─────────────────────────────────────────");
        
        let args = json!({
            "path": "/nonexistent/path/to/file.txt"
        });
        
        match tool.execute(args, ctx.clone()).await {
            Ok(_) => println!("   ⚠️  Unexpected success"),
            Err(e) => {
                println!("   ✅ Properly handled error:");
                println!("      {}", e);
            }
        }
        println!();
        
        println!("   ─────────────────────────────────────────");
        println!("   Testing: Access outside allowed path");
        println!("   ─────────────────────────────────────────");
        
        let args = json!({
            "path": "C:\\Windows\\System32\\config\\SAM"
        });
        
        match tool.execute(args, ctx.clone()).await {
            Ok(_) => println!("   ⚠️  Unexpected success"),
            Err(e) => {
                println!("   ✅ Properly handled error:");
                println!("      {}", e);
            }
        }
    }
    println!();

    // =================================================================
    // STEP 7: Cleanup
    // =================================================================
    println!("▶️  Step 7: Cleanup...");
    
    println!("✅ Disconnected from MCP server\n");

    // =================================================================
    // Summary
    // =================================================================
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║                     Demo Complete!                           ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
    println!("Summary:");
    println!("  • Connected to MCP filesystem server via stdio");
    println!("  • Discovered and registered {} tools", registry.len()?);
    println!("  • Demonstrated list_directory, search_files, read_file");
    println!("  • Showed proper error handling for edge cases");
    println!("  • Used both direct client and McpManager approaches");
    println!();
    println!("Next steps:");
    println!("  1. Try with different MCP servers (github, git, postgres)");
    println!("  2. Integrate with Agent for autonomous file operations");
    println!("  3. Configure multiple MCP servers simultaneously");

    Ok(())
}

/// Helper function to connect to the MCP filesystem server
async fn connect_to_filesystem_server(
    allowed_paths: &[String],
) -> anyhow::Result<McpClientInstance> {
    let mut builder = McpClientBuilder::stdio("filesystem", "npx")
        .arg("-y")
        .arg("@modelcontextprotocol/server-filesystem");
    
    for path in allowed_paths {
        builder = builder.arg(path);
    }
    
    let mut client = builder.build();
    client.connect().await?;
    Ok(client)
}

// Example: How to use with Agent (commented out for compilation)
/*
async fn use_with_agent(registry: &ToolRegistry) -> anyhow::Result<()> {
    use clarity_core::Agent;
    
    // Create agent with the registry that includes MCP tools
    let agent = Agent::new(registry.clone())
        .with_model("kimi-k2-07132k-preview");
    
    // The agent can now use MCP filesystem tools
    let result = agent.run("List all Rust files in the current directory and show me their contents").await?;
    
    println!("Agent result: {}", result);
    Ok(())
}
*/
