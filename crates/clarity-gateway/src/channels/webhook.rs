//! 通用 Webhook 渠道实现
//!
//! 支持：
//! - 接收 HTTP POST 请求
//! - 自定义认证头（用于飞书/钉钉等平台）
//! - 流式响应（通过 SSE 或分块响应）
//! - 可用于接入：飞书、钉钉、企业微信等

#![allow(dead_code)]

use async_trait::async_trait;
use axum::{
    extract::{Json, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use clarity_core::Agent;

use super::{Channel, ChannelConfig, ChannelError};

/// Webhook Channel 状态
#[derive(Debug, Clone, Copy, PartialEq)]
enum WebhookState {
    Stopped,
    Running,
}

/// 通用 Webhook 渠道
pub struct WebhookChannel {
    config: ChannelConfig,
    state: Arc<RwLock<WebhookState>>,
    port: u16,
    auth_header: Option<String>,
    auth_token: Option<String>,
    webhook_secret: Option<String>,
}

impl WebhookChannel {
    pub fn new(config: ChannelConfig) -> Self {
        // 从 extra 配置中提取端口和认证信息
        let (port, auth_header, auth_token) = config
            .extra
            .as_ref()
            .map(|extra| {
                (
                    extra.get("port").and_then(|v| v.as_u64()).unwrap_or(18791) as u16,
                    extra
                        .get("auth_header")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    extra
                        .get("auth_token")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                )
            })
            .unwrap_or((18791, None, None));

        // 优先使用 config.webhook_secret，否则回退到 extra 中的 auth_token
        let webhook_secret = config.webhook_secret.clone().or_else(|| auth_token.clone());

        Self {
            state: Arc::new(RwLock::new(WebhookState::Stopped)),
            config,
            port,
            auth_header,
            auth_token,
            webhook_secret,
        }
    }

    /// 构建 webhook router（可复用于测试）
    pub fn create_router(&self, agent: Arc<Agent>) -> Result<Router, ChannelError> {
        let app_state = WebhookAppState {
            agent: agent.clone(),
            auth_header: self.auth_header.clone(),
            auth_token: self.auth_token.clone(),
            webhook_secret: self.webhook_secret.clone(),
        };

        Ok(Router::new()
            .route("/webhook", post(webhook_handler))
            .route("/webhook/:platform", post(webhook_handler_with_platform))
            .with_state(Arc::new(app_state)))
    }

    /// 启动 HTTP 服务器接收 webhook
    async fn start_server(&self, agent: Arc<Agent>) -> Result<(), ChannelError> {
        let addr = format!("0.0.0.0:{}", self.port);

        let app = self.create_router(agent)?;

        let listener = tokio::net::TcpListener::bind(&addr).await.map_err(|e| {
            ChannelError::ConnectionFailed(format!("Failed to bind to {}: {}", addr, e))
        })?;

        info!("[Webhook] Server listening on http://{}", addr);
        info!("[Webhook] Webhook URL: http://{}/webhook", addr);

        // 设置状态为运行
        *self.state.write().await = WebhookState::Running;

        // 启动服务器
        axum::serve(listener, app)
            .await
            .map_err(|e| ChannelError::Unknown(format!("Server error: {}", e)))?;

        Ok(())
    }

    /// 验证请求认证
    fn verify_auth(&self, headers: &HeaderMap) -> Result<(), ChannelError> {
        // 如果没有配置认证，允许所有请求
        let (Some(header_name), Some(expected_token)) = (&self.auth_header, &self.auth_token)
        else {
            return Ok(());
        };

        // 从请求头中获取认证信息
        let provided_token = headers
            .get(header_name)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| {
                ChannelError::AuthFailed(format!("Missing required header: {}", header_name))
            })?;

        // 支持 "Bearer token" 格式
        let provided_token = provided_token
            .strip_prefix("Bearer ")
            .unwrap_or(provided_token);

        if provided_token != expected_token {
            return Err(ChannelError::AuthFailed("Invalid token".to_string()));
        }

        Ok(())
    }
}

#[async_trait]
impl Channel for WebhookChannel {
    fn name(&self) -> &str {
        "webhook"
    }

    async fn start(&self, agent: Arc<Agent>) -> Result<(), ChannelError> {
        if !self.config.enabled {
            warn!("[Webhook] Channel is disabled");
            return Ok(());
        }

        info!("[Webhook] Starting channel on port {}...", self.port);
        self.start_server(agent).await
    }

    async fn stop(&self) -> Result<(), ChannelError> {
        info!("[Webhook] Stopping channel...");
        *self.state.write().await = WebhookState::Stopped;
        Ok(())
    }
}

// ==================== Webhook Handler ====================

/// Webhook 应用状态
struct WebhookAppState {
    agent: Arc<Agent>,
    auth_header: Option<String>,
    auth_token: Option<String>,
    webhook_secret: Option<String>,
}

/// 通用 Webhook 请求
#[derive(Debug, Deserialize, Serialize)]
pub struct WebhookRequest {
    /// 消息内容
    pub message: Option<String>,
    /// 用户 ID
    pub user_id: Option<String>,
    /// 用户名
    pub username: Option<String>,
    /// 额外元数据
    pub metadata: Option<serde_json::Value>,
    /// 飞书/钉钉等平台特定字段
    pub text: Option<String>,
    pub content: Option<String>,
    pub msg_type: Option<String>,
}

/// Webhook 响应
#[derive(Debug, Serialize, Deserialize)]
pub struct WebhookResponse {
    pub success: bool,
    pub message: Option<String>,
    pub response: Option<String>,
    pub error: Option<String>,
}

/// 通用 webhook 处理器
async fn webhook_handler(
    State(state): State<Arc<WebhookAppState>>,
    headers: HeaderMap,
    Json(req): Json<WebhookRequest>,
) -> impl IntoResponse {
    info!("[Webhook] Received request");

    // 验证认证
    if let Some(ref header) = state.auth_header {
        if let Some(ref token) = state.auth_token {
            let provided = headers
                .get(header)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");

            // 支持 "Bearer token" 格式
            let provided = provided.strip_prefix("Bearer ").unwrap_or(provided);

            if provided != token {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(WebhookResponse {
                        success: false,
                        message: None,
                        response: None,
                        error: Some("Unauthorized".to_string()),
                    }),
                );
            }
        }
    }

    // 提取消息内容
    let message = req
        .message
        .or(req.text)
        .or_else(|| req.content.clone())
        .unwrap_or_default();

    if message.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(WebhookResponse {
                success: false,
                message: None,
                response: None,
                error: Some("Empty message".to_string()),
            }),
        );
    }

    let user_id = req.user_id.unwrap_or_else(|| "anonymous".to_string());
    info!("[Webhook] Message from {}: {}", user_id, message);

    // 使用 Agent 处理消息
    let agent = (*state.agent).clone();
    match agent.run(&message).await {
        Ok(response) => (
            StatusCode::OK,
            Json(WebhookResponse {
                success: true,
                message: Some("Processed".to_string()),
                response: Some(response),
                error: None,
            }),
        ),
        Err(e) => {
            error!("[Webhook] Agent error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(WebhookResponse {
                    success: false,
                    message: None,
                    response: None,
                    error: Some(e.to_string()),
                }),
            )
        }
    }
}

/// 带平台标识的 webhook 处理器
async fn webhook_handler_with_platform(
    axum::extract::Path(platform): axum::extract::Path<String>,
    State(state): State<Arc<WebhookAppState>>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    info!("[Webhook] Received request for platform: {}", platform);

    // 根据平台解析请求
    let message = match platform.as_str() {
        "feishu" | "lark" => parse_feishu_request(&body),
        "dingtalk" | "dingding" => parse_dingtalk_request(&body),
        "wecom" | "wechat-work" => parse_wecom_request(&body),
        _ => {
            // 通用解析
            match serde_json::from_slice::<WebhookRequest>(&body) {
                Ok(req) => req.message.or(req.text).or(req.content),
                Err(_) => None,
            }
        }
    };

    let message = match message {
        Some(msg) if !msg.is_empty() => msg,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(WebhookResponse {
                    success: false,
                    message: None,
                    response: None,
                    error: Some("Failed to parse message".to_string()),
                }),
            )
                .into_response();
        }
    };

    // 验证认证（平台特定）
    if let Err(e) =
        verify_platform_auth(&platform, &headers, &body, state.webhook_secret.as_deref())
    {
        return (
            StatusCode::UNAUTHORIZED,
            Json(WebhookResponse {
                success: false,
                message: None,
                response: None,
                error: Some(e.to_string()),
            }),
        )
            .into_response();
    }

    // 处理消息
    let agent = (*state.agent).clone();
    match agent.run(&message).await {
        Ok(response) => {
            // 根据平台格式化响应
            let response_body = format_platform_response(&platform, &response);
            (StatusCode::OK, response_body).into_response()
        }
        Err(e) => {
            error!("[Webhook] Agent error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(WebhookResponse {
                    success: false,
                    message: None,
                    response: None,
                    error: Some(e.to_string()),
                }),
            )
                .into_response()
        }
    }
}

// ==================== Platform-specific parsers ====================

/// 解析飞书/Lark 请求
fn parse_feishu_request(body: &[u8]) -> Option<String> {
    #[derive(Deserialize)]
    struct FeishuRequest {
        #[serde(rename = "event")]
        event: Option<FeishuEvent>,
        #[serde(rename = "action")]
        action: Option<FeishuAction>,
    }

    #[derive(Deserialize)]
    struct FeishuEvent {
        message: Option<FeishuMessage>,
    }

    #[derive(Deserialize)]
    struct FeishuMessage {
        content: Option<String>,
    }

    #[derive(Deserialize)]
    struct FeishuAction {
        value: Option<serde_json::Value>,
    }

    let req: FeishuRequest = serde_json::from_slice(body).ok()?;

    // 提取消息内容
    let content = req.event?.message?.content?;

    // 飞书消息内容可能是 JSON 字符串
    if content.starts_with('{') {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(text) = json.get("text").and_then(|v| v.as_str()) {
                return Some(text.to_string());
            }
        }
    }

    Some(content)
}

/// 解析钉钉请求
fn parse_dingtalk_request(body: &[u8]) -> Option<String> {
    #[derive(Deserialize)]
    struct DingTalkRequest {
        #[serde(rename = "text")]
        text: Option<DingTalkText>,
        #[serde(rename = "content")]
        content: Option<String>,
    }

    #[derive(Deserialize)]
    struct DingTalkText {
        content: String,
    }

    let req: DingTalkRequest = serde_json::from_slice(body).ok()?;

    req.text.map(|t| t.content).or(req.content)
}

/// 解析企业微信请求
fn parse_wecom_request(body: &[u8]) -> Option<String> {
    #[derive(Deserialize)]
    struct WeComRequest {
        #[serde(rename = "Content")]
        content: Option<String>,
        #[serde(rename = "Text")]
        text: Option<WeComText>,
    }

    #[derive(Deserialize)]
    struct WeComText {
        #[serde(rename = "Content")]
        content: String,
    }

    let req: WeComRequest = serde_json::from_slice(body).ok()?;

    req.content.or_else(|| req.text.map(|t| t.content))
}

/// 验证平台特定的认证
pub fn verify_platform_auth(
    platform: &str,
    headers: &HeaderMap,
    body: &[u8],
    secret: Option<&str>,
) -> Result<(), ChannelError> {
    // 如果没有配置 secret，跳过验证
    let secret = match secret {
        Some(s) if !s.is_empty() => s,
        _ => return Ok(()),
    };

    match platform {
        "feishu" | "lark" => {
            // 飞书使用 X-Lark-Signature 验证
            let signature = headers
                .get("X-Lark-Signature")
                .and_then(|v| v.to_str().ok())
                .ok_or_else(|| ChannelError::AuthFailed("Missing X-Lark-Signature".to_string()))?;

            let timestamp = headers
                .get("X-Lark-Request-Timestamp")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");

            let nonce = headers
                .get("X-Lark-Request-Nonce")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");

            let body_str = std::str::from_utf8(body).unwrap_or("");
            let sign_string = format!("{}\n{}\n{}", timestamp, nonce, body_str);

            let computed = compute_hmac_sha256_base64(secret, &sign_string);

            if !constant_time_str_eq(&computed, signature) {
                return Err(ChannelError::AuthFailed(
                    "Invalid Feishu signature".to_string(),
                ));
            }

            Ok(())
        }
        "dingtalk" | "dingding" => {
            // 钉钉使用 timestamp + sign 验证
            #[derive(Deserialize)]
            struct DingTalkAuthBody {
                timestamp: Option<String>,
                sign: Option<String>,
            }

            let auth_body: DingTalkAuthBody = serde_json::from_slice(body)
                .map_err(|e| ChannelError::AuthFailed(format!("Invalid DingTalk body: {}", e)))?;

            let timestamp = auth_body
                .timestamp
                .ok_or_else(|| ChannelError::AuthFailed("Missing timestamp".to_string()))?;
            let sign = auth_body
                .sign
                .ok_or_else(|| ChannelError::AuthFailed("Missing sign".to_string()))?;

            let sign_string = format!("{}\n{}", timestamp, secret);
            let computed = compute_hmac_sha256_base64(secret, &sign_string);

            if !constant_time_str_eq(&computed, &sign) {
                return Err(ChannelError::AuthFailed(
                    "Invalid DingTalk signature".to_string(),
                ));
            }

            Ok(())
        }
        "wecom" | "wechat-work" => {
            // 企业微信暂时直接通过
            Ok(())
        }
        _ => {
            // 通用平台直接通过
            Ok(())
        }
    }
}

/// 计算 HMAC-SHA256 并进行 Base64 编码
pub fn compute_hmac_sha256_base64(secret: &str, message: &str) -> String {
    use base64::Engine;
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    type HmacSha256 = Hmac<Sha256>;

    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC can take key of any size");
    mac.update(message.as_bytes());
    let result = mac.finalize();
    let bytes = result.into_bytes();
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// Constant-time string comparison to mitigate timing attacks.
fn constant_time_str_eq(a: &str, b: &str) -> bool {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    if a_bytes.len() != b_bytes.len() {
        return false;
    }
    let mut result = 0u8;
    for (x, y) in a_bytes.iter().zip(b_bytes.iter()) {
        result |= x ^ y;
    }
    result == 0
}

/// 格式化平台特定的响应
fn format_platform_response(platform: &str, response: &str) -> String {
    match platform {
        "feishu" | "lark" => serde_json::json!({
            "msg_type": "text",
            "content": {
                "text": response
            }
        })
        .to_string(),
        "dingtalk" | "dingding" => serde_json::json!({
            "msgtype": "text",
            "text": {
                "content": response
            }
        })
        .to_string(),
        "wecom" | "wechat-work" => serde_json::json!({
            "msgtype": "text",
            "text": {
                "content": response
            }
        })
        .to_string(),
        _ => serde_json::json!({
            "success": true,
            "response": response
        })
        .to_string(),
    }
}

// ==================== Webhook Sender ====================

/// Webhook 发送器 - 用于向外部系统发送 webhook
pub struct WebhookSender {
    client: reqwest::Client,
}

impl WebhookSender {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    /// 发送 webhook 到指定 URL
    pub async fn send(
        &self,
        url: &str,
        payload: &serde_json::Value,
        headers: Option<Vec<(String, String)>>,
    ) -> Result<(), ChannelError> {
        let mut request = self.client.post(url).json(payload);

        // 添加自定义请求头
        if let Some(headers) = headers {
            for (key, value) in headers {
                request = request.header(&key, value);
            }
        }

        let response = request.send().await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(ChannelError::SendFailed(format!(
                "Webhook failed: {}",
                error_text
            )));
        }

        Ok(())
    }

    /// 发送到飞书 webhook
    pub async fn send_feishu(&self, webhook_url: &str, message: &str) -> Result<(), ChannelError> {
        let payload = serde_json::json!({
            "msg_type": "text",
            "content": {
                "text": message
            }
        });

        self.send(webhook_url, &payload, None).await
    }

    /// 发送到钉钉 webhook
    pub async fn send_dingtalk(
        &self,
        webhook_url: &str,
        message: &str,
    ) -> Result<(), ChannelError> {
        let payload = serde_json::json!({
            "msgtype": "text",
            "text": {
                "content": message
            }
        });

        self.send(webhook_url, &payload, None).await
    }
}

impl Default for WebhookSender {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_webhook_request_parsing() {
        let json = r#"{
            "message": "Hello",
            "user_id": "user123",
            "username": "Test User"
        }"#;

        let req: WebhookRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.message, Some("Hello".to_string()));
        assert_eq!(req.user_id, Some("user123".to_string()));
    }

    #[test]
    fn test_feishu_request_parsing() {
        let json = br#"{
            "event": {
                "message": {
                    "content": "{\"text\": \"Hello from Feishu\"}"
                }
            }
        }"#;

        let result = parse_feishu_request(json);
        assert_eq!(result, Some("Hello from Feishu".to_string()));
    }

    #[test]
    fn test_dingtalk_request_parsing() {
        let json = br#"{
            "text": {
                "content": "Hello from DingTalk"
            }
        }"#;

        let result = parse_dingtalk_request(json);
        assert_eq!(result, Some("Hello from DingTalk".to_string()));
    }

    #[test]
    fn test_format_platform_response() {
        let response = format_platform_response("feishu", "Test message");
        assert!(response.contains("msg_type"));
        assert!(response.contains("Test message"));

        let response = format_platform_response("dingtalk", "Test message");
        assert!(response.contains("msgtype"));
        assert!(response.contains("Test message"));
    }

    #[test]
    fn test_webhook_response_serialization() {
        let resp = WebhookResponse {
            success: true,
            message: Some("OK".to_string()),
            response: Some("Result".to_string()),
            error: None,
        };

        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"success\":true"));
    }

    #[test]
    fn test_dingtalk_signature_verification() {
        let secret = "test_secret";
        let timestamp = "1234567890";
        let sign_string = format!("{}\n{}", timestamp, secret);
        let sign = compute_hmac_sha256_base64(secret, &sign_string);

        let body = format!(
            r#"{{"timestamp":"{}","sign":"{}","text":{{"content":"Hello"}}}}"#,
            timestamp, sign
        );

        let headers = HeaderMap::new();
        let result = verify_platform_auth("dingtalk", &headers, body.as_bytes(), Some(secret));
        assert!(result.is_ok());
    }

    #[test]
    fn test_dingtalk_signature_mismatch() {
        let body = r#"{"timestamp":"1234567890","sign":"invalid_sign","text":{"content":"Hello"}}"#;
        let headers = HeaderMap::new();
        let result =
            verify_platform_auth("dingtalk", &headers, body.as_bytes(), Some("test_secret"));
        assert!(result.is_err());
    }

    #[test]
    fn test_feishu_signature_verification() {
        let secret = "test_secret";
        let timestamp = "1234567890";
        let nonce = "abc123";
        let body_str = r#"{"event":{"message":{"content":"Hello"}}}"#;
        let sign_string = format!("{}\n{}\n{}", timestamp, nonce, body_str);
        let sign = compute_hmac_sha256_base64(secret, &sign_string);

        let mut headers = HeaderMap::new();
        headers.insert("X-Lark-Signature", sign.parse().unwrap());
        headers.insert("X-Lark-Request-Timestamp", timestamp.parse().unwrap());
        headers.insert("X-Lark-Request-Nonce", nonce.parse().unwrap());

        let result = verify_platform_auth("feishu", &headers, body_str.as_bytes(), Some(secret));
        assert!(result.is_ok());
    }

    #[test]
    fn test_feishu_signature_mismatch() {
        let mut headers = HeaderMap::new();
        headers.insert("X-Lark-Signature", "invalid_sign".parse().unwrap());
        headers.insert("X-Lark-Request-Timestamp", "1234567890".parse().unwrap());
        headers.insert("X-Lark-Request-Nonce", "abc123".parse().unwrap());

        let body = r#"{"event":{"message":{"content":"Hello"}}}"#;
        let result = verify_platform_auth("feishu", &headers, body.as_bytes(), Some("test_secret"));
        assert!(result.is_err());
    }
}
