# Clarity 移动端架构设计

> 目标：为 Clarity 构建 Android / iOS 移动端应用，优先 Android。
> 决策：路线 B（Rust 核心 + 原生 UI），先复用现有 Rust 引擎，通过 UniFFI 暴露给 Kotlin Compose / SwiftUI。

---

## 1. 决策摘要

| 维度 | 选择 | 理由 |
|------|------|------|
| 平台 | Android 优先，iOS 后续 | 你的环境已完整配置 Android 工具链；iOS 需要 macOS + Xcode，可后续补齐 |
| 技术路线 | B. Rust core + Native UI | 原生体验最好，符合 clarity "前端 crate 互不 import" 的不变量，syncthing-mobile 已验证该模式 |
| 移动端能力 | 对话 + 记忆同步 | 先验证核心循环可用性；文件/MCP/代码编辑等复杂工具后续按需下放 |
| LLM | 云端 API + Claw | 移动端不自建本地推理；通过 claw/gateway 联邦调用云端或桌面端算力 |

---

## 2. 可复用公共接口映射

移动端不重新实现 Agent 循环、记忆系统、LLM 调用。下表说明现有 crate 如何被复用。

| 移动端所需能力 | 复用的 crate / 模块 | 复用方式 |
|----------------|---------------------|----------|
| Agent 循环、ToolRegistry、Approval | `clarity-core::agent` | Rust 内部直接调用，不暴露给 FFI |
| 用户 ↔ Agent 事件协议 | `clarity-wire::{Wire, WireMessage}` | FFI 层消费事件流，序列化为移动端类型 |
| 事实/记忆持久化 | `clarity-memory::{MemoryStore, SessionStoreV2}` | Rust 内部初始化，移动端通过 FFI 查询 |
| 共享数据契约 | `clarity-contract::{Message, ToolCall, ApprovalMode}` | 作为 FFI 边界的参数类型 |
| LLM Provider | `clarity-llm::{KimiLlm, DeepSeekProvider, ...}` | Rust 内部构造，移动端只传 profile / API key |
| 联邦/远程任务 | `clarity-claw::{quick_chat, create_remote_task}` | 移动端可直接通过 HTTP 调用 Gateway |
| 配置加载 | `clarity-core::config::Config` | Rust 内部加载 TOML，移动端通过 FFI 读取/设置 |

> 原则：FFI 边界越薄越好。移动端的 Kotlin/Swift 只持有 `ClarityMobileRuntime` 句柄、接收 `UiEvent`、发送 `UserCommand`，不直接触碰 Agent 内部状态。

---

## 3. 总体架构

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                              Mobile UI Layer                                │
│  Android (Kotlin Compose)              │  iOS (SwiftUI)                       │
│  - ChatScreen                          │  - ChatView                          │
│  - ThreadListScreen                    │  - ThreadListView                    │
│  - SettingsScreen                      │  - SettingsView                      │
│  - ViewModel (StateFlow)               │  - ViewModel (ObservableObject)      │
└──────────────┬─────────────────────────┴──────────────┬──────────────────────┘
               │ UniFFI / Foreign Function Interface     │
               ▼                                         ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                         clarity-mobile-core (new crate)                     │
│  ┌─────────────┐   ┌─────────────┐   ┌─────────────┐   ┌─────────────────┐  │
│  │   Runtime   │   │    FFI      │   │   Config    │   │   SyncManager   │  │
│  │   (tokio)   │◄──│   Bridge    │◄──│   Adapter   │◄──│  (claw/gateway) │  │
│  └──────┬──────┘   └─────────────┘   └─────────────┘   └─────────────────┘  │
│         │                                                                    │
│         ▼                                                                    │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │                        Existing Clarity Stack                         │   │
│  │  clarity-core  →  clarity-wire  →  clarity-memory  →  clarity-llm   │   │
│  │  clarity-contract  →  clarity-claw  →  clarity-gateway              │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 4. 详细架构树

```
clarity/
├── crates/
│   ├── clarity-core/              # 已有：Agent 循环、配置、工具
│   ├── clarity-wire/              # 已有：WireMessage 事件协议
│   ├── clarity-memory/            # 已有：记忆/会话持久化
│   ├── clarity-contract/          # 已有：跨 crate 数据契约
│   ├── clarity-llm/               # 已有：LLM Provider 实现
│   ├── clarity-claw/              # 已有：联邦/远程任务
│   ├── clarity-gateway/           # 已有：HTTP API 网关
│   └── clarity-mobile-core/       # 新增：移动端 Rust FFI 包装
│       ├── Cargo.toml
│       ├── build.rs               # uniffi-build 生成绑定
│       ├── src/
│       │   ├── lib.rs             # 模块导出 + FFI 入口
│       │   ├── runtime.rs         # tokio Runtime + Agent 生命周期
│       │   ├── bridge.rs          # 供 UniFFI 调用的同步 API
│       │   ├── events.rs          # 移动端 UiEvent 映射
│       │   ├── commands.rs        # 移动端 UserCommand 映射
│       │   ├── config.rs          # 移动端配置读写
│       │   ├── memory.rs          # 记忆/会话查询接口
│       │   └── sync.rs            # claw/gateway 同步逻辑
│       └── clarity_mobile_core.udl # UniFFI 接口定义（或 proc-macro）
│
├── mobile/                        # 新增：移动端原生工程
│   ├── android/
│   │   ├── app/
│   │   │   ├── build.gradle.kts
│   │   │   ├── src/main/
│   │   │   │   ├── AndroidManifest.xml
│   │   │   │   ├── java/com/juice094/clarity/mobile/
│   │   │   │   │   ├── ClarityApplication.kt
│   │   │   │   │   ├── MainActivity.kt
│   │   │   │   │   ├── di/
│   │   │   │   │   │   └── AppModule.kt
│   │   │   │   │   ├── ui/
│   │   │   │   │   │   ├── theme/
│   │   │   │   │   │   │   ├── Color.kt
│   │   │   │   │   │   │   ├── Theme.kt
│   │   │   │   │   │   │   └── Type.kt
│   │   │   │   │   │   ├── components/
│   │   │   │   │   │   │   ├── ChatBubble.kt
│   │   │   │   │   │   │   ├── MessageInput.kt
│   │   │   │   │   │   │   ├── ThreadItem.kt
│   │   │   │   │   │   │   └── LoadingIndicator.kt
│   │   │   │   │   │   └── screens/
│   │   │   │   │   │       ├── ChatScreen.kt
│   │   │   │   │   │       ├── ThreadListScreen.kt
│   │   │   │   │   │       └── SettingsScreen.kt
│   │   │   │   │   └── viewmodel/
│   │   │   │   │       ├── ChatViewModel.kt
│   │   │   │   │       └── ThreadListViewModel.kt
│   │   │   │   └── jniLibs/           # cargo-ndk 编译产物 libclarity_mobile_core.so
│   │   │   │       └── arm64-v8a/
│   │   └── rust-bridge/
│   │       └── build.gradle.kts       # 调用 cargo-ndk 编译 Rust
│   │
│   ├── ios/
│   │   ├── ClarityMobile/
│   │   │   ├── ClarityMobileApp.swift
│   │   │   ├── Core/
│   │   │   │   ├── ClarityRuntime.swift     # UniFFI 生成类的包装
│   │   │   │   └── EventPublisher.swift
│   │   │   ├── UI/
│   │   │   │   ├── ChatView.swift
│   │   │   │   ├── ThreadListView.swift
│   │   │   │   └── SettingsView.swift
│   │   │   └── ViewModels/
│   │   │       ├── ChatViewModel.swift
│   │   │       └── ThreadListViewModel.swift
│   │   └── rust/
│   │       ├── build-ios.sh           # cargo build --target aarch64-apple-ios
│   │       └── ClarityMobileCoreFFI.modulemap
│   │
│   └── shared/
│       └── wire-schema/
│           └── wire_message.json      # 可选：WireMessage JSON Schema 供类型生成
│
└── docs/
    └── mobile-architecture.md         # 本文档
```

---

## 5. FFI 桥接层设计（clarity-mobile-core）

### 5.1 核心原则

- **同步 FFI**：UniFFI 对 async 支持有限，Rust 侧用 `tokio::runtime::Handle` 将异步调用 `block_on`。
- **事件流**：移动端启动一个后台线程/协程循环调用 `runtime.poll_event()`，阻塞直到新事件到达。
- **无复杂类型穿越**：所有复杂对象在 Rust 侧持有 `Arc<Mutex<T>>`，FFI 只传 `u64` 句柄或 JSON 字符串。

### 5.2 推荐的 UniFFI 接口

```udl
// clarity-mobile-core.udl
namespace clarity_mobile_core {
    // 初始化与生命周期
    RuntimeHandle runtime_create(Config config);
    void runtime_destroy(RuntimeHandle handle);
    string runtime_version();

    // 用户命令（UI → Rust）
    void send_message(RuntimeHandle handle, string thread_id, string content);
    string create_thread(RuntimeHandle handle, string? title);
    void switch_thread(RuntimeHandle handle, string thread_id);
    void delete_thread(RuntimeHandle handle, string thread_id);
    void stop_generation(RuntimeHandle handle);
    void update_profile(RuntimeHandle handle, ProviderProfile profile);

    // 事件流（Rust → UI）
    UiEvent? poll_event(RuntimeHandle handle, u64 timeout_ms);

    // 记忆查询
    sequence<Fact> search_facts(RuntimeHandle handle, string query, u32 limit);
    sequence<ThreadSummary> list_threads(RuntimeHandle handle);
};

// 配置
dictionary Config {
    string data_dir;
    string? default_profile;
    sequence<ProviderProfile> profiles;
    boolean sync_with_gateway;
    string? gateway_url;
};

dictionary ProviderProfile {
    string name;
    string provider;        // "kimi", "openai", "deepseek", "anthropic"
    string model;
    string? api_key;
    string? base_url;
};

// 事件枚举（精简自 WireMessage）
[Enum]
interface UiEvent {
    TurnBegin(string turn_id, string user_input);
    ContentPart(string turn_id, string text);
    ToolCall(string turn_id, string id, string name, string arguments_json);
    ToolResult(string turn_id, string id, string result_json);
    TurnEnd(string turn_id);
    Usage(string turn_id, u32 prompt_tokens, u32 completion_tokens);
    StatusUpdate(string turn_id, string message);
    ThreadActive(string thread_id, string? title);
    ThreadList(sequence<ThreadSummary> threads);
    Error(string code, string message);
};

// 线程摘要
dictionary ThreadSummary {
    string thread_id;
    string? title;
    string updated_at;
};

// 事实
dictionary Fact {
    i64 id;
    string fact;
    sequence<string> tags;
    string? session_id;
    string created_at;
};
```

### 5.3 Rust 侧伪代码

```rust
// src/bridge.rs
use std::sync::Arc;
use uniffi;
use parking_lot::Mutex;
use tokio::runtime::Runtime;

pub struct RuntimeHandle {
    pub rt: Arc<Runtime>,
    pub inner: Arc<Mutex<MobileRuntime>>,
}

#[uniffi::export]
impl RuntimeHandle {
    pub fn send_message(&self, thread_id: String, content: String) -> Result<(), MobileError> {
        let inner = self.inner.clone();
        self.rt.block_on(async move {
            inner.lock().send_message(&thread_id, &content).await
        })
    }

    pub fn poll_event(&self, timeout_ms: u64) -> Option<UiEvent> {
        let inner = self.inner.clone();
        self.rt.block_on(async move {
            tokio::time::timeout(
                Duration::from_millis(timeout_ms),
                inner.lock().ui_receiver.recv()
            ).await.ok().flatten()
        })
    }

    // ... 其他方法
}
```

---

## 6. 移动端数据流

### 6.1 发送消息

```text
User types → Compose TextField
       │
       ▼
ChatViewModel.sendMessage(threadId, text)
       │
       ▼
RuntimeHandle.send_message()  [UniFFI / JNI]
       │
       ▼
MobileRuntime.send_message()  [Rust]
       │
       ▼
Agent.run(&prompt)            [clarity-core]
       │
       ▼
LLM stream → WireMessage → WireUISide
       │
       ▼
MobileRuntime 将 WireMessage 映射为 UiEvent
       │
       ▼
poll_event() 返回 UiEvent 到 Kotlin
       │
       ▼
ChatViewModel 更新 StateFlow → Compose 重绘
```

### 6.2 事件消费模型

Android 侧启动一个 `clarityEventLoop` 协程：

```kotlin
viewModelScope.launch(Dispatchers.IO) {
    while (isActive) {
        val event = runtime.pollEvent(timeoutMs = 5000)
        if (event != null) {
            _uiEvents.send(event)
        }
    }
}
```

> 注意：`poll_event` 是阻塞调用，必须放在 `Dispatchers.IO`；每次超时后重新 poll，确保应用切到后台时不会无限阻塞。

---

## 7. 存储与配置策略

### 7.1 本地存储

| 数据 | 位置 | 实现 |
|------|------|------|
| 会话消息 | `SessionStoreV2` (SQLite) | 复用 `clarity-memory::session_store_v2` |
| 记忆事实 | `MemoryStore` (SQLite FTS5) | 复用 `clarity-memory::MemoryStore` |
| 用户配置 | TOML / DataStore | Rust 侧写 `config.toml`，Kotlin 侧用 `DataStore` 做 UI 缓存 |
| 附件/图片 | 应用私有目录 | 移动端自己管理，路径传给 Rust |

### 7.2 目录约定

```text
Android:  /data/data/com.juice094.clarity.mobile/files/clarity/
iOS:      ~/Library/Application Support/com.juice094.clarity.mobile/

.clarity/
├── config.toml
├── sessions_v2.sqlite
├── memory.sqlite
└── cache/
```

### 7.3 与桌面端同步（可选 v1.1）

- 通过 `clarity-gateway` 的 `/api/v2/threads` 和 `/v1/chat/completions` 接口。
- 移动端作为 "claw node" 注册，拉取/推送会话快照。
- 初始 MVP 可先不同步，只做本地对话。

---

## 8. 与 Claw / Gateway 的关系

你提到 "云端 API + claw"。移动端有两种用法：

### 8.1 直连模式（MVP）

移动端自己持有 API key，直接调用 `clarity-llm` 中的 provider。

```text
Mobile → clarity-mobile-core → clarity-llm → Kimi/OpenAI/DeepSeek API
```

### 8.2 联邦模式（后续）

移动端只作为 thin client，复杂推理交给桌面端或私有网关。

```text
Mobile → clarity-claw::quick_chat(gateway_url)
              │
              ▼
         clarity-gateway → clarity-core (桌面端或服务器)
              │
              ▼
         云端 LLM API
```

> 联邦模式适合：不想在手机上存 API key、需要复用桌面端已配置的环境、或需要调用桌面端工具。

---

## 9. 实施路线图

### Phase 0：基础设施（1-2 周）

- [ ] 新建 `crates/clarity-mobile-core`
- [ ] 引入 `uniffi` 依赖，配置 `build.rs`
- [ ] 配置 `cargo-ndk` 交叉编译 Android `aarch64-linux-android`
- [ ] 验证 Kotlin 能成功调用 `runtime_version()`

### Phase 1：最小对话循环（2-3 周）

- [ ] FFI 实现 `send_message` + `poll_event`
- [ ] Android Compose 实现 ChatScreen
- [ ] 集成 `clarity-core` 的 `Agent` + `clarity-llm` provider
- [ ] 支持文字流式输出（ContentPart / TurnEnd）

### Phase 2：线程与记忆（2 周）

- [ ] FFI 暴露 `list_threads` / `create_thread` / `switch_thread`
- [ ] ThreadListScreen
- [ ] 接入 `SessionStoreV2` 持久化
- [ ] 接入 `MemoryStore` 事实查询

### Phase 3：配置与多 Provider（1-2 周）

- [ ] SettingsScreen：选择 provider、输入 API key、选择模型
- [ ] 安全存储 API key（Android Keystore / iOS Keychain）
- [ ] 配置从 Rust TOML 与移动端 DataStore 双向同步

### Phase 4：iOS 与联邦同步（后续）

- [ ] iOS SwiftUI 版本
- [ ] 通过 `clarity-gateway` 与桌面端会话同步
- [ ] 推送通知（claw 任务完成）

---

## 10. 风险与缓解

| 风险 | 影响 | 缓解 |
|------|------|------|
| UniFFI async 支持复杂 | 中 | FFI 层全部同步化，内部 `block_on`；事件流用 poll 模型 |
| `clarity-core` 依赖文件系统路径 | 中 | 移动端提供 app 私有目录，禁止访问外部存储 |
| Android NDK / JNI 编译链复杂 | 中 | 用 `cargo-ndk` + `rust-bridge` Gradle 插件自动化 |
| API key 本地存储安全 | 高 | Android Keystore / iOS Keychain；Rust 侧不持久化明文 |
| 包体积过大 | 中 | 启用 LTO、strip symbols、按 ABI 分包；后续可用 `cargo profile` |
| egui 路线诱惑 | 低 | 已决策路线 B，MVP 期间不回头 |

---

## 11. 下一步建议

1. 在 `clarity/crates/` 下创建 `clarity-mobile-core`，写好 `Cargo.toml` 与最小 FFI。
2. 在 `clarity/mobile/android/` 下创建空 Android 项目，配置 Gradle 调用 `cargo-ndk`。
3. 跑通第一个端到端用例：Kotlin 按钮点击 → Rust 返回 `"clarity-mobile-core v0.3.0"`。
4. 再接入 `Agent::run` 实现第一个对话。

---

*文档版本：v1.0*  
*更新日期：2026-06-19*
