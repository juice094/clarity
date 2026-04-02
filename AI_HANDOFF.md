# AI 交接文档

> 致后续会话的 AI 助手：你好！这是 Project Clarity 的开发上下文，请仔细阅读后再与用户交互。

---

## 项目概览

**Project Clarity** 是一个 Rust 实现的 AI Agent 框架，融合 Claude Code 体验与 OpenClaw 网关架构。

```
目标：CCA 架构
├── C - Claude-Code 级终端体验
├── C - OpenClaw 级网关能力  
└── A - Agent 编排核心
```

---

## 项目结构

```
C:\Users\22414\Desktop\clarity
├── Cargo.toml                    # Workspace 根配置
├── DEV_LOG.md                    # 开发日志（你在这里）
├── AI_HANDOFF.md                 # 本文档
├── HUMAN_GUIDE.md                # 用户指南
├── README.md                     # 项目说明
├── crates/
│   ├── clarity-core/             # 核心引擎 ⭐ 最重要
│   │   ├── src/
│   │   │   ├── agent.rs         # Agent Loop（需接入LLM）
│   │   │   ├── llm.rs           # LLM Provider（已完成）
│   │   │   ├── registry.rs      # Tool Registry
│   │   │   ├── tools/           # 7个内置工具
│   │   │   └── ...
│   ├── clarity-tui/              # 终端UI
│   └── clarity-gateway/          # HTTP网关
└── examples/
    ├── claude_code_compat.rs    # Kimi Code 示例 ⭐
    ├── kimi_demo.rs
    └── ollama_demo.rs
```

---

## 关键信息

### 1. LLM 配置（已验证可用）

```powershell
# Kimi Code（推荐，每周1024次免费）
$env:ANTHROPIC_BASE_URL="https://api.kimi.com/coding/"
$env:ANTHROPIC_AUTH_TOKEN="sk-kimi-7wIafvpXmFAZAdBwBHsCQyXaPJ0zQrGbETdKgOjnhQdtXfkbRh2zayGqkFAeAvTz"
$env:ANTHROPIC_MODEL="kimi-for-coding"
```

**API 测试状态**: ✅ 已通过 PowerShell 测试，连接正常

### 2. 架构特点

- **协议自动检测**: `llm.rs` 根据 `base_url` 自动选择 Anthropic/OpenAI 协议
- **配置兼容**: 同时支持 `ANTHROPIC_*` (Claude Code风格) 和 `KIMI_*` (Clarity风格)
- **工具系统**: Trait-based，已注册7个内置工具

### 3. 当前状态

| 模块 | 完成度 | 待办 |
|------|--------|------|
| clarity-core | 85% | Agent.run() 需接入真实LLM |
| clarity-tui | 80% | 需集成LLM调用（目前是模拟响应） |
| clarity-gateway | 75% | Admin UI 页面待完善 |

---

## 技术细节

### LLM Provider 使用

```rust
// 在 agent.rs 或 TUI 中
use clarity_core::{KimiLlm, LlmFactory};
use std::sync::Arc;

// 方式一：自动检测环境变量
let llm = LlmFactory::auto()?;

// 方式二：手动创建
let llm = KimiLlm::from_env()?;  // 读取 ANTHROPIC_* 或 KIMI_*

// 创建 Agent
let agent = Agent::with_config(registry, config)
    .with_llm(Arc::new(llm));

// 运行对话
let response = agent.run("Hello").await?;
```

### 关键代码位置

- **LLM Trait**: `crates/clarity-core/src/agent.rs:166` (`LlmProvider`)
- **Kimi 实现**: `crates/clarity-core/src/llm.rs:23` (`KimiLlm`)
- **Agent Loop**: `crates/clarity-core/src/agent.rs:291` (`Agent.run()`)
- **TUI 入口**: `crates/clarity-tui/src/main.rs`

### 协议检测逻辑

```rust
// llm.rs 中自动检测
let protocol = if base_url.contains("kimi.com/coding") || base_url.contains("anthropic") {
    Protocol::Anthropic  // 使用 /v1/messages
} else {
    Protocol::OpenAI      // 使用 /chat/completions
};
```

---

## 常见任务

### 1. 添加新工具

```rust
// 在 crates/clarity-core/src/tools/ 新建文件
use async_trait::async_trait;
use serde_json::Value;

pub struct MyTool;

#[async_trait]
impl Tool for MyTool {
    fn name(&self) -> &str { "my_tool" }
    fn description(&self) -> &str { "工具描述" }
    fn parameters(&self) -> Value { json!({...}) }
    
    async fn execute(&self, args: Value, ctx: ToolContext) -> Result<Value, ToolError> {
        // 实现逻辑
        Ok(json!("result"))
    }
}

// 在 registry.rs 注册
registry.register(MyTool::new())?;
```

### 2. 添加新 LLM Provider

```rust
// 实现 LlmProvider trait
#[async_trait]
impl LlmProvider for MyLlm {
    async fn complete(&self, messages: &[Message], tools: &Value) -> Result<LlmResponse, AgentError> {
        // HTTP 调用
        // 解析响应
        Ok(LlmResponse { content, tool_calls, is_complete })
    }
}
```

### 3. 调试 TUI

```bash
cd C:\Users\22414\Desktop\clarity
cargo run --bin clarity-tui
```

---

## 注意事项

### 1. 环境变量优先级

```rust
// LlmFactory::auto() 检测顺序:
1. ANTHROPIC_BASE_URL / ANTHROPIC_AUTH_TOKEN  → KimiLlm (Anthropic协议)
2. KIMI_API_KEY / KIMI_BASE_URL               → KimiLlm (自动检测协议)
3. OPENAI_API_KEY                             → OpenAiCompatibleLlm
```

### 2. API Key 安全

- Key 已硬编码在示例中（用户提供的测试Key）
- 生产环境应改为环境变量读取
- **不要**将有效 Key 提交到 Git

### 3. 已知问题

- TUI 目前是模拟响应，需集成真实 LLM 调用
- `Agent.run()` 需要接入 `LlmProvider`
- 工具调用结果回传尚未实现

---

## 参考资源

- **Claude Code 源码**: `C:\Users\22414\Desktop\claude-code-haha-main`
  - 查看 `src/services/api/client.ts` 了解 Anthropic SDK 使用
  - 查看 `src/utils/model/providers.ts` 了解多提供商支持

- **OpenClaw Rust**: `C:\Users\22414\Desktop\openclaw-main` (如存在)
  - 参考 Gateway 实现
  - 参考 WASM 插件系统

---

## 快速开始命令

```powershell
# 1. 检查配置
.\test_config.ps1

# 2. 运行 Kimi Code 示例
.\run_with_kimi.ps1

# 3. 或手动运行
$env:ANTHROPIC_BASE_URL="https://api.kimi.com/coding/"
$env:ANTHROPIC_AUTH_TOKEN="sk-kimi-7wIafvpXmFAZAdBwBHsCQyXaPJ0zQrGbETdKgOjnhQdtXfkbRh2zayGqkFAeAvTz"
cargo run --example claude_code_compat
```

---

## 用户背景

- **水平**: 中级开发者，对 Rust 熟悉但写不快
- **需求**: 需要一个透明、可扩展的 Agent 框架
- **痛点**: 云端 OpenClaw 不可见，需要本地可控方案
- **目标**: 融合 Claude Code 体验 + OpenClaw 网关 + 本地模型支持

---

请根据以上信息继续协助用户开发。如有疑问，查看 `DEV_LOG.md` 获取更详细历史记录。
