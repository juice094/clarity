---
title: Provider 与模型配置
category: Development
date: 2026-06-13
tags: [provider, model, config, secrets]
---

# Provider 与模型配置

> 推荐通过 `models.toml` 配置多 Provider/多别名，而非仅依赖环境变量。加密 key 工具见 [`clarity-secrets`](../../crates/clarity-secrets)。

---

## 零配置启动（新手推荐）

Clarity 支持**环境变量自动发现**。只要设置对应 API Key，启动时自动识别：

```powershell
# PowerShell 临时示例
$env:OPENAI_API_KEY = "sk-..."
$env:DEEPSEEK_API_KEY = "sk-..."
$env:KIMI_API_KEY = "sk-..."

# 永久写入系统环境变量
[Environment]::SetEnvironmentVariable("OPENAI_API_KEY", "sk-...", "User")
```

| 提供商 | 环境变量 | 说明 |
|--------|----------|------|
| OpenAI | `OPENAI_API_KEY` | |
| DeepSeek | `DEEPSEEK_API_KEY` | |
| Kimi (Moonshot) | `KIMI_API_KEY` | |
| Kimi Code | `KIMI_CODE_API_KEY` | 编程计划专用，key 以 `sk-kimi-` 开头 |
| Anthropic | `ANTHROPIC_AUTH_TOKEN` | |
| Ollama | `OLLAMA_HOST` | 可选，默认 `http://localhost:11434` |
| Local GGUF | `CLARITY_LOCAL_MODEL_PATH` | 或自动扫描 `~/models/*.gguf` |

Settings Panel 的 Provider 下拉框**只显示已检测到可用 Key 的提供商**。

---

## 各家 API 文档与 Base URL

| 提供商 | 协议 | Base URL | 官方文档 |
|--------|------|----------|----------|
| OpenAI | `openai_chat` | `https://api.openai.com/v1` | https://platform.openai.com/docs |
| DeepSeek | `openai_chat` | `https://api.deepseek.com/v1` | https://api-docs.deepseek.com |
| Kimi (Moonshot) | `openai_chat` | `https://api.moonshot.cn/v1` | https://platform.moonshot.cn/docs |
| Kimi Code | `openai_chat` | `https://api.kimi.com/coding/v1` | 同上 |
| Anthropic | `anthropic_messages` | `https://api.anthropic.com` | https://docs.anthropic.com |
| Ollama | `ollama` | `http://localhost:11434` | https://github.com/ollama/ollama/blob/main/docs/api.md |
| Local GGUF | `local` | — | Candle 原生推理，无 HTTP |

---

## 高级配置：`models.toml`

### 搜索路径（优先级从高到低）

1. `$env:CLARITY_MODELS_CONFIG`
2. `./.clarity/models.toml`
3. `~/.config/clarity/models.toml`
4. 内置 env-var fallback

### 完整示例

```toml
[providers.openai]
protocol = "openai_chat"
base_url = "https://api.openai.com/v1"
api_key_env = "OPENAI_API_KEY"

[providers.deepseek]
protocol = "openai_chat"
base_url = "https://api.deepseek.com/v1"
api_key_env = "DEEPSEEK_API_KEY"

[providers.moonshot]
protocol = "openai_chat"
base_url = "https://api.moonshot.cn/v1"
api_key_env = "KIMI_API_KEY"

[[models]]
alias = "deepseek-v4-pro"
provider = "deepseek"
model_id = "deepseek-v4-pro"
# 推荐加密 key。加密工具：cargo run -p clarity-secrets --example encrypt_key
api_key = "enc2:..."
# 失败时依次尝试这些 alias
fallback_aliases = ["kimi-k2", "gpt-4o"]
tags = ["cheap", "coding", "long-context"]

[[models]]
alias = "kimi-k2"
provider = "moonshot"
model_id = "kimi-k2.6"
tags = ["coding"]
pricing = { input_per_1m = 0.5, output_per_1m = 0.5 }

[[models]]
alias = "router"
provider = "router"
model_id = "router:cheap"
```

### per-alias 覆盖

别名可覆盖 provider 层设置，包括加密后的 `api_key`。`router:<hint>` 别名由运行时路由器在请求时解析（`cheap`/`coding`/`vision`/`tools`/`fast`/显式 alias）。

---

## 离线模式（Local GGUF）

无需任何 API Key，纯本地推理：

```powershell
# 方式一：环境变量指定模型路径
$env:CLARITY_LOCAL_MODEL_PATH = "C:\path\to\model.gguf"

# 方式二：放到默认扫描目录
copy model.gguf ~\models\

# 方式三：首次启动走 onboarding 引导下载
```

支持的模型架构：Qwen2、Qwen2.5、DeepSeek-R1-Distill-Qwen（GGUF 格式）。

---

## Settings Panel 使用流程

1. 启动 `clarity-egui`
2. 点击左上角 ⚙️ Settings
3. Provider：从下拉框选择（仅显示已配置/检测到的）
4. Model：自动联动显示该 Provider 下的可用模型
5. API Key：可选输入（留空则读取环境变量）
6. 点击 Save → 自动 reload LLM

**API Key 输入框支持环境变量语法**：输入 `${env:OPENAI_API_KEY}` 可避免密钥落盘。

---

## 故障排查

| 症状 | 原因 | 解决 |
|------|------|------|
| Provider 下拉框为空 | 没有检测到任何 API Key | 检查环境变量，或直接在 Settings 输入 API Key |
| "Failed to create LLM provider" | API Key 无效或网络不通 | 检查 Key 有效性；确认 base_url 可访问 |
| 本地模型无法加载 | 路径错误或格式不支持 | 确认 `.gguf` 后缀；检查 `CLARITY_LOCAL_MODEL_PATH` |
| 切换 Provider 后仍用旧模型 | `ensure_llm` 未触发 reload | 保存 Settings 时会自动 reload |

---

*最后更新：2026-06-13*
