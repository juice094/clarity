# Project Clarity 渠道集成架构设计

## 1. 执行摘要

本文档定义了 Project Clarity 的渠道集成架构，目标是支持 **15+ 平台**（超越 Nanobot 的 10+）。经过对 Nanobot 的深入研究，我们提出一个**模块化、异步优先、可扩展**的架构设计。

## 2. Nanobot 设计分析

### 2.1 Nanobot 架构概览

```
┌─────────────────────────────────────────────────────────────┐
│                     nanobot (single binary)                 │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │  Auth &     │  │  Agentic    │  │  Tool Runtime       │  │
│  │  Credits    │  │  Loop       │  │                     │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │  STT / TTS  │  │  Memory     │  │  Channel Adapters   │  │
│  │             │  │  Engine     │  │  (6 platforms)      │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                              │
        ┌─────────────────────┼─────────────────────┐
        ▼                     ▼                     ▼
   ┌─────────┐          ┌──────────┐         ┌────────────┐
   │Telegram │          │ Discord  │         │   Slack    │
   │  Bot    │          │ Gateway  │         │Socket Mode │
   └─────────┘          └──────────┘         └────────────┘
```

### 2.2 Nanobot Channels 目录结构

```
nanobot/
├── channels/               # 聊天平台集成
│   ├── telegram.py         # Telegram Bot API (long polling)
│   ├── discord.py          # Discord Gateway + REST
│   ├── slack.py            # Slack Socket Mode / HTTP
│   ├── line.py             # LINE Messaging API
│   ├── facebook.py         # Facebook Messenger
│   ├── whatsapp.py         # WhatsApp Web bridge
│   ├── feishu.py           # 飞书 WebSocket
│   ├── dingtalk.py         # 钉钉 Stream
│   ├── mochat.py           # Claw IM HTTP polling
│   ├── qq.py               # QQ Bot (optional feature)
│   └── __init__.py
├── bus/                    # 消息路由
├── config/schema.py        # 各渠道配置定义
└── ...
```

### 2.3 Nanobot 设计优点

1. **统一配置系统**: 所有渠道配置集中管理，环境变量自动映射
2. **协议适配灵活**: 支持 Webhook、WebSocket、长轮询多种方式
3. **消息格式统一**: 内部使用标准化的消息格式
4. **Session 跨渠道同步**: 用户在不同渠道的对话共享上下文
5. **Feature Flag 控制**: 可选编译特性（如 QQ、飞书 WebSocket）

### 2.4 Nanobot 设计可改进之处

| 问题 | 影响 | Clarity 改进方案 |
|------|------|------------------|
| 单一代码库 | 增加编译时间，渠道间耦合 | 独立 `clarity-channels` crate |
| 缺乏 Trait 抽象 | 新增渠道需修改多处 | 定义 `Channel` trait 统一接口 |
| 错误处理不一致 | 部分渠道 panic 而非返回错误 | 统一 `ChannelError` 类型 |
| 重连逻辑分散 | 每个渠道独立实现 | 抽象 `Transport` 层统一管理 |
| 限流策略缺失 | 容易被平台封禁 | 内置令牌桶限流器 |
| 消息格式转换硬编码 | 难以扩展新字段 | 使用 `serde` 中间表示 |

## 3. Clarity 渠道架构设计

### 3.1 架构决策：独立 Crate

我们选择 **独立 crate (`clarity-channels`)** 而非 monolithic 设计：

**理由：**
1. **编译隔离**: 修改单个渠道不触发全量重编译
2. **依赖精简**: 仅安装需要的渠道依赖
3. **版本独立**: 各渠道可独立迭代版本
4. **可选特性**: 通过 Cargo features 精确控制
5. **测试隔离**: 每个渠道独立测试套件

```
Workspace Structure:
┌────────────────────────────────────────────────────────────┐
│                      Project Clarity                       │
├────────────────────────────────────────────────────────────┤
│  clarity-gateway/          # 主网关，轻量依赖              │
│  ├─ Cargo.toml             # 仅依赖 clarity-channels       │
│  └─ src/main.rs                                              │
├────────────────────────────────────────────────────────────┤
│  clarity-channels/         # 渠道集成 crate                │
│  ├─ Cargo.toml             # 定义各渠道 feature            │
│  ├─ src/lib.rs             # 公共 trait 和类型             │
│  ├─ src/transport/         # 传输层抽象                    │
│  └─ src/adapters/          # 各渠道适配器                  │
├────────────────────────────────────────────────────────────┤
│  clarity-core/             # 核心业务逻辑                  │
└────────────────────────────────────────────────────────────┘
```

### 3.2 核心架构图

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                            Clarity Gateway                                   │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                      ChannelManager                                  │   │
│  │   ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                │   │
│  │   │  Telegram   │  │   Discord   │  │    Slack    │   ...          │   │
│  │   │  Adapter    │  │  Adapter    │  │  Adapter    │                │   │
│  │   └──────┬──────┘  └──────┬──────┘  └──────┬──────┘                │   │
│  └──────────┼────────────────┼────────────────┼────────────────────────┘   │
│             │                │                │                             │
│  ┌──────────▼────────────────▼────────────────▼────────────────────────┐   │
│  │                        Transport Layer                              │   │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐              │   │
│  │  │   Webhook    │  │  WebSocket   │  │ Long Polling │              │   │
│  │  │   Handler    │  │   Client     │  │   Client     │              │   │
│  │  └──────────────┘  └──────────────┘  └──────────────┘              │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                     Unified Message Bus                              │   │
│  │         ┌──────────────┐         ┌──────────────┐                   │   │
│  │         │   Inbound    │────────▶│   Outbound   │                   │   │
│  │         │   Pipeline   │         │   Pipeline   │                   │   │
│  │         └──────────────┘         └──────────────┘                   │   │
│  │                  │                        │                          │   │
│  │                  ▼                        ▼                          │   │
│  │         ┌──────────────┐         ┌──────────────┐                   │   │
│  │         │   Format     │         │   Format     │                   │   │
│  │         │   Converter  │         │   Converter  │                   │   │
│  │         └──────────────┘         └──────────────┘                   │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 3.3 协议差异处理

通过 `Transport` trait 抽象不同协议：

```rust
// Transport 层抽象
#[async_trait]
pub trait Transport: Send + Sync {
    type Config: TransportConfig;
    type Error: std::error::Error;

    /// 启动传输层
    async fn start(&self, handler: MessageHandler) -> Result<(), Self::Error>;
    
    /// 停止传输层
    async fn stop(&self) -> Result<(), Self::Error>;
    
    /// 发送消息
    async fn send(&self, message: OutboundMessage) -> Result<(), Self::Error>;
    
    /// 健康检查
    async fn health_check(&self) -> HealthStatus;
}

// Webhook 传输实现
pub struct WebhookTransport {
    config: WebhookConfig,
    axum_router: Option<Router>,
}

// WebSocket 传输实现  
pub struct WebSocketTransport {
    config: WebSocketConfig,
    ws_client: Option<WebSocketStream>,
    reconnect_policy: ReconnectPolicy,
}

// 长轮询传输实现
pub struct LongPollingTransport {
    config: PollingConfig,
    http_client: reqwest::Client,
    poll_interval: Duration,
}
```

### 3.4 统一消息格式

```rust
/// 渠道无关的统一消息格式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedMessage {
    /// 消息唯一标识
    pub id: MessageId,
    
    /// 渠道类型
    pub channel: ChannelType,
    
    /// 会话标识（跨渠道统一）
    pub session_id: SessionId,
    
    /// 发送者信息
    pub sender: SenderInfo,
    
    /// 接收者信息
    pub recipient: RecipientInfo,
    
    /// 消息内容
    pub content: MessageContent,
    
    /// 消息类型
    pub message_type: MessageType,
    
    /// 附件列表
    pub attachments: Vec<Attachment>,
    
    /// 回复链
    pub reply_to: Option<MessageId>,
    
    /// 元数据（渠道特定）
    pub metadata: serde_json::Value,
    
    /// 时间戳
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub enum MessageContent {
    Text { content: String, format: TextFormat },
    Image { url: Option<String>, data: Option<Vec<u8>>, mime_type: String },
    Audio { url: Option<String>, duration: Option<u32>, transcript: Option<String> },
    Video { url: Option<String>, duration: Option<u32>, thumbnail: Option<String> },
    File { name: String, size: u64, mime_type: String, url: Option<String> },
    Location { latitude: f64, longitude: f64, address: Option<String> },
    Template { template_id: String, params: HashMap<String, String> },
    Interactive { components: Vec<InteractiveComponent> },
}
```

### 3.5 Channel Trait 定义

```rust
/// 渠道适配器核心 Trait
#[async_trait]
pub trait Channel: Send + Sync {
    /// 渠道类型标识
    fn channel_type(&self) -> ChannelType;
    
    /// 渠道名称
    fn name(&self) -> &str;
    
    /// 初始化渠道
    async fn initialize(&mut self, config: ChannelConfig) -> ChannelResult<()>;
    
    /// 启动渠道监听
    async fn start(&self) -> ChannelResult<()>;
    
    /// 停止渠道
    async fn stop(&self) -> ChannelResult<()>;
    
    /// 发送消息
    async fn send_message(&self, message: OutboundMessage) -> ChannelResult<MessageId>;
    
    /// 编辑消息
    async fn edit_message(&self, message_id: MessageId, new_content: MessageContent) -> ChannelResult<()>;
    
    /// 删除消息
    async fn delete_message(&self, message_id: MessageId) -> ChannelResult<()>;
    
    /// 获取用户信息
    async fn get_user(&self, user_id: &str) -> ChannelResult<UserInfo>;
    
    /// 获取群组成员
    async fn get_chat_members(&self, chat_id: &str) -> ChannelResult<Vec<UserInfo>>;
    
    /// 设置消息处理器
    fn on_message<F>(&self, handler: F) where F: Fn(InboundMessage) + Send + Sync;
    
    /// 支持的能力检查
    fn capabilities(&self) -> ChannelCapabilities;
    
    /// 健康检查
    async fn health_check(&self) -> HealthStatus;
}

/// 渠道能力标志
#[derive(Debug, Clone, Copy, Default)]
pub struct ChannelCapabilities {
    pub supports_text: bool,
    pub supports_markdown: bool,
    pub supports_html: bool,
    pub supports_images: bool,
    pub supports_audio: bool,
    pub supports_video: bool,
    pub supports_files: bool,
    pub supports_locations: bool,
    pub supports_typing_indicator: bool,
    pub supports_read_receipt: bool,
    pub supports_reactions: bool,
    pub supports_threads: bool,
    pub supports_buttons: bool,
    pub supports_carousel: bool,
    pub max_message_length: Option<usize>,
    pub max_file_size: Option<u64>,
    pub rate_limit_per_second: Option<u32>,
}
```

## 4. 消息流转序列图

### 4.1 入站消息处理

```
┌──────────┐     ┌──────────────┐     ┌──────────────┐     ┌─────────────────┐
│ Telegram │     │   Telegram   │     │   Unified    │     │   Clarity       │
│  Server  │────▶│   Adapter    │────▶│   Converter  │────▶│   Core          │
└──────────┘     └──────────────┘     └──────────────┘     └─────────────────┘
      │                  │                     │                    │
      │ 1. Webhook POST  │                     │                    │
      │─────────────────▶│                     │                    │
      │                  │ 2. Verify signature │                    │
      │                  │ (HMAC verification) │                    │
      │                  │────────────────────▶│                    │
      │                  │                     │                    │
      │                  │ 3. Parse Telegram   │                    │
      │                  │    Message          │                    │
      │                  │─────────────────────│                    │
      │                  │                     │ 4. Convert to      │
      │                  │                     │    UnifiedMessage  │
      │                  │                     │───────────────────▶│
      │                  │                     │                    │
      │                  │                     │                    │ 5. Process
      │                  │                     │                    │    Message
      │                  │                     │                    │──────────▶
      │                  │                     │                    │
      │                  │ 6. Return 200 OK    │                    │
      │◀─────────────────│◀────────────────────│◀───────────────────│
```

### 4.2 出站消息处理

```
┌─────────────────┐     ┌──────────────┐     ┌──────────────┐     ┌──────────┐
│   Clarity       │     │   Unified    │     │   Discord    │     │ Discord  │
│   Core          │────▶│   Converter  │────▶│   Adapter    │────▶│  Server  │
└─────────────────┘     └──────────────┘     └──────────────┘     └──────────┘
         │                      │                     │                  │
         │ 1. Send Message      │                     │                  │
         │─────────────────────▶│                     │                  │
         │                      │ 2. Format for       │                  │
         │                      │    Discord          │                  │
         │                      │    (Markdown)       │                  │
         │                      │────────────────────▶│                  │
         │                      │                     │ 3. Send via      │
         │                      │                     │    REST API      │
         │                      │                     │─────────────────▶│
         │                      │                     │                  │
         │                      │                     │ 4. Return        │
         │                      │                     │    MessageId     │
         │◀─────────────────────│◀────────────────────│◀─────────────────│
```

## 5. 错误处理与重试策略

```rust
/// 渠道错误类型
#[derive(Debug, thiserror::Error)]
pub enum ChannelError {
    #[error("Authentication failed: {0}")]
    AuthenticationError(String),
    
    #[error("Rate limited, retry after {retry_after}s")]
    RateLimited { retry_after: u64 },
    
    #[error("Network error: {0}")]
    NetworkError(#[from] reqwest::Error),
    
    #[error("Invalid message format: {0}")]
    InvalidFormat(String),
    
    #[error("Message too large: {size} > {max}")]
    MessageTooLarge { size: usize, max: usize },
    
    #[error("Unsupported feature: {0}")]
    UnsupportedFeature(String),
    
    #[error("Channel not initialized")]
    NotInitialized,
    
    #[error("Transport error: {0}")]
    TransportError(String),
}

/// 重试策略
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
    pub exponential_base: f64,
    pub retryable_errors: Vec<ChannelErrorKind>,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(30),
            exponential_base: 2.0,
            retryable_errors: vec![
                ChannelErrorKind::NetworkError,
                ChannelErrorKind::RateLimited,
            ],
        }
    }
}
```

## 6. 限流与熔断

```rust
/// 渠道限流器
pub struct RateLimiter {
    /// 令牌桶
    bucket: Arc<Mutex<TokenBucket>>,
    /// 每渠道配置
    config: RateLimitConfig,
}

pub struct RateLimitConfig {
    /// 每秒请求数
    pub requests_per_second: u32,
    /// 突发容量
    pub burst_capacity: u32,
    /// 每分钟消息数
    pub messages_per_minute: u32,
}

/// 熔断器
pub struct CircuitBreaker {
    state: Arc<RwLock<CircuitState>>,
    config: CircuitBreakerConfig,
    failure_count: AtomicU32,
    last_failure_time: AtomicU64,
}

pub enum CircuitState {
    Closed,      // 正常
    Open,        // 熔断
    HalfOpen,    // 半开测试
}
```

## 7. 配置结构

```rust
/// 渠道配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChannelsConfig {
    /// 全局配置
    pub global: GlobalChannelConfig,
    
    /// 各渠道配置
    #[serde(default)]
    pub telegram: Option<TelegramConfig>,
    
    #[serde(default)]
    pub discord: Option<DiscordConfig>,
    
    #[serde(default)]
    pub slack: Option<SlackConfig>,
    
    #[serde(default)]
    pub feishu: Option<FeishuConfig>,
    
    #[serde(default)]
    pub whatsapp: Option<WhatsAppConfig>,
    
    #[serde(default)]
    pub line: Option<LineConfig>,
    
    // ... more channels
}

/// Telegram 配置示例
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TelegramConfig {
    pub enabled: bool,
    pub bot_token: String,
    pub webhook_url: Option<String>,
    pub webhook_secret: Option<String>,
    pub allowed_users: Option<Vec<String>>,
    pub polling_interval: Option<u64>,
    pub rate_limit: Option<RateLimitConfig>,
}
```

## 8. 监控与可观测性

```rust
/// 渠道指标
#[derive(Default)]
pub struct ChannelMetrics {
    /// 入站消息数
    pub inbound_messages: Counter,
    /// 出站消息数
    pub outbound_messages: Counter,
    /// 错误数
    pub errors: CounterVec,
    /// 消息处理延迟
    pub processing_duration: Histogram,
    /// 连接状态
    pub connection_state: Gauge,
    /// 活跃连接数
    pub active_connections: Gauge,
}

/// 追踪上下文
#[derive(Debug)]
pub struct ChannelSpan {
    pub channel_type: ChannelType,
    pub message_id: MessageId,
    pub session_id: SessionId,
    pub trace_id: String,
}
```

## 9. 安全考虑

1. **Webhook 签名验证**: 每个渠道的 Webhook 都需验证 HMAC 签名
2. **Token 管理**: 使用 secrecy crate 防止密钥泄露
3. **输入验证**: 所有入站消息进行内容验证
4. **沙箱隔离**: 文件下载在隔离目录进行
5. **审计日志**: 所有消息操作记录审计日志

---

*文档版本: 1.0*
*最后更新: 2026-04-03*
