# Clarity 长程路线图（基于依赖关系与工程理论）

> 生成时间：2026-05-01
> 方法：依赖优先 + 风险驱动 + 增量交付
> 参考资料：cc-haha v0.1.8 / openclaw v2026.4.29 / 三方对比: `2026-05-01-cchaha-openclaw-comparison-and-roadmap.md`

---

## 一、现状依赖图

### 当前 crate 依赖

```
clarity-contract (37 lines, 2 types) ─────────────────────────┐
                                                               │
                                                               ▼
clarity-memory (6.9K) ───────┐                       clarity-core (35.8K) ────┐
                              │                              │                  │
                              ▼                              ▼                  ▼
clarity-wire (1K) ─────┐  clarity-gateway (6.8K)    clarity-egui (6.4K)   clarity-claw (0.5K)
                       │         │                                              │
                       ▼         ▼                                              ▼
                    [claw ← wire ← gateway ← core]
```

### 问题

| 问题 | 严重度 | 原因 |
|------|--------|------|
| **God crate** | 🔴 | `clarity-core` 35.8K 行，任何变更都需重新编译全部下游 |
| **Provider 枚举硬编码** | 🔴 | 5 个 Provider 在 `core::llm::api` 中 enum，每加一个改源码 |
| **Contract 形同虚设** | 🟡 | 只有 2 个类型，核心类型（Message, Error, Trait）仍在 core 中 |
| **Gateway 依赖过深** | 🟡 | 直接引用 `core::agent::AgentController` 等内部类型 |
| **Claw 依赖全量 core** | 🟢 | 仅为 TaskSummary 和 resolve_gateway_url 就链接整个 core |

---

## 二、依赖约束分析

按"被依赖数"排序，定义优先解耦顺序：

| 模块 | 被依赖数 | 复杂度 | 风险 | 推荐策略 |
|------|---------|--------|------|---------|
| `clarity-contract` | 1 (core) | 低 | 低 | **优先膨胀** — 安全，纯类型迁移 |
| `core::llm::provider` | 3+ (gateway, egui, tui) | 高 | 高 | **Strangler** — 新旧共存，逐步替换 |
| `core::types` | 5+ (所有下游) | 中 | 中 | **批量迁移** — 先移类型，后移逻辑 |
| `core::error` | 4+ | 低 | 低 | **快速迁移** — 纯 Error 类型 |
| `core::mcp` | 1 (gateway) | 中 | 中 | **Feature gate** — 已有部分隔离 |
| `core::agent` | 2 (gateway, egui) | 高 | 高 | **Trait 化** — 定义接口，保留实现 |

### 关键依赖链（Critical Path）

```
contract-extraction → type-migration → error-migration
                                            │
                    provider-trait ←────────┘
                         │
               gateway-api-expansion
                    │            │
            egui-ui-work    claw-enhance
                    │
              plugin-sdk
```

可见：**contract-extraction** 是整个路线的根部阻塞点，没有稳定的 contract，
provider trait 没有落脚点，gateway API 没有共享类型，plugin SDK 无从谈起。

---

## 三、路线图结构

采用 **"双轨并行 + 四层递进"** 结构：

```
Track A (架构层)     Track B (功能层)
─────────────────────────────────────────
Layer 4: 生态系统
  Plugin SDK     ←──   Channel Adapters

Layer 3: 展现层
  Provider UI    ←──   Claw Depth + Mobile

Layer 2: 网关层
  Gateway API 扩展 ←──  MCP/Cron/Search API

Layer 1: 核心层
  Provider Schema    │  egui P0 功能补齐
  Contract 提取      │  后台任务 UI
  Error 类型迁移     │  子代理进度面板
                     │  Claw 增强
─────────────────────────────────────────
    时间 →
```

**Track A**（左轨）— 架构解耦，偿还技术债务，为平台化铺路。初期无用户可见产出，但解除所有下游阻塞。

**Track B**（右轨）— 功能交付，每个 Iteration 产出用户可见价值。初期不依赖 Track A。

两条轨道前 3 层相互独立，到 Layer 4 汇合。

---

## 四、Phase 序列

### Phase 0 —— 快速价值交付（当前窗口，1-2 天）

**目标**：用现有架构交付最高频的用户需求，零架构风险。

**依赖条件**：Gateway 已有 `/v1/tasks` 和 `/v1/parallel` 端点。

#### Track B-0：egui 功能补齐

| 工作项 | 输入 | 产出 |
|--------|------|------|
| `TaskCreatePanel` | Gateway POST `/v1/tasks` | 弹窗式新建任务（名称+prompt+最大迭代数） |
| `TaskCancelButton` | Gateway DELETE `/v1/tasks/:id` | 运行中任务的取消操作 |
| `SubAgentProgress` | Gateway POST `/v1/parallel` 响应 | 并行子代理实时状态面板 |
| `TaskListPanel` | Gateway GET `/v1/tasks` | 完整任务列表（状态+进度+操作） |

#### Track B-0：Claw 增强

| 工作项 | 输入 | 产出 |
|--------|------|------|
| 快捷输入弹窗 | Tao 原生窗口 + `/v1/chat/completions` | 托盘左键点击弹出输入框 |
| 任务创建(Tray Menu) | Gateway POST `/v1/tasks` | 右键菜单创建任务 |
| 取消任务(Tray Menu) | Gateway DELETE `/v1/tasks/:id` | 运行任务可取消 |

**验收**：用户可在 egui 中创建/查看/取消后台任务，可在 Claw 中快捷输入。

---

### Phase 1 —— 核心解耦 + API 扩展（1-2 周）

**目标**：解除 God crate 阻塞，为平台化奠基；同时扩展 Gateway API 面。

#### Track A-1：Contract 提取

```
迁移路径：
  core::types::{Message, MessageRole, StreamDelta}  →  contract
  core::error::{AgentError, ToolError}              →  contract
  core::llm::api::{LlmProvider, ModelInfo}          →  contract (trait)
  core::tools                       →  contract (trait)
  core::registry                    →  contract (trait)
```

**策略**：增量迁移，每个类型迁移独立 PR，core 保留 `pub use clarity_contract::*` 重导出。
下游代码零修改。重导出在全部迁移完成后一次性移除。

**风险控制**：每个类型迁移后运行完整测试套件 + clippy。

| 步骤 | 类型 | 工作量 | 风险 |
|------|------|--------|------|
| 1.1 | `Message`, `MessageRole` | ~100 行迁移 | 低 |
| 1.2 | `AgentError`, `ToolError` | ~200 行迁移 | 低 |
| 1.3 | `StreamDelta` | ~50 行迁移 | 低 |
| 1.4 | `ToolCall` 已迁移 ✅ | — | — |
| 1.5 | `LlmProvider` trait（当前 enum → trait） | ~500 行设计+实现 | **高** |

#### Track A-1：Provider Trait 化（Strangler 模式）

```
当前：
  pub enum LlmProvider { Kimi, OpenAI, Anthropic, Google, Local }
  
目标：
  pub trait LlmProvider: Send + Sync {
      fn name(&self) -> &str;
      fn chat(&self, request: ChatRequest) -> Result<ChatResponse>;
      fn chat_stream(&self, request: ChatRequest) -> Result<Box<dyn Stream>>;
  }
  
Strangler 桥接：
  // 旧代码仍用 enum
  // 新代码通过 Registry 获取 Box<dyn LlmProvider>
  // 逐步迁移调用点
  // 最终移除 enum
```

**设计产物**：`docs/plans/provider-schema-design.md`

#### Track B-1：Gateway API 扩展

| 端点 | 方法 | 说明 | 依赖 |
|------|------|------|------|
| `/api/mcp/servers` | GET/POST | MCP 服务器 CRUD | core::mcp（已有） |
| `/api/mcp/servers/:id` | GET/DELETE | 单个 MCP 服务器操作 | core::mcp（已有） |
| `/api/cron/tasks` | GET/POST | Cron 任务管理 | core::background（已有） |
| `/api/cron/tasks/:id` | GET/DELETE | 单个 Cron 任务操作 | core::background（已有） |
| `/api/search` | POST | 跨会话全文检索 | core::memory（已有） |
| `/api/export/session/:id` | GET | 会话导出 | core::server |

**时机**：这些 API 完全基于现有 core 接口，不依赖 Track A 的解耦。
可以与 Contract 提取并行推进。

#### Track B-1：egui Provider UI

**依赖条件**：Track A-1 Provider trait 设计完成，不等待实现完成。
可先按新 Schema 设计 UI 原型，硬编码 mock 数据验证交互。

---

### Phase 2 —— Claw 深度 + Provider 实现（2-3 周）

**目标**：Claw 达到 openclaw daemon 的 80% 能力；Provider 系统迁移完成。

#### Track A-2：Provider 迁移

| 步骤 | 内容 | 工作量 |
|------|------|--------|
| 2.1 | Provider Schema 定义（TOML） | ½ 天 |
| 2.2 | 内置 Provider 适配器（5 个） | 1 天 |
| 2.3 | Registry + 动态注册 | 1 天 |
| 2.4 | LLM Factory 重构 | 1 天 |
| 2.5 | Config 系统对接 | ½ 天 |
| 2.6 | 旧 enum 清理 | ½ 天 |
| 2.7 | 测试 + 边界用例 | 1 天 |

#### Track B-2：Claw 深度

| 工作项 | 参考 | 说明 |
|--------|------|------|
| 自动启动注册 | openclaw daemon | Windows 注册表/任务计划 |
| 状态持久化 | openclaw daemon | SQLite 存储最近任务、通知历史 |
| Wire 深度集成 | openclaw gateway-broadcast | 从网关被动接收事件，消除轮询 |
| 健康探针 | openclaw heartbeat | 自检 + 故障恢复 |
| 多托盘菜单 | openclaw daemon | 任务分组、快捷操作 |

#### Track B-2：egui 日志 + 终端面板

| 面板 | 说明 |
|------|------|
| LogPanel | Gateway 日志实时流 |
| TerminalPanel | 内置 Shell（参考 cc-haha `TerminalSettings.tsx`） |

#### Track B-2x：Jumpy Workflow Orchestration（实验性能力落地）

> **来源**：`crates/clarity-core/src/agent/jumpy/`（MVP 已完成，需接入系统避免孤岛）  
> **论文**：arXiv:2602.19634 — 将 Skill 视为可组合的"短跑专家"，通过离线世界模型实现时间抽象

**目标**：将跳跃世界模型从实验模块转化为生产级编排能力。

| 工作项 | 优先级 | 依赖 | 说明 |
|--------|--------|------|------|
| Session Store → SkillObservation pipeline | P1 | Phase B Session Schema | 从历史会话自动提取 (skill_id, before, after) 观测 |
| LLM-Augmented Predictor | P2 | Local LLM 或 API Provider | 无历史时零样本预测状态转移 |
| Flow 节点：`InvokeSkill` + `PredictCheckpoint` | P2 | — | 现有 Flow 系统零侵入扩展 |
| Subagent 委托绑定 | P2 | Phase C AgentPool | `SkillComposer` 回调接入 `AgentPool::spawn()` |
| headless `jumpy` CLI 子命令 | P2 | — | JSON 输出供外部工具（Kimi CLI 等）消费 |
| A/B 验证（≥20 条轨迹） | P3 | 以上全部 | 对比 Jumpy 规划 vs 传统 Plan 的成功率与 token 消耗 |

**架构约束**：
- 不修改现有 `Agent::run()` / `FlowRunner` / `Plan` 的公共接口
- 通过回调解耦（`execute_skill_fn`），可独立测试
- 预测器纯离线学习，不依赖环境交互

---

### Phase 3 —— 平台化（3-4 周）

**目标**：Plugin SDK + Channel 适配器，达到 cc-haha 的插件能力。

#### Track A-3：Contract 冻结

**条件**：全部核心类型已迁移到 `clarity-contract`，API 稳定。
**动作**：标记 `clarity-contract` v1.0，移除 `clarity-core` 的重导出。

```
稳定 API 清单：
  contract::Message, contract::MessageRole
  contract::StreamDelta
  contract::AgentError, contract::ToolError
  contract::ToolCall, contract::FunctionCall, contract::ToolResult
  contract::LlmProvider (trait)
  contract::Tool (trait)
  contract::Registry
```

#### Track A/B-3：Plugin SDK

```
设计原则（参考 openclaw plugin-sdk）：
  - Plugin = 实现 contract::Plugin trait 的动态库
  - 每个 Plugin 可注册 Tool + Provider + Skill
  - 沙箱执行（WASM 或独立进程）
  - 版本管理 + 依赖声明

MVP 范围：
  1. Plugin trait 定义（contract 中）
  2. PluginLoader（文件系统扫描 + 动态加载）
  3. 2 个参考实现（一个 Tool plugin，一个 Provider plugin）
```

#### Track A/B-3：Channel Adapter SDK

```
设计原则（参考 cc-haha adapters + openclaw channels）：
  - Channel = 实现 contract::Channel trait
  - 标准化 inbound/outbound 接口
  - 独立于 core 循环，通过 Gateway WebSocket 通信

MVP 范围：
  1. Channel trait 定义（contract 中）
  2. Telegram 适配器（解除 CVE 后）
  3. Slack 适配器增强
```

---

## 五、风险矩阵

| 风险 | 概率 | 影响 | 缓解措施 |
|------|------|------|---------|
| Provider trait 设计过度工程 | 中 | 延误 | MVP 先行：只抽象 chat() 和 chat_stream() |
| Contract 提取破坏现有代码 | 低 | 高 | 保留重导出 + 每步全量测试 |
| egui 渲染性能瓶颈 | 低 | 中 | 先 mock 确认交互，再对接真实数据 |
| Gateway API 不稳定被迫 break change | 中 | 中 | v1 API 承诺向后兼容，新增用 v2 前缀 |
| Claw 轮询 vs 推送切换复杂 | 中 | 低 | 保留轮询降级路径 |

---

## 六、里程碑时间线

```
Week 1-2          Week 3-4          Week 5-6          Week 7-10
─────────────────────────────────────────────────────────────────────
Phase 0 ──┐
egui P0   │
Claw 增强  │
          │
Phase 1   ├─────────────────┐
Contract  │                  │
提取       │                  │
Gateway   │                  │
API 扩展   │                  │
          │                  │
Phase 2   │                  ├─────────────────┐
Provider   │                  │                  │
迁移       │                  │                  │
Claw Depth │                  │                  │
          │                  │                  │
Phase 3   │                  │                  ├─────────────────┐
Plugin SDK│                  │                  │                  │
Channels  │                  │                  │                  │
─────────────────────────────────────────────────────────────────────
         egui 可用性     API 完备度       Claw 成熟度      平台化
         里程碑           里程碑           里程碑           里程碑
```

### 里程碑定义

| 里程碑 | 时间 | 验收标准 |
|--------|------|---------|
| **M0: egui 可用性** | Week 1 | 用户可在 GUI 中创建/查看/取消后台任务；子代理进度可见；系统托盘可快捷输入 |
| **M1: API 完备度** | Week 3 | Gateway 覆盖 MCP/Cron/Search 管理 API；Contract 提取完成 80%；Provider trait 设计冻结 |
| **M2: Claw 成熟度** | Week 6 | Claw 支持自动启动、状态持久化、Wire 事件推送；Provider 系统完全 trait 化，枚举已移除 |
| **M3: 平台化** | Week 10 | Plugin SDK MVP 可用；至少 1 个外部 Plugin 可加载运行；Channel SDK MVP |

---

## 七、与 cc-haha / openclaw 的关键对标

| Clarity 里程碑 | 对标 cc-haha 等价物 | 对标 openclaw 等价物 |
|---------------|--------------------|--------------------|
| M0 | `NewTaskModal.tsx` + `ScheduledTasks.tsx` | `src/tasks/` + `src/daemon/` |
| M1 | `src/server/api/mcp.ts` + `cronService.ts` | `src/gateway/server-methods*.ts` |
| M2 | `src/server/services/providerService.ts` | `src/gateway/*.ts` + `extensions/` |
| M3 | `src/server/services/pluginService.ts` | `src/plugins/` + `packages/plugin-sdk/` |

---

## 八、当前窗口执行计划（Phase 0）

详见 `2026-05-01-cchaha-openclaw-comparison-and-roadmap.md` 第五章。

优先顺序：
1. **Track B-0a**: egui `TaskCreatePanel` + `TaskCancelButton` + `TaskListPanel`
2. **Track B-0b**: egui `SubAgentProgress` 面板
3. **Track B-0c**: Claw 快捷输入弹窗 + 任务创建菜单
4. **Track B-0d**: `docs/plans/provider-schema-design.md`

每条完成后立即合并，传递用户可见价值。
