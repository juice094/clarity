# Clarity 项目快照

> 生成时间：2026-05-01 12:30 (第三次巡检 — 增量更新)
> 分支：`phase2/protocol-pilot` @ `b5609b18`
> 工作区：`C:\Users\22414\dev\third_party\clarity`

---

## 自 12:00 以来的变化（30 分钟）

**持续活跃 — 18 文件修改，+412 / -204 行，2 个新未跟踪文件**

### 🆕 新增子系统

| 类别 | 内容 | 状态 |
|------|------|------|
| **i18n** | 国际化模块 `i18n.rs` — `Locale` enum (EnUS/ZhCN)，静态中文翻译 HashMap，`t()` 便捷方法 | **新增** |
| **Custom Titlebar** | 自定义标题栏 — `render_titlebar()`，`with_decorations(false)`，窗口拖拽 + 三按钮（关闭/最大化/最小化） | **新增** |

### 核心架构进展（上一轮延续）

| 类别 | 内容 | 状态 |
|------|------|------|
| **B1** | `PersistingApprovalRuntime` — 审批持久化包装器（委托模式，挂载 `MemoryStore`） | ✅ **已实现** |
| **B2** | Approval Request ID 一致性校验（`controller.rs` 中验证 pending 列表） | ✅ **已实现** |
| **B3** | Agent 身份统一 — `Plan`/`PlanStep`/`PlanResult` 移至 `types.rs`，`agent/mod.rs` 保留向后兼容 re-export | ✅ **已实现** |
| **C1** | `ProviderSelectionPolicy` trait + `DefaultProviderSelectionPolicy`（纯同步、无副作用）+ 单元测试 | ✅ **已实现** |
| **C2** | 策略不触发网络探测 — 由调用方负责（`network_available` 参数传入） | ✅ **已实现** |
| **P1-1** | `AgentTypeDefinition`/`LaborMarket` 移出 `subagents/registry.rs` 到 `types.rs`，打破 `background↔subagents` 循环依赖 | ✅ **已实现** |

### UI 主题深度演进

- **主色**: Warm Copper (#c98a5e)，搭配 cool blue-gray 背景（`#12141e` → `#1e2030` 层级渐变）
- **Overlay 层级**: 5 级透明度层 — 从 `overlay_subtle`(3%) 到 `overlay_strong`(18%)
- **阴影系统**: 4 级 z-depth — `shadow_card` / `shadow_panel` / `shadow_modal` / `shadow_toast`
- **语义表面色**: `tool_call_bg` / `code_block_bg` / `mood_bg` — 内容类型级差异
- **间距**: 8px 基线网格，扩展 `space_40`(5×)
- 所有状态色 (ok/warn/danger) 也调整为 Warm Copper 配套的 warm-muted 色系

### 待提交的工作区变更

18 个已修改文件（全部在 `clarity-egui` crate），2 个新未跟踪文件：
- `crates/clarity-egui/src/i18n.rs` — 国际化支持
- `260501-备注.md`（内容片段）

### 尚未开始

- Phase 1（主 Agent 上下文汇流 — GitContext + 文件树 + 记忆注入）
- Phase 2+（Plan 并行执行、UI 统一层）
- Approval SQLite 持久化（B1 的存储后端替代）
- 子代理面板、团队协调 UI、Console 面板

---

## 开发节奏观察

用户从 **Phase C 架构解耦**自然过渡到 **Phase A UI 基础设施**建设（自定义标题栏 + i18n + 主题细化）。方向明确：先用原生 egui 打造精良的桌面体验底子，再继续推进功能层。工作模式是高频增量提交。
