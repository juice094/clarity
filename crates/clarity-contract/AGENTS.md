# Agent 指引 — clarity-contract

## 构建

```bash
cargo build -p clarity-contract
```

## 测试

```bash
cargo test -p clarity-contract --lib
```

## 关键文件

- `src/lib.rs` — 入口与核心类型重导出：`ToolCall`、`Message`、`MessageRole`、`StreamDelta`
- `src/tool.rs` — `Tool` trait（`async_trait`）、`ToolContext`、`ApprovalMode`、`SharedTool`/`BoxedTool`
- `src/llm.rs` — `LlmProvider` trait、`LlmResponse`、`ProviderCapabilities`、`Pricing`
- `src/error.rs` — `AgentError`、`ToolError`、`ContractResult`、路径消毒 `sanitize_path_str`
- `src/federation.rs` — `FederationNode` trait、`FederationMessage`、`Capability`、`TaskSpec`
- `src/capability.rs` — `CapabilityToken`、`TokenError`、沙箱/白名单/只读校验

## 约定

- 错误处理使用 `thiserror` 定义的内部错误类型（`AgentError`、`ToolError`）
- 所有错误类型实现 `Clone`，确保可跨 async 边界传递
- 异步 trait 使用 `#[async_trait]`
- 路径消毒为强制操作：`sanitize_path_str` 防止主机路径泄露
- 保持轻量依赖：本 crate 用于解耦 `clarity-core` 巨石模块
- `clarity-core` 通过 `pub use clarity_contract::*` 向后兼容重导出
