# Clarity 架构Benchmark报告：与业界成熟项目的对比分析

> 调研日期：2026-04-18
> 调研范围：Aider、Open Interpreter、Continue.dev、Zed AI、Claude Code
> 评估维度：人格系统、工具调用、上下文管理、UI设计、本地推理

---

## 一、执行摘要

Clarity 当前处于 **"功能可用但架构未成熟"** 的阶段。与业界标杆相比，核心差距不在功能缺失，而在**架构深度的系统性不足**：

- **人格系统**：静态模板拼接 vs 业界动态条件组装
- **工具调用**：单路径执行 vs 业界并行调用+分层权限
- **上下文管理**：无压缩策略 vs 业界5级compaction
- **UI设计**：单向对话流 vs 业界双向上下文构建
- **本地推理**：mock骨架 vs 本机已就绪的kalosm/candle栈

**好消息**：本机CUDA环境、kalosm 0.4、Qwen2.5模型权重、agri-paper参考代码全部就绪，接入真实本地推理的门槛极低。

---

## 二、参考项目概览

### 2.1 Aider — 终端编程Agent标杆

| 属性 | 详情 |
|------|------|
| 定位 | 终端AI编程助手，专注代码编辑与Git工作流 |
| Star | 51,000+ |
| 核心模式 | Architect + Editor 双角色协作 |
| 人格配置 | `.aider.conf.yml` + `CONVENTIONS.md`（YAML+Markdown混合） |
| 模型切换 | 运行时 `/model` 命令切换，支持本地模型 |
| Git集成 | 自动commit，/undo回滚 |
| 特色 | 编码规范可配置、多语言polyglot支持 |

**架构亮点**：
- **分层配置**：全局 `.aider.conf.yml` → 项目级 `CONVENTIONS.md` → 运行时 `/model` 切换
- **双代理模式**：Architect（规划）+ Editor（执行）分离，降低单一prompt复杂度
- **Prompt Caching**：利用Anthropic prompt caching减少重复token成本

### 2.2 Open Interpreter — 本地代码执行Agent

| 属性 | 详情 |
|------|------|
| 定位 | 让LLM在本地安全执行代码（Python/JS/Shell） |
| 核心模式 | 自然语言→代码生成→用户确认→本地执行 |
| 安全模型 | 执行前强制用户确认（可配置白名单） |
| 本地推理 | 原生支持Ollama、LM Studio等OpenAI-compatible端点 |
| 配置 | `default.yaml`：模型、context_window、max_tokens |
| 特色 | 将终端变成自然语言界面 |

**架构亮点**：
- **安全沙箱**：代码执行前必须用户确认，危险操作高亮警告
- **本地优先**：`--local` 模式默认小上下文（3000 tokens）适配消费级硬件
- **状态重置**：`interpreter.messages = []` 一键清空对话上下文

### 2.3 Continue.dev — IDE嵌入式AI框架

| 属性 | 详情 |
|------|------|
| 定位 | VS Code/JetBrains插件，模块化AI助手 |
| 核心模式 | Chat / Autocomplete / Edit / Agent 四模态 |
| 上下文组装 | 动态上下文提供者（context providers），尊重.gitignore |
| MCP集成 | 原生支持MCP服务器（memory、sequential-thinking、context7等） |
| Agent模式 | 请求分解→依赖分析→实施计划→序列化文件编辑 |
| 特色 | 增量流式输出、inline Apply/Insert/Copy动作 |

**架构亮点**：
- **Context Provider架构**：代码片段→文件→文件夹→Git diff，层级化上下文注入
- **Agent编排**：高阶指令自动分解为多步骤，每步可独立执行/回滚
- **MCP即插即用**：通过 `.continue/mcpServers/*.yaml` 配置外部服务

### 2.4 Zed AI — 编辑器原生AI

| 属性 | 详情 |
|------|------|
| 定位 | 编辑器内置AI（非插件），CRDT-based实时协作 |
| 核心模式 | Assistant Panel + Inline Assist + Slash Commands |
| Assistant Panel | **可编辑文本缓冲区**（整个对话是一个可编辑文件） |
| Inline Assist | `ctrl+enter` 选中代码→自然语言转换→diff呈现 |
| 模型支持 | Anthropic/OpenAI/Google/Ollama/Zed AI五_provider |
| Prompt Library | 保存/复用自定义系统提示 |
| 特色 | 无隐藏系统提示，用户完全可控上下文 |

**架构亮点**：
- **可编辑对话缓冲区**：用户可直接编辑历史消息、分叉对话、删除噪声上下文
- **上下文即代码**：`/file`、`/diagnostic`、`/terminal` 等命令像代码一样注入上下文
- **Workflow模式**：`/workflow` 命令生成结构化多步骤计划，带步骤解析UI
- **Streaming Diff Protocol**：自定义流式diff协议与CRDT buffer集成，token级实时编辑

### 2.5 Claude Code — 生产级Agent CLI（架构最复杂）

| 属性 | 详情 |
|------|------|
| 定位 | Anthropic官方CLI，通用编程Agent |
| 系统提示 | ~50个工具，条件逻辑动态组装，多层级 |
| 权限模型 | 分层：只读免确认 / 写入需确认 / 危险操作警告 |
| 上下文管理 | **5级compaction策略** + 33K token保留缓冲区 |
| 记忆系统 | `CLAUDE.md`（项目级持久）+ `/memory`（跨会话）+ Skills（可激活） |
| 桥接系统 | 双向通信层连接VS Code/JetBrains扩展 |
| 特色 | `/compact`手动压缩、并行工具调用、自动记忆提取 |

**架构亮点**（来自泄露源码与文档）：
- **Context Engineering Layer**：6个子系统协作填充上下文窗口
  1. 动态系统提示构造（初始化行为指令）
  2. 双记忆系统（短期+长期）
  3. 工具结果优化（压缩冗余输出）
  4. 系统提醒与错误恢复（运行时定向注入）
  5. 自适应上下文压缩（token预算耗尽时）
  6. 会话持久化与自适应记忆
- **Prompt Assembly**：非静态字符串，而是条件逻辑+工具定义+用户内容+技能附件的动态组装
- **Compaction Tiers**：5种压缩策略按侵略性递增应用
- **Skills vs MCP**：Skills=可激活的指令包（专家模式），MCP=外部工具协议，二者互补

---

## 三、五维对比分析表

### 3.1 人格/系统提示管理

| 维度 | Clarity | Aider | Continue | Zed | Claude Code |
|------|---------|-------|----------|-----|-------------|
| **配置方式** | 三层模板文件(Identity/Yuan/Ishiki) | `.aider.conf.yml` + `CONVENTIONS.md` | 插件设置JSON | Prompt Library + `.rules` | `CLAUDE.md` + Skills + 动态组装 |
| **动态性** | 静态拼接，运行时变量替换 | 静态配置，可运行时切换 | 动态上下文组装 | 用户可编辑上下文 | **条件逻辑动态组装** |
| **层级** | 项目级(agent_dir) | 全局→项目→会话 | 会话级 | 项目级`.rules` | 全局→项目→目录→会话→Skills |
| **持久化** | 模板文件 | 配置文件 | IDE设置 | 项目文件 | `CLAUDE.md`跨会话持久 |
| **粒度** | YuanType四种预设 | 可自定义system-prompt | 模型级配置 | 规则文件 | **50+工具条件注入** |
| **用户可控** | 需修改模板文件 | 命令行参数+文件 | GUI设置 | **完全可见可编辑** | `/context`查看token分配 |

**Clarity差距**：
- ❌ 无动态条件组装（所有模型看到相同的system prompt）
- ❌ 无用户可编辑的上下文缓冲区（Zed的杀手特性）
- ❌ 无跨会话持久化（每次重启丢失人格状态）
- ❌ 粒度粗糙（仅4种YuanType，无法按需加载技能）

### 3.2 工具调用架构

| 维度 | Clarity | Aider | Continue | Zed | Claude Code |
|------|---------|-------|----------|-----|-------------|
| **调用方式** | 单路径串行 | 串行 | **Agent模式多步骤** | Inline Assist单步 | **并行工具调用** |
| **权限控制** | read_only布尔值 | 无（代码直接执行） | 依赖MCP服务器 | 无 | **分层权限模型** |
| **确认机制** | 无（Gateway直接执行） | 无 | 无 | 无 | **只读免确认/写入需确认/危险警告** |
| **工具数量** | 19个内置+MCP | 文件操作+Git | MCP即插即用 | 无内置（纯AI） | ~50个内置+Skills+MCP |
| **执行反馈** | 文本渲染在聊天流 | diff展示 | inline Apply/Insert | diff呈现 | **结构化输出+错误恢复** |
| **Tool Schema** | JSON Schema | 无 | MCP标准 | 无 | **动态工具定义注入** |

**Clarity差距**：
- ❌ **调用翻倍bug**：stream-vs-complete路径导致内部调用倍增（已确认）
- ❌ 无并行工具调用（多次独立查询需多轮往返）
- ❌ 无分层权限（Gateway的bash直接执行，无确认）
- ❌ 工具结果无优化（完整输出塞入上下文，不压缩）

### 3.3 上下文/记忆管理

| 维度 | Clarity | Aider | Continue | Zed | Claude Code |
|------|---------|-------|----------|-----|-------------|
| **窗口监控** | 无 | 无 | 无 | 无 | **状态栏实时token百分比** |
| **压缩策略** | 无 | 无 | 无 | 无 | **5级compaction** |
| **记忆持久化** | SQLite(memory.db) | 无 | MCP Memory Server | 无 | **`CLAUDE.md` + `/memory`** |
| **记忆触发** | MemoryTicker(每5轮) | 无 | 手动 | 无 | **自动提取+手动管理** |
| **上下文组装** | SystemPromptBuilder静态拼接 | 无特殊 | Context Provider动态 | 用户手动`/file` | **Context Engineering Layer** |
| **token预算** | 硬编码 | 无 | 无 | 无 | **33K保留缓冲区+自适应阈值** |

**Clarity差距**：
- ❌ 无上下文窗口监控（用户不知道何时会超限）
- ❌ 无压缩策略（长会话必然OOM或截断）
- ❌ MemoryTicker过于简单（固定5轮，非智能触发）
- ❌ 无token预算管理（system prompt占多少、对话占多少不可见）

### 3.4 UI/交互设计

| 维度 | Clarity | Aider | Continue | Zed | Claude Code |
|------|---------|-------|----------|-----|-------------|
| **界面类型** | TUI + Web IDE | TUI | IDE插件 | 原生编辑器 | TUI |
| **代码编辑** | Monaco Editor(嵌入) | 无（外部编辑器） | 内联编辑 | **原生CRDT编辑器** | 无 |
| **上下文可视** | 不可见 | 不可见 | 部分可见 | **完全可见可编辑** | `/context`命令查看 |
| **文件树** | 需点击"加载项目" | 无 | 原生IDE文件树 | **原生文件树常驻** | `/ls`命令 |
| **工具输出** | 污染聊天流 | diff展示 | inline动作卡片 | diff+可折叠 | **结构化输出面板** |
| **对话可控** | 不可编辑历史 | 不可编辑 | 不可编辑 | **可编辑、可分叉** | 有限编辑 |

**Clarity差距**：
- ❌ Web UI空间拥挤（Editor+Chat上下分屏，无独立工具输出区）
- ❌ 文件树不常驻（需每次点击"加载项目"触发glob）
- ❌ 工具执行结果污染对话流（shell输出直接塞进聊天）
- ❌ 无上下文可视化（用户看不到发给模型的完整prompt）

### 3.5 本地推理集成

| 维度 | Clarity | Aider | Continue | Zed | Claude Code |
|------|---------|-------|----------|-----|-------------|
| **本地模型** | mock骨架 | Ollama/LM Studio | Ollama | Ollama | 不支持本地 |
| **推理框架** | 无（规划中kalosm） | 外部端点 | 外部端点 | 外部端点 | 云API only |
| **CUDA支持** | 本机已就绪但未接入 | 外部端点 | 外部端点 | 外部端点 | N/A |
| **模型发现** | 无 | `--model`参数 | 设置配置 | 设置配置 | N/A |
| **量化支持** | N/A | 依赖外部 | 依赖外部 | 依赖外部 | N/A |
| **Feature Flag** | 规划中 | 无 | 无 | 无 | N/A |

**Clarity现状**：
- ✅ 本机RTX 4060 + nvcc 12.6 + CUDA 13.2
- ✅ kalosm 0.4 + candle 已编译
- ✅ Qwen2.5 7B/14B GGUF 模型权重已下载
- ✅ agri-paper/rust_llm_poc 参考代码已验证
- ❌ clarity-core 中仍是mock实现

---

## 四、关键架构缺陷评定

### 4.1 P0（阻塞生产使用）

| 缺陷 | 影响 | 证据 |
|------|------|------|
| **stream-vs-complete调用翻倍** | API成本倍增、响应延迟增加 | `agent/mod.rs` `run_streaming_loop` doubling mechanism |
| **Gateway无权限确认** | bash/write工具直接执行，安全隐患 | handlers.rs直接调用agent.run() |
| **无上下文压缩** | 长会话必然超限或截断 | 无compaction模块 |

### 4.2 P1（严重影响体验）

| 缺陷 | 影响 | 对标 |
|------|------|------|
| **system prompt静态拼接** | 所有场景共用同一prompt，无法按需加载 | Claude Code条件逻辑组装 |
| **MemoryTicker固定5轮** | 触发时机粗糙，可能过早/过晚 | Claude Code自动记忆提取 |
| **Web UI无独立工具输出区** | 对话流被shell输出污染 | Zed diff呈现+可折叠 |
| **文件树不常驻** | 工作空间可知性差 | Zed/KimiClaw常驻文件树 |

### 4.3 P2（优化项）

| 缺陷 | 影响 | 对标 |
|------|------|------|
| **无并行工具调用** | 多文件查询需多轮往返 | Claude Code并行调用 |
| **无prompt library** | 无法保存/复用自定义人格 | Zed Prompt Library |
| **TUI无Architect模式** | 单一代理承担规划+执行 | Aider Architect+Editor分离 |
| **无上下文可视化** | 用户不知道模型看到了什么 | Claude Code `/context` |

---

## 五、分阶段改进规划

### Phase 1：紧急修复（1-2天）

| 任务 | 文件 | 目标 |
|------|------|------|
| 修复stream-vs-complete调用翻倍 | `agent/mod.rs` | 消除重复LLM调用 |
| Gateway添加只读/写入权限分层 | `handlers.rs` + `AgentConfig` | 危险操作需确认 |
| TUI终端恢复逻辑 | 已完成 ✅ | - |
| 删除domain.rs越界代码 | 已完成 ✅ | - |

### Phase 2：核心架构升级（1周）

| 任务 | 参考 | 目标 |
|------|------|------|
| 接入真实Kalosm本地推理 | agri-paper POC | RTX 4060本地运行Qwen2.5 7B |
| 实现上下文压缩（至少1级） | Claude Code compaction | 防止长会话OOM |
| 重构SystemPromptBuilder为动态组装 | Claude Code prompt assembly | 条件加载工具定义 |
| Web UI添加独立工具输出面板 | Zed diff呈现 | 不污染对话流 |

### Phase 3：体验优化（2周）

| 任务 | 参考 | 目标 |
|------|------|------|
| 常驻文件树+自动刷新 | Zed/KimiClaw | 工作空间一目了然 |
| Prompt Library（保存/复用自定义人格） | Zed Prompt Library | 用户可创建自定义Agent |
| 上下文可视化面板 | Claude Code `/context` | 用户可见完整system prompt |
| 并行工具调用 | Claude Code | 减少多查询往返 |
| Memory系统智能化 | Claude Code auto-extract | 自动提取关键记忆 |

### Phase 4：高级特性（长期）

| 任务 | 参考 | 目标 |
|------|------|------|
| Architect+Editor双模式 | Aider | 规划与执行分离 |
| 技能系统（Skills） | Claude Code Skills | 可激活的专家模式 |
| Bridge系统（IDE扩展） | Claude Code Bridge | VS Code/JetBrains插件 |
| 5级compaction完整实现 | Claude Code | 生产级上下文管理 |

---

## 六、优先级矩阵

```
影响高 │ P0:调用翻倍修复   P1:动态prompt组装
       │ P0:权限分层      P1:上下文压缩
       │
       │ P2:并行工具调用  P2:Prompt Library
       │ P2:上下文可视化   P3:Architect模式
       │
影响低 │ P3:Bridge系统    P3:5级compaction
       │ P3:Skills系统
       └───────────────────────────────────
         工作量小          工作量大
```

**推荐启动顺序**：
1. **立即**：修复调用翻倍（1小时）+ 接入Kalosm（1-2天）
2. **本周**：动态prompt组装 + 上下文压缩
3. **下周**：Web UI重构（常驻文件树+工具输出面板）
4. **后续**：Skills系统 + Bridge系统

---

## 七、结论

Clarity的当前架构类似于 **"2024年初级Agent框架"** 的水平，而业界标杆（Claude Code、Zed AI、Continue.dev）已达到 **"2025-2026年生产级Context Engineering"** 水平。

**核心差距公式**：
```
差距 = (动态上下文组装 + 分层权限 + 自适应压缩 + 并行工具调用) - (静态模板拼接 + 无权限控制 + 无压缩 + 串行执行)
```

**最大优势**：本机CUDA+kalosm栈已就绪，一旦接入本地推理，Clarity将成为少数同时支持**云API+本地CUDA推理+TUI+Web IDE**的Rust Agent框架，差异化优势明显。

**建议立即行动**：修复调用翻倍bug → 接入Kalosm本地推理 → 动态prompt组装。这三项完成后，Clarity将从"demo级"跃升为"可用级"。
