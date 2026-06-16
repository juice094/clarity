# Clarity 优化工作总结

> 生成时间：2026-06-16
> 状态：已合并到 `main`
> 相关 PR：#13（健康优化）、#14（S6 Pretext / Thread / Rollout / Hermes）

## 已完成的优化项

### 1. 构建健康与 CI
- ✅ 解除 Hermes 特性构建阻塞（`clarity-memory` 已补全 `hermes` feature 与 `MemoryError::Hermes`）。
- ✅ 在 `.github/workflows/ci.yml` 新增 `hermes-feature-check` job，覆盖 `ubuntu-latest` / `windows-latest`。
- ✅ 完整质量门全绿：
  - `cargo check --workspace --lib --bins --exclude clarity-slint`
  - `cargo test --workspace --lib --exclude clarity-slint`
  - `cargo test --workspace --bins --exclude clarity-slint -- --test-threads=2`
  - `cargo test --workspace --doc --exclude clarity-slint -- --test-threads=2`
  - `cargo test -p clarity-integration-tests --lib`
  - `cargo clippy --workspace --lib --bins --tests --exclude clarity-slint -- -D warnings`
  - `cargo fmt --all -- --check`

### 2. 仓库清理
- ✅ 将 `assets/` 下 35 个临时截图/标注产物归档并清理。
- ✅ 将 `coverage/` 与 `coverage_summary.json` 加入 `.gitignore`。
- ✅ 移除仓库根目录 Phase 1.5 迁移临时脚本：
  - `fix_phase15.py`
  - `fix_phase15_2.py`
  - `egui_check_errors*.log`
  - `clippy_egui_deadcode.log`

### 3. 依赖治理
- ✅ 在 `Cargo.toml` workspace dependencies 中统一：
  - `base64 = "0.22"`
  - `clap = "4.6"`
  - `anstyle = "1.0"`
  - `anstream = "1.0"`
  - `clap_builder = "4.6"`
- ✅ 将 `clarity-core`、`clarity-gateway`、`clarity-channels`、`clarity-tools`、`clarity-headless` 中的 `base64` / `clap` 改为 `{ workspace = true }`。
- ✅ 卫生升级：`tao 0.35 → 0.35.3`，`tray-icon 0.22.2 → 0.24.1`。

### 4. 安全审计
- ✅ 更新 `.cargo/audit.toml`，忽略并记录：
  - `RUSTSEC-2024-0429`（glib VariantStrIter unsound）
  - `RUSTSEC-2024-0370`（proc-macro-error unmaintained，需上游 tao/tray-icon GTK4 迁移后解除）
- ✅ `cargo audit --deny unsound --deny yanked` 通过（14 allowed warnings）。

### 5. 测试覆盖率提升
新增测试覆盖以下原 0% / 低覆盖率模块：

| 模块 | 新增测试数 | 说明 |
|------|-----------|------|
| `clarity-gateway/src/handlers/tasks.rs` | 10 | 任务 CRUD、并行任务、404/400 错误 |
| `clarity-gateway/src/ws.rs` | 7 | WebSocket 消息序列化、upgrade、ping/pong |
| `clarity-gateway/src/handlers/mcp.rs` | 8 | MCP 服务器 CRUD、配置路径解析 |
| `clarity-channels/src/chkit/util.rs` | 11 | `strip_tool_call_tags` 各种输入 |
| `clarity-telemetry/src/tracing_layer.rs` | 9 | 事件类型推断、span ID 转换、Layer 端到端 |
| `clarity-core/src/tools/task.rs` | 8 | 任务工具元数据、错误路径 |
| `clarity-core/src/mcp/tools.rs` | 5 | McpToolWrapper 元数据与执行错误 |

### 6. 超大文件拆分
- ✅ 将 `crates/clarity-channels/src/chkit/wechat.rs`（~2,932 行）拆分为：
  - `wechat/mod.rs` — 公共 `WeChatChannel` 与 `Channel` trait 实现
  - `wechat/types.rs` — 常量、类型、辅助枚举
  - `wechat/crypto.rs` — AES/ECB 加解密
  - `wechat/parsing.rs` — 入站消息解析、附件提取
  - `wechat/state.rs` — token/cursor/allowlist 持久化
  - `wechat/api.rs` — QR 登录、收发消息、健康检查
  - `wechat/media.rs` — 附件加载、CDN 上传下载

## 仍可后续推进的优化

1. **继续拆分其他超大文件**：
   - `crates/clarity-mcp/src/enhanced.rs`（~2,069 行）
   - `crates/clarity-egui/src/main.rs`（~1,496 行）
   - `crates/clarity-core/src/agent/tests.rs`（~1,375 行）
   - `crates/clarity-llm/src/lib.rs`（~1,372 行）

2. **补齐剩余 0% 覆盖率模块**：
   - `clarity-claw/src/tray/mod.rs`
   - `clarity-core/src/client.rs`

3. **持续跟踪依赖去重**：
   - `base64 0.13.1` 仍由 `tokenizers` / `spm_precompiled` 间接引入，需上游更新。
   - `gemm` / `faer` 重复版本同样依赖上游 Candle 对齐。

4. **解除安全忽略**：
   - 跟踪 tauri-apps/tao#1104（GTK4 迁移），待 tray-icon/tao 发版后移除 `RUSTSEC-2024-0370` 忽略。
