# clarity-llm

LLM Provider System for Project Clarity.

## 职责

- **多提供商统一接口** — `LlmProvider` trait 封装 DeepSeek / Kimi / OpenAI / Anthropic / Ollama / Local GGUF 等后端
- **Provider 自适应** — `Policy` 层根据网络状态、模型能力自动选择最佳提供商
- **ReliableProvider** — fallback 链：主 provider 失败时自动切换备用
- **本地推理** — Candle 原生 GGUF 支持（Qwen2、DeepSeek-R1-Distill），零外部依赖
- **流式响应** — `StreamChunk` + `DraftEvent` 三态流（Clear / Progress / Content）
- **工具调用协议** — 统一 `ToolCall` / `ToolResult` 序列化，适配 OpenAI / Anthropic 格式差异

## 关键类型

- `LlmProvider` — 核心 trait：`complete()` 同步完成 + `complete_stream()` 流式完成
- `LlmResponse` — LLM 返回内容 + tool_calls + 完成标记
- `Message` / `MessageRole` — 标准化对话消息格式
- `DeepSeekProvider` / `OllamaProvider` / `LocalGgufProvider` — 具体实现
- `ReliableProvider` — 包装多个 provider 的容错层
- `ModelRegistry` — 运行时模型注册与查询

## 测试

```bash
cargo test -p clarity-llm --lib
```
