---
title: Clarity ↔ Obsidian 双向同步评估
category: Note
date: 2026-07-07
tags: [research, sync, obsidian, knowledge-field, basidiocarp]
---

# Clarity ↔ Obsidian 双向同步评估

> Type: Architecture decision support for Knowledge Field vault integration
> Trigger: 评估 Knowledge Field 下一代能力时，需明确 Clarity 与 Obsidian vault 之间是双向同步、单向导出还是单向索引
> Status: 推荐策略已给出；若必须双向同步，提供最小可行架构草图
> Related: [`2026-07-07-basidiocarp-reference.md`](./2026-07-07-basidiocarp-reference.md)

---

## 1. 问题定义

Clarity 的 `clarity-knowledge` 负责管理本地知识索引、知识图谱与检索。Obsidian 是人类可读、可编辑的 Markdown vault。两者之间的关系有三种候选模式：

1. **Clarity → Obsidian 单向导出**：Clarity 是真相源（source of truth），Obsidian 只是人类可读视图。
2. **Obsidian → Clarity 单向索引**：Obsidian 是真相源，Clarity 只读取、索引、检索，不写回。
3. **双向同步**：两个方向都可写，需要冲突解决与循环更新防护。

本笔记对比三种模式，并基于 Basidiocarp 的「Hyphae 是真相源，Obsidian 是投影」设计给出建议。

---

## 2. 三种模式对比

### 2.1 Clarity → Obsidian 单向导出

| 维度 | 评估 |
|------|------|
| **数据一致性** | 高。Clarity 内部状态是唯一的真相源，Obsidian 文件是只读投影。 |
| **冲突风险** | 无。Obsidian 的修改不被写回，因此不存在冲突。 |
| **实现复杂度** | 低。只需一个导出器（exporter），将 memory / memoir / session 等类型渲染为 Markdown + frontmatter。 |
| **用户体验** | 中。用户可以在 Obsidian 中阅读、搜索，但编辑不会被保留；需要明确告知这是“只读镜像”。 |
| **适用场景** | Clarity 作为 Agent 记忆与知识中枢，Obsidian 作为人类审阅、分享、长期归档的视图。 |

**参考**：Basidiocarp hyphae 的 `hyphae export obsidian` 即采用此模式，将 frontmatter 中的 `clarity_id` 和 `type` 作为投影标记。

---

### 2.2 Obsidian → Clarity 单向索引

| 维度 | 评估 |
|------|------|
| **数据一致性** | 高。Obsidian 文件是真相源，Clarity 只消费不修改。 |
| **冲突风险** | 低。Clarity 不写入 Obsidian，不存在写冲突。 |
| **实现复杂度** | 低–中。需要文件系统 watcher（`notify`）、Markdown 解析、frontmatter 提取、知识图谱构建。当前 `clarity-knowledge` 已基本具备这些能力。 |
| **用户体验** | 高。用户在 Obsidian 中自由编辑，Clarity 提供增强检索与 Agent 记忆。 |
| **适用场景** | 用户已有的 Obsidian vault 是主工作区，Clarity 作为智能层附加其上。 |

**风险**：Clarity 内部生成的记忆（如 session 总结、提取的 fact）无法回到 Obsidian，可能导致“Clarity 记得、Obsidian 不记得”的分裂。

---

### 2.3 双向同步

| 维度 | 评估 |
|------|------|
| **数据一致性** | 低–中。需要复杂的版本向量或时间戳机制，且难以保证最终一致性。 |
| **冲突风险** | 高。任何两边同时修改的场景都可能产生冲突。 |
| **实现复杂度** | 高。需要 watcher、事务队列、冲突解决、循环更新防护、重命名传播。 |
| **用户体验** | 表面高，实际脆弱。用户可能在两边编辑，冲突解决若不够智能会导致数据丢失。 |
| **适用场景** | 仅当用户明确要求“把 Obsidian 当作 Clarity 的可写前端”时才考虑。 |

---

## 3. 双向同步的困难点

### 3.1 重命名后链接更新

Obsidian 使用 wikilinks（`[[note]]`、`[[note#heading]]`、`[[note|alias]]`）。当用户在 Obsidian 或 Clarity 中重命名文件时：

- 所有指向该文件的 wikilinks 必须同步更新。
- 当前 `clarity-knowledge/src/index.rs` 的 `rename_file` 已实现单向重命名传播（Clarity 内部更新链接并写回文件），但双向场景下需要区分“用户主动重命名”与“同步导致的重命名”，否则会产生循环更新。
- 需要稳定的 `clarity_id`（如 ULID）作为文件的身份标识，文件名只作为显示路径。

### 3.2 冲突解决策略

| 策略 | 优点 | 缺点 |
|------|------|------|
| **Last-Write-Wins（LWW）** | 实现简单 | 容易静默丢失用户编辑；mtime 不可靠（git checkout、解压、同步工具都会改 mtime）。 |
| **人工合并** | 最稳妥 | 成本高，打断用户工作流。 |
| **基于 frontmatter 版本** | 可区分结构化数据与用户正文；对 Clarity 生成的字段用 DB 优先，对用户 Markdown 正文用文件优先。 | 需要维护版本号/时间戳，且 frontmatter 可能被用户或 Obsidian 插件修改。 |

### 3.3 循环更新风险

典型循环：

1. Clarity 导出更新到 Obsidian 文件 A。
2. Obsidian watcher 检测到文件 A 变化，触发 Clarity 重新索引。
3. Clarity 认为文件 A 被用户修改，写回数据库。
4. 数据库变化又触发导出，再次修改文件 A。

**缓解**：
- 写回 Obsidian 时设置 frontmatter 标记 `clarity_sync: true` 或 `clarity_export: <version>`。
- watcher 忽略由 Clarity 自己写入的文件事件（通过进程级写标记或临时文件锁）。
- 所有写操作经过事件队列，幂等去重。

### 3.4 文件系统 watcher 与数据库事务一致性

- `notify` 事件是异步、可能乱序、可能丢失的。
- SQLite 事务是原子的，但文件系统操作不是。
- 若 Clarity 在写入 DB 后崩溃，Obsidian 文件可能未更新；反之亦然。
- 需要“ staging + commit”语义：先把变更写入 staging 表/临时文件，确认两边都成功后提交版本号。

---

## 4. 推荐策略

### 4.1 默认：Clarity → Obsidian 单向导出

与 Basidiocarp「Hyphae 是真相源，Obsidian 是投影」的设计一致：

> **Clarity（Hyphae）是真相源；Obsidian vault 是人类可读、可搜索、可分享的投影。**

**实施要点**：
- 导出内容类型：memory、memoir、session summary、extracted facts。
- frontmatter 固定包含 `clarity_id`（ULID）、`clarity_type`（`memory`/`memoir`/`session`）、`clarity_version`（单调递增整数）、`clarity_export: true`。
- 文件夹布局参考 Basidiocarp：按类型分目录，支持 redaction 规则（敏感记忆不导出）。
- Obsidian 侧显示 `请勿在此编辑，修改会被覆盖` 的免责声明（可在模板中生成注释）。

**优点**：
- 与现有 `clarity-knowledge` 的只读索引能力不冲突。
- 无冲突、无循环更新、实现简单。
- 保留 Clarity 对记忆结构和版本的完全控制。

### 4.2 可选：Obsidian → Clarity 单向索引

如果用户的主要工作区是 Obsidian，且 Clarity 只是附加智能层：

- Clarity 只读取用户指定的 Obsidian vault 目录。
- 对 Obsidian 文件不做任何写回操作。
- Clarity 内部生成的记忆仍保留在 Clarity 内部，可通过单独界面查询。

**缺点**：Obsidian 中的编辑无法直接影响 Clarity 的“记忆”层，只能影响“知识”层。需要明确区分这两个概念。

### 4.3 不推荐：双向同步

除非有强烈的业务需求，否则不建议默认提供双向同步。双向同步的复杂度、冲突风险和维护成本远高于其价值。

---

## 5. 如果必须做双向同步：最小可行架构草图

若产品决策强制要求双向同步，建议采用以下最小可行架构（MVP），并限制范围：

### 5.1 核心原则

1. **单方向单次写入**：同一时刻只由一个系统持有“写锁”。
2. **版本时钟优于文件时间**：使用 frontmatter 中的 `clarity_version` 和 DB 中的 `sync_version` 作为权威版本，而不是 mtime。
3. **身份标识与文件名解耦**：每个文件/记忆实体有稳定的 `clarity_id`；文件名变化通过 `clarity_id` 跟踪。
4. **事件队列 + 幂等去重**：所有文件事件和 DB 事件进入队列，按 `clarity_id` 去重，避免循环更新。

### 5.2 架构分层

```
┌─────────────────────────────────────────────────────────────┐
│  Obsidian vault (Markdown + frontmatter)                    │
└──────────────┬──────────────────────────────────────────────┘
               │ 文件事件 (notify)
               ▼
┌─────────────────────────────────────────────────────────────┐
│  Sync Adapter                                               │
│  - 过滤 Clarity 自己产生的事件                               │
│  - 解析 frontmatter → 变更操作 (Create/Update/Rename/Delete) │
└──────────────┬──────────────────────────────────────────────┘
               │ 写入 Staging
               ▼
┌─────────────────────────────────────────────────────────────┐
│  Sync Staging (SQLite 表)                                   │
│  - pending_changes(clarity_id, source, target_version, op)   │
│  - 按 clarity_id 合并/去重                                   │
└──────────────┬──────────────────────────────────────────────┘
               │ 冲突检测
               ▼
┌─────────────────────────────────────────────────────────────┐
│  Conflict Resolver                                          │
│  - DB 结构化字段（tags/facts/links）→ DB 优先                │
│  - Markdown 正文 → 文件优先                                  │
│  - 同字段两边都改 → 人工合并或 LWW（需配置）                 │
└──────────────┬──────────────────────────────────────────────┘
               │ 原子提交
               ▼
┌─────────────────────────────────────────────────────────────┐
│  Clarity DB (clarity-memory / clarity-knowledge)            │
└──────────────┬──────────────────────────────────────────────┘
               │ 导出事件（受控）
               ▼
┌─────────────────────────────────────────────────────────────┐
│  Exporter                                                   │
│  - 仅导出 DB 变化，标记 `clarity_export: true`               │
│  - 写文件时抑制 watcher 回调                                 │
└─────────────────────────────────────────────────────────────┘
```

### 5.3 冲突规则（MVP）

| 数据归属 | 来源 | 冲突规则 |
|----------|------|----------|
| 结构化元数据 | `clarity-memory` / `clarity-knowledge` | DB 优先。例如 tags、extracted facts、links、importance score。 |
| Markdown 正文 | Obsidian / 用户 | 文件优先。Clarity 不覆盖用户正文，除非显式触发“重新生成”。 |
| 标题与文件名 | 用户 | 以 Obsidian 为准；Clarity 通过 `clarity_id` 跟踪，并在 DB 中保存 `display_path`。 |
| 两边同时修改同一字段 | — | 生成冲突记录，进入 `conflicts` 表，由用户或 Agent 在 UI 中手动解决。默认不自动 LWW。 |

### 5.4 循环更新防护

1. **写标记**：Exporter 写文件前设置进程级标志 `is_exporting = true`，watcher 检测到变化后检查该标志，若为真则忽略。
2. **版本跳过**：watcher 解析 frontmatter，若 `clarity_export: true` 或 `clarity_version == db_sync_version`，则不触发 DB 写入。
3. **事件幂等**：Staging 表以 `(clarity_id, target_version)` 为唯一键，重复事件直接丢弃。

### 5.5 重命名传播

1. 用户在 Obsidian 中重命名 `old.md` → `new.md`。
2. watcher 捕获到 `Rename(old, new)` 事件。
3. Sync Adapter 通过 `clarity_id` 找到 DB 记录，更新 `display_path`。
4. Exporter 扫描所有 Markdown 中的 wikilinks，替换 `[[old]]` → `[[new]]`，并写回（带 `clarity_export` 标记，抑制循环）。
5. 当前 `InMemoryIndex::rename_file` 中的链接更新逻辑可复用，但需加写标记防护。

### 5.6 需要修改/新增的文件

```
crates/clarity-knowledge/src/sync.rs              # 新增：SyncAdapter / Staging / ConflictResolver
crates/clarity-knowledge/src/exporter.rs          # 新增或复用：Clarity → Obsidian 导出器
crates/clarity-knowledge/src/index.rs             # 修改：rename_file 加写标记、事件入队
crates/clarity-knowledge/src/retrieval.rs         # 可能无需修改
crates/clarity-memory/src/store.rs                # 新增 sync_version / clarity_id 字段
crates/clarity-egui/src/...                       # 新增冲突解决 UI（如需要）
```

---

## 6. 决策建议

| 场景 | 推荐模式 | 理由 |
|------|----------|------|
| Clarity 是 Agent 记忆中枢，Obsidian 只是人类视图 | **单向导出** | 简单、无冲突、与 Basidiocarp 一致。 |
| 用户已有 Obsidian vault，Clarity 只做增强检索 | **单向索引** | 不破坏用户工作流，实现简单。 |
| 产品明确要求 Obsidian 可编辑并同步回 Clarity | **双向同步（MVP）** | 必须接受高复杂度、限定冲突规则、提供冲突 UI。 |

**短期行动**：实现单向导出 PoC，验证 frontmatter 布局、导出模板、redaction 规则。
**中期行动**：在单向导出稳定后，评估是否有真实用户强烈需求双向同步；若没有，持续拒绝。

---

*本笔记用于 Knowledge Field vault 集成策略的跨会话继承。*
