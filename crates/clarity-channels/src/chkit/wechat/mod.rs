//! WeChat personal iLink Bot channel.
//!
//! Note: the iLink consent screen ("Connect X to Weixin") shows the bot name
//! from the iLink developer portal, not from Clarity config. Users who
//! register their own iLink bot will see their own name there.

mod api;
mod crypto;
mod media;
mod parsing;
mod state;
mod types;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use parking_lot::Mutex;

use crate::chkit::channel::{Channel, ChannelMessage, SendMessage};
use crate::chkit::pairing::PairingGuard;
use crate::chkit::wechat::types::{
    CDN_BASE_URL, DEFAULT_API_BASE_URL, LONG_POLL_TIMEOUT_MS, https_base_url,
    wechat_cli_string_with_args,
};

/// WeChat iLink Bot channel — long-polls the iLink Bot API for updates.
pub struct WeChatChannel {
    /// Bot token obtained via QR-code login; `None` until first login.
    bot_token: RwLock<Option<String>>,
    /// iLink bot ID (account ID); set after QR login.
    account_id: RwLock<Option<String>>,
    /// API base URL.
    api_base_url: String,
    /// CDN base URL.
    cdn_base_url: String,
    /// The alias key under `[channels.wechat.<alias>]` this handle is
    /// bound to. Used to scope peer-group writes and resolver lookups.
    alias: String,
    /// Resolves inbound external peers from canonical state at message-time.
    /// No cache (see AGENTS.md "ABSOLUTE RULE — SINGLE SOURCE OF TRUTH").
    peer_resolver: Arc<dyn Fn() -> Vec<String> + Send + Sync>,
    /// Pairing guard for /bind flow.
    pairing: Option<PairingGuard>,
    /// HTTP client for API requests.
    client: reqwest::Client,
    /// Per-user context_token cache (accountId:userId -> token).
    context_tokens: Mutex<HashMap<String, String>>,
    /// Per-user typing_ticket cache (userId -> ticket).
    typing_tickets: Mutex<HashMap<String, String>>,
    /// Persisted getUpdates cursor.
    cursor: Mutex<String>,
    /// Typing indicator task handle.
    typing_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
    /// State directory for persisting token & cursor.
    state_dir: PathBuf,
    /// Workspace directory used for storing inbound attachments and resolving
    /// `/workspace/...` paths from generated replies.
    workspace_dir: Option<PathBuf>,
}

impl WeChatChannel {
    /// Create a new WeChat iLink channel handle.
    ///
    /// `peer_resolver` returns the current allowlist. When empty, the channel
    /// enters pairing mode and prints a one-time bind code to the terminal.
    pub fn new(
        alias: impl Into<String>,
        peer_resolver: Arc<dyn Fn() -> Vec<String> + Send + Sync>,
        api_base_url: Option<String>,
        cdn_base_url: Option<String>,
        state_dir: Option<PathBuf>,
    ) -> anyhow::Result<Self> {
        let api_base_url = https_base_url("api_base_url", api_base_url, DEFAULT_API_BASE_URL)?;
        let cdn_base_url = https_base_url("cdn_base_url", cdn_base_url, CDN_BASE_URL)?;

        let has_peers = !peer_resolver().is_empty();
        let pairing = if has_peers {
            None
        } else {
            let guard = PairingGuard::new(true, &[]);
            if let Some(code) = guard.pairing_code() {
                println!(
                    "  {}",
                    wechat_cli_string_with_args("cli-wechat-pairing-required", &[("code", &code)],)
                );
                println!(
                    "     {}",
                    wechat_cli_string_with_args(
                        "cli-wechat-send-bind-command",
                        &[("command", crate::chkit::wechat::types::WECHAT_BIND_COMMAND)],
                    )
                );
            }
            Some(guard)
        };

        let state_dir = state_dir.unwrap_or_else(|| {
            dirs::home_dir()
                .map(|u| u.join(".clarity").join("wechat"))
                .unwrap_or_else(|| PathBuf::from(".clarity/wechat"))
        });

        let mut channel = Self {
            bot_token: RwLock::new(None),
            account_id: RwLock::new(None),
            api_base_url,
            cdn_base_url,
            alias: alias.into(),
            peer_resolver,
            pairing,
            client: reqwest::Client::new(),
            context_tokens: Mutex::new(HashMap::new()),
            typing_tickets: Mutex::new(HashMap::new()),
            cursor: Mutex::new(String::new()),
            typing_handle: Mutex::new(None),
            state_dir,
            workspace_dir: None,
        };

        // Try to load persisted state
        channel.load_persisted_state();
        Ok(channel)
    }

    /// Set the workspace directory used to resolve `/workspace/...` attachment
    /// paths and store inbound files.
    pub fn with_workspace_dir(mut self, dir: PathBuf) -> Self {
        self.workspace_dir = Some(dir);
        self
    }

    pub(crate) fn has_token(&self) -> bool {
        self.bot_token.read().map(|t| t.is_some()).unwrap_or(false)
    }

    pub(crate) fn get_token(&self) -> Option<String> {
        self.bot_token.read().ok().and_then(|t| t.clone())
    }

    pub(crate) fn set_context_token(&self, user_id: &str, token: &str) {
        self.context_tokens
            .lock()
            .insert(user_id.to_string(), token.to_string());
        self.save_sync_data();
    }

    pub(crate) fn get_context_token(&self, user_id: &str) -> Option<String> {
        self.context_tokens.lock().get(user_id).cloned()
    }

    pub(crate) fn is_user_allowed(&self, user_id: &str) -> bool {
        let peers = (self.peer_resolver)();
        crate::chkit::allowlist::is_user_allowed(
            &peers,
            user_id,
            crate::chkit::allowlist::Match::Sensitive,
        )
    }

    pub(crate) fn extract_bind_code(text: &str) -> Option<&str> {
        let mut parts = text.split_whitespace();
        let command = parts.next()?;
        if command != crate::chkit::wechat::types::WECHAT_BIND_COMMAND {
            return None;
        }
        parts.next().map(str::trim).filter(|code| !code.is_empty())
    }
}

#[async_trait]
impl Channel for WeChatChannel {
    fn name(&self) -> &str {
        "wechat"
    }

    async fn send(&self, message: &SendMessage) -> anyhow::Result<()> {
        let recipient = &message.recipient;
        let content = crate::chkit::util::strip_tool_call_tags(&message.content);
        let context_token = self.get_context_token(recipient);

        if context_token.is_none() {
            crate::record!(
                WARN,
                crate::chkit::log::Event::new(module_path!(), crate::chkit::log::Action::Note)
                    .with_outcome(crate::chkit::log::EventOutcome::Unknown)
                    .with_attrs(::serde_json::json!({"recipient": recipient})),
                "no context_token for , message may fail to associate"
            );
        }

        let (text_without_markers, attachments) =
            crate::chkit::wechat::parsing::parse_attachment_markers(&content);
        if !attachments.is_empty() {
            if !text_without_markers.is_empty() {
                self.send_text(recipient, &text_without_markers, context_token.as_deref())
                    .await?;
            }

            for attachment in &attachments {
                self.send_attachment(recipient, attachment, context_token.as_deref())
                    .await?;
            }
            return Ok(());
        }

        if let Some(attachment) =
            crate::chkit::wechat::parsing::parse_path_only_attachment(&content)
        {
            return self
                .send_attachment(recipient, &attachment, context_token.as_deref())
                .await;
        }

        self.send_text(recipient, &content, context_token.as_deref())
            .await
    }

    async fn listen(&self, tx: tokio::sync::mpsc::Sender<ChannelMessage>) -> anyhow::Result<()> {
        // Ensure we're logged in (QR scan if needed)
        self.ensure_logged_in().await?;

        crate::record!(
            INFO,
            crate::chkit::log::Event::new(module_path!(), crate::chkit::log::Action::Note),
            "channel listening for messages..."
        );

        let mut cursor = self.cursor.lock().clone();
        let mut long_poll_timeout_ms = LONG_POLL_TIMEOUT_MS;
        let mut consecutive_failures: u32 = 0;

        loop {
            let token = match self.get_token() {
                Some(t) => t,
                None => {
                    crate::record!(
                        ERROR,
                        crate::chkit::log::Event::new(
                            module_path!(),
                            crate::chkit::log::Action::Fail
                        )
                        .with_outcome(crate::chkit::log::EventOutcome::Failure),
                        "token lost, attempting re-login..."
                    );
                    if let Err(e) = self.ensure_logged_in().await {
                        crate::record!(
                            ERROR,
                            crate::chkit::log::Event::new(
                                module_path!(),
                                crate::chkit::log::Action::Fail
                            )
                            .with_outcome(crate::chkit::log::EventOutcome::Failure)
                            .with_attrs(::serde_json::json!({"error": format!("{}", e)})),
                            "re-login failed"
                        );
                        tokio::time::sleep(crate::chkit::wechat::types::BACKOFF_DELAY).await;
                        continue;
                    }
                    match self.get_token() {
                        Some(t) => t,
                        None => {
                            tokio::time::sleep(crate::chkit::wechat::types::BACKOFF_DELAY).await;
                            continue;
                        }
                    }
                }
            };

            let body = serde_json::json!({
                "get_updates_buf": cursor,
                "base_info": crate::chkit::wechat::types::build_base_info()
            });

            let result = tokio::time::timeout(
                crate::chkit::wechat::types::long_poll_client_timeout(long_poll_timeout_ms),
                self.client
                    .post(self.api_url("getupdates"))
                    .headers(crate::chkit::wechat::api::build_headers(Some(&token)))
                    .json(&body)
                    .timeout(std::time::Duration::from_millis(long_poll_timeout_ms))
                    .send(),
            )
            .await;

            let resp = match result {
                Ok(Ok(r)) => r,
                Ok(Err(e)) => {
                    consecutive_failures += 1;
                    crate::record!(WARN, crate::chkit::log::Event::new(module_path!(), crate::chkit::log::Action::Note).with_outcome(crate::chkit::log::EventOutcome::Unknown).with_attrs(::serde_json::json!({"consecutive_failures": consecutive_failures, "MAX_CONSECUTIVE_FAILURES": crate::chkit::wechat::types::MAX_CONSECUTIVE_FAILURES, "e": e.to_string()})), "getUpdates error (/)");
                    if consecutive_failures >= crate::chkit::wechat::types::MAX_CONSECUTIVE_FAILURES
                    {
                        consecutive_failures = 0;
                        tokio::time::sleep(crate::chkit::wechat::types::BACKOFF_DELAY).await;
                    } else {
                        tokio::time::sleep(crate::chkit::wechat::types::RETRY_DELAY).await;
                    }
                    continue;
                }
                Err(_) => {
                    // Client-side timeout — normal for long-poll, just retry
                    crate::record!(
                        DEBUG,
                        crate::chkit::log::Event::new(
                            module_path!(),
                            crate::chkit::log::Action::Note
                        ),
                        "getUpdates: client-side timeout, retrying"
                    );
                    continue;
                }
            };

            let data: serde_json::Value = match resp.json().await {
                Ok(v) => v,
                Err(e) => {
                    consecutive_failures += 1;
                    crate::record!(
                        WARN,
                        crate::chkit::log::Event::new(
                            module_path!(),
                            crate::chkit::log::Action::Note
                        )
                        .with_outcome(crate::chkit::log::EventOutcome::Unknown)
                        .with_attrs(::serde_json::json!({"error": format!("{}", e)})),
                        "getUpdates parse error"
                    );
                    if consecutive_failures >= crate::chkit::wechat::types::MAX_CONSECUTIVE_FAILURES
                    {
                        consecutive_failures = 0;
                        tokio::time::sleep(crate::chkit::wechat::types::BACKOFF_DELAY).await;
                    } else {
                        tokio::time::sleep(crate::chkit::wechat::types::RETRY_DELAY).await;
                    }
                    continue;
                }
            };

            // Check for API errors
            let ret = data.get("ret").and_then(|v| v.as_i64()).unwrap_or(0);
            let errcode = data.get("errcode").and_then(|v| v.as_i64()).unwrap_or(0);
            let is_error = ret != 0 || errcode != 0;

            if is_error {
                if errcode == crate::chkit::wechat::types::SESSION_EXPIRED_ERRCODE
                    || ret == crate::chkit::wechat::types::SESSION_EXPIRED_ERRCODE
                {
                    crate::record!(
                        ERROR,
                        crate::chkit::log::Event::new(
                            module_path!(),
                            crate::chkit::log::Action::Fail
                        )
                        .with_outcome(crate::chkit::log::EventOutcome::Failure),
                        &format!(
                            "session expired (errcode {}), pausing for {} min",
                            crate::chkit::wechat::types::SESSION_EXPIRED_ERRCODE,
                            crate::chkit::wechat::types::SESSION_PAUSE_DURATION.as_secs() / 60
                        )
                    );
                    // Clear token so we re-login after pause
                    if let Ok(mut t) = self.bot_token.write() {
                        *t = None;
                    }
                    self.context_tokens.lock().clear();
                    self.save_sync_data();
                    tokio::time::sleep(crate::chkit::wechat::types::SESSION_PAUSE_DURATION).await;
                    // Try to re-login
                    if let Err(e) = self.ensure_logged_in().await {
                        crate::record!(
                            ERROR,
                            crate::chkit::log::Event::new(
                                module_path!(),
                                crate::chkit::log::Action::Fail
                            )
                            .with_outcome(crate::chkit::log::EventOutcome::Failure)
                            .with_attrs(::serde_json::json!({"error": format!("{}", e)})),
                            "re-login after session expiry failed"
                        );
                    }
                    consecutive_failures = 0;
                    continue;
                }

                consecutive_failures += 1;
                let errmsg = data.get("errmsg").and_then(|v| v.as_str()).unwrap_or("");
                crate::record!(WARN, crate::chkit::log::Event::new(module_path!(), crate::chkit::log::Action::Note).with_outcome(crate::chkit::log::EventOutcome::Unknown).with_attrs(::serde_json::json!({"ret": ret, "errcode": errcode, "errmsg": errmsg, "consecutive_failures": consecutive_failures, "MAX_CONSECUTIVE_FAILURES": crate::chkit::wechat::types::MAX_CONSECUTIVE_FAILURES})), "getUpdates failed: ret= errcode= errmsg= (/)");
                if consecutive_failures >= crate::chkit::wechat::types::MAX_CONSECUTIVE_FAILURES {
                    consecutive_failures = 0;
                    tokio::time::sleep(crate::chkit::wechat::types::BACKOFF_DELAY).await;
                } else {
                    tokio::time::sleep(crate::chkit::wechat::types::RETRY_DELAY).await;
                }
                continue;
            }

            consecutive_failures = 0;

            // Update cursor
            if let Some(new_cursor) = data
                .get("get_updates_buf")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
            {
                cursor = new_cursor.to_string();
                *self.cursor.lock() = cursor.clone();
                self.save_sync_data();
            }

            if let Some(next_timeout) = data
                .get("longpolling_timeout_ms")
                .and_then(|v| v.as_u64())
                .filter(|timeout| *timeout > 0)
            {
                long_poll_timeout_ms = next_timeout;
            }

            // Process messages
            let msgs = data
                .get("msgs")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            for msg in &msgs {
                let from_user_id = msg
                    .get("from_user_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if from_user_id.is_empty() {
                    continue;
                }

                // Cache context_token
                if let Some(ctx_token) = msg.get("context_token").and_then(|v| v.as_str())
                    && !ctx_token.is_empty()
                {
                    self.set_context_token(from_user_id, ctx_token);
                }

                let items = msg
                    .get("item_list")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();

                let message_id = msg
                    .get("message_id")
                    .and_then(|v| v.as_u64())
                    .map(|id| id.to_string())
                    .unwrap_or_else(|| format!("wechat_{}", uuid::Uuid::new_v4()));

                let text = crate::chkit::wechat::parsing::extract_text_from_items(&items);

                // Check authorization
                if !self.is_user_allowed(from_user_id) {
                    self.handle_unauthorized_message(from_user_id, &text).await;
                    continue;
                }

                let attachment_content =
                    self.try_build_attachment_content(&items, &message_id).await;
                let content = match (attachment_content, text.is_empty()) {
                    (Some(marker), true) => marker,
                    (Some(marker), false) => format!("{marker}\n\n{text}"),
                    (None, false) => text,
                    (None, true) => continue,
                };

                let timestamp = msg
                    .get("create_time_ms")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0)
                    / 1000; // Convert to seconds

                let channel_msg = ChannelMessage {
                    id: message_id,
                    sender: from_user_id.to_string(),
                    reply_target: from_user_id.to_string(),
                    content,
                    channel: "wechat".to_string(),
                    channel_alias: Some(self.alias.clone()),
                    timestamp,
                    thread_ts: None,
                    interruption_scope_id: None,
                    attachments: Vec::new(),
                    subject: None,
                };

                if tx.send(channel_msg).await.is_err() {
                    crate::record!(
                        INFO,
                        crate::chkit::log::Event::new(
                            module_path!(),
                            crate::chkit::log::Action::Note
                        ),
                        "channel receiver dropped, stopping"
                    );
                    return Ok(());
                }
            }
        }
    }

    async fn health_check(&self) -> bool {
        self.health_check().await
    }

    async fn start_typing(&self, recipient: &str) -> anyhow::Result<()> {
        self.start_typing(recipient).await
    }

    async fn stop_typing(&self, recipient: &str) -> anyhow::Result<()> {
        self.stop_typing(recipient).await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    #[test]
    fn wechat_channel_name() {
        let ch = WeChatChannel::new(
            "wechat_test_alias",
            Arc::new(|| vec!["*".into()]),
            None,
            None,
            Some("/tmp/test-wechat".into()),
        )
        .unwrap();
        assert_eq!(ch.name(), "wechat");
    }

    #[test]
    fn wechat_channel_rejects_http_api_base_url() {
        let result = WeChatChannel::new(
            "wechat_test_alias",
            Arc::new(|| vec!["*".into()]),
            Some("http://ilink.example.test".into()),
            None,
            Some("/tmp/test-wechat".into()),
        );
        assert!(result.is_err());

        let err = result.err().unwrap();
        assert!(err.to_string().contains("api_base_url must use https://"));
    }

    #[test]
    fn wechat_channel_rejects_http_cdn_base_url() {
        let result = WeChatChannel::new(
            "wechat_test_alias",
            Arc::new(|| vec!["*".into()]),
            None,
            Some("http://cdn.example.test".into()),
            Some("/tmp/test-wechat".into()),
        );
        assert!(result.is_err());

        let err = result.err().unwrap();
        assert!(err.to_string().contains("cdn_base_url must use https://"));
    }

    #[test]
    fn extract_bind_code_valid() {
        assert_eq!(
            WeChatChannel::extract_bind_code("/bind ABC123"),
            Some("ABC123")
        );
    }

    #[test]
    fn extract_bind_code_no_code() {
        assert_eq!(WeChatChannel::extract_bind_code("/bind"), None);
    }

    #[test]
    fn extract_bind_code_wrong_command() {
        assert_eq!(WeChatChannel::extract_bind_code("/start"), None);
    }

    #[test]
    fn is_user_allowed_wildcard() {
        let ch = WeChatChannel::new(
            "wechat_test_alias",
            Arc::new(|| vec!["*".into()]),
            None,
            None,
            Some("/tmp/test-wechat".into()),
        )
        .unwrap();
        assert!(ch.is_user_allowed("anyone@im.wechat"));
    }

    #[test]
    fn is_user_allowed_specific() {
        let ch = WeChatChannel::new(
            "wechat_test_alias",
            Arc::new(|| vec!["user1@im.wechat".into()]),
            None,
            None,
            Some("/tmp/test-wechat".into()),
        )
        .unwrap();
        assert!(ch.is_user_allowed("user1@im.wechat"));
        assert!(!ch.is_user_allowed("user2@im.wechat"));
    }
}
