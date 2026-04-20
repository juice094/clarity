# Skill / MCP / Tool / API 协议关系与 Clarity 概念重构方案

> 分析日期：2026-04-20
> 依据：Clarity 源码审计 + Anthropic 官方 Skill 白皮书 + MCP 协议规范 + 开源生态调研
> 目标：解决 Clarity 中 "Skill" 与 "Tool" 概念重叠的架构债务，建立与行业共识对齐的四层模型

---

## 一、当前问题诊断

### 1.1 代码层面的概念混乱

Clarity `clarity-core` 中同时存在两套**完全平行且互不兼容**的执行系统：

| 维度 | `Skill` 系统 | `Tool` 系统 |
|------|-------------|------------|
| **Trait** | `Skill`（输入 `&str`，输出 `String`） | `Tool`（输入 `Value`，输出 `Value`） |
| **Registry** | `SkillRegistry`（非线程安全，无 `Clone`） | `ToolRegistry`（`Arc<RwLock<...>>`，线程安全） |
| **LLM 可调用** | ❌ 无 `parameters()`，无法生成 JSON Schema | ✅ 有 `parameters()`，支持 function calling |
| **状态管理** | 内存 `Mutex`（`ThinkSkill` 推理链 / `TodoSkill` 列表） | 无状态或文件持久化 |
| **生产使用** | **零引用** — `SkillRegistry` 仅出现在自身测试和 doc comments 中 | Agent 核心依赖 |
| **错误类型** | `SkillError`（独立错误层次） | `ToolError` / `AgentError` |

**关键发现**：`Skill` 系统是一个**未被接入 Agent 主循环的孤儿模块**。`Agent` 完全不感知 `Skill` 的存在，所有 LLM 交互都通过 `ToolRegistry` 完成。

### 1.2 术语混用

`agent/mod.rs` 中：

```rust
/// Get skill definitions from the tool registry
fn get_skill_definitions(&self) -> Vec<String> {
    match self.registry.get_tool_schemas() {  // ← 从 ToolRegistry 获取
        // ... 渲染为 "## Available Tools"
    }
}
```

函数名叫 `get_skill_definitions`，数据源是 `ToolRegistry`，渲染标题是 `Available Tools`。这说明项目作者在演进过程中将 "Skill" 和 "Tool" 当同义词使用，导致架构边界彻底模糊。

### 1.3 功能重复与数据孤岛

| 概念 | Skill 实现 | Tool 实现 | 问题 |
|------|-----------|----------|------|
| 思考 | `ThinkSkill`（内存推理链） | `ThinkTool`（no-op logger） | Skill 有状态但不可持久化；Tool 无状态但可被 LLM 调用 |
| 待办 | `TodoSkill`（内存列表） | `TodoTool`（磁盘 `~/.clarity/todos.json`） | 两者数据不互通，用户无所适从 |

---

## 二、行业共识：四层协议模型

基于 Anthropic 官方 Skill 白皮书、MCP 协议规范、Semantic Kernel 架构以及 `osai` 等开源项目的实践，行业已形成如下分层共识：

```
┌─────────────────────────────────────────────────────────────────────┐
│  第四层 · 应用层（Application）                                       │
│  Plugin — 完整的应用定制封装                                          │
│  例：claw 层（常驻生活助手）、window 层（查询入口）、cli 层（工程入口）    │
│  特征：系统提示词覆盖 + 模型选择 + 可用能力白名单 + UI 定制               │
├─────────────────────────────────────────────────────────────────────┤
│  第三层 · 编排层（Orchestration）                                     │
│  Skill — 多步工作流的知识封装 / 提示工程                                │
│  例："代码审查 Skill"、"部署发布 Skill"、"故障排查 Skill"                │
│  特征：Markdown + YAML 配置、渐进式披露、错误处理路径、工具编排序列        │
├─────────────────────────────────────────────────────────────────────┤
│  第二层 · 能力层（Capability）                                        │
│  Tool — 单次可调用的原子能力                                          │
│  例：FileRead、Bash、WebSearch、MCP 桥接工具                           │
│  特征：JSON Schema 参数、无状态（或外部持久化）、可被 LLM function calling │
├─────────────────────────────────────────────────────────────────────┤
│  第一层 · 协议层（Protocol）                                          │
│  MCP / HTTP / stdio / gRPC — 标准化通信接口                            │
│  例：MCP JSON-RPC 2.0、REST API、本地进程管道                          │
│  特征：传输无关、语言无关、关注点分离                                   │
└─────────────────────────────────────────────────────────────────────┘
```

### 2.1 各层关系的核心原则

**原则 1：Skill ≠ Tool，Skill = Tool 的编排者**

> "MCP provides access (专业厨房)；Skill provides expertise (食谱)。"  
> — Anthropic 官方 Skill 白皮书

- **Tool** 回答 "我能做什么"（原子能力）。
- **Skill** 回答 "如何用好这些能力完成复杂任务"（工作流知识）。

**原则 2：MCP 是协议，不是能力**

MCP（Model Context Protocol）位于**协议层**，它标准化了 Host ↔ Server 之间的通信方式：
- `tools/list` / `tools/call` — 能力发现与调用
- `resources/list` / `resources/read` — 上下文数据获取
- `prompts/list` / `prompts/get` — 提示模板分发

MCP **不定义**工具的具体语义，只定义**如何连接**。语义由 Tool 层定义，编排由 Skill 层定义。

**原则 3：Plugin 是面向用户的封装，Skill 是面向 LLM 的封装**

- **Plugin**（如 `osai` 中的 `/plugin run web-researcher`）是**应用层**概念，包含完整的环境配置、系统提示词、模型选择。
- **Skill** 是**编排层**概念，可以被多个 Plugin 复用。同一个 "部署发布 Skill" 可以被 claw 层（轻量通知）和 cli 层（完整操作）同时引用。

**原则 4：渐进式披露（Progressive Disclosure）**

Skill 对 LLM 隐藏实现细节：
- LLM 看到的是 "使用 `deploy_skill`"
- 实际执行的是 "先 `git_status` → 再 `shell_build` → 再 `shell_deploy` → 错误时 `shell_rollback`"

这种分层让 LLM 不需要在每次对话中重新学习复杂流程，同时让开发者可以在不改动 LLM 提示词的情况下优化执行策略。

---

## 三、Clarity 概念重构方案

### 3.1 当前状态 → 目标状态映射

| 当前概念 | 当前定位（错误） | 目标定位 | 处理方式 |
|---------|----------------|---------|---------|
| `Skill` trait + `SkillRegistry` | 与 `Tool` 平行的执行系统 | **删除** — 概念和实现均错误 | 移除整个 `skill/` 目录（`mod.rs`、`think.rs`、`todo.rs`） |
| `ThinkSkill` / `TodoSkill` | 内存状态、命令行风格 | **删除** — 功能已被 Tool 覆盖 | `ThinkTool` 已存在；`TodoTool` 已持久化 |
| `ThinkTool` / `TodoTool` | 能力层原子工具 | **保留** — 正确定位 | 作为 ToolRegistry 中的标准工具 |
| MCP 桥接（`McpToolWrapper`） | 能力层原子工具 | **保留** — 正确定位 | 继续通过 `ToolRegistry` 注册为普通 Tool |
| **新 Skill 系统** | 不存在 | **新建** — 编排层工作流模板 | Markdown + YAML frontmatter，参考 Anthropic SKILL.md |

### 3.2 新建 Skill 系统的架构设计

#### 3.2.1 文件格式：SKILL.md

借鉴 Anthropic 官方规范：

```markdown
---
id: deploy-rust-service
name: Deploy Rust Service
version: 1.0.0
description: |
  安全地将 Rust 服务部署到生产环境，包含构建、测试、滚动发布、
  健康检查和回退流程。
tools:
  - git_status
  - shell_build
  - shell_test
  - shell_deploy
  - web_fetch
tags: [deploy, rust, production]
---

## 前提检查

1. 确认当前分支是 `main` 且工作区干净
2. 确认 CI 最近一次运行通过

## 执行步骤

1. **构建** — 在 release 模式下编译，验证无警告：
   ```bash
   cargo build --release
   ```

2. **测试** — 运行完整测试套件：
   ```bash
   cargo test --workspace
   ```

3. **部署** — 执行滚动发布：
   ```bash
   ./scripts/deploy.sh
   ```

4. **健康检查** — 轮询 `/health` 端点直到返回 200：
   ```
   使用 web_fetch 工具，URL: https://api.example.com/health
   ```

## 错误处理

- 如果构建失败：停止部署，向用户报告编译错误
- 如果测试失败：停止部署，提示用户修复测试
- 如果健康检查 3 次失败：自动执行 `./scripts/rollback.sh`

## 输出格式

最终向用户汇报：版本号、部署耗时、健康检查结果、回退状态（如适用）
```

#### 3.2.2 核心类型设计

```rust
/// 编排层工作流模板 —— 不对 LLM 暴露实现细节
#[derive(Debug, Clone)]
pub struct Skill {
    pub meta: SkillMeta,      // YAML frontmatter
    pub body: String,         // Markdown 正文（作为额外上下文注入）
}

#[derive(Debug, Clone, Deserialize)]
pub struct SkillMeta {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub tools: Vec<String>,   // 该 Skill 允许使用的 Tool 白名单
    pub tags: Vec<String>,
}

/// Skill 注册表 —— 只读，线程安全，可被多个 Agent 实例共享
#[derive(Clone, Default)]
pub struct SkillRegistry {
    skills: Arc<HashMap<String, Skill>>,
}

impl SkillRegistry {
    /// 从目录加载所有 SKILL.md 文件
    pub fn load_from_dir(path: &Path) -> Result<Self, SkillError>;
    
    /// 根据标签或关键词搜索匹配的 Skill
    pub fn find_relevant(&self, query: &str) -> Vec<&Skill>;
    
    /// 获取 Skill 的完整系统提示词片段（meta + body）
    pub fn build_context(&self, skill_id: &str) -> Option<String>;
}
```

#### 3.2.3 与 Agent 的集成方式

**方式 A：显式调用（用户指定）**

```rust
agent.run_with_skill("deploy-rust-service", "部署最新版本").await
```

此时 Agent 的系统提示词追加 Skill 的 `body` 内容，且 `ToolRegistry` 被过滤为仅包含 Skill `tools` 白名单中的工具。

**方式 B：隐式匹配（自动激活）**

```rust
// Agent::run 内部自动检测
let relevant = skill_registry.find_relevant(query.as_ref());
if let Some(skill) = relevant.first() {
    system_prompt.push_str(&skill.build_context());
    // 可选：限制可用工具为 skill.tools 白名单
}
```

**方式 C：Plugin 层绑定（入口差异化）**

```rust
// claw 层默认加载 "生活助手" Skill
// cli 层默认加载 "工程开发" Skill
// window 层默认加载 "快速查询" Skill
```

### 3.3 与 MCP 的关系澄清

```
┌────────────────────────────────────────────────────────────────────────┐
│                           Clarity Agent                                │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────────────┐ │
│  │  Plugin 层      │  │  Skill 层       │  │  ToolRegistry           │ │
│  │  (claw/cli/     │  │  (工作流模板)    │  │  (原子能力注册表)        │ │
│  │   window)       │  │  SKILL.md       │  │                         │ │
│  └────────┬────────┘  └────────┬────────┘  └────────────┬────────────┘ │
│           │                    │                        │              │
│           │    "我要部署"       │  "使用 deploy skill"   │              │
│           └────────────────────┴────────────────────────┘              │
│                                                     │                  │
│                              ┌──────────────────────┘                  │
│                              ▼                                         │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │  MCP Client Layer                                                 │  │
│  │  ┌─────────────┐  ┌─────────────┐  ┌──────────────────────────┐  │  │
│  │  │ StdioClient │  │ HttpClient  │  │ SseClientStub (TODO)     │  │  │
│  │  └──────┬──────┘  └──────┬──────┘  └────────────┬─────────────┘  │  │
│  │         │                │                      │                │  │
│  │         └────────────────┴──────────────────────┘                │  │
│  │                          │                                       │  │
│  │                          ▼ JSON-RPC 2.0                         │  │
│  │              ┌───────────────────────┐                          │  │
│  │              │   MCP Server (外部)    │                          │  │
│  │              │  filesystem / github  │                          │  │
│  │              └───────────────────────┘                          │  │
│  └──────────────────────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────────────────────┘
```

**关系总结**：

| 关系 | 说明 |
|------|------|
| **Skill → Tool** | Skill 编排 Tool，一个 Skill 可以调用多个 Tool 按序执行 |
| **Skill → MCP** | Skill 不直接感知 MCP；它编排的是 ToolRegistry 中的 Tool，无论这些 Tool 是原生实现还是 MCP 桥接 |
| **MCP → Tool** | MCP 桥接器（`McpToolWrapper` / `McpToolAdapter`）将 MCP Server 的能力注册为 ToolRegistry 中的普通 Tool |
| **Plugin → Skill** | Plugin 绑定 Skill，提供完整的应用上下文（系统提示词 + 模型 + 能力白名单） |
| **Plugin → Tool** | Plugin 也可以直接限制 ToolRegistry 的子集，不经过 Skill 层 |

---

## 四、迁移路线图

### Phase 1：清理（当前会话可完成）

- [ ] 删除 `crates/clarity-core/src/skill/mod.rs`、`think.rs`、`todo.rs`
- [ ] 从 `lib.rs` 移除 `pub mod skill`
- [ ] 从 `prelude.rs`（如果存在）移除 Skill 相关导出
- [ ] 将 `agent/mod.rs` 中的 `get_skill_definitions()` 重命名为 `get_tool_descriptions()`
- [ ] 确保所有测试通过

### Phase 2：Skill 系统重建（下一波工作）

- [ ] 新建 `crates/clarity-core/src/skills/` 目录（复数形式，避免与旧 `skill/` 混淆）
- [ ] 实现 `SkillMeta`、`Skill`、`SkillRegistry` 类型
- [ ] 实现 `SkillLoader` —— 从目录解析 SKILL.md 文件（YAML frontmatter + Markdown body）
- [ ] 实现 `SkillContextInjector` —— 将 Skill 内容注入 Agent 系统提示词
- [ ] 实现 Tool 白名单过滤 —— 当使用 Skill 时，仅暴露允许的工具给 LLM
- [ ] 编写 3-5 个示例 SKILL.md（deploy、code-review、bug-investigate、onboarding）

### Phase 3：与 Plugin/入口系统整合

- [ ] 在 `AgentConfig` 中增加 `default_skill: Option<String>`
- [ ] 在 `claw` / `cli` / `window` 入口分别绑定不同的默认 Skill
- [ ] 支持运行时 `/skill list`、`/skill use <id>` 命令（类似 `osai` 的交互方式）

### Phase 4：MCP 能力补全

- [ ] 实现 `resources/list`、`resources/read`（当前仅实现 `tools/*`）
- [ ] 实现 SSE Transport（当前为 Stub）
- [ ] 在 Skill 中支持引用 MCP Resource URI（如 `resource://github-repo/PRs`）

---

## 五、参考与引用

1. **Anthropic Model Context Protocol Specification** — https://modelcontextprotocol.io
2. **Anthropic Skill 白皮书** — https://docs.anthropic.com/en/docs/skills/overview
3. **"Skill vs MCP: 食谱与厨房"** — 掘金深度解读（Anthropic 官方类比）
4. **Semantic Kernel + MCP 整合** — Microsoft Azure OpenAI 集成实践
5. **osai (Swift Desktop Agent)** — GitHub 开源项目，同时实现 MCP / Plugin / Skill / Task / Memory 五层
6. **Agent Skills Survey Paper** (arXiv:2602.12430) — "skills supply the 'what to do' and MCP supplies the 'how to connect'"

---

*本文档是 Clarity 架构现代化 Wave 2 的输入，供后续实施波次参考。*
