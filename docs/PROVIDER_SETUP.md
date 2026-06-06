---
title: Clarity 模型提供商配置速查
category: Guide
date: 2026-05-16
tags: [guide]
---

# Clarity 模型提供商配置速查

> 快速上手：设置环境变量 → 启动 clarity-egui → Settings Panel 选择 Provider → 开始对话

---

## 零配置启动（推荐新手）

Clarity 支持**环境变量自动发现**。只要在系统里设置对应 API Key，启动时自动识别：

```powershell
# PowerShell 示例（临时，当前会话有效）
$env:OPENAI_API_KEY = "sk-..."
$env:DEEPSEEK_API_KEY = "sk-..."
$env:KIMI_API_KEY = "sk-..."

# 或者写入系统环境变量（永久）
[Environment]::SetEnvironmentVariable("OPENAI_API_KEY", "sk-...", "User")
```

| 提供商 | 环境变量 | 自动识别条件 |
|--------|----------|-------------|
| OpenAI | `OPENAI_API_KEY` | 变量存在即可 |
| DeepSeek | `DEEPSEEK_API_KEY` | 变量存在即可 |
| Kimi (Moonshot) | `KIMI_API_KEY` | 变量存在即可 |
| Kimi Code | `KIMI_CODE_API_KEY` | 变量存在即可（优先级高于普通 Kimi）|
| Anthropic | `ANTHROPIC_AUTH_TOKEN` | 变量存在即可 |
| Ollama | `OLLAMA_HOST` | 可选，默认 `http://localhost:11434` |
| Local GGUF | `CLARITY_LOCAL_MODEL_PATH` | 或自动扫描 `~/models/*.gguf` |

启动 `clarity-egui` 后，Settings Panel 的 Provider ComboBox 会**只显示已检测到可用 Key 的提供商**。

---

## 各家 API 文档与 Base URL

| 提供商 | 协议 | Base URL | 官方文档 |
|--------|------|----------|----------|
| **OpenAI** | `openai_chat` | `https://api.openai.com/v1` | https://platform.openai.com/docs |
| **DeepSeek** | `openai_chat` | `https://api.deepseek.com` | https://api-docs.deepseek.com/zh-cn/ |
| **DeepSeek (Anthropic)** | `anthropic_messages` | `https://api.deepseek.com/anthropic` | 同上 |
| **Kimi (Moonshot)** | `openai_chat` | `https://api.moonshot.cn/v1` | https://platform.moonshot.cn/docs |
| **Kimi Code** | `openai_chat` | `https://api.kimi.com/coding/v1` | 同上，Coding 专用 endpoint |
| **Anthropic** | `anthropic_messages` | `https://api.anthropic.com` | https://docs.anthropic.com/en/api/getting-started |
| **Ollama** | `ollama` | `http://localhost:11434` | https://github.com/ollama/ollama/blob/main/docs/api.md |
| **Local GGUF** | `local` | — (本地文件) | 使用 Candle 原生推理，无 HTTP |

---

## 高级配置：`models.toml`

如果环境变量不够用（比如需要多账号、自定义 endpoint、第三方代理），创建 `models.toml`：

**搜索路径**（优先级从高到低）：
1. `CLARITY_MODELS_CONFIG` 环境变量指定的路径
2. `./.clarity/models.toml`（项目本地）
3. `~/.config/clarity/models.toml`（用户级）

### 完整示例

```toml
# 提供商定义
[providers.openai]
protocol = "openai_chat"
base_url = "https://api.openai.com/v1"
api_key_env = "OPENAI_API_KEY"

[providers.deepseek]
protocol = "openai_chat"
base_url = "https://api.deepseek.com/v1"
api_key_env = "DEEPSEEK_API_KEY"

[providers.kimi]
protocol = "openai_chat"
base_url = "https://api.moonshot.cn/v1"
api_key_env = "KIMI_API_KEY"

[providers.anthropic]
protocol = "anthropic_messages"
base_url = "https://api.anthropic.com"
api_key_env = "ANTHROPIC_AUTH_TOKEN"

[providers.ollama]
protocol = "ollama"
base_url = "http://localhost:11434"
# ollama 不需要 api_key

[providers.local]
protocol = "local"
extra = { model_path = "~/models/Qwen2.5-7B-Instruct.Q4_K_M.gguf" }

# 模型别名定义（UI 下拉框中显示的名称）
[[models]]
alias = "gpt-4o"
provider = "openai"
model_id = "gpt-4o"

[[models]]
alias = "deepseek-chat"
provider = "deepseek"
model_id = "deepseek-chat"

[[models]]
alias = "deepseek-reasoner"
provider = "deepseek"
model_id = "deepseek-reasoner"

[[models]]
alias = "kimi-k2"
provider = "kimi"
model_id = "kimi-k2.6"

[[models]]
alias = "claude-sonnet"
provider = "anthropic"
model_id = "claude-3-5-sonnet-20241022"

[[models]]
alias = "llama3-8b"
provider = "ollama"
model_id = "llama3:8b"

[[models]]
alias = "local-qwen"
provider = "local"
model_id = "Qwen2.5-7B-Instruct"
```

---

## Settings Panel 使用流程

```
1. 启动 clarity-egui
2. 点击左上角 ⚙️ Settings
3. Provider: 从下拉框选择（仅显示已配置/检测到的）
4. Model: 自动联动显示该 Provider 下的可用模型
5. API Key: 可选输入（留空则读取环境变量）
6. 点击 Save → 自动 reload LLM
```

**API Key 输入框支持环境变量语法**：输入 `${env:OPENAI_API_KEY}` 可避免密钥落盘。

---

## 离线模式（Local GGUF）

无需任何 API Key，纯本地推理：

```powershell
# 方式一：环境变量指定模型路径
$env:CLARITY_LOCAL_MODEL_PATH = "C:\Users\22414\Desktop\model\Qwen2.5-7B-Instruct.Q4_K_M.gguf"

# 方式二：放到默认扫描目录
copy model.gguf ~\models\

# 方式三：首次启动走 onboarding 引导下载
# clarity-egui 首次启动 → 选择"下载本地模型" → 自动下载 Qwen2.5-1.5B-Instruct
```

支持的模型架构：Qwen2、Qwen2.5、DeepSeek-R1-Distill-Qwen（GGUF 格式）。

---

## 故障排查

| 症状 | 原因 | 解决 |
|------|------|------|
| Provider 下拉框为空 | 没有检测到任何 API Key | 检查环境变量，或直接在 Settings 输入 API Key |
| "Failed to create LLM provider" | API Key 无效或网络不通 | 检查 Key 有效性；确认 base_url 可访问 |
| 本地模型无法加载 | 路径错误或格式不支持 | 确认 `.gguf` 后缀；检查 `CLARITY_LOCAL_MODEL_PATH` |
| Model 下拉框不联动 | provider 切换后 model 未刷新 | 保存 Settings 后自动刷新；或重启应用 |
| 切换 Provider 后仍用旧模型 | `ensure_llm` 未触发 reload | 保存 Settings 时会自动 reload |

---

*上次更新：2026-04-28（Sprint 12 完成后）*
