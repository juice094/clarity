//! DeepSeek 设备登录版 Provider
//!
//! 通过 Android App 的私有 `/api/v0/*` 接口提供对话能力，无需官方 API Key。
//! 支持两种凭证模式：
//! 1. 直接传入 `device_token`（从 App MMKV 提取的 `key_user_info.token`）
//! 2. 手机号 + 密码自动登录（首次调用时完成登录并刷新 token）
//!
//! PoW 求解使用 crate 内纯 Rust 实现的 `deepseek_pow`，无 Node.js / WASM 运行时依赖。
//!
//! # clarity-gateway 集成
//!
//! 在 `models.toml` 中配置：
//!
//! ```toml
//! [providers.deepseek-device]
//! protocol = "deepseek_device"
//!
//! [[models]]
//! alias = "deepseek-device"
//! provider = "deepseek-device"
//! model_id = "deepseek-chat"
//! api_key = "你的 MMKV token"
//! ```
//!
//! 或在 gateway 管理 API 中设置 `protocol = "deepseek_device"`。
//! Token 也支持通过环境变量 `DEEPSEEK_DEVICE_TOKEN` 传入。

use async_trait::async_trait;
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use clarity_contract::{
    AgentError, LlmProvider, LlmResponse, Message, MessageRole, ProviderCapabilities, StreamDelta,
};
use parking_lot::RwLock;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue, USER_AGENT};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::spawn_blocking;
use tracing::debug;

/// DeepSeek 设备登录 Provider 配置
#[derive(Debug, Clone)]
pub struct DeepSeekDeviceConfig {
    /// 基础 URL，默认 `https://chat.deepseek.com`
    pub base_url: String,
    /// 客户端版本，与 App 保持一致
    pub client_version: String,
    /// 设备 ID，任意稳定字符串即可
    pub device_id: String,
    /// 凭证
    pub credentials: DeepSeekDeviceCredentials,
    /// 对话选项（思考、搜索、模型类型）
    pub options: DeepSeekDeviceOptions,
}

impl Default for DeepSeekDeviceConfig {
    fn default() -> Self {
        Self {
            base_url: "https://chat.deepseek.com".to_string(),
            client_version: "2.1.8".to_string(),
            device_id: "clarity-device".to_string(),
            credentials: DeepSeekDeviceCredentials::Token(String::new()),
            options: DeepSeekDeviceOptions::default(),
        }
    }
}

/// 单次对话选项
#[derive(Debug, Clone)]
pub struct DeepSeekDeviceOptions {
    /// 是否开启深度思考（对应 App 的 R1 / expert 模式）
    pub thinking_enabled: bool,
    /// 是否开启联网搜索
    pub search_enabled: bool,
    /// 模型类型映射。常见值：
    /// - `"default"`：普通对话
    /// - `"expert"`：深度思考（deepseek-reasoner）
    /// - `"vision"`：图片理解
    pub model_type: String,
}

impl Default for DeepSeekDeviceOptions {
    fn default() -> Self {
        Self {
            thinking_enabled: false,
            search_enabled: false,
            model_type: "default".to_string(),
        }
    }
}

impl DeepSeekDeviceOptions {
    /// 从用户可见的 model_id 推导内部 model_type
    pub fn from_model_id(model_id: &str) -> Self {
        let model_type = match model_id {
            "deepseek-reasoner" | "expert" => "expert",
            "deepseek-vision" | "vision" => "vision",
            _ => "default",
        };
        Self {
            thinking_enabled: model_type == "expert",
            search_enabled: false,
            model_type: model_type.to_string(),
        }
    }
}

/// 凭证模式
#[derive(Debug, Clone)]
pub enum DeepSeekDeviceCredentials {
    /// 直接使用 refresh token
    Token(String),
    /// 手机号 + 密码登录
    Password {
        /// 手机号
        mobile: String,
        /// 密码
        password: String,
    },
}

/// DeepSeek 设备登录 Provider
#[derive(Debug, Clone)]
pub struct DeepSeekDeviceProvider {
    config: DeepSeekDeviceConfig,
    client: reqwest::Client,
    /// 当前 access token，登录/刷新后写入。
    /// Provider 本身保持无状态：每个 `complete`/`stream` 调用会创建新的 DeepSeek session。
    token: Arc<RwLock<Option<String>>>,
}

impl DeepSeekDeviceProvider {
    /// 从配置创建
    pub fn new(config: DeepSeekDeviceConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(120))
                .connect_timeout(Duration::from_secs(15))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            token: Arc::new(RwLock::new(None)),
        }
    }

    /// 便捷构造：仅使用 token
    pub fn with_token(token: impl Into<String>) -> Self {
        Self::with_token_and_options(token, DeepSeekDeviceOptions::default())
    }

    /// 便捷构造：token + 自定义选项
    pub fn with_token_and_options(
        token: impl Into<String>,
        options: DeepSeekDeviceOptions,
    ) -> Self {
        let config = DeepSeekDeviceConfig {
            credentials: DeepSeekDeviceCredentials::Token(token.into()),
            options,
            ..Default::default()
        };
        Self::new(config)
    }

    /// 便捷构造：手机号 + 密码
    pub fn with_password(mobile: impl Into<String>, password: impl Into<String>) -> Self {
        let config = DeepSeekDeviceConfig {
            credentials: DeepSeekDeviceCredentials::Password {
                mobile: mobile.into(),
                password: password.into(),
            },
            ..Default::default()
        };
        Self::new(config)
    }

    /// 构建通用请求头
    fn headers(&self, token: Option<&str>) -> Result<HeaderMap, AgentError> {
        let mut headers = HeaderMap::new();
        headers.insert("x-client-platform", HeaderValue::from_static("android"));
        headers.insert(
            "x-client-version",
            HeaderValue::from_str(&self.config.client_version)
                .map_err(|e| AgentError::Llm(format!("invalid client version: {}", e)))?,
        );
        headers.insert("x-client-locale", HeaderValue::from_static("zh_CN"));
        headers.insert(
            "x-client-bundle-id",
            HeaderValue::from_static("com.deepseek.chat"),
        );
        headers.insert("x-client-timezone-offset", HeaderValue::from_static("0"));
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static("DeepSeek/2.1.8 Android/36"),
        );
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        if let Some(t) = token {
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {}", t))
                    .map_err(|e| AgentError::Llm(format!("invalid token: {}", e)))?,
            );
        }
        Ok(headers)
    }

    /// 获取有效 token，未登录时自动登录/刷新
    async fn ensure_token(&self) -> Result<String, AgentError> {
        {
            let read = self.token.read();
            if let Some(t) = read.clone() {
                return Ok(t);
            }
        }

        self.obtain_token().await
    }

    /// 强制重新获取 token（用于 401 重试）
    async fn obtain_token(&self) -> Result<String, AgentError> {
        let token = match &self.config.credentials {
            DeepSeekDeviceCredentials::Token(t) => {
                // 用 token 刷新，验证是否有效
                self.refresh_token(t).await?
            }
            DeepSeekDeviceCredentials::Password { mobile, password } => {
                self.login(mobile, password).await?
            }
        };

        *self.token.write() = Some(token.clone());
        Ok(token)
    }

    /// 清除缓存的 token
    fn clear_token(&self) {
        *self.token.write() = None;
    }

    /// 手机号 + 密码登录
    async fn login(&self, mobile: &str, password: &str) -> Result<String, AgentError> {
        let url = format!("{}/api/v0/users/login", self.config.base_url);
        let body = json!({
            "mobile": mobile,
            "area_code": null,
            "password": password,
            "device_id": self.config.device_id,
            "os": "android",
        });

        let response = self
            .client
            .post(&url)
            .headers(self.headers(None)?)
            .json(&body)
            .send()
            .await
            .map_err(|e| AgentError::Llm(format!("login request failed: {}", e)))?;

        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(AgentError::Llm(format!(
                "login failed ({}): {}",
                status, text
            )));
        }

        let resp: DeepSeekResponse = serde_json::from_str(&text).map_err(|e| {
            AgentError::Llm(format!("login response parse failed: {} | {}", e, text))
        })?;

        let biz_data = resp
            .data
            .biz_data
            .ok_or_else(|| AgentError::Llm(format!("login biz_data missing: {}", text)))?;
        biz_data
            .get("token")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| AgentError::Llm("login response missing token".to_string()))
    }

    /// 用 refresh token 换取 access token
    async fn refresh_token(&self, refresh_token: &str) -> Result<String, AgentError> {
        let url = format!("{}/api/v0/users/current", self.config.base_url);
        let mut headers = self.headers(None)?;
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", refresh_token))
                .map_err(|e| AgentError::Llm(format!("invalid refresh token: {}", e)))?,
        );

        let response = self
            .client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .map_err(|e| AgentError::Llm(format!("token refresh failed: {}", e)))?;

        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(AgentError::Llm(format!(
                "token refresh failed ({}): {}",
                status, text
            )));
        }

        let resp: DeepSeekResponse = serde_json::from_str(&text).map_err(|e| {
            AgentError::Llm(format!("token refresh parse failed: {} | {}", e, text))
        })?;

        let biz_data = resp
            .data
            .biz_data
            .ok_or_else(|| AgentError::Llm(format!("token refresh biz_data missing: {}", text)))?;
        biz_data
            .get("token")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| AgentError::Llm("token refresh response missing token".to_string()))
    }

    /// 创建新的 chat session。
    ///
    /// Provider 保持无状态：每次 `complete`/`stream` 都创建独立 session，
    /// 避免多会话共享同一个 DeepSeek session 导致上下文串扰。
    async fn create_chat_session(&self, token: &str) -> Result<String, AgentError> {
        let url = format!("{}/api/v0/chat_session/create", self.config.base_url);
        let response = self
            .client
            .post(&url)
            .headers(self.headers(Some(token))?)
            .json(&json!({"character_id": null}))
            .send()
            .await
            .map_err(|e| AgentError::Llm(format!("create session failed: {}", e)))?;

        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(AgentError::Llm(format!(
                "create session failed ({}): {}",
                status, text
            )));
        }

        let resp: DeepSeekResponse = serde_json::from_str(&text).map_err(|e| {
            AgentError::Llm(format!("session create parse failed: {} | {}", e, text))
        })?;

        resp.data
            .biz_data
            .ok_or_else(|| AgentError::Llm(format!("session create biz_data missing: {}", text)))?
            .get("chat_session")
            .and_then(|s| s.get("id"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| AgentError::Llm("session create response missing id".to_string()))
    }

    /// 获取 PoW 挑战
    async fn create_pow_challenge(&self, token: &str) -> Result<PowChallenge, AgentError> {
        let url = format!("{}/api/v0/chat/create_pow_challenge", self.config.base_url);
        let response = self
            .client
            .post(&url)
            .headers(self.headers(Some(token))?)
            .json(&json!({"target_path": "/api/v0/chat/completion"}))
            .send()
            .await
            .map_err(|e| AgentError::Llm(format!("pow challenge failed: {}", e)))?;

        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(AgentError::Llm(format!(
                "pow challenge failed ({}): {}",
                status, text
            )));
        }

        let resp: DeepSeekResponse = serde_json::from_str(&text).map_err(|e| {
            AgentError::Llm(format!("pow challenge parse failed: {} | {}", e, text))
        })?;

        let challenge_value = resp
            .data
            .biz_data
            .ok_or_else(|| AgentError::Llm(format!("pow challenge biz_data missing: {}", text)))?
            .get("challenge")
            .cloned()
            .ok_or_else(|| AgentError::Llm("pow challenge missing challenge object".to_string()))?;

        serde_json::from_value(challenge_value)
            .map_err(|e| AgentError::Llm(format!("pow challenge deserialize failed: {}", e)))
    }

    /// 调用纯 Rust PoW 求解器
    async fn solve_pow(&self, challenge: &PowChallenge) -> Result<u64, AgentError> {
        debug!(
            "solving PoW algorithm={} difficulty={}",
            challenge.algorithm, challenge.difficulty
        );

        if challenge.algorithm != "DeepSeekHashV1" {
            return Err(AgentError::Llm(format!(
                "unsupported PoW algorithm: {}",
                challenge.algorithm
            )));
        }

        let challenge_hex = challenge.challenge.clone();
        let salt = challenge.salt.clone();
        let expire_at = challenge.expire_at;
        let difficulty = challenge.difficulty;

        let answer = spawn_blocking(move || {
            crate::deepseek_pow::solve_pow_auto(&challenge_hex, &salt, expire_at, difficulty)
        })
        .await
        .map_err(|e| AgentError::Llm(format!("PoW solver task failed: {}", e)))?
        .ok_or_else(|| AgentError::Llm("PoW no solution found within difficulty".to_string()))?;

        debug!("PoW answer: {}", answer);
        Ok(answer)
    }

    /// 构造 x-ds-pow-response header
    fn build_pow_response(
        &self,
        challenge: &PowChallenge,
        answer: u64,
    ) -> Result<String, AgentError> {
        let payload = json!({
            "algorithm": &challenge.algorithm,
            "challenge": &challenge.challenge,
            "salt": &challenge.salt,
            "signature": &challenge.signature,
            "answer": answer,
            "target_path": &challenge.target_path,
        });
        let json_str = serde_json::to_string(&payload)
            .map_err(|e| AgentError::Llm(format!("pow payload serialize failed: {}", e)))?;
        Ok(BASE64.encode(json_str.as_bytes()))
    }

    /// 调用 chat completion，返回 SSE 字节流
    async fn chat_completion_stream(
        &self,
        token: &str,
        session_id: &str,
        prompt: &str,
    ) -> Result<reqwest::Response, AgentError> {
        let challenge = self.create_pow_challenge(token).await?;
        let answer = self.solve_pow(&challenge).await?;
        let pow_response = self.build_pow_response(&challenge, answer)?;

        let url = format!("{}/api/v0/chat/completion", self.config.base_url);
        let mut headers = self.headers(Some(token))?;
        headers.insert(
            "x-ds-pow-response",
            HeaderValue::from_str(&pow_response)
                .map_err(|e| AgentError::Llm(format!("invalid pow response: {}", e)))?,
        );
        headers.insert("accept", HeaderValue::from_static("text/event-stream"));

        let opts = &self.config.options;
        let body = json!({
            "chat_session_id": session_id,
            "parent_message_id": null,
            "prompt": prompt,
            "ref_file_ids": [],
            "thinking_enabled": opts.thinking_enabled,
            "search_enabled": opts.search_enabled,
            "audio_id": null,
            "preempt": false,
            "model_type": &opts.model_type,
            "action": null,
        });

        let response = self
            .client
            .post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .map_err(|e| AgentError::Llm(format!("chat completion request failed: {}", e)))?;

        let status = response.status();
        if !status.is_success() {
            let err = response.text().await.unwrap_or_default();
            return Err(AgentError::Llm(format!(
                "chat completion failed ({}): {}",
                status, err
            )));
        }

        Ok(response)
    }

    /// 解析单条 SSE JSON Patch 事件。
    ///
    /// 返回 `(content_delta, reasoning_delta)`。普通文本进入 `content`，
    /// 深度思考过程进入 `reasoning`。
    fn parse_patch_event(
        &self,
        event: &str,
        buffer: &mut PatchBuffer,
    ) -> (Option<String>, Option<String>) {
        let event = event.trim();
        if event.is_empty() {
            return (None, None);
        }

        // ponytail: DeepSeek device stream terminates with a bare `FINISHED`
        // sentinel (sometimes JSON-encoded as `{"v":"FINISHED"}`). It is not
        // user content and must never be appended to the response.
        if event == "FINISHED" || event == "\"FINISHED\"" {
            return (None, None);
        }

        let value: Value = match serde_json::from_str(event) {
            Ok(v) => v,
            Err(_) => return (None, None),
        };

        // 完整快照事件 {"v": {...}}
        if let Some(v) = value.get("v") {
            if let Some(response) = v.get("response") {
                if let Some(fragments) = response.get("fragments").and_then(|f| f.as_array()) {
                    let mut content = String::new();
                    let mut reasoning = String::new();
                    for fragment in fragments {
                        let fragment_type = fragment
                            .get("fragment_type")
                            .or_else(|| fragment.get("type"))
                            .and_then(|t| t.as_str());
                        let is_reasoning = fragment_type == Some("REASONING");
                        if let Some(text) = fragment.get("content").and_then(|c| c.as_str()) {
                            if is_reasoning {
                                reasoning.push_str(text);
                            } else {
                                content.push_str(text);
                            }
                        }
                    }
                    // 保存最后一个 fragment 的信息用于后续 patch
                    if let Some(last) = fragments.last() {
                        buffer.last_fragment_type = last
                            .get("fragment_type")
                            .or_else(|| last.get("type"))
                            .and_then(|t| t.as_str())
                            .map(|s| s.to_string());
                        if let Some(text) = last.get("content").and_then(|c| c.as_str()) {
                            buffer.last_fragment_content = text.to_string();
                        }
                    }
                    return (
                        if content.is_empty() {
                            None
                        } else {
                            Some(content)
                        },
                        if reasoning.is_empty() {
                            None
                        } else {
                            Some(reasoning)
                        },
                    );
                }
            }
            // 裸值事件 {"v": "xxx"}，追加到最后一个 fragment
            if let Some(text) = v.as_str() {
                if text == "FINISHED" {
                    return (None, None);
                }
                buffer.last_fragment_content.push_str(text);
                if buffer.last_fragment_type.as_deref() == Some("REASONING") {
                    return (None, Some(text.to_string()));
                }
                return (Some(text.to_string()), None);
            }
        }

        // Patch 事件 {"p": "...", "o": "APPEND", "v": "..."}
        if let Some(_path) = value.get("p").and_then(|p| p.as_str()) {
            if value.get("o").and_then(|o| o.as_str()) == Some("APPEND") {
                if let Some(text) = value.get("v").and_then(|v| v.as_str()) {
                    if text == "FINISHED" {
                        return (None, None);
                    }
                    buffer.last_fragment_content.push_str(text);
                    // 根据路径判断类型：/response/fragments/{n}/content
                    // REASONING fragment 的 content 也走同一路径，需依赖缓冲区的类型标记
                    if buffer.last_fragment_type.as_deref() == Some("REASONING") {
                        return (None, Some(text.to_string()));
                    }
                    return (Some(text.to_string()), None);
                }
            }
        }

        (None, None)
    }
}

/// JSON Patch 解析状态缓冲
#[derive(Debug, Default)]
struct PatchBuffer {
    last_fragment_content: String,
    last_fragment_type: Option<String>,
}

impl DeepSeekDeviceProvider {
    /// 执行一次 completion 调用，支持 password 模式下 token 过期后重试一次。
    async fn run_completion_with_retry<F, Fut, T>(
        &self,
        prompt: &str,
        f: F,
    ) -> Result<T, AgentError>
    where
        F: Fn(reqwest::Response) -> Fut,
        Fut: std::future::Future<Output = Result<T, AgentError>>,
    {
        let mut attempts = 0;
        loop {
            let token = self.ensure_token().await?;
            let session_id = self.create_chat_session(&token).await?;
            match self
                .chat_completion_stream(&token, &session_id, prompt)
                .await
            {
                Ok(response) => return f(response).await,
                Err(AgentError::Llm(msg)) if msg.contains("401") || msg.contains("403") => {
                    self.clear_token();
                    if matches!(
                        &self.config.credentials,
                        DeepSeekDeviceCredentials::Password { .. }
                    ) && attempts == 0
                    {
                        debug!("token expired, retrying login once");
                        attempts += 1;
                        continue;
                    }
                    return Err(AgentError::Llm(format!(
                        "auth failed and no retry possible: {}",
                        msg
                    )));
                }
                Err(e) => return Err(e),
            }
        }
    }
}

#[async_trait]
impl LlmProvider for DeepSeekDeviceProvider {
    async fn complete(
        &self,
        messages: &[Message],
        _tools: &Value,
    ) -> Result<LlmResponse, AgentError> {
        let prompt = messages
            .last()
            .filter(|m| m.role == MessageRole::User)
            .map(|m| m.content.clone())
            .unwrap_or_default();

        if prompt.is_empty() {
            return Err(AgentError::Llm("empty user prompt".to_string()));
        }

        self.run_completion_with_retry(&prompt, |response| async move {
            let bytes = response
                .bytes()
                .await
                .map_err(|e| AgentError::Llm(format!("read completion stream failed: {}", e)))?;
            let text = String::from_utf8_lossy(&bytes);

            let mut buffer = PatchBuffer::default();
            let mut content = String::new();
            let mut reasoning = String::new();
            for line in text.lines() {
                if let Some(data) = line.strip_prefix("data:") {
                    let data = data.trim_start();
                    if data == "[DONE]" || data == "FINISHED" {
                        break;
                    }
                    let (c, r) = self.parse_patch_event(data, &mut buffer);
                    if let Some(chunk) = c {
                        content.push_str(&chunk);
                    }
                    if let Some(chunk) = r {
                        reasoning.push_str(&chunk);
                    }
                }
            }

            Ok(LlmResponse {
                content,
                tool_calls: vec![],
                is_complete: true,
            })
        })
        .await
    }

    fn stream(
        &self,
        messages: &[Message],
        _tools: &Value,
    ) -> Result<mpsc::Receiver<Result<StreamDelta, AgentError>>, AgentError> {
        let prompt = messages
            .last()
            .filter(|m| m.role == MessageRole::User)
            .map(|m| m.content.clone())
            .unwrap_or_default();

        if prompt.is_empty() {
            return Err(AgentError::Llm("empty user prompt".to_string()));
        }

        let self_clone = self.clone();
        let (tx, rx) = mpsc::channel(128);

        tokio::spawn(async move {
            let tx_err = tx.clone();
            let result = self_clone
                .run_completion_with_retry(&prompt, {
                    let self_clone2 = self_clone.clone();
                    move |response| {
                        let tx = tx.clone();
                        let self_clone3 = self_clone2.clone();
                        async move {
                            let mut stream = response.bytes_stream();
                            let mut buffer = PatchBuffer::default();
                            let mut line_buffer = String::new();
                            use futures::StreamExt;

                            while let Some(chunk) = stream.next().await {
                                let bytes = chunk.map_err(|e| {
                                    AgentError::Llm(format!("stream chunk error: {}", e))
                                })?;
                                let text = String::from_utf8_lossy(&bytes);
                                line_buffer.push_str(&text);

                                while let Some(pos) = line_buffer.find('\n') {
                                    let line = line_buffer[..pos].to_string();
                                    line_buffer = line_buffer[pos + 1..].to_string();

                                    if let Some(data) = line.strip_prefix("data:") {
                                        let data = data.trim_start();
                                        if data == "[DONE]" || data == "FINISHED" {
                                            return Ok(());
                                        }
                                        let (c, r) =
                                            self_clone3.parse_patch_event(data, &mut buffer);
                                        if (c.is_some() || r.is_some())
                                            && tx
                                                .send(Ok(StreamDelta {
                                                    content: c,
                                                    reasoning_content: r,
                                                    tool_calls: vec![],
                                                }))
                                                .await
                                                .is_err()
                                        {
                                            return Ok(());
                                        }
                                    }
                                }
                            }

                            Ok(())
                        }
                    }
                })
                .await;

            if let Err(e) = result {
                let _ = tx_err.send(Err(e)).await;
            }
        });

        Ok(rx)
    }

    fn set_prompt_cache_key(&self, _key: &str) {
        // 设备端 API 不支持 prompt caching
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            native_tool_calling: false,
            prompt_caching: false,
            vision: false,
            pricing: None,
        }
    }
}

// ==================== API 响应类型 ====================

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct DeepSeekResponse {
    code: i32,
    #[serde(default)]
    msg: String,
    data: DeepSeekBizResponse,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct DeepSeekBizResponse {
    #[serde(default)]
    _biz_code: i32,
    #[serde(default)]
    _biz_msg: String,
    #[serde(default)]
    biz_data: Option<Value>,
}

#[derive(Debug, Deserialize, Clone)]
struct PowChallenge {
    algorithm: String,
    challenge: String,
    salt: String,
    signature: String,
    difficulty: u64,
    #[serde(rename = "expire_at")]
    expire_at: u64,
    #[serde(rename = "target_path")]
    target_path: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_creation_token() {
        let provider = DeepSeekDeviceProvider::with_token("test-token");
        assert!(matches!(
            provider.config.credentials,
            DeepSeekDeviceCredentials::Token(_)
        ));
    }

    #[test]
    fn test_provider_creation_password() {
        let provider = DeepSeekDeviceProvider::with_password("13800138000", "password");
        assert!(matches!(
            provider.config.credentials,
            DeepSeekDeviceCredentials::Password { .. }
        ));
    }

    #[test]
    fn test_capabilities() {
        let provider = DeepSeekDeviceProvider::with_token("test");
        let caps = provider.capabilities();
        assert!(!caps.native_tool_calling);
        assert!(!caps.prompt_caching);
        assert!(!caps.vision);
    }

    #[test]
    fn test_pow_response_encoding() {
        let provider = DeepSeekDeviceProvider::with_token("test");
        let challenge = PowChallenge {
            algorithm: "DeepSeekHashV1".to_string(),
            challenge: "abc".to_string(),
            salt: "salt".to_string(),
            signature: "sig".to_string(),
            difficulty: 1000,
            expire_at: 1234567890,
            target_path: "/api/v0/chat/completion".to_string(),
        };
        let response = provider.build_pow_response(&challenge, 42).unwrap();
        let decoded = BASE64.decode(response).unwrap();
        let json: Value = serde_json::from_slice(&decoded).unwrap();
        assert_eq!(json["answer"], 42);
        assert_eq!(json["algorithm"], "DeepSeekHashV1");
    }

    #[test]
    fn test_parse_patch_events() {
        let provider = DeepSeekDeviceProvider::with_token("test");
        let mut buffer = PatchBuffer::default();

        // 完整快照
        let (content, reasoning) = provider.parse_patch_event(
            r#"{"v":{"response":{"fragments":[{"type":"TEXT","content":"Hello"}]}}}"#,
            &mut buffer,
        );
        assert_eq!(content.as_deref(), Some("Hello"));
        assert_eq!(reasoning, None);

        // APPEND patch
        let (content, reasoning) = provider.parse_patch_event(
            r#"{"p":"response/fragments/-1/content","o":"APPEND","v":"!"}"#,
            &mut buffer,
        );
        assert_eq!(content.as_deref(), Some("!"));
        assert_eq!(reasoning, None);

        // 裸值事件
        let (content, reasoning) = provider.parse_patch_event(r#"{"v":" World"}"#, &mut buffer);
        assert_eq!(content.as_deref(), Some(" World"));
        assert_eq!(reasoning, None);
    }

    #[test]
    fn test_parse_reasoning_patch_events() {
        let provider = DeepSeekDeviceProvider::with_token("test");
        let mut buffer = PatchBuffer::default();

        // 完整快照包含 REASONING fragment
        let (content, reasoning) = provider.parse_patch_event(
            r#"{"v":{"response":{"fragments":[{"type":"REASONING","content":"思考中"}]}}}"#,
            &mut buffer,
        );
        assert_eq!(content, None);
        assert_eq!(reasoning.as_deref(), Some("思考中"));

        // 后续追加到 REASONING fragment
        let (content, reasoning) = provider.parse_patch_event(
            r#"{"p":"response/fragments/-1/content","o":"APPEND","v":"..."}"#,
            &mut buffer,
        );
        assert_eq!(content, None);
        assert_eq!(reasoning.as_deref(), Some("..."));
    }

    #[test]
    fn test_options_from_model_id() {
        let opts = DeepSeekDeviceOptions::from_model_id("deepseek-chat");
        assert_eq!(opts.model_type, "default");
        assert!(!opts.thinking_enabled);

        let opts = DeepSeekDeviceOptions::from_model_id("deepseek-reasoner");
        assert_eq!(opts.model_type, "expert");
        assert!(opts.thinking_enabled);

        let opts = DeepSeekDeviceOptions::from_model_id("deepseek-vision");
        assert_eq!(opts.model_type, "vision");
        assert!(!opts.thinking_enabled);
    }

    #[test]
    fn test_parse_patch_event_ignores_finished_sentinel() {
        let provider = DeepSeekDeviceProvider::with_token("test");
        let mut buffer = PatchBuffer::default();

        // Bare JSON-encoded sentinel.
        let (content, reasoning) = provider.parse_patch_event(r#"{"v":"FINISHED"}"#, &mut buffer);
        assert_eq!(content, None);
        assert_eq!(reasoning, None);

        // Plain SSE data line value.
        let (content, reasoning) = provider.parse_patch_event("FINISHED", &mut buffer);
        assert_eq!(content, None);
        assert_eq!(reasoning, None);

        // Patch form sentinel (should not be treated as content).
        let (content, reasoning) = provider.parse_patch_event(
            r#"{"p":"response/state","o":"APPEND","v":"FINISHED"}"#,
            &mut buffer,
        );
        assert_eq!(content, None);
        assert_eq!(reasoning, None);
    }
}
