# Project Clarity 渠道实现路线图

## 1. 优先级总览

| 优先级 | 渠道 | 协议 | 难度 | 预估工时 |
|--------|------|------|------|----------|
| P0 | Telegram | Long Polling / Webhook | 低 | 2d |
| P0 | Discord | Gateway WebSocket | 中 | 3d |
| P0 | Slack | Socket Mode | 中 | 3d |
| P1 | Feishu (飞书) | WebSocket | 中 | 2d |
| P1 | WhatsApp | Cloud API | 中 | 3d |
| P2 | LINE | Webhook | 低 | 2d |
| P2 | WeChat Work | Webhook | 中 | 3d |
| P2 | DingTalk (钉钉) | Stream | 中 | 2d |
| P3 | Facebook Messenger | Webhook | 低 | 2d |
| P3 | Email (IMAP/SMTP) | Polling | 中 | 3d |
| P3 | QQ | WebSocket | 高 | 5d |
| P4 | Microsoft Teams | Graph API | 中 | 4d |
| P4 | Google Chat | Webhook | 低 | 2d |
| P4 | Matrix | Client-Server API | 中 | 3d |
| P4 | Web (SSE/WebSocket) | WebSocket | 中 | 2d |

---

## 2. P0 优先级：核心渠道（前5个）

### 2.1 Telegram - 最先实现

**为什么优先：**
- 全球最大的 IM 平台之一（8亿+用户）
- Bot API 简单稳定，文档完善
- Rust 生态有成熟的 `teloxide` 库
- Webhook 和长轮询都支持
- 开发测试成本低

**技术选型：**
| 组件 | 选型 | 版本 | 理由 |
|------|------|------|------|
| SDK | teloxide | 0.15+ | 功能完整，社区活跃 |
| HTTP | reqwest | 0.12+ | teloxide 内部使用 |
| 解析 | serde | 1.0+ | 标准选择 |

**依赖配置：**
```toml
[dependencies]
teloxide = { version = "0.15", features = ["macros", "webhooks-axum"] }
reqwest = { version = "0.12", features = ["json"] }
serde = { version = "1.0", features = ["derive"] }
```

**实现要点：**
```rust
use teloxide::{prelude::*, types::Message};

pub struct TelegramChannel {
    bot: Bot,
    config: TelegramConfig,
}

#[async_trait]
impl Channel for TelegramChannel {
    fn channel_type(&self) -> ChannelType {
        ChannelType::Telegram
    }
    
    async fn start(&self) -> ChannelResult<()> {
        // 支持长轮询或 Webhook
        match &self.config.webhook_url {
            Some(url) => self.start_webhook(url).await,
            None => self.start_polling().await,
        }
    }
    
    async fn send_message(&self, msg: OutboundMessage) -> ChannelResult<MessageId> {
        // 处理消息分块（Telegram 限制 4096 字符）
        let chunks = split_message(&msg.content, 4096);
        for chunk in chunks {
            self.bot.send_message(chat_id, chunk).await?;
        }
        Ok(message_id)
    }
}
```

**风险与缓解：**
- 国内访问需要代理 → 配置 SOCKS5/HTTP 代理支持
- 文件大小限制 20MB → 分片上传或大文件转链接

---

### 2.2 Discord - 企业/社区必备

**为什么优先：**
- 全球最大的社区平台
- 开发者群体集中
- 支持丰富的消息格式（Embed、按钮、菜单）
- Socket Mode 无需公网 IP

**技术选型：**
| 组件 | 选型 | 版本 | 理由 |
|------|------|------|------|
| SDK | serenity | 0.12+ | batteries-included，易用 |
| 备选 | twilight | 0.16+ | 模块化，适合高级定制 |

**对比分析：**
```
serenity vs twilight:
- serenity: 上手快，功能全，文档好，适合快速开发
- twilight: 模块化，可定制，适合大型项目

Clarity 选择 serenity，原因：
1. 减少样板代码
2. 内置命令框架
3. 活跃的社区支持
```

**依赖配置：**
```toml
[dependencies]
serenity = { version = "0.12", default-features = false, features = [
    "client",
    "gateway",
    "rustls_backend",
    "model",
    "cache",
] }
tokio = { version = "1.40", features = ["rt-multi-thread"] }
```

**实现要点：**
```rust
use serenity::{
    client::{Client, Context, EventHandler},
    model::{channel::Message, gateway::Ready},
};

pub struct DiscordChannel {
    client: Client,
    config: DiscordConfig,
}

#[async_trait]
impl EventHandler for DiscordHandler {
    async fn message(&self, ctx: Context, msg: Message) {
        // 转换为 UnifiedMessage
        let unified = self.convert_message(msg).await;
        self.tx.send(unified).await.ok();
    }
}

impl DiscordChannel {
    /// 支持消息分块（Discord 限制 2000 字符）
    async fn send_chunked(&self, channel_id: ChannelId, content: &str) {
        for chunk in content.chars().collect::<Vec<_>>().chunks(2000) {
            let text: String = chunk.iter().collect();
            channel_id.say(&self.http, text).await.ok();
        }
    }
}
```

**特性支持：**
- ✅ 文本消息
- ✅ Embed 消息
- ✅ 按钮和交互组件
- ✅ 线程消息
- ✅ 回复引用
- ✅ 表情反应
- ✅ 打字指示器

---

### 2.3 Slack - 企业办公首选

**为什么优先：**
- 企业级市场领导者
- Socket Mode 无需公网 IP
- Block Kit 强大的消息格式
- 丰富的应用生态

**技术选型：**
| 组件 | 选型 | 版本 | 理由 |
|------|------|------|------|
| SDK | slack-morphism-rust | 2.8+ | 官方推荐，功能完整 |
| HTTP | hyper | 1.0+ | 底层支持 |
| WebSocket | tokio-tungstenite | 0.24+ | 标准选择 |

**依赖配置：**
```toml
[dependencies]
slack-morphism = { version = "2.8", features = ["hyper", "axum"] }
slack-morphism-hyper = "2.8"
tokio-tungstenite = "0.24"
```

**协议选择：**
```
Socket Mode vs HTTP Mode:

Socket Mode (推荐):
- 优点: 无需公网 IP，自动重连，适合自托管
- 缺点: 依赖 WebSocket，稍微复杂

HTTP Mode:
- 优点: 简单，稳定，适合云部署
- 缺点: 需要公网 HTTPS 端点

Clarity 默认 Socket Mode，可选 HTTP Mode
```

**实现要点：**
```rust
use slack_morphism::{
    prelude::*,
    socket_mode::{SlackSocketModeListener, SlackSocketModeListeners},
};

pub struct SlackChannel {
    client: SlackClient,
    listener: Option<SlackSocketModeListener>,
}

#[async_trait]
impl Channel for SlackChannel {
    async fn start(&self) -> ChannelResult<()> {
        let listener = SlackSocketModeListener::new(
            &self.config.app_token,
            self.build_listener_callbacks(),
        );
        listener.serve().await.map_err(|e| e.into())
    }
    
    async fn send_message(&self, msg: OutboundMessage) -> ChannelResult<MessageId> {
        // Slack Block Kit 格式转换
        let blocks = self.convert_to_blocks(&msg.content);
        let post_msg_req = SlackApiChatPostMessageRequest::new(
            msg.channel_id.into(),
            msg.content.to_string(),
        ).with_blocks(blocks);
        
        let response = self.client.chat_post_message(&post_msg_req).await?;
        Ok(response.ts.into())
    }
}
```

**特性支持：**
- ✅ Socket Mode
- ✅ Block Kit 消息
- ✅ 斜杠命令
- ✅ 快捷菜单
- ✅ 模态对话框
- ✅ 富文本格式

---

### 2.4 Feishu (飞书) - 国内市场必备

**为什么优先：**
- 国内头部企业协作平台
- 字节跳动生态支持
- API 完善，WebSocket 稳定
- 国内用户基数大

**技术选型：**
| 组件 | 选型 | 版本 | 理由 |
|------|------|------|------|
| SDK | open-lark | 0.15+ | 最完整的 Rust SDK |
| 备选 | larkrs-client | 0.1+ | 轻量级备选 |
| WebSocket | tokio-tungstenite | 0.24+ | 标准选择 |

**依赖配置：**
```toml
[dependencies]
open-lark = { version = "0.15", default-features = false, features = [
    "auth",
    "communication",
    "webhook-card",
] }
tokio-tungstenite = "0.24"
```

**实现要点：**
```rust
use open_lark::{
    client::Client,
    service::im::IMService,
};

pub struct FeishuChannel {
    client: Client,
    ws_client: Option<FeishuWebSocketClient>,
}

impl FeishuChannel {
    /// 飞书支持事件订阅和 WebSocket 两种模式
    /// 推荐 WebSocket 模式，无需公网 IP
    async fn start_websocket(&self) -> ChannelResult<()> {
        let ws_client = FeishuWebSocketClient::new(
            self.config.app_id.clone(),
            self.config.app_secret.clone(),
        );
        
        ws_client.on_event(move |event| {
            Box::pin(async move {
                self.handle_event(event).await;
            })
        }).await;
        
        Ok(())
    }
    
    /// 消息卡片格式转换
    async fn send_card_message(&self, msg: OutboundMessage) -> ChannelResult<()> {
        let card = json!({
            "config": { "wide_screen_mode": true },
            "elements": self.convert_content_to_elements(&msg.content),
        });
        
        self.client.im().send_card_message(
            &msg.recipient.id,
            card,
        ).await.map_err(|e| e.into())
    }
}
```

**风险与缓解：**
- 国内访问需考虑备案 → 支持代理配置
- 消息卡片格式复杂 → 提供 Builder 模式

---

### 2.5 WhatsApp - 全球消息之王

**为什么优先：**
- 全球 20亿+ 用户
- 官方 Cloud API 稳定
- 商务场景刚需
- 支持模板消息

**技术选型：**
| 组件 | 选型 | 版本 | 理由 |
|------|------|------|------|
| SDK | whatsapp-business-rs | 0.2+ | 专为 Business API 设计 |
| HTTP | reqwest | 0.12+ | 异步 HTTP |
| Webhook | axum | 0.8+ | 内置 webhook 服务器 |

**依赖配置：**
```toml
[dependencies]
whatsapp-business-rs = { version = "0.2", features = ["server"] }
reqwest = { version = "0.12", features = ["json"] }
axum = "0.8"
```

**实现要点：**
```rust
use whatsapp_business_rs::{
    client::Client,
    server::{Server, WebhookHandler, IncomingMessage},
};

pub struct WhatsAppChannel {
    client: Client,
    server: Option<Server>,
}

#[async_trait]
impl WebhookHandler for WhatsAppHandler {
    async fn handle_message(&self, ctx: EventContext, msg: IncomingMessage) {
        // WhatsApp 支持多种消息类型
        let unified = match msg.message_type {
            MessageType::Text(text) => self.convert_text(text),
            MessageType::Image(image) => self.convert_media(image).await,
            MessageType::Audio(audio) => self.convert_audio(audio).await,
            MessageType::Document(doc) => self.convert_document(doc).await,
            _ => UnifiedMessage::default(),
        };
        
        self.tx.send(unified).await.ok();
    }
}

impl WhatsAppChannel {
    /// 发送模板消息（WhatsApp 强制要求首次交互使用模板）
    async fn send_template(
        &self,
        to: &str,
        template_name: &str,
        params: HashMap<String, String>,
    ) -> ChannelResult<MessageId> {
        self.client
            .message(&self.config.phone_number_id)
            .send_template(to, template_name, params)
            .await
            .map_err(|e| e.into())
    }
}
```

**限制与处理：**
- 必须使用模板消息发起对话 → 内置模板管理
- 媒体文件需先上传 → 异步媒体处理器
- 24小时会话窗口 → 会话状态追踪

---

## 3. 技术选型汇总

### 3.1 核心依赖

```toml
[dependencies]
# 异步运行时
tokio = { version = "1.40", features = ["rt-multi-thread", "macros", "sync"] }

# 序列化
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# HTTP 客户端
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }

# Web 框架（用于 Webhook）
axum = "0.8"
tower = "0.5"

# WebSocket
tokio-tungstenite = "0.24"

# 工具
async-trait = "0.1"
thiserror = "2.0"
anyhow = "1.0"
tracing = "0.1"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1.11", features = ["v4", "serde"] }
dashmap = "6.1"

# 安全配置
secrecy = "0.10"
```

### 3.2 各渠道 SDK 版本建议

| 渠道 | Crate | 版本 | 特性 |
|------|-------|------|------|
| Telegram | teloxide | 0.15 | macros, webhooks-axum |
| Discord | serenity | 0.12 | client, gateway, rustls_backend |
| Slack | slack-morphism | 2.8 | hyper, axum |
| Feishu | open-lark | 0.15 | auth, communication |
| WhatsApp | whatsapp-business-rs | 0.2 | server |
| LINE | line-bot-sdk-rust | 0.3 | default |
| WeChat | wechat-minapp | 0.8 | default |
| DingTalk | 自建 HTTP 客户端 | - | - |
| Email | async-imap + async-smtp | 0.10/0.9 | - |
| Matrix | matrix-sdk | 0.7 | default |

---

## 4. 实施建议

### 4.1 开发顺序

```
第1周：基础设施
├── 定义 Channel trait
├── 实现 Transport 抽象层
├── 设置测试框架
└── 编写第一个渠道 (Telegram) 作为范例

第2周：核心渠道
├── 完成 Discord
├── 完成 Slack
├── 统一消息格式测试
└── 性能基准测试

第3周：国内渠道
├── 完成 Feishu
├── 完成 DingTalk
└── 国内网络适配

第4周：其他渠道
├── 完成 WhatsApp
├── 完成 LINE
└── 文档完善
```

### 4.2 质量保证

- **单元测试**: 每个渠道适配器 >= 80% 覆盖率
- **集成测试**: 使用各平台的 Sandbox 环境
- **负载测试**: 模拟 1000+ 并发连接
- **故障注入**: 测试重连、限流、熔断逻辑

### 4.3 扩展指南

新增渠道的步骤：

1. 在 `src/adapters/` 创建新模块
2. 实现 `Channel` trait
3. 在 `ChannelRegistry` 注册
4. 添加配置结构体
5. 编写单元测试
6. 更新文档

---

## 5. 风险与缓解

| 风险 | 可能性 | 影响 | 缓解措施 |
|------|--------|------|----------|
| SDK 维护停滞 | 中 | 高 | 封装抽象层，便于替换实现 |
| API 破坏性变更 | 中 | 中 | 版本锁定，自动化测试覆盖 |
| 国内网络限制 | 高 | 高 | 代理支持，多区域部署 |
| 平台政策变更 | 低 | 高 | 关注官方公告，快速响应 |
| 消息格式不兼容 | 中 | 中 | 统一消息中间表示 |

---

*文档版本: 1.0*
*最后更新: 2026-04-03*
