# S3.1 Settings 真相源审计 — 2026-05-11

> Type: Pre-refactor audit (S3.1 deliverable)
> Status: 完成，作为 S3.2-5 的设计依据
> Trigger: S3 启动需要先确认现有访问模式

---

## 1. 修正：不是 3 源真相，是 2 真相 + 1 镜像

原始诊断声称 settings 存在三处：
- `settings_edit: GuiSettings` (in egui SettingsStore)
- `cached_settings: Mutex<GuiSettings>` (in egui AppState)
- `ACTIVE_CONFIG: Mutex<Option<RuntimeProviderConfig>>` (in clarity-llm)

**审计后修正**：实际是 **2 个独立真相 + 1 个完全镜像**。

### 1.1 TRUTH A — UI Settings

- **可变态**: `SettingsStore::settings_edit` (GuiSettings)
- **镜像**: `AppState::cached_settings` (Mutex<GuiSettings>) — async context 副本
- **磁盘形态**: `~/.config/clarity/gui-settings.json` (via `GuiSettings::load/save`)
- **写入点（settings_edit）**:
  - settings panels (用户编辑)
  - `app_logic.rs:599,622,640` — `save_settings_*` 系列
  - `onboarding.rs:331-337` — 首次启动模型选择
  - `handlers/mod.rs:183` — OAuth login result
- **镜像同步**: 每次 settings_edit 改了，`auto_save_settings/save_settings_and_reload/save_settings_internal/onboarding` 都立即 `*cached_settings.lock() = settings_edit.clone()`
- **读取点（cached_settings.lock）**:
  - `app_state.rs:218` — `ensure_llm` 读 settings 决定 provider
  - `app_logic.rs:53,104` — network probe URL
  - `app_logic.rs:161` — OAuth token refresh
  - `panels/sidebar.rs:74` — gateway 显示
  - `panels/chat/message_list.rs:624` — message rendering

### 1.2 TRUTH B — Runtime Provider Config

- **可变态**: `clarity_llm::runtime::ACTIVE_CONFIG` (Mutex<Option<RuntimeProviderConfig>>)
- **磁盘形态**: 无（纯运行时）
- **写入点**:
  - `provider_tab.rs:585` — 仅在 "Apply" 按钮被点时通过 `set_provider_config(cfg)` 写入
- **读取点**:
  - `ensure_llm` 优先检查 (`app_state.rs:237`)
  - `build_from_active_config` (`runtime.rs:84`)

### 1.3 关键事实

- **TRUTH A 和 TRUTH B 不自动同步**
- 它们之间的同步只发生在用户点 "Apply" 按钮时
- 在 `ensure_llm` 中 TRUTH B 优先（如果存在）

---

## 2. 这是一个真实 bug 的根因

### 2.1 Bug 场景一：Profile 切换不 reload LLM

```
1. 用户启动时 Apply 了 profile A → ACTIVE_CONFIG 含 cfg_A
2. 用户切换到 profile B：
   - settings_edit.active_profile = "B"
   - apply_profile_overlay 应用 B 的 overlay 到 settings_edit
   - cached_settings 同步更新
3. 但 ACTIVE_CONFIG 仍是 cfg_A
4. ensure_llm 被调用：
   - 第 1 行读 cached_settings → 是 B 的配置
   - 但第 4 行检查 ACTIVE_CONFIG → 是 cfg_A → 用 cfg_A 加载 LLM
5. 结果：UI 显示 profile B，LLM 实际用 profile A 的 API key / model
```

### 2.2 Bug 场景二：Apply 后再改 provider

```
1. 用户 Apply provider X → ACTIVE_CONFIG = cfg_X
2. 用户在 settings 改 provider Y（未 Apply）
   - settings_edit.provider = "Y"
   - cached_settings 同步
3. 触发 reload_llm（如改 approval mode 时）
4. ensure_llm 用 ACTIVE_CONFIG (X) → LLM 用 X 加载
5. UI 显示 Y，LLM 用 X
```

### 2.3 Bug 场景三：Onboarding 后状态混淆

```
1. 用户完成 onboarding：settings_edit.provider = "local" + path
2. cached_settings 同步
3. ACTIVE_CONFIG 永远不会被设置（因为 onboarding 不调 set_provider_config）
4. ensure_llm 走 fallback 路径（settings 决定）→ 暂时 OK
5. 用户随后在 settings 点 Apply provider X
6. ACTIVE_CONFIG = X
7. 用户切换回 local（settings_edit）→ ACTIVE_CONFIG 仍是 X
```

---

## 3. S3 修正目标

原 S3 目标"激活 SettingsViewModel"**不足以解决根因**。SettingsViewModel 只覆盖 TRUTH A 内部一致性，**不解决 TRUTH A → TRUTH B 的派生缺失**。

### 3.1 修正后的 S3 真实目标

**建立 TRUTH A → TRUTH B 的派生关系**：

```
Before (现状):
   TRUTH A (settings) ─x─ 不同步 ─x─→ TRUTH B (ACTIVE_CONFIG)
                                   ↓
                            手动 Apply 按钮触发

After (目标):
   TRUTH A (settings) ──── derive() ────→ Derived ActiveConfig
                                              │
                                              ▼
                                       ensure_llm 直接使用
```

### 3.2 删除 ACTIVE_CONFIG 作为可变态

- 删除 `set_provider_config()`
- 删除 `clear_provider_config()`
- 改 `get_active_config(settings: &Settings) -> Option<RuntimeProviderConfig>` 为纯函数
- `build_from_active_config(settings) -> ...` 接受 settings 参数

### 3.3 "Apply" 按钮的新语义

不再是"写入 ACTIVE_CONFIG"，而是：
- 触发 `set_provider_config` 之类的副作用消失
- 改为"持久化 settings_edit + 触发 reload_llm"
- 用户改 settings → 立即生效（不需要显式 Apply）

或保留显式 Apply 但作为"确认 + reload"语义。

### 3.4 SettingsViewModel 的处理

- 当前在 clarity-core 内部，被 egui/tui 标 `#[allow(dead_code)]`
- S3 完成后**仍可保留**（作为 settings 内部一致性 helper）
- **或者删除**（如果 settings_edit 本身已经够单源）

后者更激进。决定推迟到 S3.5。

---

## 4. 修正后的 S3 子阶段

```
✅ S3.1  审计 + 文档（本步骤）

S3.2  合并 settings_edit ↔ cached_settings
       目标：去除镜像，保留可异步访问性
       方案：把 cached_settings 改为 Arc<RwLock<GuiSettings>>，
            settings_edit 改为 cached_settings.read().clone() 的快照
       风险：低（纯重命名 + 锁类型变化）

S3.3  让 RuntimeProviderConfig 派生自 Settings
       目标：删除 ACTIVE_CONFIG 全局可变态
       方案：
         - clarity_llm::runtime::derive_provider_config(settings: &Settings) -> Option<RuntimeProviderConfig>
         - ensure_llm 调用 derive 而非 get_active_config
       风险：中（触及 LLM 加载路径）

S3.4  删除 set_provider_config / get_active_config / ACTIVE_CONFIG
       目标：纯函数化 LLM 配置层
       风险：中（破坏性变更，但 caller 集中在 provider_tab.rs:585）

S3.5  根据 S3.4 结果决定 SettingsViewModel 去留
```

总预估：4 工作日（修订自原 5d，因为修正了真实问题面）。

---

## 5. S3.2 启动条件

立即可启动。前置：本审计报告 + S4-α/β 验证的 widget 测试模式作为安全网。

## 6. S3.4 风险缓解

ACTIVE_CONFIG 删除最危险。缓解策略：
- 同 PR 同时更新 ensure_llm + provider_tab.rs "Apply" 处理
- 保留 RuntimeProviderConfig 类型本身（只是不再用 mutex 缓存）
- 端到端测试：模拟 profile 切换 + Apply + reload 三场景

## 7. SettingsViewModel 不动的理由

如先前 ADR-006 §1.3 修正发现，SettingsViewModel 已被 tui 的 `cached_view_commands` 直接使用（通过 `commands()`）。完整删除会破坏 tui。

S3 不试图清理 SettingsViewModel。它作为"settings 内部一致性 helper"保留。Phase D 抽 frontend-ir 时再处理。

---

End of audit.
