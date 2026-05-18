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

- `src/lib.rs` — 入口与 provider 重导出
- `src/api.rs` — `LlmProvider` trait、`LlmResponse`、`Message` 核心类型
- `src/deepseek.rs` — DeepSeek / OpenAI 兼容 provider
- `src/ollama.rs` — Ollama 本地 API provider
- `src/local_gguf.rs` — Candle 原生 GGUF 推理（`local-llm` feature）
- `src/reliable.rs` — `ReliableProvider` fallback 链实现
- `src/policy.rs` — 离线/在线自适应 provider 选择策略
- `src/mesh/` — 多 provider 负载均衡与 circuit breaker
