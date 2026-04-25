//! Cross-window verification: devbase MCP tool tier filtering.
//!
//! Validates that devbase v0.2.3 correctly filters tools when
//! DEVBASE_MCP_TOOL_TIERS=stable,beta is set via McpClientBuilder::env().

use clarity_core::mcp::{McpClient, McpClientBuilder};

const DEVBASE_EXE: &str = r"C:\Users\22414\Desktop\devbase\target\release\devbase.exe";

#[tokio::test]
async fn test_devbase_tool_tier_filtering() {
    // 1. Build client with tier env
    let mut client = McpClientBuilder::stdio("devbase", DEVBASE_EXE)
        .arg("mcp")
        .env("DEVBASE_MCP_TOOL_TIERS", "stable,beta")
        .build();

    client.connect().await.expect("devbase mcp connect failed");

    // 2. List tools
    let tools = client.list_tools().await.expect("list_tools failed");

    println!("=== devbase tools/list (stable+beta) ===");
    println!("Total tools returned: {}", tools.len());
    for tool in &tools {
        println!(
            "  - {}: {}",
            tool.name,
            tool.description.as_deref().unwrap_or("(no desc)")
        );
    }

    // 3. Verify count: stable+beta tier filter active
    // Note: devbase v0.2.3 returns 14 tools (stable 5 + beta 9);
    // the draft in meeting room listed 13, but `devkit_query` is also included.
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
    println!("Tool names: {:?}", names);
    assert_eq!(
        tools.len(),
        14,
        "Expected 14 tools (stable+beta), got {}. Names: {:?}",
        tools.len(),
        names
    );

    // 4. Verify no experimental tools present
    let experimental_tools = [
        "devkit_digest",
        "devkit_paper_index",
        "devkit_experiment_log",
        "devkit_code_metrics",
        "devkit_module_graph",
    ];
    for tool in &tools {
        assert!(
            !experimental_tools.contains(&tool.name.as_str()),
            "Experimental tool '{}' should have been filtered out",
            tool.name
        );
    }

    // 5. Spot-check description quality (non-empty and meaningful)
    let mut empty_desc_count = 0;
    let mut short_desc_count = 0;
    for tool in &tools {
        let desc = tool.description.as_deref().unwrap_or("");
        if desc.is_empty() {
            empty_desc_count += 1;
        }
        if desc.len() < 20 {
            short_desc_count += 1;
        }
    }
    println!(
        "Description quality: empty={}, short(<20)={}",
        empty_desc_count, short_desc_count
    );

    // Soft assertions (log warnings, don't fail)
    if empty_desc_count > 0 {
        eprintln!(
            "WARNING: {} tools have empty descriptions",
            empty_desc_count
        );
    }
    if short_desc_count > 3 {
        eprintln!(
            "WARNING: {} tools have very short descriptions",
            short_desc_count
        );
    }

    client.disconnect().await.ok();
}

#[tokio::test]
async fn test_devbase_all_tools_without_filter() {
    // Control test: without tier filter, all 19 tools should be exposed
    let mut client = McpClientBuilder::stdio("devbase", DEVBASE_EXE)
        .arg("mcp")
        .build();

    client.connect().await.expect("devbase mcp connect failed");
    let tools = client.list_tools().await.expect("list_tools failed");

    println!("=== devbase tools/list (no filter) ===");
    println!("Total tools returned: {}", tools.len());

    // Backward compatible: all 19 tools
    assert_eq!(
        tools.len(),
        19,
        "Expected 19 tools without filter, got {}",
        tools.len()
    );

    client.disconnect().await.ok();
}

#[tokio::test]
async fn test_devbase_stable_only() {
    // Narrow filter: stable only = 5 tools
    let mut client = McpClientBuilder::stdio("devbase", DEVBASE_EXE)
        .arg("mcp")
        .env("DEVBASE_MCP_TOOL_TIERS", "stable")
        .build();

    client.connect().await.expect("devbase mcp connect failed");
    let tools = client.list_tools().await.expect("list_tools failed");

    println!("=== devbase tools/list (stable only) ===");
    println!("Total tools returned: {}", tools.len());

    assert_eq!(
        tools.len(),
        5,
        "Expected 5 stable tools, got {}",
        tools.len()
    );

    client.disconnect().await.ok();
}
