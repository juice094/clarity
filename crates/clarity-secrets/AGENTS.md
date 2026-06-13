# Agent 指引 — clarity-secrets

## 构建

```bash
cargo build -p clarity-secrets
```

## 测试

```bash
cargo test -p clarity-secrets --lib
```

## 关键文件

- `src/lib.rs` — `SecretStore`, `SecretError`, encryption format

## 约定

- `SecretStore::load_or_create` is the preferred entry point
- All plaintext is UTF-8; binary secrets should be base64-encoded by callers
- Key files store a hex-encoded 32-byte key; any other length is `SecretError::InvalidFormat`
- Legacy plaintext values are returned as-is; callers decide when to re-encrypt

## 红线

- 不得将密钥材料写入日志或错误消息
- 不得依赖 `clarity-core` 或任何 frontend crate
- 不得在生产代码中使用 `unwrap`/`expect`/`panic`（测试文件除外）
- 所有 `pub` 类型和函数必须有 `///` 文档注释
