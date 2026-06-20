# DeepSeek 设备登录 Chat 阶段说明

> 状态：Chat 可用，Work/Claw 暂不暴露  
> 生效范围：`clarity-llm` / `clarity-gateway` / `clarity-egui`（后续 UI 接入）

---

## 1. 目标

在 **Chat 会话模式** 下提供接近 DeepSeek Android App 原生的对话体验：

- 无需官方 API Key，使用 App 设备登录 token 或手机号密码
- 支持普通对话（`deepseek-chat`）和深度思考（`deepseek-reasoner`）
- 深度思考过程透传给 UI（`StreamDelta.reasoning_content`）
- **Work / Claw 模式暂不可用**，通过 `chat-only` tag 隔离

---

## 2. 架构位置

```
clarity-llm
├── deepseek_device.rs      # DeepSeekDeviceProvider（无状态）
├── deepseek_pow.rs         # DeepSeekHashV1 PoW 求解器（纯 Rust + rayon）
├── model_registry.rs       # ProtocolType::DeepSeekDevice + chat-only 过滤
└── registry_table.rs       # "deepseek-device" family

clarity-contract
└── StreamDelta             # 新增 reasoning_content 字段
```

`DeepSeekDeviceProvider` 位于 LLM Provider 层，**不感知 Chat/Work/Claw**。上层通过 `ModelRegistry::aliases_for_context("chat")` 选择可用模型，自然排除 Work/Claw。

---

## 3. 配置方式

### 3.1 `models.toml`（推荐）

```toml
[providers.deepseek-device]
protocol = "deepseek_device"

# token 模式
[[models]]
alias = "deepseek-device"
provider = "deepseek-device"
model_id = "deepseek-chat"
api_key = "从 App MMKV 提取的 device token"

# 深度思考模式
[[models]]
alias = "deepseek-reasoner"
provider = "deepseek-device"
model_id = "deepseek-reasoner"
api_key = "从 App MMKV 提取的 device token"

# 手机号密码模式（token 过期自动刷新）
[[models]]
alias = "deepseek-device-pwd"
provider = "deepseek-device"
model_id = "deepseek-chat"
[providers.deepseek-device.extra]
mobile = "136****"
password = "你的密码"
```

### 3.2 环境变量

| 变量 | 用途 |
|---|---|
| `DEEPSEEK_DEVICE_TOKEN` | token 模式 |
| `DEEPSEEK_DEVICE_MOBILE` + `DEEPSEEK_DEVICE_PASSWORD` | 密码模式 |

### 3.3 管理 API

```bash
curl -X POST http://localhost:port/api/admin/alias \
  -H "Content-Type: application/json" \
  -d '{
    "alias": "deepseek-device",
    "provider": "deepseek-device",
    "model_id": "deepseek-chat",
    "api_key": "你的 token",
    "protocol": "deepseek_device"
  }'
```

---

## 4. 模型映射（APK 逆向确认）

| `model_id` | App 内部 `model_type` | 特性 |
|---|---|---|
| `deepseek-chat` | `default` | 普通对话 |
| `deepseek-reasoner` | `expert` | 深度思考，自动开启 `thinking_enabled` |
| `deepseek-vision` | `vision` | 图片理解（暂未支持图片输入） |

来源：`base-apk-decompiled/sources/defpackage/cd5.java` 中 `model_type` 分支判断。

---

## 5. 请求体字段（APK 逆向确认）

`ChatFullCompletionRequest`（`/api/v0/chat/completion`）：

```json
{
  "chat_session_id": "...",
  "parent_message_id": null,
  "prompt": "用户输入",
  "ref_file_ids": [],
  "thinking_enabled": false,
  "search_enabled": false,
  "audio_id": null,
  "preempt": false,
  "model_type": "default",
  "action": null
}
```

- `thinking_enabled` / `search_enabled`：可选 bool，默认 false
- `model_type`：可选 string，默认由模型映射决定

---

## 6. Provider 无状态化

每次 `complete` / `stream` 调用：

1. 获取/刷新 token
2. **新建 DeepSeek `chat_session_id`**
3. 请求 `/api/v0/chat/completion`

这样 gateway 中多个 Chat 会话复用同一个 provider 实例时，不会互相串扰。

Password 模式下 token 过期（401/403）会自动重新登录一次；token 模式下过期会返回明确错误。

---

## 7. Work / Claw 隔离

`"deepseek-device"` family 默认带有 `tags = ["chat-only"]`。

```rust
let chat_aliases = registry.aliases_for_context("chat");   // 包含 deepseek-device
let work_aliases = registry.aliases_for_context("work");   // 排除 chat-only
let claw_aliases = registry.aliases_for_context("claw");   // 排除 chat-only
```

解除限制：从 `registry_table.rs` 中移除 `"chat-only"` tag，或在上层调用时选择 `"chat"` 上下文。

---

## 8. 已知限制

- Token 有效期约 24 小时，token 模式需手动更新
- 不支持 tools（设备端 API 本身无 tools）
- 不支持 vision 图片输入（先让文本对话跑通）
- 每次请求新建 session，额外一次 `/api/v0/chat_session/create` HTTP 调用
- 搜索触发结果仅记录到日志，尚未透传给 UI

---

## 9. 后续工作

1. UI 接入：Chat 模式下显示 `StreamDelta.reasoning_content` 的折叠面板
2. 搜索触发结果通过自定义字段或 tool_result 返回给 UI
3. 支持 vision 图片上传
4. 解除 Work/Claw 限制（如后续设计确定需要）
