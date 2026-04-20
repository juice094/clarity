# Clarity 现代化全面分析报告

> 接管日期: 2026-04-20
> 分析范围: 全 Workspace 代码 + 文档 + 竞品生态
> 方法: 实机审计 + 文档审查 + 开源调研

---

## 一、工程思想确认（Phase 1）

### 1.1 核心哲学（从 README / AGENTS / 代码中提炼）

| 思想 | 定义 | 代码体现 | 健康度 |
|------|------|---------|--------|
| **本地优先 (Local-first)** | 数据主权，最小化外部依赖 | `cargo run` 直接运行；除 MCP 外零 Node.js；本地 SQLite 存储 | ✅ 强 |
| **模型中立 (Model-agnostic)** | 不绑定单一 LLM | `LlmProvider` trait + `ModelRegistry` TOML 配置；支持 6+ 提供商 | ✅ 强 |
| **流式优先 (Stream-first)** | 实时响应，拒绝阻塞 | `Agent::run_streaming()` 优先调用 `llm.stream()`；SSE 全链路 | ✅ 强 |
| **Soul-UI 解耦** | 执行层与表现层分离 | `clarity-wire` 广播通道；TUI/Gateway/Web 独立消费 | ✅ 强 |
| **渐进式增强** | 每阶段独立可验证 | Feature flag 体系（`sqlite`/`embedding`/`mcp`/`local-llm`） | ✅ 强 |
| **三层差异化** | 不同入口不同认知边界 | claw(轻量)/window(只读)/cli(完整) 工具分层 | ⚠️ 设计完成，实现中 |
| **零构建步骤** | 开箱即用 | `cargo run -p clarity-tui` 即可对话 | ✅ 强 |

### 1.2 架构原则评估

```
┌─────────────────────────────────────────────────────────────┐
│  原则                │  符合度  │  例外/风险                     │
├─────────────────────────────────────────────────────────────┤
│  单一职责            │  B+     │  agent/mod.rs (1914行) 过大    │
│  依赖倒置            │  B      │  Gateway 需求反灌 Core Op enum │
│  开闭原则            │  A-     │  Trait 扩展性好，但 MCP 硬编码多 │
│  接口隔离            │  A      │  LlmProvider / StorageBackend  │
│  最小 surprise       │  B      │ 两个 run_streaming 入口易混淆  │
└─────────────────────────────────────────────────────────────┘
```

### 1.3 工程文化诊断

**优势**:
- 文档驱动：AGENTS.md 记录了耦合警告、已知问题、安全注意事项
- 测试文化：420+ 测试，clippy 零警告
- 诚实化机制：REALITY_CHECK 文档主动承认夸大描述
- 安全优先：MCP 命令校验、Approval 模式、敏感文件检测

**劣势**:
- 文档与代码不同步：多个 ARCHITECTURE.md 被归档，因为"含未实现组件"
- 历史包袱：`agent/mod.rs` 1914 行，承担了过多职责
- 前端技能栈缺口：Web UI 是手写原生 JS，无构建工具但也无类型安全

---

## 二、开源项目现状（Phase 2）

### 2.1 竞品生态位定位

```
                    专业度 ↑
                         │
     claude-code    ●    │    ●  OpenHands (替代人类)
     codex          ●    │
     kimi-cli       ●    │
                         │
    ─────────────────────┼────────────────────→ 基础设施化
                         │
     5ire            ●   │    ●  dify (to B)
     **Clarity**     ●   │    ●  ollama (模型运行时)
     openclaw        ●   │
                         │
                    消费级 ↓
```

### 2.2 关键竞品对比

| 项目 | 技术栈 | 核心优势 | Clarity 可借鉴 | 威胁等级 |
|------|--------|---------|---------------|---------|
| **Rig** (Rust) | Rust | 类型安全的 LLM 链；结构化输出 | `OneShotParser` 模式；Prompt 模板系统 | 🟡 中 |
| **Dify** | Python/Vue | 可视化工作流；RAG 完整 pipeline | 知识库管理 UI；Prompt 版本控制 | 🟡 中 |
| **aider** | Python | 代码编辑精度极高；git 集成深 | diff 应用策略；多文件编辑协调 | 🟡 中 |
| **claude-code** | TS/Node | 终端体验极致；上下文感知强 | TUI 交互细节；代码分析能力 | 🔴 高 |
| **CrewAI** | Python | 多角色协作；任务委托清晰 | 角色模板系统；任务链可视化 | 🟢 低 |
| **elizaOS** | TS | 社交代理；插件市场 | 人格模板社区化；记忆系统 | 🟢 低 |
| **5ire** | Rust/Tauri | 桌面 AI 基础设施；知识库 | 桌面端体验；本地 embedding | 🟡 中 |

### 2.3 Rust 生态关键 crate 评估

| crate | 用途 | 成熟度 | 引入建议 |
|-------|------|--------|---------|
| `sqlite-vec` | SQLite 向量扩展 | ⭐⭐⭐ 活跃 | Phase 2: 向量持久化 |
| `rig-core` | LLM 应用框架 | ⭐⭐⭐ 活跃 | 参考设计，非引入 |
| `fastembed-rs` | 本地 embedding | ⭐⭐ 较新 | Phase 3: 可选 feature |
| `candle` | 纯 Rust ML | ⭐⭐⭐ 成熟 | 长期：本地模型推理 |
| `ort` | ONNX Runtime | ⭐⭐ 复杂 | 暂缓：Windows DLL 问题 |
| `schemars` | JSON Schema | ⭐⭐⭐ 成熟 | 立即：工具参数结构化 |

---

## 三、工程化模块解析（Phase 3）

### 3.1 代码规模与质量矩阵

| Crate | 文件数 | 代码行 | 测试数 | 测试率 | 最大文件 | 健康度 |
|-------|--------|--------|--------|--------|---------|--------|
| clarity-core | 45 | ~8,500 | 334 | ~75% | agent/mod.rs (1914) | B+ |
| clarity-memory | 12 | ~2,800 | 57 | ~90% | store.rs (310) | A- |
| clarity-wire | 1 | ~400 | 8 | ~80% | lib.rs (400) | A |
| clarity-gateway | 8 | ~2,200 | 22 | ~60% | handlers.rs (~500) | B |
| clarity-tui | 14 | ~3,500 | 0 | 0% | app.rs (~600) | C |
| clarity-claw | 3 | ~500 | 0 | 0% | main.rs (~300) | C |

### 3.2 模块依赖图

```
                           ┌─────────────┐
                           │  clarity-   │
                           │    claw     │
                           └──────┬──────┘
                                  │
┌─────────────┐  ┌─────────────┐  │  ┌─────────────┐
│ clarity-tui │  │clarity-     │◄─┼──┤clarity-     │
│             │  │  gateway    │  │  │  wire       │
└──────┬──────┘  └──────┬──────┘  │  └──────┬──────┘
       │                │         │         │
       └────────────────┴─────────┴─────────┘
                          │
                    ┌─────┴─────┐
                    │ clarity-  │
                    │   core    │
                    └─────┬─────┘
                          │
                    ┌─────┴─────┐
                    │clarity-   │
                    │ memory    │
                    └───────────┘
```

**关键发现**:
- `clarity-core` 是单点瓶颈：所有上层都依赖它
- `clarity-wire` 是良好解耦的通信层
- `clarity-memory` 独立性好，但接口未充分利用

### 3.3 紧耦合热点（代码级）

| 热点 | 位置 | 影响 | 重构建议 |
|------|------|------|---------|
| **Agent 巨类** | `agent/mod.rs:1914` | 测试难写；变更影响面广 | 拆分为 `AgentLoop`, `AgentState`, `AgentConfig` |
| **Op enum 反灌** | `agent/ops.rs` | Gateway 需求污染 Core | 提取 `GatewayOp` 到 gateway crate |
| **LLM monolith** | `llm/mod.rs:1217` | SSE 解析与请求构建混合 | 提取 `SseParser`, `RequestBuilder` |
| **AppState 膨胀** | `gateway/handlers.rs` | 冗余字段；职责不清 | 使用 `agent.registry()` 替代冗余引用 |
| **Tool 重复逻辑** | `tools/*.rs` | 文件读写模式重复 | 提取 `FileTool` base trait |

### 3.4 技术债务清单（量化）

```
TODO/FIXME/XXX 统计:
  clarity-core:     ~23 个
  clarity-memory:   ~8 个
  clarity-gateway:  ~12 个
  clarity-tui:      ~15 个
  
未使用导入 (dead_code 风险):
  通过 cargo check 发现 ~6 个 warning（当前 clippy 零警告，但 rustc 可能有）
  
硬编码魔法值:
  - token 阈值: 8192（compaction）
  - 端口: 18790/18800（gateway）
  - chunk 大小: 无统一配置
```

---

## 四、现代化规划方案（Phase 4）

### 4.1 战略定位校准

**当前定位**: "分布式 AI 认知容器" — 同一个智能体在三层中以不同形态存在
**目标定位**: "个人 AI 的标准运行时" — 人机交互的底层支撑系统

**差距**: 从"容器"到"运行时"需要：
1. 更完整的工具能力（当前仅 kimi-cli 的 40%）
2. 更强大的本地认知能力（RAG、向量搜索）
3. 更优雅的扩展机制（Plugin/Skill > MCP 裸协议）

### 4.2 现代化路线图

```
Wave 0: 架构固本（2 周）        ← 串行：所有后续工作依赖
├── 0A: Agent 拆分（mod.rs → 3-4 个模块）
├── 0B: Op enum 解耦（GatewayOp 外迁）
├── 0C: LLM 层重构（SseParser 提取）
└── 0D: 统一配置系统（TOML 替代 env 变量）

Wave 1: P0 生存缺口（2 周）      ← 并行：4 个子任务无依赖
├── 1A: Web 三连击（SearchWeb + FetchURL + Browse）
├── 1B: Task 系统暴露为工具（List/Output/Stop）
├── 1C: ReadMediaFile（图片 OCR / 音频转录）
└── 1D: TUI 基础测试（截图测试或 mock 测试）

Wave 2: P1 体验升级（2 周）      ← 部分并行
├── 2A: Think 工具（依赖：无）
├── 2B: PlanMode（依赖：Think 完成）
├── 2C: AskUser 扩展（依赖：ApprovalRuntime 改造）
└── 2D: Notify 系统（依赖：claw 层通知能力）

Wave 3: RAG 向量知识库（3 周）   ← 串行三阶段
├── 3A: BM25 + Hybrid Search（零依赖）
├── 3B: 增量索引 + Chunking（依赖：3A）
├── 3C: sqlite-vec 持久化（依赖：3B）
└── 3D: 外部 Embedding API（依赖：3C）

Wave 4: 扩展机制（2 周）         ← 可并行
├── 4A: Skill 系统（Markdown + TOML 技能包）
├── 4B: Plugin 包装层（MCP 之上）
└── 4C: Sandbox 权限分级（文件白名单）

Wave 5: 性能与 polish（1 周）
├── 5A: 性能基准测试（RAG 查询延迟、内存占用）
├── 5B: Gateway Session 持久化
└── 5C: 跨平台 CI 矩阵
```

### 4.3 依赖关系图（DAG）

```
                    [Wave 0: 架构固本]
                           │
           ┌───────────────┼───────────────┐
           │               │               │
           ▼               ▼               ▼
    [Wave 1: P0 缺口]  [Wave 2: P1 体验]  [Wave 3: RAG]
           │               │               │
           └───────────────┴───────────────┘
                           │
                           ▼
                    [Wave 4: 扩展]
                           │
                           ▼
                    [Wave 5: Polish]
```

**关键路径**: Wave 0 → Wave 1/Wave 3 并行 → Wave 4 → Wave 5
**总工期估算**: 10-12 周（2.5-3 个月）

---

## 五、并串行推进策略（Phase 5）

### 5.1 第一批：立即启动（本周）

**串行任务链 A — 架构固本**:
1. `agent/mod.rs` 拆分（提取 `AgentLoop`, `AgentState`, `AgentConfig`）
2. `Op` enum 解耦（`GatewayOp` 迁移到 gateway crate）
3. `SseParser` 提取（LLM 层模块化）

**并行任务 B — RAG 起步**:
- BM25 实现（基于现有 TF-IDF 扩展）
- Hybrid Search 设计（FTS5 + BM25）

**并行任务 C — Web 工具**:
- `SearchWebTool` 实现（reqwest + 搜索引擎 API）
- `FetchURLTool` 实现（reqwest + HTML 解析）

### 5.2 每批验收标准

| 批次 | 验收标准 | 自动验证 |
|------|---------|---------|
| Wave 0 | `cargo test --workspace` 通过；clippy 零警告；无功能回归 | ✅ CI |
| Wave 1 | 每个新工具有单元测试；TUI 能调用 Web 搜索 | ✅ 测试 + 手工 |
| Wave 2 | PlanMode 端到端可用；AskUser 能接收文本回复 | ✅ 手工 |
| Wave 3 | RAG 查询延迟 < 100ms（1000 facts）；精度 > 80% | ✅ 基准测试 |
| Wave 4 | Skill 包可加载/卸载；Plugin 有示例 | ✅ 测试 |
| Wave 5 | 全平台编译通过；性能报告生成 | ✅ CI |

### 5.3 风险与对冲

| 风险 | 概率 | 影响 | 对冲策略 |
|------|------|------|---------|
| sqlite-vec Windows 编译失败 | 中 | Wave 3 延期 | 准备 fallback：纯 Rust HNSW |
| Agent 拆分引入回归 | 高 | Wave 0 阻塞 | 拆分前建立完整集成测试基线 |
| Web 搜索 API 不稳定 | 低 | Wave 1 受阻 | 支持多搜索引擎（DuckDuckGo/SearXNG） |
| TUI 测试困难 | 高 | 质量难保证 | 引入 `ratatui` 测试框架或截图对比 |

---

## 六、总结与决策点

### 6.1 当前健康度评分

```
架构设计:        ████████░░  80/100  (良好，有扩展性)
代码质量:        ███████░░░  70/100  (测试好，但巨类和技术债务)
功能完整度:      █████░░░░░  50/100  (仅 kimi-cli 的 40%)
文档质量:        ███████░░░  65/100  (多但不同步，archive 频繁)
生态竞争力:      █████░░░░░  55/100  (Rust 原生是优势，功能有缺口)
─────────────────────────────────────
综合评分:        ██████░░░░  64/100
```

### 6.2 关键决策点

1. **Agent 拆分策略**：保守（提取 2-3 个模块）还是激进（完整领域驱动拆分）？
2. **Web 搜索实现**：调用外部 API（Serper/SearXNG）还是本地爬虫（scraper）？
3. **RAG 路线**：Phase 1 纯 Rust BM25 → Phase 2 sqlite-vec → Phase 3 外部 Embedding？
4. **TUI 测试**：投入资源建立自动化测试，还是保持手工验证？

### 6.3 下一步行动

等待用户确认路线后，立即启动 **Wave 0A（Agent 拆分）** + **Wave 1A（Web 工具）** + **Wave 3A（BM25）** 三条线并行。

---

*本报告为 living document，每完成一个 Wave 更新一次。*
