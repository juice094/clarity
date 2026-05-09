# Clarity 长期发展计划书 2026

> 版本: v1.0
> 日期: 2026-05-09
> 范围: clarity-core / clarity-subagents / clarity-memory / clarity-llm / clarity-egui / clarity-tools / 新装 Skills 协同
> 基线: 10-crate workspace, 830 tests pass, SubagentOrchestrator trait 注入完成

---

## 一、项目目的（Why）

### 1.1 存在性命题

Clarity 不是又一个 Claude Code 复刻。它的核心目的是：

> **构建一个 Rust-native、全本地优先、可拆分为独立服务的 AI Agent 运行时，使"一人公司 + AI 军团"的架构在单机上可验证、可审计、可复现。**

这意味着三个不可妥协的约束：

1. **本地优先（Local-First）**：LLM 可以是本地 GGUF，数据不离开本机，Hard Veto 禁止云强制
2. **可提取性（Extractability）**：任何模块必须在半天内提取为独立 crate 并写出 50 字 README
3. **可审计性（Auditability）**：Agent 的每一次思考、工具调用、上下文压缩都有结构化日志，可回放

### 1.2 解决什么真实问题

从知识库「AI生产力幻觉」章节提取的宏观洞察：

| 痛点 | 现状 | Clarity 的解法 |
|------|------|---------------|
| AI 脑炸：协同消耗 > 独自编码 | 工程师同时使用 3-5 个 AI 工具 | **统一运行时**：TUI/eGUI/Gateway/Headless 共享同一个 Agent 内核 |
| 局部优化全局拥堵 | 写得快，但审查/测试/需求确认卡住 | **Plan Mode + Agent Team**：复杂任务先出方案再执行，子 Agent 并行 + CR 审查 |
| 经验溢价清零 | 微软 70 法则 + 监控软件 | **Jumpy Predictor + Memory Compiler**：经验沉淀为可复用的预测模型和结构化记忆 |
| 管理期望通胀 | 效率提升 → KPI 翻倍 | **Token 计量 + Budget 配额**：将 Agent 工作量量化，防止无限生成 |

### 1.3 与竞品的差异化定位

```
                    云端强制
                         │
    Cursor ──────────────┼───────────── Claude Code
    (闭源)               │               (闭源)
                         │
    OpenClaw ────────────┼───────────── KimiClaw
    (开源但 Electron)    │               (云端)
                         │
    ─────────────────────┼──────────────────────
                         │
    GenericAgent ◄───────┼───────► Clarity
    (3300行极简)         │        (10-crate 工业级)
    上下文密度最大化      │        可提取 + 可审计 + 本地优先
                         │
                    本地优先
```

---

## 二、愿景目标（What）

### 2.1 终极状态（12 个月）

> **Clarity Runtime 成为一个可被第三方项目以 `cargo add clarity-core` 引入的 Agent 基础设施，同时 clarity-egui 作为"AI 员工工作台"支持 16+ 个并行 Agent 的监控与编排。**

具体画像：

- 开发者运行 `cargo install clarity-headless`，5 分钟后得到一个支持 Plan Mode 的本地 Agent CLI
- 团队部署 `clarity-gateway` 作为内部 AI 服务网关，32 并发限制 + SQLite WAL 持久化
- 项目经理打开 `clarity-egui`，在甘特图面板看到 5 个子 Agent 的实时进度，关键路径自动标红
- 所有 Agent 的上下文压缩、记忆编译、工具调用都有结构化事件流，可导出为审计日志

### 2.2 可衡量目标（OKR 风格）

| 维度 | 当前基线 | 6 个月目标 | 12 个月目标 |
|------|---------|-----------|------------|
| **代码健康** | 10 crates, 3.3MB, 830 tests | 12 crates, core < 800KB | 15 crates, core < 600KB |
| **提取能力** | llm/tools 已提取，subagents 待提取 | subagents/memory 提取完成 | egui/gateway 可选提取 |
| **技能生态** | 6 built-in skills | 15 built-in + 10 user skills | 30+ skills，Skill Graph 2.0 |
| **前端完整度** | egui 662KB，20+ 面板 | 全部功能覆盖 TUI  parity | 超越 TUI，独有编排面板 |
| **记忆系统** | 4 级编译 + BM25 + embedding | 外部记忆自动归档 | 跨会话知识图谱 |
| **测试覆盖** | 830 lib tests | 1000+ tests + e2e | 1200+ tests + fuzz |
| **文档** | AGENTS.md + README | 每个 crate 有 API 文档 | 完整 book + 教程 |

---

## 三、阶段路线图（When + How）

### Phase 1：基础设施收尾（0-2 个月）

**目的**：把正在进行中的架构债务清零，使代码基进入"可安全扩展"状态。

| 任务 | 具体行动 | 产出 | 与 Skills 协同 |
|------|---------|------|---------------|
| **clarity-subagents 提取** | 迁移 `subagents/` → 独立 crate；调整所有 import | `crates/clarity-subagents/` 编译通过 | `gitlab-cli-guide`：用 MR 管理迁移代码审查 |
| **interior-mutability** | `SubagentManager` `&mut self` → `RwLock` 内部可变性 | trait `&self` 无需变更 | — |
| **compaction tier2 桥接** | `CompactionService` 调用 `MemoryCompiler` 四级管道 | 超阈值对话自动编译归档 | `flashcard-studio`：编译输出可生成复习卡片 |
| **测试补全** | subagents 提取后补充集成测试 | 测试数 ≥ 900 | `dev-guide-writer`：生成测试编写教程 |
| **前端技能接入** | egui 集成新装 data-viz 技能 | 甘特图/数据可视化面板可用 | `gantt-chart-builder`, `data-viz-gen` |

**退出标准**：`cargo test --workspace` ≥ 900 pass，`cargo check --workspace` 0 warning。

### Phase 2：智能编排层（2-4 个月）

**目的**：从"单 Agent 执行"升级到"多 Agent 协作编排"。

| 任务 | 具体行动 | 产出 | 与 Skills 协同 |
|------|---------|------|---------------|
| **dynamic-model-routing** | `clarity-llm` 新增 `ModelRouter` trait | 任务类型 → 自动选择 fast/deep/local 模型 | `corr-insight`：分析模型选择策略的准确率相关性 |
| **subagent-cr-reviewer** | `AgentTeam` 新增 `reviewer` 角色 | 子 Agent 结果自动 CR 审查 | `gitlab-cli-guide`：CR 结果可提交为 GitLab MR 评论 |
| **skill-graph-v2** | Skill 拆三层：原子/业务/编排 | `skills/` 目录重构，支持 DSL | `code-to-chart`：生成 Skill Graph 架构图 |
| **plan-mode-enhanced** | Plan 支持条件分支、循环、异常处理 | `PlanExecutionController` 支持复杂控制流 | `gantt-chart-builder`：Plan 步骤 → 甘特图自动渲染 |
| **background-reliable** | TaskManager 支持优先级抢占、超时熔断 | Cron + Worker + 告警闭环 | — |

**退出标准**：一个 5 步骤 Plan 可在 egui 中可视化进度，关键路径自动识别。

### Phase 3：知识工程 + 外部集成（4-6 个月）

**目的**：让 Clarity 从"对话工具"进化为"知识工作者"。

| 任务 | 具体行动 | 产出 | 与 Skills 协同 |
|------|---------|------|---------------|
| **knowledge-graph** | 新建 `clarity-graph` crate；代码符号 → 节点，调用关系 → 边 | 仓库知识图谱可视化 | `code-to-chart`：图谱导出为 Mermaid/SVG |
| **memory-externalization** | 超阈值会话自动归档到 `.clarity/archive/` | 跨会话检索可用 | `database-inspector`：归档数据用 SQLite 存储，可浏览 |
| **information-diet** | 新建 `clarity-focus` crate；Token 预算分配器 | Agent 自动忽略低优先级消息 | — |
| **drawio-skill** | 内置 Mermaid/D2/DrawIO 渲染 skill | 架构图/流程图/ER 图一键生成 | `code-to-chart`：代码 → 架构图 pipeline |
| **email-integration** | Agent 可发送周报/告警邮件 | 自动化报告输出 | `html-email-builder`, `html-mail-builder` |
| **database-skill** | Agent 可探索本地 SQLite/PostgreSQL | 数据驱动的 Agent 决策 | `database-inspector` |

**退出标准**：输入一个 GitHub 仓库 URL，Clarity 能在 10 分钟内输出：架构图 + 关键文件分析 + 改进建议。

### Phase 4：生态化 + 商业化准备（6-12 个月）

**目的**：使 Clarity 可被外部开发者复用，探索可持续模式。

| 任务 | 具体行动 | 产出 |
|------|---------|------|
| **agent-economy** | LaborMarket + Token 计量 → 预算配额 | 子 Agent 间可分配预算 |
| **plugin-system** | WASM 插件沙箱 | 第三方可安全扩展工具 |
| **cloud-connector**（可选） | 本地优先前提下，可选连接远程模型 | 不触碰 Hard Veto 的混合架构 |
| **documentation-book** | mdBook 完整文档 | `cargo install clarity-headless` 有完整 Quick Start |
| **community-template** | 开源 Skill 模板仓库 | 社区可贡献 skills |

---

## 四、Skill 协同矩阵（新装 10 Skills × 开发阶段）

```
Skill                    Phase 1    Phase 2    Phase 3    Phase 4
─────────────────────────────────────────────────────────────────
code-to-chart            ████       ████       ████       ████
  └─ 架构图可视化         迁移审查    SkillGraph  知识图谱    文档输出

gantt-chart-builder      ░░░░       ████       ████       ░░░░
  └─ 项目进度可视化        —          Plan绑定    任务编排    —

data-viz-gen             ████       ░░░░       ████       ░░░░
  └─ 数据仪表盘           egui集成   —          报告输出    —

database-inspector       ░░░░       ░░░░       ████       ░░░░
  └─ 数据库探索           —          —          归档浏览    —

corr-insight             ░░░░       ████       ░░░░       ░░░░
  └─ 相关性分析           —          模型路由   —           —

dev-guide-writer         ████       ░░░░       ░░░░       ████
  └─ 教程生成             测试文档   —          —           社区文档

flashcard-studio         ████       ░░░░       ░░░░       ░░░░
  └─ 记忆卡片             编译输出   —          —           —

gitlab-cli-guide         ████       ████       ░░░░       ░░░░
  └─ GitLab集成           MR审查     CR提交     —           —

html-email-builder       ░░░░       ░░░░       ████       ████
  └─ 邮件模板             —          —          报告邮件    自动化

html-mail-builder        ░░░░       ░░░░       ████       ████
  └─ 邮件模板(事务性)      —          —          通知邮件    自动化
```

---

## 五、风险与缓解

| 风险 | 概率 | 影响 | 缓解措施 |
|------|------|------|---------|
| **clarity-subagents 提取引入循环依赖** | 中 | 高 | 提取前用 `devkit_dependency_graph` 验证；trait 已注入，理论上无循环 |
| **egui 前端与 core 耦合过重无法并行开发** | 高 | 中 | egui 只通过 `clarity-wire` EventBus 与 core 通信，禁止直接调用 core 内部类型 |
| **新装 Skills 的 Python 脚本引入供应链风险** | 低 | 中 | 已执行 security-audit；后续用 `uv run --isolated` 执行脚本；禁止 `pip install` |
| **动态模型路由准确率不足** | 中 | 中 | 先内置规则引擎（启发式），再逐步引入 predictor 数据驱动优化 |
| **知识图谱构建成本过高** | 中 | 低 | 基于已有 `clarity-memory` BM25 + embedding 索引，不做全量 AST 解析 |

---

## 六、度量仪表盘（每月 Review）

```bash
# 代码健康
cargo test --workspace --lib 2>&1 | grep "test result:"
devkit_code_metrics --repo_id=clarity

# 耦合检查
devkit_dependency_graph --repo_id=clarity --direction=outgoing
cargo check -p clarity-core && echo "core standalone OK"

# 技能生态
ls ~/.config/agents/skills/ | wc -l
ls clarity/skills/ | wc -l

# 测试覆盖
cargo tarpaulin --workspace --out Stdout 2>/dev/null | grep "Coverage"
```

---

## 七、附录：与 AGENTS.md 的契约对齐

本计划书严格遵守以下 Hard Veto：

- [x] **本地 LLM 优先**：`clarity-llm` 支持 LocalGguf，永远是第一选择
- [x] **Rust 核心模块不可外包**：所有核心 crate（core/subagents/memory/llm）由 juice094 工作区本地开发
- [x] **禁止项目广度 > 5 核心工具**：核心运行时限定为 5 个（core, subagents, memory, llm, wire），其余为可选层
- [x] **禁止永久删除**：所有重构迁移保留在 `archive/` 或 `target/_archive/`
- [x] **禁止 Docker/RAG(Qdrant)/GUI(Electron)**：egui 是纯 Rust + glow，无 Electron

---

*本计划书每月由人类（juice094）Review 一次，AI 不得自行调整里程碑。*
