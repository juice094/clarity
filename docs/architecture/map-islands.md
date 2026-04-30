# 架构地图 · 能力孤岛索引

> 用途：防止"已有能力被重复实现"
> 更新触发：新能力落地、旧能力激活、能力废弃

---

## 孤岛定义

**能力孤岛** = 代码已存在、测试已通过、但**未被主流程（egui + 常用工作流）激活**的模块或功能。

子代理在接到任务时，应先查本索引，避免重复造轮子。

---

## 🔵 已就绪待 UI 激活

### IS-1 — 子代理 spawn（SubAgent）

| 属性 | 值 |
|------|---|
| 代码位置 | `crates/clarity-core/src/subagents/` |
| 测试状态 | ✅ 全绿（`subagents::tests`） |
| 主流程状态 | ❌ egui 无 spawn UI；只能通过 tool call 或代码调用 |
| 入口 | `subagents::builder::build_subagent()` / `SubAgent::spawn()` |
| 激活路径 | egui 添加 "Spawn SubAgent" 按钮 → 调用 builder → 在独立线程运行 |
| 关联 | `tools::task.rs` 内部已使用（task 工具会 spawn 子代理） |

### IS-2 — 子代理并行批处理（Parallel SubAgent）

| 属性 | 值 |
|------|---|
| 代码位置 | `crates/clarity-core/src/subagents/parallel.rs` |
| 测试状态 | ✅ 全绿 |
| 主流程状态 | ❌ 无 UI 暴露 |
| 入口 | `subagents::parallel::SubAgentBatch` |
| 激活路径 | 类似 IS-1，添加批量 spawn UI |

### IS-3 — 子代理权限 Token（Sandbox）

| 属性 | 值 |
|------|---|
| 代码位置 | `crates/clarity-core/src/subagents/token.rs` |
| 测试状态 | ✅ 全绿（`verify_allowed_tool`, `verify_read_only_blocks_write`） |
| 主流程状态 | ⚠️ 生产流程未接入；测试覆盖但运行时未校验 |
| 入口 | `subagents::token::Token::verify_*` |
| 激活路径 | 在 `subagents::runner::run()` 执行前插入 Token 校验 |

### IS-4 — 背景任务调度（Cron + Worker Pool）

| 属性 | 值 |
|------|---|
| 代码位置 | `crates/clarity-core/src/background/` |
| 测试状态 | ✅ 全绿（cron、worker、store 均有测试） |
| 主流程状态 | ❌ egui 无 cron 面板；工具存在但无前端 |
| 入口 | `tools::cron`（schedule_cron / list_cron / cancel_cron） |
| 激活路径 | egui Settings 或独立面板暴露 cron 管理 |

### IS-5 — Skill 系统（发现 / 加载 / 注册 / 执行）

| 属性 | 值 |
|------|---|
| 代码位置 | `crates/clarity-core/src/skills/` |
| 测试状态 | ✅ 全绿 |
| 主流程状态 | ⚠️ 发现/加载/注册已有；egui 无 Skill 管理面板 |
| 入口 | `skills::discovery::scan_project_skills()` / `skills::registry::activate_by_path()` |
| 激活路径 | egui Sidebar 添加 Skill 分组（Sprint 12 提及） |

### IS-6 — 能力发现协议（Capability Discovery）

| 属性 | 值 |
|------|---|
| 代码位置 | `crates/clarity-core/src/capability.rs` |
| 测试状态 | ✅ 全绿（`test_egui_all_modes` 等） |
| 主流程状态 | ⚠️ 枚举和测试已有；`CapabilityRegistry::supported_approval_modes(surface)` 未在 egui 启动时调用 |
| 入口 | `capability::CapabilityRegistry` |
| 激活路径 | egui 启动时查询 `supported_approval_modes(Gui)`，禁用不可用选项 |

---

## 🟡 已有但边缘化

### IS-7 — TUI 前端（clarity-tui）

| 属性 | 值 |
|------|---|
| 代码位置 | `crates/clarity-tui/` |
| 测试状态 | ✅ 6 tests passed |
| 主流程状态 | ❌ 非主力；功能远落后于 egui |
| 入口 | `cargo run -p clarity-tui` |
| 用途 | egui 崩溃时的 fallback；服务器环境下的轻量前端 |

### IS-8 — 无头模式（clarity-headless）

| 属性 | 值 |
|------|---|
| 代码位置 | `crates/clarity-headless/` |
| 测试状态 | 未知 |
| 主流程状态 | ❌ 未激活 |
| 用途 | CI/自动化场景的无 GUI 运行 |

### IS-9 — CLI 入口（clarity-claw）

| 属性 | 值 |
|------|---|
| 代码位置 | `crates/clarity-claw/` |
| 测试状态 | 未知 |
| 主流程状态 | ❌ 未激活 |
| 用途 | 命令行直接启动 Agent（非 TUI/GUI） |

### IS-10 — 守护进程（Daemon）

| 属性 | 值 |
|------|---|
| 代码位置 | `crates/clarity-core/src/daemon.rs` |
| 测试状态 | ✅ 全绿（锁获取/释放、过期清理） |
| 主流程状态 | ⚠️ 测试覆盖但生产未使用 |
| 入口 | `daemon::acquire_lock()` / `daemon::release_lock()` |
| 用途 | 防止 clarity 多实例同时运行 |

### IS-11 — 自动梦境（AutoDream）

| 属性 | 值 |
|------|---|
| 代码位置 | `crates/clarity-core/src/autodream.rs` |
| 测试状态 | ✅ 全绿 |
| 主流程状态 | ❌ 未接入主流程 |
| 用途 | 定时触发 Agent 自主思考/总结 |

### IS-12 — 人格/角色系统（Personality）

| 属性 | 值 |
|------|---|
| 代码位置 | `crates/clarity-core/src/personality/` |
| 测试状态 | ✅ 全绿 |
| 主流程状态 | ⚠️ 框架已有，但 "格雷" 是硬编码注入，非动态人格切换 |
| 入口 | `personality::domain::parse_domain_persona()` |
| 用途 | 支持多角色人格定义和动态加载 |

### IS-13 — 模型下载（Model Download）

| 属性 | 值 |
|------|---|
| 代码位置 | `crates/clarity-core/src/model_download.rs` |
| 测试状态 | ✅ 全绿 |
| 主流程状态 | ✅ **已激活**（v0.3.1 egui Settings 有下载进度条） |
| 备注 | 此能力**已出岛**，但仍列于此作为状态参考 |

---

## 🔴 已归档 / 废弃

### IS-14 — Tauri 前端（clarity-tauri）

| 属性 | 值 |
|------|---|
| 代码位置 | `dev/third_party/archived/`（或已删除） |
| 状态 | ❌ **已归档移出仓库**（AGENTS.md 确认） |
| 教训 | Tauri 2 替代 Electron 后仍被归档，说明前端技术栈收敛于 egui |

---

## 使用指南

**子代理接到任务时**：

1. 查本索引 → 能力是否已存在？
2. 若存在 → 读对应代码位置的 `mod.rs` 和 `tests.rs`
3. 若缺失 → 再新建模块

**示例**：

> 任务："给 egui 加一个 spawn 子代理的按钮"

查索引 → IS-1 已存在 → 读 `subagents::builder` → 在 egui 添加 UI 调用 → 无需新建子代理核心逻辑

> 任务："给 Agent 加一个新工具"

查索引 → 工具注册已有（`registry.rs`）→ 在 `tools/` 下新增文件 → 注册到 `registry.rs` → 符合现有模式

---

*本文件由 AI 会话维护。能力激活后应更新状态（🔵 → 🟢），废弃后标 🔴。*
