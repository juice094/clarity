#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    missing_docs,
    unsafe_code
)]
//! Webhook channel E2E validation for feishu/dingtalk/wecom
//!
//! Uses Tower `ServiceExt::oneshot` to test the webhook router without
//! binding real network ports. All tests use `MockLlm` to avoid external
//! API calls.

use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode};
use http_body_util::BodyExt;
use std::sync::Arc;
use tower::ServiceExt;

use clarity_core::agent::{Agent, AgentConfig, MockLlm};
use clarity_core::registry::ToolRegistry;
use clarity_gateway::channels::ChannelConfig;
use clarity_gateway::channels::webhook::{
    WebhookChannel, WebhookRequest, WebhookResponse, compute_hmac_sha256_base64,
};

fn create_test_agent() -> Arc<Agent> {
    let registry = ToolRegistry::with_builtin_tools();
    let config = AgentConfig::new()
        .with_max_iterations(5)
        .with_read_only(false);
    let agent = Agent::with_config(registry, config).with_llm(Arc::new(MockLlm));
    Arc::new(agent)
}

async fn read_json_body(res: axum::response::Response) -> serde_json::Value {
    let body = res.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

// ============================================================================
// Generic /webhook endpoint
// ============================================================================

#[tokio::test]
async fn test_webhook_generic_success() {
    let agent = create_test_agent();
    let config = ChannelConfig::new().enabled();
    let channel = WebhookChannel::new(config);
    let router = channel.create_router(&agent).unwrap();

    let req_body = serde_json::to_string(&WebhookRequest {
        message: Some("Hello from generic webhook".to_string()),
        user_id: Some("user123".to_string()),
        username: None,
        metadata: None,
        text: None,
        content: None,
        msg_type: None,
    })
    .unwrap();

    let res = router
        .oneshot(
            Request::builder()
                .uri("/webhook")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(req_body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body: WebhookResponse =
        serde_json::from_slice(&res.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert!(body.success);
    assert_eq!(body.message, Some("Processed".to_string()));
    assert!(body.response.is_some());
}

#[tokio::test]
async fn test_webhook_generic_auth_failure() {
    let agent = create_test_agent();
    let extra = serde_json::json!({
        "auth_header": "X-Webhook-Token",
        "auth_token": "secret_token"
    });
    let config = ChannelConfig::new()
        .enabled()
        .with_webhook_secret("secret_token");
    // 手动构造 config 以设置 extra
    let mut config = config;
    config.extra = Some(extra);

    let channel = WebhookChannel::new(config);
    let router = channel.create_router(&agent).unwrap();

    let req_body = serde_json::to_string(&WebhookRequest {
        message: Some("Hello".to_string()),
        user_id: None,
        username: None,
        metadata: None,
        text: None,
        content: None,
        msg_type: None,
    })
    .unwrap();

    // 请求不带认证头 → 401
    let res = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/webhook")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(req_body.clone()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // 请求带错误 token → 401
    let res = router
        .oneshot(
            Request::builder()
                .uri("/webhook")
                .method("POST")
                .header("content-type", "application/json")
                .header("X-Webhook-Token", "wrong_token")
                .body(Body::from(req_body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_webhook_generic_empty_message() {
    let agent = create_test_agent();
    let config = ChannelConfig::new().enabled();
    let channel = WebhookChannel::new(config);
    let router = channel.create_router(&agent).unwrap();

    let req_body = serde_json::to_string(&WebhookRequest {
        message: Some("".to_string()),
        user_id: None,
        username: None,
        metadata: None,
        text: None,
        content: None,
        msg_type: None,
    })
    .unwrap();

    let res = router
        .oneshot(
            Request::builder()
                .uri("/webhook")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(req_body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let body: WebhookResponse =
        serde_json::from_slice(&res.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert!(!body.success);
    assert!(body.error.as_ref().unwrap().contains("Empty message"));
}

#[tokio::test]
async fn test_webhook_generic_bearer_auth_success() {
    let agent = create_test_agent();
    let extra = serde_json::json!({
        "auth_header": "Authorization",
        "auth_token": "my_bearer_token"
    });
    let mut config = ChannelConfig::new().enabled();
    config.extra = Some(extra);

    let channel = WebhookChannel::new(config);
    let router = channel.create_router(&agent).unwrap();

    let req_body = serde_json::to_string(&WebhookRequest {
        message: Some("Hello with Bearer".to_string()),
        user_id: None,
        username: None,
        metadata: None,
        text: None,
        content: None,
        msg_type: None,
    })
    .unwrap();

    let res = router
        .oneshot(
            Request::builder()
                .uri("/webhook")
                .method("POST")
                .header("content-type", "application/json")
                .header("Authorization", "Bearer my_bearer_token")
                .body(Body::from(req_body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body: WebhookResponse =
        serde_json::from_slice(&res.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert!(body.success);
}

// ============================================================================
// Feishu /webhook/feishu endpoint
// ============================================================================

#[tokio::test]
async fn test_webhook_feishu_success() {
    let agent = create_test_agent();
    let secret = "feishu_test_secret";
    let config = ChannelConfig::new().enabled().with_webhook_secret(secret);
    let channel = WebhookChannel::new(config);
    let router = channel.create_router(&agent).unwrap();

    let timestamp = "1234567890";
    let nonce = "abc123";
    let body_json = r#"{"event":{"message":{"content":"{\"text\":\"Hello Feishu\"}"}}}"#;
    let sign_string = format!("{}\n{}\n{}", timestamp, nonce, body_json);
    let signature = compute_hmac_sha256_base64(secret, &sign_string).unwrap();

    let res = router
        .oneshot(
            Request::builder()
                .uri("/webhook/feishu")
                .method("POST")
                .header("content-type", "application/json")
                .header("X-Lark-Signature", &signature)
                .header("X-Lark-Request-Timestamp", timestamp)
                .header("X-Lark-Request-Nonce", nonce)
                .body(Body::from(body_json))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body = read_json_body(res).await;
    assert_eq!(body["msg_type"], "text");
    assert!(body["content"]["text"].as_str().is_some());
}

#[tokio::test]
async fn test_webhook_feishu_auth_failure() {
    let agent = create_test_agent();
    let secret = "feishu_test_secret";
    let config = ChannelConfig::new().enabled().with_webhook_secret(secret);
    let channel = WebhookChannel::new(config);
    let router = channel.create_router(&agent).unwrap();

    let body_json = r#"{"event":{"message":{"content":"{\"text\":\"Hello Feishu\"}"}}}"#;

    // 缺少签名头
    let res = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/webhook/feishu")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(body_json))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // 错误签名
    let res = router
        .oneshot(
            Request::builder()
                .uri("/webhook/feishu")
                .method("POST")
                .header("content-type", "application/json")
                .header("X-Lark-Signature", "invalid_signature")
                .header("X-Lark-Request-Timestamp", "1234567890")
                .header("X-Lark-Request-Nonce", "abc123")
                .body(Body::from(body_json))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_webhook_feishu_plain_content() {
    let agent = create_test_agent();
    let secret = "feishu_test_secret";
    let config = ChannelConfig::new().enabled().with_webhook_secret(secret);
    let channel = WebhookChannel::new(config);
    let router = channel.create_router(&agent).unwrap();

    let timestamp = "1234567890";
    let nonce = "abc123";
    // 飞书消息 content 直接是文本（不是 JSON 字符串）
    let body_json = r#"{"event":{"message":{"content":"Plain text message"}}}"#;
    let sign_string = format!("{}\n{}\n{}", timestamp, nonce, body_json);
    let signature = compute_hmac_sha256_base64(secret, &sign_string).unwrap();

    let res = router
        .oneshot(
            Request::builder()
                .uri("/webhook/feishu")
                .method("POST")
                .header("content-type", "application/json")
                .header("X-Lark-Signature", &signature)
                .header("X-Lark-Request-Timestamp", timestamp)
                .header("X-Lark-Request-Nonce", nonce)
                .body(Body::from(body_json))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body = read_json_body(res).await;
    assert_eq!(body["msg_type"], "text");
}

// ============================================================================
// DingTalk /webhook/dingtalk endpoint
// ============================================================================

#[tokio::test]
async fn test_webhook_dingtalk_success() {
    let agent = create_test_agent();
    let secret = "dingtalk_test_secret";
    let config = ChannelConfig::new().enabled().with_webhook_secret(secret);
    let channel = WebhookChannel::new(config);
    let router = channel.create_router(&agent).unwrap();

    let timestamp = "1234567890";
    let sign_string = format!("{}\n{}", timestamp, secret);
    let sign = compute_hmac_sha256_base64(secret, &sign_string).unwrap();

    let body_json = format!(
        r#"{{"timestamp":"{}","sign":"{}","text":{{"content":"Hello DingTalk"}}}}"#,
        timestamp, sign
    );

    let res = router
        .oneshot(
            Request::builder()
                .uri("/webhook/dingtalk")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(body_json))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body = read_json_body(res).await;
    assert_eq!(body["msgtype"], "text");
    assert!(body["text"]["content"].as_str().is_some());
}

#[tokio::test]
async fn test_webhook_dingtalk_auth_failure() {
    let agent = create_test_agent();
    let secret = "dingtalk_test_secret";
    let config = ChannelConfig::new().enabled().with_webhook_secret(secret);
    let channel = WebhookChannel::new(config);
    let router = channel.create_router(&agent).unwrap();

    // 缺少 sign
    let body_json = r#"{"timestamp":"1234567890","text":{"content":"Hello"}}"#;
    let res = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/webhook/dingtalk")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(body_json))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // 错误 sign
    let body_json = r#"{"timestamp":"1234567890","sign":"invalid","text":{"content":"Hello"}}"#;
    let res = router
        .oneshot(
            Request::builder()
                .uri("/webhook/dingtalk")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(body_json))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_webhook_dingtalk_content_field() {
    let agent = create_test_agent();
    let secret = "dingtalk_test_secret";
    let config = ChannelConfig::new().enabled().with_webhook_secret(secret);
    let channel = WebhookChannel::new(config);
    let router = channel.create_router(&agent).unwrap();

    let timestamp = "1234567890";
    let sign_string = format!("{}\n{}", timestamp, secret);
    let sign = compute_hmac_sha256_base64(secret, &sign_string).unwrap();

    // 使用顶层 content 字段（替代 text.content）
    let body_json = format!(
        r#"{{"timestamp":"{}","sign":"{}","content":"Direct content"}}"#,
        timestamp, sign
    );

    let res = router
        .oneshot(
            Request::builder()
                .uri("/webhook/dingtalk")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(body_json))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body = read_json_body(res).await;
    assert_eq!(body["msgtype"], "text");
}

// ============================================================================
// WeCom /webhook/wecom endpoint
// ============================================================================

#[tokio::test]
async fn test_webhook_wecom_success() {
    let agent = create_test_agent();
    let config = ChannelConfig::new().enabled();
    let channel = WebhookChannel::new(config);
    let router = channel.create_router(&agent).unwrap();

    // 企业微信暂时不验证签名
    let body_json = r#"{"Content":"Hello WeCom"}"#;

    let res = router
        .oneshot(
            Request::builder()
                .uri("/webhook/wecom")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(body_json))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body = read_json_body(res).await;
    assert_eq!(body["msgtype"], "text");
    assert_eq!(body["text"]["content"], "This is a mock response");
}

#[tokio::test]
async fn test_webhook_wecom_text_object() {
    let agent = create_test_agent();
    let config = ChannelConfig::new().enabled();
    let channel = WebhookChannel::new(config);
    let router = channel.create_router(&agent).unwrap();

    let body_json = r#"{"Text":{"Content":"Hello WeCom Object"}}"#;

    let res = router
        .oneshot(
            Request::builder()
                .uri("/webhook/wecom")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(body_json))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body = read_json_body(res).await;
    assert_eq!(body["msgtype"], "text");
}

#[tokio::test]
async fn test_webhook_wecom_empty_message() {
    let agent = create_test_agent();
    let config = ChannelConfig::new().enabled();
    let channel = WebhookChannel::new(config);
    let router = channel.create_router(&agent).unwrap();

    let body_json = r#"{"Content":""}"#;

    let res = router
        .oneshot(
            Request::builder()
                .uri("/webhook/wecom")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(body_json))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

// ============================================================================
// Unknown platform
// ============================================================================

#[tokio::test]
async fn test_webhook_unknown_platform_generic_parse() {
    let agent = create_test_agent();
    let config = ChannelConfig::new().enabled();
    let channel = WebhookChannel::new(config);
    let router = channel.create_router(&agent).unwrap();

    let body_json = r#"{"message":"Hello unknown platform"}"#;

    let res = router
        .oneshot(
            Request::builder()
                .uri("/webhook/some_platform")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(body_json))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body = read_json_body(res).await;
    assert_eq!(body["success"], true);
    assert_eq!(body["response"], "This is a mock response");
}

#[tokio::test]
async fn test_webhook_unknown_platform_bad_request() {
    let agent = create_test_agent();
    let config = ChannelConfig::new().enabled();
    let channel = WebhookChannel::new(config);
    let router = channel.create_router(&agent).unwrap();

    // 无法解析的消息
    let body_json = r#"{"unknown_field":"value"}"#;

    let res = router
        .oneshot(
            Request::builder()
                .uri("/webhook/unknown")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(body_json))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

// ============================================================================
// Signature edge cases
// ============================================================================

#[tokio::test]
async fn test_webhook_no_secret_skips_auth() {
    let agent = create_test_agent();
    // 不配置 webhook_secret
    let config = ChannelConfig::new().enabled();
    let channel = WebhookChannel::new(config);
    let router = channel.create_router(&agent).unwrap();

    let body_json = r#"{"event":{"message":{"content":"{\"text\":\"No secret\"}"}}}"#;

    let res = router
        .oneshot(
            Request::builder()
                .uri("/webhook/feishu")
                .method("POST")
                .header("content-type", "application/json")
                // 不携带任何签名头
                .body(Body::from(body_json))
                .unwrap(),
        )
        .await
        .unwrap();

    // 没有配置 secret 时，认证直接跳过
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_webhook_feishu_signature_computation() {
    let secret = "test_secret";
    let timestamp = "1234567890";
    let nonce = "abc123";
    let body = r#"{"event":{"message":{"content":"Hello"}}}"#;
    let sign_string = format!("{}\n{}\n{}", timestamp, nonce, body);
    let signature = compute_hmac_sha256_base64(secret, &sign_string).unwrap();

    let mut headers = HeaderMap::new();
    headers.insert("X-Lark-Signature", signature.parse().unwrap());
    headers.insert("X-Lark-Request-Timestamp", timestamp.parse().unwrap());
    headers.insert("X-Lark-Request-Nonce", nonce.parse().unwrap());

    let result = clarity_gateway::channels::webhook::verify_platform_auth(
        "feishu",
        &headers,
        body.as_bytes(),
        Some(secret),
    );
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_webhook_dingtalk_signature_computation() {
    let secret = "test_secret";
    let timestamp = "1234567890";
    let sign_string = format!("{}\n{}", timestamp, secret);
    let sign = compute_hmac_sha256_base64(secret, &sign_string).unwrap();

    let body = format!(
        r#"{{"timestamp":"{}","sign":"{}","text":{{"content":"Hello"}}}}"#,
        timestamp, sign
    );
    let headers = HeaderMap::new();

    let result = clarity_gateway::channels::webhook::verify_platform_auth(
        "dingtalk",
        &headers,
        body.as_bytes(),
        Some(secret),
    );
    assert!(result.is_ok());
}
