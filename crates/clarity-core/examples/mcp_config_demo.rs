//! MCP Configuration Demo
//!
//! This example demonstrates loading an `mcp.json` configuration,
//! starting all configured MCP servers via `McpManager`, registering
//! discovered tools into a `ToolRegistry`, and running a simple
//! agent prompt.
//!
//! ## Setup
//!
//! 1. Copy `examples/mcp_config_example.json` to `~/.config/clarity/mcp.json`
//!    (or create your own).
//! 2. Adjust server commands/args to match your environment.
//! 3. Run:
//!    ```
//!    cargo run --example mcp_config_demo
//!    ```

use clarity_core::agent::{AgentConfig, MockLlm};
use clarity_core::mcp::config::McpConfig;
use clarity_core::mcp::McpManager;
use clarity_core::{Agent, ToolRegistry};
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let config = match McpConfig::load_default() {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Failed to load default MCP config: {}", e);
            eprintln!("You can create one at ~/.config/clarity/mcp.json");
            eprintln!("See examples/mcp_config_example.json for a sample.");
            std::process::exit(1);
        }
    };

    let manager = McpManager::from_config(&config).await;

    let registry = ToolRegistry::with_builtin_tools();
    manager.register_all(&registry);

    println!("Registered {} MCP tool(s)", manager.tools().len());
    for tool in manager.tools() {
        println!(
            "  - {}: {}",
            tool.name(),
            tool.description().unwrap_or("no description")
        );
    }

    let agent = Agent::with_config(registry, AgentConfig::new()).with_llm(Arc::new(MockLlm));

    let response = agent
        .run("Say hello and list what tools you have access to")
        .await?;
    println!("Agent response: {}", response);

    Ok(())
}
