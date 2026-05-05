# Sprint 27-A2 实现计划：`prompt_cache_key` 策略层

> 状态：计划就绪，待执行  
> 基线：`main` @ Sprint 36.5 完成  
> 优先级：P0  
> 预估改动：~11 个文件

---

## 已知现状（调研结论）

1. **A1 已完成**：`SystemPromptBuilder::build_split()` 已把 base/tools/skills/approval/security 归为 `static_prompt`，git/active_files/metadata/memory 归为 `dynamic_prompt`。
2. **已有基础设施**：
   - `ChatCompletionRequest`（OpenAI 兼容层）已有 `prompt_cache_key: Option<String>` 字段并参与序列化。
   - `OpenAiCompatibleLlm` 已有 `prompt_cache_key` 字段和 `set_prompt_cache_key` 方法。
   - `ProviderCapabilities` 已有 `prompt_caching: bool` 标志位。
   - `Agent::build_messages_with_cache()` 已计算 `static_hash`（但使用不稳定的 `DefaultHasher`），并在变化时调用 `llm.clear_cache()`。
3. **关键阻塞**：`AgentInner.llm` 的类型是 `Option<Arc<dyn LlmProvider>>`，而 `LlmProvider::set_prompt_cache_key` 当前签名是 `&mut self`，**无法通过 `Arc` 调用**。这是 A2 必须解决的核心矛盾。
4. **Provider 矩阵**：
   - **OpenAI 兼容系**（OpenAI、DeepSeek、Kimi、KimiCode/OAuth、LlamaServer）：代码层面已有 `prompt_cache_key` 请求字段，只缺稳定 hash 注入。
   - **Anthropic**：`set_prompt_cache_key` 是空实现，且 Anthropic 真实 API 使用 `cache_control`（message-level），非顶层 `prompt_cache_key`。
   - **Ollama / LocalGguf / Kalosm**：本地模型，不支持服务端 prefix caching；`LocalGgufProvider` 的 `prompt_caching: true` 指的是本地 KV 缓存。

---

## 最小实现方案

### 1. 稳定 hash 计算（`agent/prompt.rs`）

**动作**：把 `build_messages_with_cache()` 中的 `std::collections::hash_map::DefaultHasher` 替换为 `sha2::Sha256`（`sha2 = "0.10"` 已是 `clarity-core` 依赖），输出固定长度 hex string。

```rust
// 替换前
let mut hasher = DefaultHasher::new();
static_prompt.hash(&mut hasher);
hasher.finish().to_string()

// 替换后
use sha2::{Sha256, Digest};
let hash = Sha256::digest(static_prompt.as_bytes());
let static_hash = hex::encode(hash);  // 或用 base16
```

**理由**：`DefaultHasher` 在 Rust 版本/平台间不稳定，不能作为跨请求/跨会话的 provider 级 cache key。

### 2. Provider trait 签名微调（`clarity-contract/src/llm.rs`）

**动作**：将 `set_prompt_cache_key` 从 `&mut self` 改为 `&self`。

```rust
fn set_prompt_cache_key(&self, key: &str);
```

**理由**：这是使 `Arc<dyn LlmProvider>` 能够调用该方法的**唯一最小改动**。该方法本质是内部状态更新（设置一个字符串），与 `clear_cache(&self)` 语义一致。所有实现者在同一仓库内，可同步修改。

### 3. 各 Provider 内部可变性改造

| Provider | 具体改动 |
|---|---|
| `OpenAiCompatibleLlm` | `prompt_cache_key: Option<String>` → `Arc<std::sync::RwLock<Option<String>>>`；trait 实现中 `write` 更新。`capabilities()` 返回 `prompt_caching: true`。 |
| `DeepSeekProvider` / `KimiLlm` / `OAuthLlm` / `LlamaServerProvider` | 删除/修改 `pub fn set_prompt_cache_key(&mut self, ...)`，trait 实现中透传 `&self` 调用 inner。 |
| `AnthropicLlm` | trait 签名改为 `&self`，保持空实现。`prompt_caching: false`。 |
| `OllamaProvider` | trait 签名改为 `&self`，保持空实现。`prompt_caching: false`。 |
| `LocalGgufProvider` | `cache_key: Option<String>` → `Mutex<Option<String>>`（或 `Arc<Mutex<...>>`）；trait 签名改为 `&self`。`prompt_caching: true`（本地 KV）。 |
| `KalosmProvider` | 同步更新签名（已废弃，仅需编译通过）。 |
| `ReliableProvider` | trait 签名改为 `&self`，遍历 `self.providers` 透传调用（因为链内 provider 也是 `Arc<dyn>`，`&self` 可直接调用）。 |
| `MockLlm` / 测试 dummy providers | 同步更新签名。 |

### 4. Agent 层注入逻辑（`agent/prompt.rs`）

在 `build_messages_with_cache()` 中，hash 变化分支里新增：

```rust
if inner.static_prompt_hash.as_ref() != Some(&static_hash) {
    if let Some(ref llm) = inner.llm {
        llm.clear_cache();
        if llm.capabilities().prompt_caching {
            llm.set_prompt_cache_key(&static_hash);
        }
    }
}
```

**行为**：
- 静态内容不变 → hash 不变 → 不碰 provider 状态。
- 静态内容变化 → 更新 hash → `clear_cache()` + `set_prompt_cache_key(new_hash)`。
- 不支持的 provider（`prompt_caching == false`）→ 只 `clear_cache()`，不发送无意义的 cache key。

### 5. Provider 门控与能力声明

- **支持服务端 prefix caching**：`OpenAiCompatibleLlm` 及其所有 wrapper（DeepSeek、Kimi、OAuth、LlamaServer）在 `capabilities()` 中显式返回 `prompt_caching: true`。
- **Anthropic**：保持 `false`，后续 Sprint 若要支持需改用 `cache_control` message-level 字段，超出 A2 最小范围。
- **本地/Ollama**：保持 `false` 或已有值。

---

## 测试计划

### A. Hash 稳定性测试（`agent/prompt.rs`）
- `test_static_hash_stable`：相同 `SystemPromptBuilder` 配置，两次 `build_split()` 产生的 `static_prompt` 的 SHA-256 hex 一致。
- `test_static_hash_sensitive_to_content`：更换 tool/skills/base 后 hash 必变。

### B. Agent 缓存策略测试（`agent/prompt.rs` 或 `agent/tests.rs`）
- 使用自定义 `TrackerLlm`（`Arc<AtomicBool>` 记录 `clear_cache` / `set_prompt_cache_key` 调用）：
  - `test_cache_key_set_on_static_change`：第一次调用 `build_messages_with_cache` 时触发 `set_prompt_cache_key`。
  - `test_cache_key_skipped_when_static_unchanged`：第二次相同调用不触发。
  - `test_cache_key_not_set_for_uncapable_provider`：`prompt_caching: false` 的 provider 只触发 `clear_cache`，不触发 `set_prompt_cache_key`。

### C. Provider 序列化测试（`llm/mod.rs`）
- `test_prompt_cache_key_in_request_body`：构造 `OpenAiCompatibleLlm`，设置 cache key，执行 `complete()` 前拦截/检查序列化后的 `ChatCompletionRequest` JSON，断言包含 `"prompt_cache_key": "<hex>"`。

### D. Provider capabilities 测试（各 provider 测试模块）
- `test_deepseek_prompt_caching_capability` / `test_openai_prompt_caching_capability`：断言 `capabilities().prompt_caching == true`。
- `test_anthropic_prompt_caching_capability` / `test_ollama_prompt_caching_capability`：断言 `capabilities().prompt_caching == false`。

---

## 风险评估

| 风险 | 缓解 |
|---|---|
| Trait `&mut self` → `&self` 是 breaking change | 所有实现者在同一仓库，编译器会一次性暴露全部需修改点；无外部 crate 依赖该 trait。 |
| `prompt_cache_key` 字段并非 DeepSeek/OpenAI 官方标准字段 | 当前代码已将其作为 Kimi Code / 内部 API 的扩展字段存在；A2 只是将其值从 session UUID 升级为稳定 hash，不引入新字段。若未来对接官方 `cache_control`，可再封装。 |
| `Arc<RwLock<...>>` 引入轻微运行时开销 | 每请求仅一次写（hash 变化时），读操作在每次 `complete`/`stream` 序列化时发生，开销可忽略。 |

---

## 文件变更清单（预估）

1. `crates/clarity-contract/src/llm.rs` — trait 签名
2. `crates/clarity-core/src/llm/mod.rs` — `OpenAiCompatibleLlm` 内部可变性 + `capabilities` + 序列化读取
3. `crates/clarity-core/src/llm/deepseek.rs` — wrapper 透传签名
4. `crates/clarity-core/src/llm/llama_server.rs` — wrapper 透传签名
5. `crates/clarity-core/src/llm/ollama.rs` — trait 签名
6. `crates/clarity-core/src/llm/local_gguf.rs` — `cache_key` 内部可变性
7. `crates/clarity-core/src/llm/kalosm.rs` — trait 签名
8. `crates/clarity-core/src/llm/reliable.rs` — trait 签名 + 透传逻辑
9. `crates/clarity-core/src/agent/prompt.rs` — SHA-256 hash + `set_prompt_cache_key` 调用
10. `crates/clarity-core/src/agent/mod.rs` — `MockLlm` 签名
11. 各处测试 dummy providers — 同步签名

---

## 验收命令

```bash
cd dev/third_party/clarity
cargo test --workspace --lib
cargo check --workspace --lib
cargo clippy --workspace --lib --bins --tests
```
