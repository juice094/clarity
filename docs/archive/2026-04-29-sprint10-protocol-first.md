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


---

## 附录：超越 Kimi CLI 视角路线（2026-04-29 纠正记录）

> **前置纠正**：此前分析将差距归结为"架构定位不同"，建议"错位竞争"。经实机代码审计后，该结论错误。Clarity 的底层能力已超过或持平 Kimi CLI，差距集中在**一层整合**——将分散在 Subagent/Headless/TUI 各层的能力统一注入主 Agent 的默认编码工作流。
>
> **核心结论**：这不是"能不能替代"的问题，是"愿不愿意花 2–4 周整合"的问题。

---

### A. 被低估的已有能力基础

| Kimi CLI 能力 | Clarity 已有基础 | 代码位置 |
|---------------|-----------------|---------|
| **Git 上下文** | `GitContext::collect()` 完整实现（分支/最近提交/未提交文件） | `subagents/runner.rs:482-548` |
| **Plan 模式** | Plan 生成 + 执行 + egui Review UI + TUI `/plan`/`/execute` | `agent/plan.rs`, `panels/chat.rs:157-223` |
| **并发工具执行** | ReAct 循环中多个 tool call 通过 `join_all` **并行执行** | `agent/run.rs:51-96` |
| **审批交互** | TUI `DiffPopup`（文件编辑审批）+ `ToolResultPopup` | `popups/diff_popup.rs` |
| **Skill 自动发现** | 自动扫描 `.clarity/skills/` 和 `.claude/skills/`，按路径模式激活 | `skills/registry.rs:111-159` |
| **Headless 脚本化** | `--prompt`/`--file`/`--plan`/`--approval` 全参数支持 | `headless/src/main.rs:22-68` |
| **代码编辑** | `file_edit` 纯字符串替换 + `_diff_preview` 变更对比 | `tools/file.rs:385-471` |

**关键发现**：`GitContext::collect` 只注入 Subagent，主 Agent 的 `SystemPromptBuilder` 完全没有调用它。这不是"没有"，是"没连上"。

---

### B. 真实的差距清单（可量化）

#### P0 — 阻塞日常编码工作流

| # | 差距 | 根因 | 工作量 |
|---|------|------|--------|
| 1 | **主 Agent 无 Git 上下文** | `GitContext::collect` 只给 Subagent，主 Agent `SystemPromptBuilder` 没调用 | 1–2 天 |
| 2 | **主 Agent 无项目文件树** | `active_file_paths` 只用于 Skill 激活，不注入 prompt | 2–3 天 |
| 3 | **file_edit 精度不足** | 纯 `String::replace`，无 AST、无多位置批量替换 | 1–2 周 |
| 4 | **TUI 缺 `/yolo` 命令** | 硬编码 `ApprovalMode::Interactive`，无法运行时切换 | 半天 |

#### P1 — 体验差距

| # | 差距 | 根因 | 工作量 |
|---|------|------|--------|
| 5 | **Headless 不支持 stdin 管道** | `read_prompt()` 只处理 `--prompt`/`--file`，不读 `std::io::stdin()` | 1 天 |
| 6 | **无自动项目元数据读取** | `SystemPromptBuilder` 不自动读取 `Cargo.toml`/`package.json` | 2–3 天 |
| 7 | **Plan 步骤顺序执行** | `execute_plan` 是 `for` 循环，未利用 `dispatch_tool_calls` 的 `join_all` 并行 | 2–3 天 |

---

### C. 从"已有基础"到"超越"的路径

**不是架构重构，是能力整合。**

#### Phase A：上下文注入（1 周）

将 Subagent 层已有的能力提升到主 Agent：

```rust
// agent/prompt.rs — SystemPromptBuilder 新增
fn auto_context(&self) -> String {
    let mut ctx = String::new();
    // 1. GitContext（已有实现，只需调用）
    if let Some(git) = GitContext::collect(&self.working_dir).await {
        ctx.push_str(&git.to_prompt_string());
    }
    // 2. 项目元数据（轻量扫描）
    if let Ok(manifest) = std::fs::read_to_string(self.working_dir.join("Cargo.toml")) {
        ctx.push_str(&format!("\n# Cargo.toml\n```toml\n{}\n```\n", &manifest[..1024]));
    }
    // 3. 文件树（浅层）
    ctx.push_str(&format!("\n# File Tree\n{}", shallow_tree(&self.working_dir, 2)));
    ctx
}
```

#### Phase B：编辑精度升级（1–2 周）

升级 `file_edit` 工具：

| 当前 | 目标 | 实现方式 |
|------|------|---------|
| 单 `old_string`/`new_string` | 支持 `Vec<{old, new}>` 批量替换 | Schema 扩展 + 循环 `replacen` |
| 无 AST | 新增 `file_edit_ast` 工具（可选） | 引入 `tree-sitter` 或 `syn`（Rust 专用） |
| 全文件 `_diff_preview` | 生成标准 unified diff | `diff` crate 或自研行级 diff |

#### Phase C：终端体验补齐（3–5 天）

- TUI 添加 `/yolo` 命令（切换 `ApprovalMode::Yolo`）
- TUI 添加 `/interactive` 命令（切回）
- Headless 添加 stdin 读取：`echo "prompt" | clarity-headless`

---

### D. 与 Kimi CLI 的对比修正

| 维度 | Kimi CLI | Clarity（当前实际） | 超越所需时间 |
|------|---------|-------------------|-------------|
| **代码编辑** | Diff 应用、AST 感知 | 字符串替换，但有 `_diff_preview` | 1–2 周 |
| **上下文提取** | 自动（git + 项目结构） | **Git 已有（Subagent）**，项目结构缺自动扫描 | 1 周 |
| **Plan 执行** | 有 | **有**，且 egui 有 Review UI | ✅ 已达标 |
| **并发编辑** | 有 | **有**（`join_all`），但 Plan 模式未利用 | 2–3 天 |
| **审批交互** | 终端内 Y/N/Always | TUI 有 DiffPopup，**缺运行时切换命令** | 半天 |
| **管道友好** | 支持 | Headless **stdout 可管道**，stdin 不可 | 1 天 |
| **离线可用** | 否 | **本地 LLM + 自动 fallback** | ✅ 已超越 |
| **多模型** | 仅限 Kimi | **OpenAI/Anthropic/Kimi/DeepSeek/Local + TOML 自定义** | ✅ 已超越 |
| **MCP 生态** | 可能有 | **stdio/HTTP/SSE 三协议完整** | ✅ 已超越 |
| **记忆持久化** | 会话级 | **SQLite + BM25 + 向量，跨会话** | ✅ 已超越 |

---

### E. 决策记录

**此前错误**：将差距归结为"架构定位不同"，建议"错位竞争"。

**纠正后**：Clarity 的底层能力（Git 上下文、Plan 模式、并发执行、Skill 系统、MCP、记忆）**已经超过或持平** Kimi CLI。差距集中在**一层整合**——将这些分散在 Subagent/Headless/TUI 各层的能力，统一注入到主 Agent 的默认编码工作流中。

**总工作量估算**：2–4 周（Phase A 1 周 + Phase B 1–2 周 + Phase C 3–5 天）。

**触发条件**：若用户确认"超越 Kimi CLI"为战略优先级，可将上述 Phase A/B/C 纳入 Sprint 11，取代原定的"egui 审批 UI"方向。
