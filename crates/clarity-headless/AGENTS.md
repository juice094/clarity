# Agent 指引 — clarity-headless

## 构建

```bash
cargo build -p clarity-headless
```

## 测试

```bash
cargo test -p clarity-headless --lib
```

## 关键文件

- `src/main.rs` — 全 crate 入口；`clap` CLI（`Run` / `Jumpy` 子命令）、`build_provider()`、`run_command()`、`jumpy_command()`

## 约定

- CLI 层错误处理使用 `anyhow::Result`；`clarity_core::AgentError` 从 core 上抛，转换为 JSON 或 stderr
- 异步使用 `tokio::runtime::Runtime`，`main()` 中创建，`block_on(async_main())`
- Provider 构造优先级：环境变量 fallback（`env_or`）→ CLI 参数覆盖
- Jumpy 世界模型支持三种预测器：`Llm`、`Historical`、`Hybrid`
- 支持 stdin pipe（`run` 和 `jumpy` 子命令均支持）
- `local-llm` feature 控制 `ProviderType::Local` 和 `LocalGgufProvider`
