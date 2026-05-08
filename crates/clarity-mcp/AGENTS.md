# Agent 指引 — clarity-mcp

## 构建

```bash
cargo build -p clarity-mcp
```

## 测试

```bash
cargo test -p clarity-mcp --lib
```

## 关键文件

- `src/lib.rs` — crate root；重导出 + legacy stdio client + credential scrubbing + result processing
- `src/config.rs` — MCP JSON 配置解析（兼容 Claude Desktop 的 mcp.json）
- `src/devkit.rs` — devbase MCP tool 返回的强类型结构体
- `src/enhanced.rs` — 增强客户端主体：transports、builders、registry、types、errors

## 约定

- 错误类型使用 `thiserror` 定义的 `McpError`
- 配置加载使用 `anyhow`
- 异步运行时使用 `tokio`
- 所有 stdio 命令必须经过 `validate_mcp_command` 校验
- 工具结果在返回 LLM 前经过 `scrub_credentials` 脱敏
