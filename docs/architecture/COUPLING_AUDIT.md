---
title: 分发与解耦健康度排查报告
category: Architecture
date: 2026-05-16
tags: [architecture]
---

# 分发与解耦健康度排查报告

> 生成时间：2026-05-01  
> 方法：提取演习（不真发布，只检查能否拆）  
> 标准：§9 架构健康纪律（AGENTS.md）

---

## 一、依赖拓扑

```
clarity-memory ◄──────┐
clarity-wire ◄─────┐  │
                   │  │
clarity-core ──────┘  │
    │                 │
    ▼                 │
clarity-gateway ──────┤
clarity-egui ─────────┤
clarity-tui ──────────┤
clarity-claw ─────────┤
clarity-headless ─────┘
```

**循环依赖检查**：✅ 无循环。memory 不依赖 core，wire 不依赖任何 clarity crate。

---

## 二、逐 crate 健康度评分

| Crate | 独立编译 | 内部依赖 | pub 比例 | 职责清晰度 | 总评 | 结论 |
|-------|---------|---------|---------|-----------|------|------|
| **clarity-wire** | ✅ | 0 | 1.0 | 高 | **A** | 现在就能发 |
| **clarity-memory** | ✅ | 0 | 1.0 | 中 | **B+** | 改描述后发 |
| **clarity-core** | ❌ (需 memory+wire) | 2 | 1.0 | 极低 | **D** | 需拆分，God Object |
| clarity-gateway | ❌ | 3 | 1.0 | 低 | C | 不发 |
| clarity-egui | ❌ | 2 | 0.99 | 低 | C | 不发 |
| clarity-tui | ❌ | 3 | 1.0 | 低 | C | 不发 |
| clarity-claw | ❌ | 3 | 1.0 | 低 | C | 不发 |
| clarity-headless | ❌ | 3 | 1.0 | 低 | C | 不发 |

---

## 三、关键问题详解

### 3.1 clarity-core — God Object（最严重）

**症状**：
- lib.rs 导出 **25 个顶级模块**，全部 `pub mod`
- MCP（~1600 行）、subagents、approval、background、skills、tools 全部混装
- 代码量估计 >8000 行（agent/ 目录下就有 12 个文件）

**具体耦合点**：

| 模块 | 行数 | 能否独立提取 | 阻塞点 |
|------|------|-------------|--------|
| `mcp/` | ~1600 | ⚠️ 需解耦 | 依赖 `crate::error::{AgentError, ToolError}` 和 `crate::tools::{Tool, ToolContext, ToolResult}` |
| `subagents/` | ~? | ⚠️ 需解耦 | 依赖 core agent 状态机 |
| `approval/` | ~? | ⚠️ 需解耦 | 依赖 core types 和 agent 状态 |
| `background/` | ~? | ⚠️ 需解耦 | 依赖 core agent 和任务系统 |
| `skills/` | ~? | ⚠️ 需解耦 | 依赖 core registry 和 tool 系统 |
| `agent/` | ~? | ❌ 核心保留 | 这是 core 的本质，不应提取 |
| `registry/` | ~? | ⚠️ 可能提取 | 工具注册表，有独立价值 |
| `llm/` | ~? | ⚠️ 可能提取 | Provider 适配层，有独立价值 |

**修复路径**：
```
clarity-core（精简）
├── agent/       ← 保留：ReAct/Plan 状态机
├── types/       ← 保留：核心类型
├── registry/    ← 可能提取为 clarity-toolkit
├── llm/         ← 可能提取为 clarity-llm
└── ...

提取为独立 crate：
├── clarity-mcp        ← 从 core/src/mcp/ 搬出
├── clarity-subagents  ← 从 core/src/subagents/ 搬出
├── clarity-approval   ← 从 core/src/approval/ 搬出
├── clarity-skills     ← 从 core/src/skills/ 搬出
```

### 3.2 clarity-memory — 描述强绑定

**问题**：
- description: `"Advanced memory storage system for Clarity"` → 强绑定叙事
- 需要回答：与 pgvector / milvus-rs / sqlite-vec 的差异是什么？

**差异化点（待验证）**：
- BM25 + Vector 混合搜索（非纯向量）
- Session 级别的记忆编译（today → week → longterm）
- 增量式 BM25 索引

**修复**：改 description 为 `"Hybrid memory storage: BM25 + vector search + SQLite persistence. Local-first."`

### 3.3 clarity-wire — 描述强绑定

**问题**：
- description: `"Wire communication channel for Soul-UI communication in Clarity"` → 强绑定

**实际价值**：
- SPMC broadcast channel（tokio broadcast 之上）
- 消息合并（ContentPart 自动合并）
- 声明式 UI 协议（ViewCommand / UserAction）
- 通用性：不限于 Soul-UI，任何后端→前端流式通信都可用

**修复**：改 description 为 `"SPMC broadcast event bus with message merging and declarative UI protocol."`

### 3.4 应用层 crates — 全部强耦合

| Crate | 依赖数 | 为什么不适合发布 |
|-------|--------|----------------|
| clarity-egui | core + wire | GUI 是产品差异化，不是库 |
| clarity-tui | core + memory + wire | TUI 是产品形态，不是库 |
| clarity-gateway | core + memory + wire | Web server 是产品入口，不是库 |
| clarity-claw | core + memory + wire | 托盘监控是产品特性，不是库 |
| clarity-headless | core + memory + wire | CLI 是产品入口，不是库 |

---

## 四、pub 比例分析

| Crate | pub | pub(crate) | 比例 | 评估 |
|-------|-----|-----------|------|------|
| clarity-core | 427 | 2 | 1.0 | ⚠️ 全部公开 = 没有内部实现细节保护 |
| clarity-memory | 68 | 0 | 1.0 | ⚠️ 同上 |
| clarity-wire | 9 | 0 | 1.0 | ✅ 接口少，全公开合理 |
| clarity-gateway | 109 | 0 | 1.0 | ⚠️ 应用层不应过度公开 |
| clarity-egui | 83 | 1 | 0.99 | ⚠️ 同上 |

**关键发现**：clarity-core 427 个 pub 接口中，lib.rs 只 re-export 了约 30 个。剩下 397 个 pub 是子模块的公开项。这意味着子模块之间可以互相直接调用，没有层间防火墙。

---

## 五、修复优先级

| 优先级 | 动作 | 工作量 | 预期收益 |
|--------|------|--------|---------|
| **P0** | 改 clarity-wire description + 发布 | 5 分钟 | 钉坐标，零成本获客 |
| **P0** | 改 clarity-memory description + 发布 | 10 分钟 | 同上 |
| **P1** | 从 core 提取 clarity-mcp | 半天 | Rust 生态最缺的基础设施 |
| **P1** | core 内部拆分：approval/background/skills 模块化 | 1-2 天 | 降低 God Object 复杂度 |
| **P2** | core 提取 registry + llm 为独立模块 | 1 周 | 可选，看外部需求 |
| **P2** | 降低 pub 比例，增加 pub(crate) 边界 | 持续 | 模块防火墙 |

---

## 六、验证命令速查

```bash
# 独立编译检查
cargo check -p clarity-wire
cargo check -p clarity-memory
cargo check -p clarity-core       # 会失败，依赖 memory+wire

# 依赖树
cargo tree -p clarity-core | grep clarity-
cargo tree -p clarity-memory | grep clarity-

# 循环依赖检查（手动）
# memory → core ? 否
cargo tree -p clarity-memory | Select-String "clarity-core"  # 无输出

# pub 统计
grep -r "^pub " crates/clarity-core/src/ | wc -l
grep -r "^pub(crate) " crates/clarity-core/src/ | wc -l
```

---

*本报告基于提取演习方法论（§9 架构健康纪律）。下次排查时复用上述命令即可。*
