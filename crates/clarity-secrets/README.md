# clarity-secrets

Encrypted secret storage for Clarity, backed by ChaCha20-Poly1305.

## 职责

- **Key management** — load existing key files or generate random 256-bit keys
- **Authenticated encryption** — encrypt/decrypt arbitrary strings with ChaCha20-Poly1305
- **Ciphertext format** — `enc2:<hex(nonce || ciphertext_and_tag)>`
- **Legacy migration** — pass through plaintext values unchanged until re-encrypted

## 关键类型

- `SecretStore` — encrypted store bound to a single key file
- `SecretError` — error enum covering IO, hex, decryption, and format failures
- `KEY_SIZE` / `NONCE_SIZE` / `TAG_SIZE` / `CIPHER_PREFIX` — format constants

## 测试

```bash
cargo test -p clarity-secrets --lib
```

## 边界与稳定性

- **Stability tier**: Stable
  - Ciphertext format is a compatibility surface; do not change without migration path
- **MSRV**: 1.85.0
- **反向依赖禁止** (No reverse dependencies):
  - 不得依赖任何 frontend/network crate
- **Library/binary classification**:
  - Library: designed for `use` by other crates
