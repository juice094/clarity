# 今日解耦路线图 · 2026-04-27

> 目标：适应分发标准（crates.io 就绪），向外便于他人使用（稳定公共 API），向内维护项目健康（减少耦合）。

---

## 一、诊断快照（已完成）

### 1.1 模块规模
| 指标 | 数值 |
|------|------|
| clarity-core .rs 文件 | 87 |
| 顶层 pub mod | 25 |
| pub / pub(crate) 比例 | 427 : 2 |
| MCP 专属文件 | 5（~2300 行） |

### 1.2 循环依赖（5 处）
| 循环 | 严重度 | 打破方式 |
|------|--------|----------|
| `background/cron` ↔ `background/store` | 🟡 中 | 提取共享类型到 `background/types.rs` |
| `tools` ↔ `subagents::token` | 🔴 高 | `CapabilityToken` 上提至 `types.rs` |
| `llm` ↔ `agent`（伪） | 🟢 低 | `llm` 直接引用 `types::FunctionCall` 而非 `agent` |
| `subagents` ↔ `agent` | 🔴 高 | `subagents` 需要 `Agent` 实例；需 trait 抽象 |
| `background` ↔ `subagents` | 🔴 高 | `background` 需 `AgentTypeDefinition`；双向任务调度 |

### 1.3 跨 Crate 依赖（下游对 core 的使用）
| 下游 | 深度 | 高频子模块 |
|------|------|-----------|
| clarity-egui | 🔴 最深 | `agent::*`, `approval::*`, `view_models::settings`, `model_download` |
| clarity-tui | 🟠 深 | `agent::*`, `mcp::*`, `skills::*` |
| clarity-gateway | 🟠 深 | `agent::*`, `background::*`, `activity::*` |
| clarity-headless | 🟡 浅 | `llm::LocalGgufConfig`, `AgentError` |

### 1.4 MCP 提取可行度
**结论：物理隔离已完成，逻辑依赖可控。**
- MCP 5 个文件全部在 `mcp/` 目录内
- 核心层（lib.rs 及以下）**无代码级反向依赖**
- MCP 单向依赖：`error::AgentError/ToolError` + `tools::{Tool,ToolContext,ToolResult}` + `registry::ToolRegistry`
- 障碍：error/tools/registry 目前和 core 其他模块深度纠缠

---

## 二、策略选项

### 选项 A：保守收缩（今日第一刀）
**动作**：把 25 个 `pub mod` 中未被下游使用的改为 `pub(crate) mod`。  
**预期**：压缩到 ~10 个 pub mod，公共 API 面缩小 60%。  
**风险**：低。仅需确认下游编译。  
**时间**：1–2 h。

### 选项 B：提取 `clarity-contract`（通用契约层）
**动作**：新建 crate，放入 `error` + `tools`（仅 trait/上下文/结果类型）+ `registry`（仅接口）。  
**预期**：MCP 提取的前置条件达成；workspace 获得稳定的底层契约。  
**风险**：中。需处理 `AgentError::Tool(ToolError)` 跨 crate 的 `#[from]` 派生。  
**时间**：2–3 h。

### 选项 C：直接提取 `clarity-mcp`
**动作**：整体移动 `mcp/` 目录到独立 crate。  
**预期**：core 减负 ~2300 行，MCP 可独立演进/发布。  
**风险**：高。必须先完成选项 B，否则依赖混乱。  
**时间**：3–4 h（含 B）。

### 选项 D：打破循环依赖
**动作**：`CapabilityToken` 上提、`Agent` trait 抽象、`background` 共享类型提取。  
**预期**：消除 5 处循环，core 内部 DAG 化。  
**风险**：高。涉及核心架构变动，测试覆盖需充分。  
**时间**：4–6 h。

---

## 三、推荐路线：A → B →（评估 C）

> 原则：先止血（收缩 API），再输血（提取契约），最后手术（MCP/循环）。

### 阶段 1：公共 API 面收缩（1–2 h）
1. 精确统计每个 `pub mod` 的跨 crate 引用次数
2. 将零引用或仅内部引用的模块降级为 `pub(crate) mod`
3. 保留的 pub mod 清单（预估）：
   - `agent`, `approval`, `background`, `error`, `llm`, `registry`, `skills`, `subagents`, `tools`, `types`
   - （移除 pub：`activity`, `autodream`, `capability`, `compaction`, `config`, `daemon`, `diff`, `hooks`, `mcp`, `memory`, `model_download`, `notifications`, `personality`, `server`, `view_models`）
4. 对下游（egui/tui/gateway/headless）进行编译修复
5. 运行 `cargo test --workspace` + `cargo clippy`

### 阶段 2：提取 `clarity-contract`（2–3 h）
1. 新建 `crates/clarity-contract/` crate
2. 迁移内容：
   - `error.rs` → `clarity-contract/src/error.rs`
   - `tools/mod.rs` 中的 `Tool` trait + `ToolContext` + `ToolResult` → `clarity-contract/src/tools.rs`
   - `registry.rs` 中的 `ToolRegistry` 接口 → `clarity-contract/src/registry.rs`
3. 处理 `AgentError::Tool(#[from] ToolError)` 跨 crate 问题：
   - 方案：在 contract 中保留两者，core 中 `pub use clarity_contract::{AgentError, ToolError}`
   - 或在 core 中新增 wrapper
4. 更新 `clarity-core/Cargo.toml` 添加 `clarity-contract` 依赖
5. 更新下游 crate 的 `Cargo.toml`，按需直接依赖 `clarity-contract`
6. 编译 + 测试 + clippy

### 阶段 3：MCP 提取 PoC（评估，不立即执行）
- 若阶段 1+2 顺利完成且剩余时间 > 2h，启动阶段 3
- 否则记录为明日优先任务

---

## 四、验收标准

| 检查项 | 通过条件 |
|--------|----------|
| 编译 | `cargo build --workspace` 0 error |
| 测试 | `cargo test --workspace --lib` ≥ 577 passed |
| Clippy | `cargo clippy --workspace -- -D warnings` 0 warning |
| 循环依赖 | `cargo modules generate graph --lib` 无循环（或手工确认 5 处中至少减少 1 处） |
| API 面 | pub mod 数量从 25 降至 ≤ 12 |
| 文档 | `cargo doc --workspace --no-deps` 通过 |

---

## 五、风险与回滚

- **风险**：下游 crate 对隐藏模块存在深层 use（如 `clarity_core::view_models::settings::ThemeConfig`）。  
  **缓解**：阶段 1 先做全量搜索确认，再修改；若误伤，立即 `git checkout` 回滚单文件。
- **风险**：`AgentError` 跨 crate 的 `#[from]` 派生失效。  
  **缓解**：保留原 error.rs 在 core 中作为 thin wrapper，contract 放基础定义。

---

*计划制定时间：2026-04-27*  
*状态：待执行*
