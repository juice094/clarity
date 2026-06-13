# clarity-tauri

> **ARCHIVED**: The Tauri frontend is no longer actively developed. It remains in the
> repository for reference but is **not built by default** in the workspace.

Clarity Tauri frontend — a Kimi-style conversation UI experiment.

## 状态

- **Stability tier**: Archived
- **Default build**: excluded from `cargo check --workspace` / `cargo test --workspace`
- **Replacement**: `clarity-egui` is the active desktop frontend; `clarity-slint` is the experimental successor

## 历史命令

```bash
# Build only this crate explicitly
cargo build -p clarity-tauri

# Run only this crate explicitly
cargo run -p clarity-tauri
```

## 保留原因

- Reference implementation for a Tauri-based conversation UI
- Contains icon assets and frontend scaffolding that may be reused
