# Sprint 10 — 协议先行解锁（Protocol-First Unlock）

> 制定日期：2026-04-29
> 基线 commit：`phase2/protocol-pilot` @ `48006e5`
> 来源：Kimi 交叉审计（devbase↔Clarity）+ 实机代码验证
> 核心原则：**协议先行，架构后置** — 数据模型解耦优先于代码结构解耦

---

## 一、背景与问题陈述

Sprint 9 Phase 2（ModelRegistry 接入 egui）已完成，但 Phase 3（多模型角色分工）因"需 Agent 架构重构"而冻结。Kimi 交叉审计识别出这是**叙事级软阻塞**而非硬前置 —— `GuiSettings` 是独立数据模型，Agent 级 Provider 覆盖**不依赖** `agent/mod.rs` 的物理拆分。

同时，F1（ModelRegistry vs LlmFactory 双轨制）和 F5（功能暴露但不可用）是用户体验和开发者认知的持续损耗点。

本 Sprint 目标：**以零架构重构成本，解锁 Phase 3 并清算 F1/F5。**

---

## 二、目标与范围

| ID | 目标 | 验收标准 | 工作量 |
|----|------|---------|--------|
| D1 | AgentProfile TOML Schema + GuiSettings 扩展 | `profiles.toml` 解析通过；Settings UI 新增 Profile 下拉框；切换 Profile 自动更新 provider/model/approval_mode | 8–12h |
| D2 | LlmFactory 功能冻结 | 所有 `pub fn` 添加 `#[deprecated]`；`AGENTS.md` 更新 Provider 新增路由表；零行为变更 | 2–4h |
| D3 | 能力发现协议（Capability Discovery） | `clarity-core` 新增 `supported_approval_modes(surface)`；egui Settings 禁用不可用模式 + tooltip | 6–10h |
| D4 | egui 冒烟测试基线 | headless 验证 `render_chat_area`/`render_sidebar` 关键组件存在性；≥3 个存在性测试 | 12–20h |

**范围边界**：
- ✅ 不拆分 `agent/mod.rs`（遵守"架构后置"原则）
- ✅ 不新增 crate（遵守 Hard Veto：项目广度 ≤ 5 核心工具）
- ✅ 不改 `ModelRegistry`/`build_provider_from_registry` 公共 API
- ❌ 不解决 egui 审批 UI / Plan 可视化 / 子代理 UI（留到 v0.4.0-beta）
- ❌ 不提取 `clarity-infrastructure`（F4 延后处理）

---

## 三、关键设计决策

### 3.1 AgentProfile 数据模型（D1）

```rust
// crates/clarity-egui/src/settings.rs
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GuiSettings {
    pub model: String,
    pub provider: String,
    pub approval_mode: String,
    pub theme: String,
    pub local_model_path: Option<String>,
    pub network_probe_url: String,
    pub language: String,
    pub api_key: Option<String>,
    // NEW: Sprint 10
    pub active_profile: Option<String>,
    #[serde(default)]
    pub profiles: HashMap<String, AgentProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentProfile {
    pub model: String,
    pub provider: String,
    pub approval_mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_model_path: Option<String>,
}
```

**TOML 文件位置**：`~/.config/clarity/profiles.toml`

```toml
[profiles.default]
model = "gpt-4o"
provider = "openai"
approval_mode = "interactive"

[profiles.greylocal]
model = "local-qwen"
provider = "local"
approval_mode = "yolo"

[profiles.research]
model = "kimi-k2"
provider = "kimi"
approval_mode = "plan"
```

**加载策略**：
1. `GuiSettings::load()` 时，若 `profiles.toml` 存在则合并到 `profiles` 字段
2. `profiles.toml` 与 `gui-settings.json` 的 `active_profile` 字段联动
3. 切换 Profile 时：`GuiSettings` 的 provider/model/approval_mode/api_key 被 Profile 值覆盖；Save 时只更新 `active_profile`，不展开 Profile 字段到顶层（避免冗余和同步问题）

**向后兼容**：无 `profiles.toml` 时行为与 Sprint 9 完全一致；`active_profile = None` 时沿用全局字段。

### 3.2 LlmFactory 功能冻结（D2）

**不删除代码**，仅建立边界：

| 规则 | 说明 |
|------|------|
| `LlmFactory::create*` 系列 | `#[deprecated(since = "0.3.2", note = "Use ModelRegistry + build_provider_from_registry()")]` |
| 新增 Provider | 禁止修改 `LlmFactory`，只允许扩展 `model_registry.rs` |
| Bug 修复 | `LlmFactory` 仅接受安全修复和编译修复 |
| 文档 | `AGENTS.md` 新增"Provider 新增检查单"：① `model_registry.rs` ② `get_available_models()` fallback ③ `build_provider_from_registry` match 分支 |

### 3.3 能力发现协议（D3）

```rust
// crates/clarity-core/src/capability.rs
pub struct CapabilityRegistry;

impl CapabilityRegistry {
    /// 查询某前端 surface 支持的审批模式
    pub fn supported_approval_modes(surface: &str) -> Vec<&'static str> {
        match surface {
            "egui" => vec!["yolo"], // Sprint 10 状态；未来扩展
            "tui" => vec!["interactive", "yolo", "plan"],
            "gateway" => vec!["yolo"], // HTTP 无状态，不支持交互审批
            "headless" => vec!["yolo", "plan"],
            _ => vec!["yolo"],
        }
    }
}
```

**egui 接入**：`SettingsViewModel::commands()` 中，`ComboBox` 的 `approval_mode` options 不再硬编码 `["interactive", "yolo", "plan"]`，而是调用 `CapabilityRegistry::supported_approval_modes("egui")`。

**渐进策略**：Sprint 10 先冻结为 `["yolo"]`（反映 egui 当前实际能力），审批 UI 上线后动态扩展。

### 3.4 egui 冒烟测试（D4）

**不测试像素**，仅验证存在性：

```rust
#[test]
fn test_chat_area_contains_input() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    // headless 运行一帧
    ctx.run(Default::default(), |ctx| {
        app.render_chat_area(ctx);
        // 通过 egui 的 memory 查询是否有 TextEdit
        let has_input = ctx.memory(|mem| {
            // 检查是否存在 id 为 "chat_input" 的 widget
            mem.data.get_temp(egui::Id::new("chat_input")).is_some()
        });
        assert!(has_input, "chat_area must contain a text input");
    });
}
```

**备选方案**：若 egui headless `Context::run` 在 CI 中不稳定，改为**逻辑级存在性测试** —— `render_chat_area` 拆分为 `build_chat_commands(&App) -> Vec<ViewCommand>` 纯函数，测试返回的 `ViewCommand` 树中是否包含 `TextInput`。

---

## 四、实施步骤（顺序执行）

### Step 1: LlmFactory 功能冻结（D2）— 零风险热身

1. `crates/clarity-core/src/llm/mod.rs`：给 `LlmFactory` 的 4 个 `pub` 方法加 `#[deprecated]`
2. `AGENTS.md`：更新"Provider 新增检查单"
3. `cargo test/clippy` 验证零行为变更
4. `git commit`

### Step 2: 能力发现协议（D3）— 小范围协议验证

1. `crates/clarity-core/src/capability.rs` 新建（~30 行）
2. `crates/clarity-core/src/lib.rs` 导出
3. `crates/clarity-core/src/view_models/settings.rs`：`approval_mode` ComboBox options 改为 `CapabilityRegistry::supported_approval_modes("egui")`
4. 追加单元测试（≥3 个 surface 的 modes 验证）
5. `cargo test/clippy` 验证
6. `git commit`

### Step 3: AgentProfile Schema（D1）— 核心交付

1. `crates/clarity-egui/src/settings.rs`：扩展 `GuiSettings` + 新增 `AgentProfile`
2. `crates/clarity-egui/src/settings.rs`：`load()` 合并 `profiles.toml`；`save()` 增量保存兼容新字段
3. `crates/clarity-egui/src/view_models/settings.rs`：新增 Profile ComboBox；切换 Profile 时联动 provider/model/approval_mode
4. `crates/clarity-egui/src/app_state.rs`：`ensure_llm` 读取 `active_profile` 覆盖（如有）
5. 单元测试（≥5 个）：Profile 切换、TOML 解析、向后兼容、Save/Load roundtrip
6. `cargo test/clippy` 验证
7. `git commit`

### Step 4: egui 冒烟测试（D4）— 质量加固

1. 评估 egui headless `Context::run` CI 稳定性
2. 若可行：2-3 个存在性测试
3. 若不可行：将 `render_chat_area`/`render_sidebar` 拆分为 `build_*_commands` 纯函数，测试 `ViewCommand` 树
4. `cargo test/clippy` 验证
5. `git commit`

---

## 五、验收标准

| # | 标准 | 验证方式 |
|---|------|---------|
| 1 | Profile 切换生效 | 手动：Settings 中选择 "research" → provider 变为 "kimi" → Save → `profiles.toml` 存在且 `active_profile = "research"` |
| 2 | 向后兼容 | 删除 `profiles.toml`，行为与 Sprint 9 完全一致 |
| 3 | LlmFactory 零行为变更 | `cargo test --workspace --lib` 全绿；所有 `LlmFactory` 调用点仍编译通过（仅 deprecation warning） |
| 4 | egui 仅暴露可用模式 | Settings 中 `approval_mode` 下拉框仅显示 `["yolo"]`（或未来扩展后的支持列表） |
| 5 | 测试增量 | ≥10 个新增测试；覆盖率不下降 |
| 6 | clippy 零警告 | `cargo clippy --workspace --lib --tests -- -D warnings` |

---

## 六、风险与回退

| 风险 | 概率 | 缓解 |
|------|------|------|
| `profiles.toml` 与 `gui-settings.json` 字段冲突 | 中 | `merge_json` 已支持未知字段保留；Profile 字段仅在 `profiles.toml` 中定义，`gui-settings.json` 只存 `active_profile` 引用 |
| `CapabilityRegistry` 硬编码 surface 列表成为新债务 | 中 | 文档明确标注"临时硬编码，审批 UI 上线后改为动态注册"；`AGENTS.md` 记录清算 TODO |
| egui headless 测试在 CI 中不稳定 | 高 | 备选方案已就绪（纯函数拆分 + ViewCommand 树测试）；不阻塞交付 |
| `AgentProfile` 设计不满足未来 Agent 架构 | 低 | Schema 仅含 provider/model/approval_mode/api_key/local_model_path 5 个字段，与 `GuiSettings` 同构；未来 Agent 架构变更时只需扩展字段，无结构破坏 |

**回退策略**：若 D1 遇到 `profiles.toml` 与现有 Settings 增量保存的冲突不可解，回退到仅实现 D2+D3，将 D1 拆分为 Sprint 11。

---

## 七、与长程路线图的对齐

| 长程目标 | Sprint 10 贡献 |
|----------|---------------|
| v0.3.2 健康基线（W3–W6） | D2/D3/D4 直接贡献测试基线与可维护性 |
| v0.4.0-beta 功能收敛（W7–W14） | D1 的 `AgentProfile` 为 Agent 级覆盖提供数据基础，Phase 3 解冻后可直接复用 |
| v0.5.0-beta 集群语义验证 | `profiles.toml` 可作为多节点配置同步的原子单元 |

---

*本计划由 Kimi 交叉审计触发，经实机代码验证后制定。任何与代码实态的冲突以代码为准。*
