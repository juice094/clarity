# DeepSeek-TUI → Clarity 技术借鉴分析

> 分析日期：2026-05-05  
> 分析对象：`C:\Users\22414\dev\third_party\DeepSeek-TUI` (v0.8.12)  
> 基线：Clarity `main` @ `b1b72660`

---

## 一、项目定位对比

| 维度 | DeepSeek-TUI | Clarity |
|------|-------------|---------|
| **目标** | 终端原生编码助手，围绕 DeepSeek V4 的 1M token 窗口 | 本地优先 AI 开发运行时，集群协作原语的单机验证 |
| **UI 栈** | ratatui（终端唯一） | eframe/egui（桌面）+ ratatui（终端）+ Axum Gateway |
| **架构风格** | "胖二进制 crate" — 几乎所有业务逻辑在 `tui` crate 中 | 功能分区 — 9 个 crate 分散复杂度 |
| **Rust 版本** | 1.88+ (edition 2024) | 1.85+ (edition 2021) |

**架构健康评估**：DeepSeek-TUI 的集中式架构与 Clarity 的分布式架构形成鲜明对照。Clarity 的 crate 拆分更符合 AGENTS.md §9 的"半天提取"纪律。

---

## 二、高价值借鉴点（按优先级排序）

### 🔴 P0 — 立即可吸收

#### 1. Prompt Cache-Prefix 稳定性规则

**DeepSeek-TUI 做法**：
- 系统提示采用 **静态层 → 易变层** 的严格顺序
- 工作集元数据注入到 **最新的 user message** (`<turn_meta>`) 而非 system prompt
- 有 **字节级一致性测试** (`assert_byte_identical`) 确保相同输入产生完全相同的前缀

**Clarity 现状**：
- `SystemPromptBuilder::build_split()` 已区分 static/dynamic，但未将 volatile 内容下沉到 user message
- 无字节级一致性测试

**借鉴方案**：
- 将 `GitContext`、`ActiveFiles`、`ProjectMetadata` 迁移到最后一条 user message 的 `<context>` 块中
- 为 `build_split()` 添加字节一致性回归测试
- **收益**：prefix cache hit 率提升，降低 KV cache 重建成本
- **工作量**：2-3 天

#### 2. LSP 作为合成用户消息

**DeepSeek-TUI 做法**：
- 编辑文件后，通过自研的轻量级 stdio JSON-RPC 客户端（~400 LOC，无 `tower-lsp`）获取诊断
- 诊断以 `<diagnostics file="...">` 块的形式作为 **synthetic user message** 注入上下文
- 模型自然看到这些诊断，无需协议改动

**Clarity 现状**：
- 无 LSP 集成

**借鉴方案**：
- 新增 `LspManager`（轻量级 stdio 客户端，7 种语言默认配置）
- 在 `ToolResult` 后添加 `LspHook`，获取编辑文件的诊断
- 以 `ContentBlock::ToolResult` 变体或附加 message 注入上下文
- **工作量**：3-4 天

#### 3. 进程级成本旁路通道

**DeepSeek-TUI 做法**：
- `static PENDING: OnceLock<Mutex<f64>>` 作为进程级成本累加器
- 后台调用者（compaction、subagent、RLM）直接 `cost_status::report(model, usage)`
- TUI 渲染循环每帧 `drain()` 并累加到会话成本

**Clarity 现状**：
- `AgentInner.daily_cost_usd` 已存在，但仅主 Agent 更新
- 子代理、compaction 的成本未回传

**借鉴方案**：
- 新增 `clarity-core/src/cost/channel.rs`
- `SubagentRunner`、compaction service、memory extraction 完成后 report
- `Agent::run()` 每轮 turn 前 `drain()` 并累加
- **工作量**：1-2 天

---

### 🟡 P1 — 中期吸收（Sprint 37-38）

#### 4. Side-Git 工作区快照

**DeepSeek-TUI 做法**：
- 在 `~/.deepseek/snapshots/<hash>/.git` 创建 side repo
- 每轮 turn 前后 `git add -A && git write-tree && git commit-tree`
- `/restore N` 通过 `--git-dir` + `--work-tree` 安全 checkout，不触碰用户 `.git`
- 7 天自动清理

**Clarity 现状**：
- 无工作区回滚机制
- 用户依赖 `git revert` 手动恢复

**借鉴方案**：
- 新增 `clarity-core/src/snapshot.rs`
- 在 `Agent::run()` 的 `finalize_sync_turn` 前后自动 snapshot
- egui 添加 `/restore` 命令
- **工作量**：5-7 天

#### 5. 子代理 Mailbox + CancellationToken 传播

**DeepSeek-TUI 做法**：
- `Mailbox` + `MailboxReceiver` 提供结构化事件信封（Started/Progress/ToolCallStarted/Completed/Failed/Cancelled/TokenUsage）
- 序列号单调递增；`mailbox.close()` 级联取消所有后代
- `session_boot_id` 区分跨会话残留 agent

**Clarity 现状**：
- `SubagentProgressEvent` 只有 4 个变体
- 无 CancellationToken 级联

**借鉴方案**：
- 扩展 `SubagentProgressEvent` 为 Mailbox 风格
- 在 `SubagentRunner` 中维护 `CancellationToken` 树
- **工作量**：3-4 天

#### 6. RLM（递归语言模型）模式

**DeepSeek-TUI 做法**：
- `rlm` 工具：将大输入加载到 Python REPL
- 子 LLM 在 REPL 中调用 `llm_query()` / `rlm_query()`
- 大输入**永不进入根 LLM 的上下文窗口**
- 批处理并行度最高 16

**Clarity 现状**：
- 无 RLM 等价物
- 长文本处理依赖 naive 分割或上下文压缩

**借鉴方案**：
- 新增 `RlmTool` — 基于 `rune` 或 `mlua` 的 Rust 内嵌脚本（避免 Python 依赖）
- **工作量**：7-10 天（较大）

---

## 三、Clarity 优于 DeepSeek-TUI 的领域

| 领域 | Clarity 优势 |
|------|-------------|
| **多前端** | egui + TUI + Gateway + Headless + Claw；DeepSeek-TUI 仅 TUI |
| **Memory 系统** | SQLite + BM25 + Vector 混合搜索；DeepSeek-TUI 仅 JSON 文件 |
| **Provider 生态** | 6 家 provider + 本地 GGUF；DeepSeek-TUI 聚焦 DeepSeek |
| **离线能力** | Candle 原生 GGUF，零外部依赖 |
| **架构解耦** | 9 crate 分离，符合提取纪律 |
| **Approval 系统** | 三层 + 持久化审批记录 |
| **Jumpy World Model** | 预测性调度 + HistoricalPredictor |

---

## 四、推荐行动

### Sprint 37（2 周）— 吸收 P0-P1

| 天数 | 任务 | 负责方 |
|------|------|--------|
| Day 1-2 | Prompt volatile-content-last 重构 | 根 Agent |
| Day 2-3 | 字节一致性回归测试 | 子代理 |
| Day 3-5 | 进程级成本旁路通道 | 根 Agent |
| Day 5-7 | LSP 轻量级 stdio 客户端 + hook | 根 Agent |
| Day 8-10 | Side-git 快照 MVP | 子代理 |
| Day 11-14 | Mailbox 扩展 + CancellationToken 级联 | 根 Agent |

### Sprint 38（2 周）— 深化 + 发行

- RLM 简化版评估
- Session 元数据前缀扫描优化
- Skills 多源扫描
- v0.3.1 Release

---

## 五、关键文件映射

| DeepSeek-TUI 文件 | 对应 Clarity 位置 | 借鉴价值 |
|------------------|------------------|---------|
| `crates/tui/src/prompts.rs` + `prompts/base.md` | `clarity-core/src/agent/prompt.rs` | 高 |
| `crates/tui/src/lsp/` | 无 | 高 — 新增模块 |
| `crates/tui/src/pricing.rs` | 无 | 中 — 新增模块 |
| `crates/tui/src/snapshot/` | 无 | 高 — 新增模块 |
| `crates/tui/src/tools/subagent/` | `clarity-core/src/subagents/` | 中 |
| `crates/tui/src/tools/rlm.rs` | 无 | 中 — 新增能力 |
