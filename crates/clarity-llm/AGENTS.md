# Agent 指引 — clarity-llm

## 构建

```bash
cargo build -p clarity-llm
```

## 测试

```bash
cargo test -p clarity-llm --lib
```

## 关键文件

- `src/lib.rs` — 入口、provider 重导出、共享 HTTP client、本地模型路径解析
- `src/api.rs` — 从 `clarity-contract` re-export `LlmProvider` trait、`LlmResponse`、`Message` 等
- `src/factory.rs` — `LlmFactory`（legacy 自动检测 / create_*）
- `src/runtime.rs` — `RuntimeProviderConfig` 与纯函数式 provider 构造
- `src/runtime_router.rs` — alias / capability hint 路由
- `src/model_registry.rs` — `ModelRegistry` TOML config、`ProtocolType`
- `src/model_listing.rs` — 枚举可用模型
- `src/catalog/` — 远程模型目录抓取与缓存
- `src/registry_table.rs` — 内置 provider family defaults
- `src/request.rs` — OpenAI-compatible 请求/响应类型与请求体 size guard
- `src/providers/openai_compatible.rs` — 通用 OpenAI-compatible provider
- `src/providers/anthropic.rs` — Anthropic Messages API provider
- `src/providers/kimi.rs` — Kimi（Moonshot）provider
- `src/providers/oauth.rs` — OAuth / Kimi Code provider
- `src/deepseek.rs` — DeepSeek OpenAI-compatible client
- `src/ollama.rs` — Ollama 本地 API provider
- `src/llama_server.rs` — llama.cpp server HTTP bridge
- `src/local_gguf.rs` — Candle 原生 GGUF 推理（`local-llm` feature）
- `src/deepseek_device.rs` / `src/deepseek_pow.rs` — DeepSeek 设备登录与 PoW
- `src/reliable.rs` — `ReliableProvider` fallback 链实现
- `src/policy.rs` — 离线/在线自适应 provider 选择策略
- `src/auth/` — OAuth / token store
- `src/sse.rs` — SSE parser
- `src/tool_payload.rs` — OpenAI/Anthropic tool payload 适配
- `src/mesh/` — 多 provider 负载均衡与 circuit breaker
