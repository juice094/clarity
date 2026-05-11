# Clarity 代码改动原则

> **Version**: 1.0
> **Status**: Mandatory — 所有 PR 适用
> **Effective**: 2026-05-11
> **Owner**: 主会话 (架构责任人)
> **Scope**: `crates/clarity-*` 全部 Rust 源文件
> **Supersedes**: 无（首次落地）

---

## 序言：为什么需要这份文档

`docs/architecture/`、`AGENTS.md`、`EGUI_LAYOUT.md`、`layout-audit-architecture-crisis.md`
等已经分别给出**局部规则**，但缺少一份**跨 crate、跨阶段、跨人员**的统一改动纪律。

历史教训：

1. Sprint 43 已宣布 egui 反模式冻结，但 `panels/dashboard.rs` / `panels/gantt.rs` 在冻结声明
   后仍引入了新的 `painter` 调用。**规则与执行脱节**。
2. `EventBus` / `view_sender` / `SettingsViewModel` 是为未来准备的协议层，但落库后**无消费者**，
   成为幻象建筑。
3. 协议变更需要在 6~8 个文件保持一致，但当前没有 lint / checklist 强制审查。

本文档以**最少必要纪律**约束所有后续 PR，违反任一原则的 PR 必须 reject 或显式豁免。

---

## P1 — 单向迁移（No Bidirectional Bridges）

**规则**：旧路径迁移到新路径**只允许一次切换**，禁止保留双向桥接以"备回退"。

**示例**：

```rust
// ❌ 反例
//   删除 EventBus 时同时保留 From<EventMsg> for WireMessage 作为"过渡期适配"
impl From<EventMsg> for WireMessage { ... }   // 留作回退？不允许

// ✅ 正例
//   直接删除 EventBus 及其 impl，git 回退即唯一回退路径
//   (commit message 必须标注: "BREAKING: removed EventBus, see ADR-XXX")
```

**理由**：
- 双向桥接是死代码温床，且会让"哪个是真相"的问题反复出现
- Git 提供可回退性，不需要在源码中冗余
- 真正不可逆的变更应当推迟到 ADR 充分讨论后再做

**例外**：
- 跨 crate 公共 API 变更，且有外部下游消费者时，可短暂保留 `#[deprecated(since = "x.y.z", note = "use ...")]`
- 例外必须有显式弃用时间表（不超过 2 个 sprint）

---

## P2 — 删除优先（Deletion Before Addition）

**规则**：每个 PR 必须**净删除代码**，或至少不增加 dead code / 不增加未使用接口。

**示例**：

```text
✅ 正例：
   PR #123: 新增 widgets/sidebar_card.rs
   ├── 新增 widgets/sidebar_card.rs (+180 行)
   └── 删除 panels/sidebar.rs:387-493 内联实现 (-150 行)
   净变化: +30 行 (但消除一处反模式)

❌ 反例：
   PR #124: 新增 widgets/sidebar_card.rs
   └── 新增 widgets/sidebar_card.rs (+180 行)
   说明: "下个 PR 替换调用方"
```

**理由**：
- 分两步走的迁移在第二步**永远不会到来**
- "下次再做"的承诺没有提交人责任绑定
- 净增加代码而无替换 = 死代码积累

**例外**：
- 新功能（非重构）允许净增加，但必须在 PR 描述中标注"新功能 / 非重构"
- 测试代码不计入净删除考核

---

## P3 — 单源真相（Single Source of Truth Per Concept）

**规则**：每个**概念**只能有一个写入点。

**当前已确立的单源**：

| 概念 | 单一写入点 | 不允许的写入路径 |
|------|----------|----------------|
| LLM 配置 | `SettingsViewModel`（待激活） | ~~`settings_edit`~~ / ~~`cached_settings`~~ / ~~`ACTIVE_CONFIG`~~ 直接修改 |
| 流式正文 | `WireMessage::ContentPart`（待落地） | ~~`run_streaming(query, callback)`~~ 的 closure |
| Theme token | `crates/clarity-egui/src/theme.rs` | 硬编码 `Color32::from_rgb(...)` 或 `+ 12.0` 字面量 |
| 协议契约 | `crates/clarity-contract/src/**` | `clarity-core` 内私有 trait 重复定义 |
| Turn 状态 | `chat_store.current_turn_id`（待引入 ADR-006） | 隐式按 message 顺序推断 |

**审查问题**（reviewer 必须回答）：

> "本 PR 是否引入了**第二份**配置、**第二条**流式通道、**第二组**颜色常量、**第二份**协议定义？"

任一答案为"是" → reject 或要求显式豁免。

**理由**：
- 双源真相导致同步逻辑散布，bug 难以定位
- 单源真相强制变更 funnel 进入审查通道

---

## P4 — 测试先行（Snapshot Before Refactor）

**规则**：每个被重构的模块必须**先有测试，再改实现**。

**适用范围**：
- UI panel 重构：先写 `egui_kittest` layout snapshot
- 业务逻辑重构：先补单元测试覆盖既有行为
- 协议层重构：先写端到端流测试（producer → consumer）

**示例工作流**：

```text
1. 写测试 → 通过当前实现   (commit 1: "test: add layout snapshot for sidebar_card")
2. 重构实现 → 测试仍通过    (commit 2: "refactor: extract sidebar_card widget")
3. 删除旧代码 → 测试仍通过  (commit 3: "remove: inline sidebar_card implementation")
```

如果测试通过当前实现而**通不过重构后**，则视为视觉/语义回归，强制要求修复或扩展测试。

**理由**：
- UI 重构最常见的失败是"功能正确但视觉漂移"
- 没有 baseline 的重构 = 凭感觉重构 = bug 工厂
- 测试是 reviewer 唯一可信的"功能等价"凭证

**例外**：
- 纯文档变更
- 删除 dead code（必须 grep 证明零引用）
- bug 修复（但必须新增 regression test）

---

## P5 — 编译可分（Atomic & Bisectable Commits）

**规则**：每个 commit 必须**单独可编译、可运行**。禁止 "WIP" / "broken intermediate" / "fix later" 中间态。

**示例**：跨多文件重构时

```text
Commit 1: "feat: add new ViewCommand interpreter (not yet called)"
  └─ 新代码加入，不调用 → 编译通过

Commit 2: "refactor: switch settings panel to ViewCommand interpreter"
  └─ 切换调用方 → 编译通过

Commit 3: "remove: legacy settings_edit direct render path"
  └─ 删除旧代码 → 编译通过
```

每一步独立 `cargo build --workspace` 通过 → `git bisect` 永远可用。

**禁止**：

```text
❌ Commit 1: "WIP: refactor protocol layer (BROKEN)"
❌ Commit 2: "WIP continuation"
❌ Commit 3: "fix WIP, finally compiles"
```

**理由**：
- CI 红 ≠ 阻塞合并；CI 红 = 历史回溯失效
- 6 个月后排查 regression 时，无法 bisect 的历史等于无历史
- 强制 atomic 也强制了**思考拆分**

**例外**：无。这是底线。

---

## P6 — Theme Token 强制（No Free Numbers）

**规则**：在 `crates/clarity-egui/src/{panels,components,widgets}/**` 下，
任何 `> 8.0` 的浮点字面量必须满足以下任一条件：

1. 路由到 `theme.space_* / text_* / radius_*` token
2. 加 `// LAYOUT-EXEMPT: <理由>` 注释，且 reviewer 在审查中挑战该豁免

**示例**：

```rust
// ❌ 反例
ui.add_space(12.0);
let rect_height = 56.0;
let line_y = btn.rect.min.y + 10.0;

// ✅ 正例
ui.add_space(theme.space_12);
let rect_height = theme.row_md;  // 如不存在，先在 theme.rs 添加
ui.add_space(theme.space_8);     // line_y 通过 vertical layout 自动处理

// ✅ 例外正例
let radar_radius = 80.0;  // LAYOUT-EXEMPT: chart canvas, decoration only
```

**8.0 阈值依据**：egui 的 `space_4` (4.0) 和 `space_8` (8.0) 是基线网格，
小于等于 8.0 的字面量在装饰性场景（如 1.0 的 stroke width）频繁出现，
强制 token 化反而增加噪声。`> 8.0` 是设计 token 的真正战场。

**CI 实现**（待落地）：

```bash
# scripts/check-layout-tokens.sh
rg --type rust -n '\b\d+\.\d+\b' \
   crates/clarity-egui/src/{panels,components,widgets}/ \
   | grep -E '\b(?!8\.0|4\.0|2\.0|1\.0|0\.5|0\.0)\d+\.\d+\b' \
   | grep -v 'LAYOUT-EXEMPT' \
   && exit 1 || exit 0
```

**理由**：
- 是 token 系统能否复活的唯一可信防线
- 没有强制，token 会再次退化为"装饰性的"

---

## P7 — 协议层不前瞻（Protocol Stays Lean）

**规则**：`clarity-wire` / `clarity-contract` 的变更必须同时满足：

1. **至少一个生产消费者**：新增类型必须有 `cargo build` 可见的 use site
2. **至少一个端到端测试**：覆盖 producer → channel → consumer 完整路径
3. **同 PR 完成 producer 和 consumer**：禁止"先发后收"模式

**示例**：

```rust
// ❌ 反例
//   在 clarity-wire 加入 ApprovalRequest 变体
//   但 producer (core::approval) 和 consumer (egui::handlers) 都未实现
pub enum WireMessage {
    ...
    ApprovalRequest { id: String, prompt: String },  // ← 谁发？谁收？
}

// ✅ 正例
//   PR 同时包含：
//   1. clarity-wire/src/lib.rs: 加 WireMessage::ApprovalRequest
//   2. clarity-core/src/approval/runtime.rs: 发出该消息
//   3. clarity-egui/src/handlers/chat.rs: 接收并触发 modal
//   4. clarity-wire/src/tests.rs: 端到端 send→recv 测试
```

**审查问题**：

> "新增的协议类型，本 PR 是否同时实现了 producer 和 consumer？"
> "本 PR 修改的协议变更，是否有端到端测试覆盖？"

**理由**：
- 协议是合约，未消费的合约是空头支票
- "为了未来扩展"是过度设计的最常见伪装
- 强制端到端 = 强制思考"这个变更真的有需求吗"

---

## 8. 争议判定模板

碰到"要不要重构 X / 要不要新增 Y"的边界情况时，按以下序列回答：

```
Q1: X / Y 是否阻塞下一个 sprint 的功能交付？
  否 → 不做
  是 → Q2

Q2: 修复 X / 实现 Y 的预估工作量 < 绕过 / 推迟的工作量 × 3？
  否 → 不做（绕过）
  是 → Q3

Q3: X / Y 是否在七条原则内有明确指引？
  否 → 先补原则（更新本文档），再决定
  是 → Q4

Q4: X / Y 是否可分为 ≤ 5 个 atomic commit（每个独立编译）？
  否 → 拆分，禁止 monolithic refactor
  是 → 开工
```

任意 Q 得"否"必须**书面记录决策**（PR 描述或 ADR）。

---

## 9. PR 审查 Checklist

每个 PR 的 reviewer 必须逐项检查：

- [ ] **P1**：没有引入双向桥接 / 临时适配层
- [ ] **P2**：净增代码已伴随等量删除，或属于明确的"新功能 / 测试"
- [ ] **P3**：未引入第二份配置、第二条通道、第二组常量、第二份协议
- [ ] **P4**：被改动的模块有先行测试覆盖
- [ ] **P5**：每个 commit 独立编译通过（`git rebase -i` 检查）
- [ ] **P6**：`panels/components/widgets` 中无未豁免的 `> 8.0` 硬编码
- [ ] **P7**：协议层变更同 PR 含 producer + consumer + e2e 测试

**任一项失败 → Request Changes，不接受"下个 PR 修"。**

---

## 10. 与现有文档的关系

本文档是**根纪律**，以下文档作为各领域细化：

| 文档 | 作用 | 关系 |
|------|------|------|
| `docs/CODE-CHANGE-PRINCIPLES.md` (本文) | 跨 crate 通用原则 | 根 |
| `crates/clarity-egui/EGUI_LAYOUT.md` | egui UI 5 条铁律 | 落实 P6 |
| `crates/clarity-egui/ARCHITECTURE.md` | egui 冷热路径规范 | 落实 P3 (Markdown 单源) |
| `docs/adr/ADR-*.md` | 单点架构决议 | 落实 P1 (重大变更需 ADR) |
| `AGENTS.md` §Sprint 冻结声明 | 阶段性范围限制 | 临时叠加，不替代本原则 |

**冲突解决**：本文档 > 其他领域文档。如其他文档与本原则冲突，以本原则为准并同步更新该文档。

---

## 11. 修订记录

| 日期 | 变更 | 提议者 |
|------|------|--------|
| 2026-05-11 | 1.0 落地，含 P1~P7 + 争议模板 + checklist | 主会话 |
