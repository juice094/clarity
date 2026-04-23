# Agent 指引 — clarity-memory

## 构建

```bash
cargo build -p clarity-memory
```

## 测试

```bash
cargo test -p clarity-memory --lib
```

## 关键文件

- `src/lib.rs` — 入口与类型重导出
- `src/store.rs` — `MemoryStore` 核心存储接口
- `src/session_store.rs` — JSONL 会话存储
- `src/compiler.rs` — 四级记忆编译器（Today → Week → Long-term → Facts）
- `src/extractor.rs` — LLM 驱动的事实提取器
- `src/backends/sqlite.rs` — SQLite + FTS5 后端

## 约定

- 错误处理使用 `thiserror` 定义的内部错误类型
- 异步使用 `tokio`
- 事实去重使用 SHA256 fingerprint
