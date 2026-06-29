# Agent Guidance for clarity-anthropic-proxy

> **Scope:** `crates/clarity-anthropic-proxy` 及其子目录。  
> **类型:** Binary crate。

## 1. 职责边界

`clarity-anthropic-proxy` 是 Clarity 的轻量 Anthropic Messages API 网关二进制：

- 对外暴露 Anthropic Messages API 兼容端点。
- 内部默认使用 DeepSeek Device Provider（通过 `clarity-llm::deepseek_device`）。
- 协议转换、工具调用解析与响应封装全部委托给 `clarity_llm::anthropic::AnthropicAdapter`。

本 crate **不** 参与 Clarity 主应用的 Agent 循环，不依赖 `clarity-core`，也不依赖任何前端 crate。

## 2. 关键不变量

1. 仅依赖 `clarity-contract`、`clarity-llm` 与基础外部 crate。
2. 不引入前端、TUI、GUI 或 Gateway 相关依赖。
3. 所有 `pub` 项必须有文档注释（workspace `missing_docs = "deny"`）。
4. 保持 Anthropic API 请求/响应格式的兼容性；变更格式需同步更新 `clarity-llm::anthropic` 中的适配器与测试。

## 3. 常用命令

```bash
# 编译
cargo check -p clarity-anthropic-proxy

# 测试
cargo test -p clarity-anthropic-proxy --lib
cargo test -p clarity-anthropic-proxy --bin cc-proxy

# Clippy
cargo clippy -p clarity-anthropic-proxy --lib --bins --tests -- -D warnings
```
