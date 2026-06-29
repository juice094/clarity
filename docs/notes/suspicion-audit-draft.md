# Clarity 疑点审计草稿

> 生成时间：2026-06-16  
> 目的：标记当前认知中「可能有问题」的代码/设计疑点，按重要性与集中程度排序，相近疑点合并为单次检查任务。

---

## 状态更新

- **2026-06-16**：任务 1（Settings 单源化审计与迁移）已完成。
  - 移除了 `clarity_llm::runtime::ACTIVE_CONFIG` 及相关函数。
  - `ensure_llm` 改为从 `cached_settings` + `ProviderRegistry` 直接派生 `RuntimeProviderConfig`。
  - `LlmBinding` 增加 `model` 字段，使同 provider 不同 model 也能正确触发重载。
  - 在 `docs/architecture/settings-provider-flow.md` 记录了新的数据流与写入点清单。
  - 新增/更新了 `clarity-egui`、`clarity-llm`、`clarity-egui/src/provider.rs` 的相关单元测试。

---

## 术语：Settings 单源化（S3.3）是什么？

**单源化 = 一个概念只有一个写入点。**

当前 `clarity-egui` 的 provider/settings 存在多条并行真相源：

| 真相源 | 位置 | 说明 |
|--------|------|------|
| UI 编辑态 | `settings_store.settings_edit` | 用户在 Settings 面板里看到的临时值 |
| 运行时缓存 | `AppState.cached_settings` | `ensure_llm` 每次读取的锁保护副本 |
| 全局可变缓存 | `clarity_llm::runtime::ACTIVE_CONFIG` | 废弃中的全局 `Mutex<Option<RuntimeProviderConfig>>` |
| 磁盘注册表 | `ProviderRegistry` / `~/.config/clarity/providers/` | 持久化的 provider 定义 |
| LLM 绑定状态 | `AppState.llm_binding` + `Agent.llm()` | 当前实际绑定的 provider 实例 |
| Profile 叠加 | `apply_profile_overlay` | 每次 `ensure_llm` 动态覆盖 settings |

**为什么这是问题**：
- provider/model 可能在 `cached_settings`、`ACTIVE_CONFIG`、`llm_binding` 之间不一致。
- `provider_tab.rs` 点 Apply 时先写 `ACTIVE_CONFIG`，再 `auto_save_settings`；`ensure_llm` 优先读 `ACTIVE_CONFIG`。一旦 `ACTIVE_CONFIG` 和磁盘 settings 不同步，UI 显示和实际运行模型就会错位。
- chat-only provider（如 deepseek-device）不走 `ACTIVE_CONFIG`，而是靠 `ProviderRegistry` 重新构建，形成两条完全不同的加载路径。
- 未来并行 session / 多窗口需要每个会话独立 provider；全局 `ACTIVE_CONFIG` 是天然阻塞。

**目标状态（S3.4）**：`ensure_llm` 每次从 `cached_settings` + `ProviderRegistry` 直接派生出 `RuntimeProviderConfig`，调用 `build_provider(&cfg)`，不再读写全局缓存。

---

## 疑点列表与排序

### P0 — 架构级，可能直接引发运行时错误

1. **Settings 多源（S3.3）** ✅ 已修复
   - 文件：`clarity-egui/src/app_state.rs`、`clarity-egui/src/panels/settings/provider_tab.rs`、`clarity-llm/src/runtime.rs`
   - 现象：`ACTIVE_CONFIG` 全局缓存与 `cached_settings` 并存；Apply 时先写全局缓存再落盘；chat-only provider 绕过该缓存。
   - 风险：配置漂移、模型错位、并行 session 阻塞。
   - 集中程度：高（3 个核心文件）。

2. **`ensure_llm` 中 `ACTIVE_CONFIG` 优先于 binding 检查** ✅ 已修复
   - 文件：`clarity-egui/src/app_state.rs:241-259`（旧位置）
   - 现象：只要 `ACTIVE_CONFIG` 存在，即使 `llm_binding` 已匹配也会重新构建 provider。
   - 风险：每次发消息可能重复构建 provider； profile 切换后旧 `ACTIVE_CONFIG` 未及时清空会导致模型不随 profile 变化。

3. **profile 叠加与 `ACTIVE_CONFIG` 的交互未定义** ✅ 已修复
   - 文件：`clarity-egui/src/app_state.rs:226`、`clarity-egui/src/app_logic.rs`
   - 现象：`apply_profile_overlay` 修改临时 settings，但 profile 切换时是否清理 `ACTIVE_CONFIG` 不清晰。
   - 风险：切换 profile 后仍使用旧 provider。

### P1 — 工程红线被批量绕过

4. **大量 crate 使用模块级 `allow(clippy::unwrap_used, expect_used, panic)`**
   - 文件：`clarity-core/src/lib.rs`、`clarity-tools/src/lib.rs`、`clarity-channels/src/lib.rs`、`clarity-memory/src/lib.rs`、`clarity-subagents/src/lib.rs`、`clarity-mcp/src/lib.rs`、`clarity-secrets/src/lib.rs`、`clarity-telemetry/src/lib.rs`、`clarity-mobile-core/src/lib.rs`、`clarity-headless/src/main.rs`、`clarity-claw/src/main.rs`、`clarity-slint/src/{lib,main}.rs`、`clarity-tauri/src/main.rs` 等。
   - 现象：把整个 crate 的 `unwrap/expect/panic` lint 关掉，而不是逐处加 `// SAFE:` 或改用错误处理。
   - 风险：隐藏潜在 panic 点，违反 AGENTS.md 工程红线；新增代码容易顺手写 `unwrap()`。
   - 集中程度：极高（跨十几个 crate）。

5. **局部 `#[allow(clippy::unwrap_used)]` / `#[allow(clippy::expect_used)]` 未说明不变量**
   - 文件：`clarity-tools/src/web_browser.rs:246`、`clarity-tools/src/web.rs:27`、`clarity-memory/src/extractor.rs` 多处、`clarity-contract/src/tool_parser.rs`、`clarity-core/src/agent/run/loop_helpers.rs` 等。
   - 现象：只加 `allow`，没有 `// SAFE:` 注释。
   - 风险：后来者无法判断这些 unwrap 是否真安全。

### P2 — 可维护性与测试质量

6. **环境依赖/条件测试**
   - 文件：`crates/clarity-llm/tests/deepseek_device_e2e.rs`（依赖 `DEEPSEEK_DEVICE_TOKEN`）、`crates/clarity-gateway/tests/*_integration_test.rs`（依赖 temp dir + 网络）。
   - 现象：部分集成测试需要外部 token 或端口，CI 中只能忽略或靠运气。
   - 风险：本地绿、CI 红；新人难以判断失败是环境还是代码问题。

7. **`#[allow(dead_code)]` / `allow(unused)` 较多**
   - 文件：`clarity-egui/src/ui/types.rs`、`clarity-egui/src/components/chat/conversation.rs`、`clarity-gateway/src/ws.rs`、`clarity-memory/src/extractor.rs` 等。
   - 现象：字段/函数标记 dead_code，或整个模块允许 unused。
   - 风险：可能是未完成的愿景功能，也可能是真死代码，持续累积技术债务。

8. **`SettingsStore.settings_vm` 标记 `#[allow(dead_code)]`**
   - 文件：`clarity-egui/src/stores/settings.rs:27-28`
   - 现象：`SettingsViewModel` 被持有但不被使用。
   - 风险：Settings 协议下沉到 core 的工作可能只完成了一半。

### P3 — 实验性/边缘维护

9. **`clarity-slint` 实验栈与 workspace lint 脱节**
   - 已部分修复 `build.rs`，但生成的 slint 代码仍含大量 `unwrap`，且该 crate 被 CI 排除。
   - 风险：实验栈继续腐烂，重新纳入 CI 时成本巨大。

10. **`clarity-tauri` 已归档但仍出现在 `crates/` 目录**
    - 已被 `Cargo.toml` exclude，但仍占用磁盘和认知。
    - 风险：新开发者可能误改；工具扫描时产生噪音。

---

## 推荐检查任务（按优先级分组）

### 任务 1：Settings 单源化审计（P0，高集中）✅ 已完成
- 输入：`clarity-egui/src/app_state.rs`、`clarity-egui/src/panels/settings/provider_tab.rs`、`clarity-llm/src/runtime.rs`
- 已执行：
  - 画出当前 settings/provider 数据流图。
  - 列出所有写入点，标记哪些写入会绕过 `cached_settings`。
  - 实施了「移除 `ACTIVE_CONFIG`」的迁移。
- 验收：
  - `ACTIVE_CONFIG` 及相关函数已从 `clarity-llm/src/runtime.rs` 删除。
  - `cargo test --workspace --lib --bins --exclude clarity-slint` 全绿。
  - `cargo clippy --workspace --lib --bins --tests --exclude clarity-slint -- -D warnings` 零警告。
  - 迁移文档见 `docs/architecture/settings-provider-flow.md`。

### 任务 2：unwrap/expect/panic  blanket-allow 清理（P1，高分散）
- 输入：所有带模块级 `allow(clippy::unwrap_used, expect_used, panic)` 的 crate。
- 输出：
  - 统计每个 crate 的 unwrap/expect/panic 数量。
  - 对核心 crate（`clarity-core`、`clarity-llm`、`clarity-memory`、`clarity-tools`）优先逐处改为 `Result` 或加 `// SAFE:`。
  - 对前端/实验 crate 可保留 allow，但必须在 AGENTS.md 中登记并给出时间表。
- 验收：核心 crate 的模块级 allow 被移除或收窄到 `cfg(test)`。

### 任务 3：环境依赖与条件测试治理（P2，中分散）
- 输入：`crates/*/tests/`、集成测试。
- 输出：
  - 标记所有依赖外部 token/网络/真实目录的测试。
  - 将其中可隔离的改为临时 fixture / mock（如已修复的 `okf_bundle_load_test`）。
  - 对确实需要外部环境的测试，统一使用 `#[cfg(feature = "e2e")]` 或文档化运行条件。
- 验收：默认 `cargo test` 不再因本地环境差异失败。

### 任务 4：dead_code / 归档 crate 清理（P2-P3，低分散）
- 输入：`#[allow(dead_code)]` 清单、`clarity-tauri` 目录。
- 输出：
  - 删除真死代码；对未完成愿景功能加 `TODO-vision` 注释或移入 `docs/notes/`。
  - 评估 `clarity-tauri` 是否彻底移除或移入 `archived/`。
- 验收：代码库净减少行数，AGENTS.md 更新归档状态。

---

## 建议的下一步

如果只能选一项：**先做任务 1（Settings 单源化审计）**。它影响面大但集中，产出是设计文档，风险低；后续任何 provider/parallel-session 工作都需要这个结论。
