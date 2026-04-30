# Sprint 12 — egui 功能补齐

> 前置: Sprint 11 验证通过 (`be39b4e`)
> 目标: 将 `clarity-core` 中已完备的能力完整暴露到 `clarity-egui`
> 周期: 2 周

---

## 一、Sprint 11 验证结论

V2 端到端验证全部通过：

| 场景 | 结果 |
|------|------|
| 上下文注入 | ✅ System Prompt 自动包含 Git 分支 + 文件树 + Cargo.toml |
| 批量替换 | ✅ `replacements` 数组 + unified diff patch 格式正确 |
| 模式切换 | ✅ `/yolo` / `/interactive` / `/planmode` 实时生效 |
| 向后兼容 | ✅ legacy `old_string`/`new_string`/`replace_all` 无回归 |

**决策**: 验证效果好 → Sprint 12 方向为 **egui 功能补齐**。

---

## 二、核心差距

egui 是当前主力 GUI 栈，但 `clarity-core` 中大量能力未暴露：

| core 能力 | egui 状态 | 阻塞日常编码? |
|-----------|----------|--------------|
| 审批交互 UI | ❌ 仅 yolo，无弹窗 | 🔴 是 — Interactive/Plan 模式实际不可用 |
| Plan 步骤可视化 | ❌ 无 | 🔴 是 — Plan 模式生成后无法分步查看/审批 |
| Skill 列表/切换 | ❌ 无 | 🟡 中 — 无法激活 Skill，依赖自动路径匹配 |
| Token 用量显示 | ❌ 无 | 🟡 中 — 无法感知消耗 |
| 后台任务面板 | 只读 | 🟡 中 — 无创建/取消/Cron 配置 |
| 子代理进度 | ❌ 无 | 🟢 低 — 当前使用场景以单 Agent 为主 |

---

## 三、执行计划

### Week 1: 审批弹窗 + Plan 可视化

**Day 1-2: 审批弹窗原型**
- 目标: Interactive 模式下 tool_call 前弹出 Diff 预览 + 确认/取消按钮
- 技术路径: `clarity-wire` 事件驱动 — core  emit `ApprovalRequest` → egui 渲染弹窗 → 用户选择后 emit `ApprovalResponse`
- 复用: `_diff_patch` 字符串直接渲染（无需重新计算 diff）

**Day 3-4: Plan 步骤面板**
- 目标: Plan 生成后展示步骤列表，支持单步审批/跳过/重试
- 技术路径: Plan 模式运行时 egui 进入 `PlanReview` 状态，渲染步骤列表

**Day 5: 集成测试**
- 验证: Interactive 模式下 file_edit 触发审批弹窗 → 确认后执行 → 取消后回滚

### Week 2: Skill UI + Token 显示 + 收尾

**Day 1-2: Skill 面板**
- 目标: Settings 旁新增 Skill 标签页，列出可用 Skill，支持手动激活/停用
- 技术路径: 复用 `SkillRegistry::active_ids()` 和 `discover_for_path()`

**Day 3: Token 用量显示**
- 目标: 聊天区域底部显示当前轮次/会话的 token 消耗
- 技术路径: `Usage` WireMessage 已发送，egui 只需接收并渲染

**Day 4-5: 回归测试 + 文档**
- 运行 `cargo test --workspace --lib` + `cargo clippy`
- 更新 `PROJECT_STATUS.md` / `AGENTS.md`

---

## 四、风险与缓解

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| egui 零单元测试，UI 回归难发现 | 高 | 中 | 新增至少 3 个纯逻辑测试（审批状态机、Plan 步骤解析、Skill 激活） |
| 审批弹窗与现有事件循环冲突 | 中 | 高 | 参考 TUI `DiffPopup` 的事件处理模式，用 Wire 事件解耦 |
| Plan 可视化复杂度超预期 | 中 | 中 | MVP 只做步骤列表 + 单步确认，不做图形化 DAG |
| 2 周时间不够 | 中 | 低 | 优先级: 审批弹窗 > Plan 步骤 > Skill > Token；低优先级可延期 |

---

## 五、验收标准

- [ ] Interactive 模式下 file_edit 触发审批弹窗，显示 unified diff patch
- [ ] Plan 模式下生成步骤列表，支持 Enter 确认执行 / Esc 跳过
- [ ] Skill 面板可手动激活/停用 Skill
- [ ] Token 用量在消息区域底部显示
- [ ] `cargo test --workspace --lib` 全绿
- [ ] `cargo clippy --workspace --lib --bins --tests -- -D warnings` 零警告

---

> 本计划受能力汇流审计协议 v1.0 统辖。
