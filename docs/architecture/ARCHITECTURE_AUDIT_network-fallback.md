---
title: 架构审计：网络探测 → Provider Fallback 设计缺陷
category: Architecture
date: 2026-05-16
tags: [architecture]
---

# 架构审计：网络探测 → Provider Fallback 设计缺陷

> 触发事件：中国网络环境下 `1.1.1.1:443` 探测超时 → `ensure_llm` 强制切 local → local 模型加载失败
> 修复版本：`69e10356`（cloud-first fallback）
> 审计日期：2026-04-28

---

## 一、故障链复盘

```
app_logic.rs:29   check_network("1.1.1.1:443") → TcpStream::connect timeout
       ↓
app_logic.rs:32   network_available.store(false)
       ↓
app_logic.rs:34   prewarm_llm() → ensure_llm()
       ↓
app_state.rs:96   !network_available && provider != "local"
       ↓
app_state.rs:101  desired_provider = "local"  ← 用户配置被静默覆盖
       ↓
app_state.rs:106  resolve_local_model_path() → ~/models/*.gguf
       ↓
local_gguf.rs:808 gguf_file::Content::read() → "failed to fill whole buffer"
```

**表面症状**：本地模型加载失败  
**实际根因**：网络探测误报导致用户显式配置被隐式篡改  
**暴露盲区**：架构层缺乏"探测→决策→执行"的分离，以及 fallback 的显式契约

---

## 二、设计缺陷分析（工程理论视角）

### 2.1 隐式状态变更（Silent State Mutation）

**违背原则**：
- **PEP 20** — "Explicit is better than implicit"
- **Postel 定律** — 保守发送、自由接收；但这里不是协议解析，是用户意图被覆盖
- **最小惊讶原则**（Principle of Least Astonishment）— 用户选 deepseek，系统 silently 切到 local

**代码体现**：
```rust
let desired_provider = if !network_available && settings.provider != "local" {
    "local".to_string()   // 用户配置被覆盖，无通知
} else {
    settings.provider.clone()
};
```

**行业标准**：
- Kubernetes `PodDisruptionBudget` — 驱逐前必须显式检查，不会静默迁移
- Envoy `outlier_detection` — 失败阈值达到后才移除 endpoint，且事件可观测
- AWS Auto Scaling — 伸缩动作通过 CloudWatch Event 暴露，不会静默变更

### 2.2 单点探测 = 单点故障

**违背原则**：
- **容错设计** — 任何单一探针都不应作为决策的唯一依据
- **拜占庭容错**（Byzantine Fault Tolerance）— 探针本身可能撒谎（被墙、被劫持）

**代码体现**：
```rust
// 一个 TCP 连接的超时就判定整个网络不可用
let available = check_network("1.1.1.1:443").await;
```

**行业标准**：
- **Prometheus blackbox_exporter** — 多 target、多 protocol、重试、alertmanager 聚合
- **Istio health check** — HTTP/HTTPS/TCP、gRPC、自定义 header、多次失败才判定
- **Chrome Network Status API** — 综合 DNS、TCP、HTTP 多层探测，且只作为 UI 提示

### 2.3 过早 fallback（Premature Fallback）

**违背原则**：
- **Circuit Breaker 模式** — 熔断应在"尝试并失败"后触发，而不是"预判失败"
- **Fail-Fast** — 让 cloud provider 的请求自然失败，错误信息更真实

**代码体现**：
```rust
// 在发请求之前就决定了 fallback
if !network_available { fallback_to_local() }
// 正确的顺序：try_cloud() → Err(_) → try_local()
```

**行业标准**：
- **Hystrix** — 熔断基于实际请求的统计（成功率、延迟），不是前置探测
- **Resilience4j** — `Retry` + `CircuitBreaker` + `Fallback` 是链式组合，不是条件分支
- **gRPC** — 连接失败才触发 Name Resolution 重试，不是预检

### 2.4 God Function / 大泥球（Big Ball of Mud）

**违背原则**：
- **单一职责**（SRP）— `ensure_llm` 同时承担：网络状态读取、provider 选择、模型加载、LLM 绑定
- **关注点分离**（SoC）— 探测、决策、执行、反馈应分属不同模块

**代码体现**：
```rust
pub async fn ensure_llm(state: &AppState) -> Result<(), EguiError> {
    // 1. 读取 settings
    // 2. 读取网络状态
    // 3. 决定 provider（含 fallback 逻辑）
    // 4. 加载 local 模型 / 创建 cloud provider
    // 5. 绑定到 Agent
    // 6. 更新全局状态
}
```

**行业标准**：
- **Clean Architecture** — Use Case 层只做编排，具体逻辑交给 Entity/Interactor
- **DDD** — `LLMProviderSelectionPolicy` 应是一个独立的 Domain Service
- **Kimi CLI 对比** — `kosong/chat_provider` 是独立模块，`soul/agent.py` 只做调用，不做探测决策

### 2.5 错误掩盖（Error Masking）

**违背原则**：
- **错误传播透明性** — 原始错误应被保留，而不是被替换
- **因果链完整性** — 调试时应能看到"A 导致 B 导致 C"

**代码体现**：
```
实际因果：1.1.1.1 超时 → 被切到 local → GGUF 解析失败
用户看到：Failed to load local model: failed to fill whole buffer
丢失信息："你原本选的是 deepseek，但被系统改成了 local"
```

**行业标准**：
- **Rust `anyhow::Error`** — `#[source]` 保留因果链
- **Go `errors.Is` / `errors.As`** — 错误包装不丢失原始信息
- **OpenTelemetry** — Span Event 记录完整决策链路

---

## 三、上下游关系与影响面

```
┌─────────────────────────────────────────────────────────────┐
│ 上游触发层                                                    │
│  ├── app_logic.rs:22-38   网络监控任务（每 30s 探测）        │
│  ├── AppState::default()   初始化 cached_settings            │
│  └── SettingsPanel          用户配置 provider/model/key      │
├─────────────────────────────────────────────────────────────┤
│ 决策层（盲区所在）                                            │
│  ├── ensure_llm()           God Function: 探测→选择→加载→绑定 │
│  ├── network_available      AtomicBool，全局可变状态         │
│  └── binding_matches()      缓存命中判断（但 key 是 provider+path）│
├─────────────────────────────────────────────────────────────┤
│ 下游执行层                                                    │
│  ├── Agent::set_llm()       无返回值，无错误传播             │
│  ├── Agent::run() / plan()  实际对话入口                     │
│  └── execute_tool_call()    工具调用（依赖已绑定的 LLM）     │
├─────────────────────────────────────────────────────────────┤
│ 副作用层                                                     │
│  ├── llm_binding            Mutex<Option<LlmBinding>>       │
│  ├── prewarm_error          Mutex<Option<String>>           │
│  └── Toast/Error Banner     UI 反馈（只显示最终错误）        │
└─────────────────────────────────────────────────────────────┘
```

**关键问题**：决策层（`ensure_llm`）同时依赖上游状态（`network_available`）和下游副作用（`llm_binding`），形成了**循环依赖**的架构 smell。

---

## 四、与竞品的架构对比

| 维度 | Clarity（修复前） | Kimi CLI | OpenAI SDK | Anthropic SDK | 行业最佳实践 |
|------|------------------|----------|------------|---------------|-------------|
| **网络探测** | 单 TCP 探针，硬编码 1.1.1.1 | 无探测，直接请求 | 无探测 | 无探测 | 多层 HTTP probe，可配置 |
| **fallback 触发** | 探测失败就 fallback | 无 fallback，直接报错 | 无 fallback | 无 fallback | 请求失败统计后熔断 |
| **配置覆盖** | 静默覆盖用户配置 | 尊重用户配置 | 尊重用户配置 | 尊重用户配置 | 显式策略配置 |
| **错误链** | 掩盖原始错误 | 完整错误传播 | 完整异常抛出 | 完整异常抛出 | `#[source]` 因果链 |
| **关注点分离** | God Function | 模块化（kosong/soul/wire） | 单一客户端 | 单一客户端 | Clean Architecture |

---

## 五、长期架构改进建议

### 5.1 短期（已修复）

- ✅ `ensure_llm` 改为 cloud-first：先尝试 cloud，失败后才 fallback
- ✅ 本地模型路径验证 `.gguf`
- ✅ DeepSeek `thinking` 支持

### 5.2 中期

1. **网络探测重构**
   ```rust
   // 建议：多探针 + 可配置 + 只做提示
   pub struct NetworkProbeConfig {
       pub endpoints: Vec<String>,      // 默认 ["api.deepseek.com:443", "www.baidu.com:443"]
       pub timeout: Duration,           // 默认 3s
       pub interval: Duration,          // 默认 30s
       pub threshold: u32,              // 连续失败次数阈值
   }
   // 探测结果只用于 UI 提示（banner），不作为决策依据
   ```

2. **Provider Selection Policy 抽象**
   ```rust
   pub trait ProviderSelectionPolicy {
       async fn select(&self, settings: &GuiSettings) -> ProviderSelection;
   }

   pub enum ProviderSelection {
       Preferred { provider: String },
       Fallback { preferred: String, fallback: String },
       AskUser { options: Vec<String> },
   }
   ```

3. **LLM Provider 健康检查**
   - 不依赖网络探测，而是直接对 provider endpoint 发 lightweight 请求（如 `{"max_tokens": 1}`）
   - 记录成功率、延迟，作为 fallback 的数据依据

### 5.3 长期

1. **状态机建模**
   ```
   LLMState:
     Uninitialized → Loading → Ready | Failed
     Ready → Reloading → Ready | Failed
     Failed → Retrying → Ready | LocalFallback
   ```

2. **事件驱动架构**
   - `NetworkStatusChanged { available: bool }`
   - `ProviderLoadFailed { provider: String, error: Error }`
   - `FallbackTriggered { from: String, to: String, reason: String }`
   - UI 订阅这些事件，给用户显式反馈

3. **配置即代码**
   - 参考 Kimi CLI 的 TOML 配置：用户显式定义 `fallback_provider`、`offline_strategy`
   - 不允许系统静默覆盖用户选择

---

## 六、核心结论

这个 bug 不是"网络探测地址选错了"这么简单。它是一个**架构设计模式缺陷**的实例：

> **把"可用性探测"当作"决策依据"，把"防御性编程"做成了"隐式状态机"。**

探测层应该只提供**信号**（signal），决策层应该基于**用户意图 + 实际尝试结果**做选择，执行层应该**透明传播错误**。三层混在一起，就产生了这个看似"模型加载失败"、实际是"配置被静默覆盖"的诡异故障。

修复后的 `ensure_llm` 虽然解决了眼前问题，但架构层仍需要中长期的重构，把探测、策略、执行彻底分离。
