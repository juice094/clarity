# clarity-llm

LLM Provider System for Project Clarity.

## 职责

- **多提供商统一接口** — `LlmProvider` trait（定义于 `clarity-contract`）在本 crate 实现 DeepSeek / Kimi / OpenAI / Anthropic / Ollama / Local GGUF 等后端
- **运行时构造** — `runtime::build_provider` 根据 `RuntimeProviderConfig` 创建 provider；`runtime_router` 按 alias / capability hint 路由
- **Provider 自适应** — `policy` 层根据网络状态、模型能力自动选择最佳提供商
- **ReliableProvider** — fallback 链：主 provider 失败时自动切换备用
- **本地推理** — Candle 原生 GGUF 支持（Qwen2、DeepSeek-R1-Distill），零外部依赖
- **模型目录** — `catalog` 拉取并缓存各 family 的远程模型列表
- **流式响应** — `sse::SseParser` 解析流式事件
- **工具调用协议** — `tool_payload` 统一 `ToolCall` / `ToolResult` 序列化，适配 OpenAI / Anthropic 格式差异

## 模块组织

```text
src/
├── api.rs                  # 从 clarity-contract re-export LlmProvider 等契约类型
├── auth/                   # OAuth / Kimi Code token 管理
├── catalog/                # 模型列表抓取、缓存、bootstrap defaults
├── deepseek.rs             # DeepSeek OpenAI-compatible client
├── deepseek_device.rs      # DeepSeek Android app device-login flow
├── deepseek_pow.rs         # DeepSeek device PoW challenge
├── factory.rs              # LlmFactory（legacy 自动检测与 create_*）
├── kalosm.rs               # Kalosm local inference stub
├── lib.rs                  # 入口、re-export、共享 HTTP client、本地模型路径解析
├── llama_server.rs         # llama.cpp server HTTP bridge
├── local_gguf.rs           # Candle native GGUF inference（local-llm feature）
├── mesh/                   # multi-provider circuit routing
├── model_listing.rs        # 枚举可用模型
├── model_registry.rs       # models.toml 解析与 provider 构造
├── ollama.rs               # Ollama HTTP API client
├── policy.rs               # provider 选择策略
├── providers/              # HTTP-based providers
│   ├── anthropic.rs
│   ├── kimi.rs
│   ├── oauth.rs
│   ├── openai_compatible.rs
│   └── mod.rs
├── registry_table.rs       # 内置 provider family defaults
├── reliable.rs             # ReliableProvider re-export
├── request.rs              # OpenAI chat-completion 类型与请求体 size guard
├── runtime.rs              # RuntimeProviderConfig / build_provider / test_connection
├── runtime_router.rs       # alias / hint 路由
├── sse.rs                  # SSE parser
└── tool_payload.rs         # OpenAI/Anthropic tool payload 适配
```

## 关键类型

- `LlmProvider` — 核心 trait（定义于 `clarity-contract`，在 `api` re-export）
- `LlmResponse` / `Message` / `MessageRole` / `StreamDelta` — 标准对话与流式增量
- `OpenAiCompatibleLlm` / `AnthropicLlm` / `KimiLlm` / `OAuthLlm` — HTTP provider 实现
- `DeepSeekProvider` / `OllamaProvider` / `LocalGgufProvider` / `LlamaServerProvider` — 其他后端
- `ReliableProvider` — 包装多个 provider 的容错层
- `ModelRegistry` / `ModelCatalogService` — 运行时模型注册与远程目录
- `RuntimeProviderConfig` / `runtime_router::RuntimeRouter` — 运行时构造与路由

## 测试

```bash
cargo test -p clarity-llm --lib
```
