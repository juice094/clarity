# 2026-05-05 项目存档 — Clarity Sprint 36.5/36.6 完成 + Sprint 37 就绪

> 归档时间：2026-05-05 23:04 CST
> 基线分支：`main` @ `91d20d73`
> 会话：Kimi CLI — juice094 工作区

---

## 一、今日提交（共 8 个 commit）

| Hash | 说明 |
|------|------|
| `91d20d73` | cleanup(egui): 移除死 toolbar.rs + 废弃文档 |
| `08f32de6` | feat(egui): Sprint 36.5 + 36.6 — cron sidebar 迁移、markdown 表格、死代码清理、FIXME-WEEK1-RISK 修复 |
| `f45bfd0b` | docs(research): DeepSeek-TUI 技术对比与可借鉴模式 |
| `b1b72660` | docs(BACKLOG): 同步 egui parity 矩阵 |
| `ff925318` | feat(core): 子代理状态持久化到磁盘 (Sprint 36-P1) |
| `69d353ce` | feat(egui): Cron + Team 协调 UI 面板 (Sprint 36) |
| `7e3539f5` | docs: AGENTS.md + README.md 更新至 Sprint 35 |
| `73bc2a3c` | feat(egui): 子代理预算进度条 (Sprint 35-C) |

---

## 二、Sprint 36.5 完成项

| ID | 事项 | 状态 |
|----|------|------|
| S36.5-A | Agent/Gateway 状态指示器从 sidebar → Workspace 面板标题栏右侧 | ✅ |
| S36.5-B | Dead code 清理：5 个未使用图标常量、`UiEvent` 死变体、`SubAgentProgress` 死字段 | ✅ |
| S36.5-C | FIXME-WEEK1-RISK ×3：rapid-Enter debounce + `stopping...` 视觉状态 + session-delete draft race | ✅ |

## 三、Sprint 36.6 完成项

| ID | 事项 | 状态 |
|----|------|------|
| S36.6-A | Cron Jobs 从右侧独立 `SidePanel::right` 迁移至左侧 sidebar 可折叠 section | ✅ |
| S36.6-B | Markdown 表格渲染 — `RenderBlock::Table` + `egui::Grid` 轻量解析器（零外部依赖） | ✅ |

## 四、质量验证

```
cargo test --workspace --lib   # 728 passed / 0 failed / 6 ignored
cargo check -p clarity-egui    # 0 warnings
git status --short             # 工作区干净（无未提交变更）
```

---

## 五、紧急事项检查

| 检查项 | 结果 | 说明 |
|--------|------|------|
| 未提交变更 | ✅ 无 | `git status` 干净 |
| 测试失败 | ✅ 无 | 728 passed / 0 failed |
| FIXME-WEEK1-RISK | ✅ 无残留 | Sprint 36.5-C 已全部修复 |
| FIXME-CRITICAL | ✅ 无 | 全代码库扫描无匹配 |
| 编译警告 | ✅ 0 | `cargo check` 0 warnings |
| panic/unimplemented! 在生产路径 | ✅ 无 | 仅测试/CLI 匹配臂有预期 panic |
| cargo audit 新增漏洞 | 🟡 未扫描 | Tauri 间接依赖历史已知，非新增 |

### 遗留 TODO（非紧急）

- `model_download.rs`: TODO(Sprint-31-debt) — 迁移至 `clarity-infrastructure`
- egui 6 个 backend integration 桩（team_create/team/cron_create/cron）— UI 已就绪，待对接 core
- `handlers/chat.rs`: decompose App 依赖 — 架构改进
- `widgets/mod.rs`: 2 个注释掉的 pub use — 待启用

---

## 六、后续规划资料完备性

| 资料 | 状态 | 路径 |
|------|------|------|
| BACKLOG 总览 | ✅ 已同步至 Sprint 36.6 | `docs/plans/BACKLOG.md` |
| Sprint 37 深度计划 | ✅ 就绪 | `docs/plans/2026-05-05-sprint27-a2-prompt-cache-key.md` |
| DeepSeek-TUI 技术对比 | ✅ 已归档 | `docs/research/deepseek-tui-comparison-2026-05-05.md` |
| AGENTS.md | ✅ 同步至 Sprint 36.6 | `AGENTS.md` |
| 解耦计划 | ✅ 未变更 | `docs/plans/2026-04-27-decoupling.md` |
| Jumpy 轨道 | ✅ 未变更 | `docs/plans/nightcrawler-drax-atom.md` |

### Sprint 37 轨道（已定稿）

| Sprint | ID | 事项 | 优先级 | 预估 | 来源 |
|--------|----|------|--------|------|------|
| 37 | S37-A | `prompt_cache_key` 策略层 | **P0** | 2–3d | DeepSeek-TUI #1 |
| 37 | S37-B | working tree 提交机制 | **P0** | 0.5d | 归档 Sprint 36.5/36.6（已完成） |
| 37 | S37-C | LSP 轻量级 stdio 客户端 | **P0** | 3–4d | DeepSeek-TUI #2 |
| 37 | S37-D | 进程级成本旁路通道 | **P0** | 1–2d | DeepSeek-TUI #3 |

### Sprint 38–39 轨道（已定稿）

| Sprint | ID | 事项 | 优先级 |
|--------|----|------|--------|
| 38 | S38-A | egui 测试基线 | P1 |
| 38 | S38-B | J5 — Jumpy 历史观测提取 | P1 |
| 38 | S38-C | Side-Git 工作区快照 MVP | P1 |
| 38 | S38-D | 子代理 Mailbox + CancellationToken 级联 | P1 |
| 39 | S39-A | KV cache 跨 turn/跨会话 | P1 |
| 39 | S39-B | `refresh_context()` 统一 | P2 |
| 39 | S39-C | RLM 简化版评估 | P2 |

---

## 七、技术债务快照

| 债务项 | 严重度 | 状态 |
|--------|--------|------|
| egui 零测试 | 🔴 重大 | 计划 Sprint 38 |
| `unwrap()` 密度 (~170) | 🟡 中 | 冻结新增，目标 ≤150 |
| Agent 空响应防御 | 🟡 中 | Sprint 14 已修复，持续监控 |
| cargo audit 上游漏洞 | 🟡 低 | Tauri 间接依赖，等上游 |
| 文档过时 | 🟢 低 | 已同步至 Sprint 36.6 |

---

## 八、已移除文件（移至回收站）

- `crates/clarity-egui/src/panels/toolbar.rs`（Sprint 14 遗留）
- `docs/archive/channel-roadmap-deprecated.md`
- `docs/archive/execution-plan-claw-era.md`
- `docs/archive/execution-plan-v2-deprecated.md`

---

*存档生成：Kimi CLI — 工作区 juice094*
*下次审视：Sprint 37 结束时*
