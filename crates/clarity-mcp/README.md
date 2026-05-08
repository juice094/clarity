# clarity-mcp

MCP (Model Context Protocol) client library for Clarity. Supports stdio, HTTP, and SSE transports.

## Overview

This crate provides JSON-RPC 2.0 MCP **client** functionality for Clarity agents, allowing connection to external MCP tool servers. It is strictly a client library — it does not implement an MCP server.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     clarity-mcp (Client)                     │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │   Stdio     │  │    HTTP     │  │        SSE          │  │
│  │ (subprocess │  │  (POST req) │  │ (GET /sse → POST)   │  │
│  │  JSON-RPC)  │  │             │  │  auto-reconnect     │  │
│  └──────┬──────┘  └──────┬──────┘  └──────────┬──────────┘  │
│         └─────────────────┴────────────────────┘             │
│                         │                                     │
│              ┌──────────┴──────────┐                         │
│              │  McpClientInstance  │                         │
│              │   (enum dispatch)   │                         │
│              └──────────┬──────────┘                         │
│                         │                                     │
│              ┌──────────┴──────────┐                         │
│              │     McpRegistry     │                         │
│              │  (multi-server mgr) │                         │
│              └─────────────────────┘                         │
├─────────────────────────────────────────────────────────────┤
│  config.rs  │  devkit.rs  │  enhanced.rs  │  lib.rs (legacy) │
└─────────────────────────────────────────────────────────────┘
```

## Features

- **Stdio Transport**: Spawn local subprocess, JSON-RPC over stdin/stdout, Windows `.cmd` retry, 120s timeout
- **HTTP Transport**: POST JSON-RPC via reqwest, custom headers + OAuth Bearer
- **SSE Transport**: GET `/sse` → discover endpoint → POST `/messages?sid=xxx`, auto-reconnection
- **Command Validation**: Blocks shell metacharacters, `..`, and relative paths; `CLARITY_MCP_ALLOWLIST` env override
- **Credential Scrubbing**: Redacts API keys, tokens, passwords, OpenAI-style keys, Google AI keys from tool results
- **Config File**: Claude Desktop-compatible `mcp.json` parsing (`~/.config/clarity/mcp.json`)
- **Registry**: Manage multiple named MCP server connections

## Usage

```rust
use clarity_mcp::{McpClient, McpClientBuilder, McpRegistry};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Stdio transport
    let mut client = McpClientBuilder::stdio("fs", "npx")
        .arg("-y")
        .arg("@modelcontextprotocol/server-filesystem")
        .arg(".")
        .build();
    client.connect().await?;
    let tools = client.list_tools().await?;

    // HTTP transport
    let mut http = McpClientBuilder::http("api", "https://api.example.com/mcp")
        .header("Authorization", "Bearer token")
        .build();
    http.connect().await?;

    // Registry for multiple servers
    let mut registry = McpRegistry::new();
    registry.register("fs", client);
    registry.register("api", http);
    registry.connect_all().await?;

    Ok(())
}
```

## Testing

```bash
# Run all tests for this crate
cargo test -p clarity-mcp --lib
```

## 边界与稳定性

- **Stability tier**: Experimental
  - Experimental: API may change before v0.4.0
- **MSRV**: 1.78.0
- **反向依赖禁止** (No reverse dependencies):
  - 可依赖 clarity-contract + clarity-wire
- **Library/binary classification**:
  - Library: designed for `use` by other crates

## License

MIT
