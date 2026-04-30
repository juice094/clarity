# Clarity 硬事实快照 · 2026-04-30

> 性质：可验证的代码库状态记录。
> 生成方式：Shell 命令扫描 + 文本提取。
> 验证方式：复现下方每条 `[VERIFY]` 命令，对比输出。

---

## 1. 版本控制状态

| 项 | 值 | 验证命令 |
|---|---|---------|
| 分支 | `phase2/protocol-pilot` | `[VERIFY]` `git branch --show-current` |
| Commit | `20049c99` | `[VERIFY]` `git rev-parse --short HEAD` |
| 生成时间 | 2026-04-30 | `[VERIFY]` `Get-Date -Format "yyyy-MM-dd"` |

---

## 2. Workspace 结构（可验证）

### 2.1 Crate 清单

```
clarity-claw
clarity-core
clarity-egui
clarity-gateway
clarity-headless
clarity-memory
clarity-tui
clarity-wire
tests/integration
```

**验证命令**：
```powershell
[VERIFY] Get-ChildItem crates | Select-Object Name
[VERIFY] Get-Content Cargo.toml | Select-String "^members" -Context 0,2
```

### 2.2 关键依赖（跨 crate）

| 下游 Crate | 上游依赖 | 验证命令 |
|-----------|---------|---------|
| `clarity-egui` | `clarity-core`, `clarity-wire` | `[VERIFY]` `Get-Content crates/clarity-egui/Cargo.toml \| Select-String "clarity-"` |
| `clarity-gateway` | `clarity-core`, `clarity-wire`, `clarity-memory` | `[VERIFY]` `Get-Content crates/clarity-gateway/Cargo.toml \| Select-String "clarity-"` |
| `clarity-core` | 无内部依赖 | `[VERIFY]` `Get-Content crates/clarity-core/Cargo.toml \| Select-String "clarity-"` |

---

## 3. 测试基线（可验证）

```bash
[VERIFY] cargo test --workspace --lib
```

### 3.1 逐 Crate 结果

| Crate | Passed | Failed | Ignored | 耗时 |
|-------|--------|--------|---------|------|
| `clarity-claw` | 6 | 0 | 0 | 0.00s |
| `clarity-core` | 427 | 0 | 6 | 10.33s |
| `clarity-gateway` | 43 | 0 | 0 | 0.05s |
| `clarity-integration-tests` | 0 | 0 | 0 | 0.00s |
| `clarity-memory` | 79 | 0 | 0 | 0.15s |
| `clarity-tui` | 6 | 0 | 0 | 0.00s |
| `clarity-wire` | 16 | 0 | 0 | 0.00s |
| **总计** | **577** | **0** | **6** | — |

### 3.2 其他基线命令

```bash
[VERIFY] cargo clippy --workspace --lib --bins --tests -- -D warnings
[VERIFY] cargo fmt --all -- --check
[VERIFY] cargo doc --no-deps
```

---

## 4. clarity-core 模块清单（可验证）

### 4.1 根模块（lib.rs 声明）

```
activity, agent, approval, autodream, background, capability, compaction,
config, daemon, diff, error, hooks, llm, mcp, memory, model_download,
notifications, personality, registry, server, skills, subagents, tools,
types, view_models
```

**验证命令**：
```powershell
[VERIFY] Get-Content crates/clarity-core/src/lib.rs | Select-String "^pub mod"
```

### 4.2 agent/ 目录文件清单

```
compaction_service.rs, config.rs, construct.rs, controller.rs, driver.rs,
enhanced.rs, execution.rs, mod.rs, ops.rs, plan.rs, prompt.rs, run.rs, tests.rs
```

**验证命令**：
```powershell
[VERIFY] Get-ChildItem crates/clarity-core/src/agent -File | Select-Object Name
```

### 4.3 tools/ 目录文件清单

```
ask_user.rs, channel.rs, computer.rs, cron.rs, file.rs, mod.rs, notify.rs,
plan.rs, search.rs, shell.rs, task.rs, team.rs, think.rs, todo.rs, web.rs, web_browser.rs
```

**验证命令**：
```powershell
[VERIFY] Get-ChildItem crates/clarity-core/src/tools -File | Select-Object Name
```

### 4.4 subagents/ 目录文件清单

```
builder.rs, mod.rs, parallel.rs, registry.rs, runner.rs, store.rs, team.rs, token.rs
```

**验证命令**：
```powershell
[VERIFY] Get-ChildItem crates/clarity-core/src/subagents -File | Select-Object Name
```

---

## 5. clarity-egui 文件清单（可验证）

```
app_logic.rs, app_state.rs, error.rs, llm_binder.rs, llm_loader.rs,
llm_policy.rs, main.rs, onboarding.rs, session.rs, settings.rs, theme.rs
```

**验证命令**：
```powershell
[VERIFY] Get-ChildItem crates/clarity-egui/src -File | Select-Object Name
```

---

## 6. 关键接口位置（基于文件存在性）

| 接口 | 文件路径 | 验证命令 |
|------|---------|---------|
| `Agent` struct | `crates/clarity-core/src/agent/mod.rs` | `[VERIFY]` `Select-String "pub struct Agent" crates/clarity-core/src/agent/mod.rs` |
| `ApprovalRuntime` trait | `crates/clarity-core/src/approval/mod.rs` | `[VERIFY]` `Select-String "pub trait ApprovalRuntime" crates/clarity-core/src/approval/mod.rs` |
| `LlmProvider` trait | `crates/clarity-core/src/llm/mod.rs` | `[VERIFY]` `Select-String "pub trait LlmProvider" crates/clarity-core/src/llm/mod.rs` |
| `WireMessage` enum | `crates/clarity-wire/src/lib.rs` | `[VERIFY]` `Select-String "pub enum WireMessage" crates/clarity-wire/src/lib.rs` |
| `Tool` trait | `crates/clarity-core/src/tools/mod.rs` | `[VERIFY]` `Select-String "pub trait Tool" crates/clarity-core/src/tools/mod.rs` |
| `AgentController` | `crates/clarity-core/src/agent/controller.rs` | `[VERIFY]` `Select-String "pub struct AgentController" crates/clarity-core/src/agent/controller.rs` |

---

## 7. 已知文档位置（可验证）

| 文档 | 路径 | 验证命令 |
|------|------|---------|
| AGENTS.md（根） | `AGENTS.md` | `[VERIFY]` `Test-Path AGENTS.md` |
| AGENTS.md（clarity） | `crates/clarity-egui/AGENTS.md` 等 | `[VERIFY]` `Get-ChildItem -Recurse -Filter AGENTS.md` |
| ai-protocol.md | `docs/ai-protocol.md` | `[VERIFY]` `Test-Path docs/ai-protocol.md` |
| ARCHITECTURE.md | `docs/ARCHITECTURE.md` | `[VERIFY]` `Test-Path docs/ARCHITECTURE.md` |
| 架构地图 | `docs/architecture/` | `[VERIFY]` `Get-ChildItem docs/architecture` |

---

## 8. 会话摘要（不可核实，仅供参考）

以下内容为本次会话的高层结论，**无法通过命令验证**，仅作为元认知参考：

1. 定义了替代现有分散式 AI 工具链的层级化多 Agent 操作系统需求（8 条功能边界）。
2. 探索了架构地图作为全 AI 开发模式下认知约束工具的可行性，审计发现 AI 输出存在系统性事实幻觉风险。

详细摘要见 `docs/architecture/SESSION_SUMMARY.md`。

---

*本文件由 Shell 扫描生成，所有事实均可通过 [VERIFY] 标记的命令复现验证。*
