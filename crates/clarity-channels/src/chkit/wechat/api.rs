//! iLink API client methods for the WeChat channel.

use std::time::Duration;

use anyhow::Context;
use base64::Engine;

use crate::chkit::wechat::{
    WeChatChannel,
    parsing::markdown_to_plain_text,
    types::{
        API_TIMEOUT, ITEM_TYPE_FILE, ITEM_TYPE_IMAGE, ITEM_TYPE_TEXT, ITEM_TYPE_VIDEO,
        MAX_QR_REFRESH, QR_POLL_TIMEOUT, QR_SCAN_TIMEOUT, WeChatAttachment, WeChatAttachmentKind,
        build_base_info, wechat_cli_string, wechat_cli_string_with_args,
    },
};

/// Generate a random X-WECHAT-UIN header value.
pub(crate) fn random_wechat_uin() -> String {
    let bytes: [u8; 4] = rand::random();
    let uint32 = u32::from_be_bytes(bytes);
    base64::engine::general_purpose::STANDARD.encode(uint32.to_string())
}

/// Build common request headers for iLink API.
pub(crate) fn build_headers(token: Option<&str>) -> reqwest::header::HeaderMap {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::CONTENT_TYPE,
        reqwest::header::HeaderValue::from_static("application/json"),
    );
    headers.insert(
        "AuthorizationType",
        reqwest::header::HeaderValue::from_static("ilink_bot_token"),
    );
    if let Ok(uin) = reqwest::header::HeaderValue::from_str(&random_wechat_uin()) {
        headers.insert("X-WECHAT-UIN", uin);
    }
    if let Some(t) = token
        && !t.is_empty()
        && let Ok(val) = format!("Bearer {t}").parse()
    {
        headers.insert("Authorization", val);
    }
    headers
}

fn render_login_qr(code: &str) -> anyhow::Result<String> {
    let payload = code.trim();
    if payload.is_empty() {
        anyhow::bail!("QR payload is empty");
    }

    let qr = qrcode::QrCode::new(payload.as_bytes()).map_err(|err| {
        crate::record!(
            ERROR,
            crate::chkit::log::Event::new(module_path!(), crate::chkit::log::Action::Fail)
                .with_outcome(crate::chkit::log::EventOutcome::Failure)
                .with_attrs(::serde_json::json!({"error": format!("{}", err)})),
            "Failed to encode WeChat QR payload"
        );
        anyhow::Error::msg(format!("Failed to encode WeChat QR payload: {err}"))
    })?;

    Ok(qr
        .render::<qrcode::render::unicode::Dense1x2>()
        .quiet_zone(true)
        .build())
}

impl WeChatChannel {
    pub(crate) fn api_url(&self, endpoint: &str) -> String {
        let base = self.api_base_url.trim_end_matches('/');
        format!("{base}/ilink/bot/{endpoint}")
    }

    /// Perform QR-code login flow. Returns (bot_token, account_id, user_id).
    pub(crate) async fn qr_login(&self) -> anyhow::Result<(String, String, Option<String>)> {
        let mut qr_refresh_count = 0u32;

        loop {
            qr_refresh_count += 1;
            if qr_refresh_count > MAX_QR_REFRESH {
                let max = MAX_QR_REFRESH.to_string();
                anyhow::bail!(
                    "{}",
                    wechat_cli_string_with_args(
                        "cli-wechat-qr-expired-giving-up",
                        &[("max", &max)],
                    )
                );
            }

            // Fetch QR code
            let qr_url = format!("{}?bot_type=3", self.api_url("get_bot_qrcode"));
            let resp = self
                .client
                .get(&qr_url)
                .timeout(API_TIMEOUT)
                .send()
                .await
                .with_context(|| wechat_cli_string("cli-wechat-qr-fetch-failed"))?;

            if !resp.status().is_success() {
                let status = resp.status().to_string();
                let body = resp.text().await.unwrap_or_default();
                anyhow::bail!(
                    "{}",
                    wechat_cli_string_with_args(
                        "cli-wechat-qr-fetch-status-failed",
                        &[("status", &status), ("body", &body)],
                    )
                );
            }

            let qr_data: serde_json::Value = resp.json().await?;
            let qrcode = qr_data
                .get("qrcode")
                .and_then(|v| v.as_str())
                .with_context(|| {
                    wechat_cli_string_with_args(
                        "cli-wechat-missing-response-field",
                        &[("field", "qrcode")],
                    )
                })?
                .to_string();
            let qrcode_img_url = qr_data
                .get("qrcode_img_content")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            // Display QR code
            let qr_attempt = qr_refresh_count.to_string();
            let qr_max = MAX_QR_REFRESH.to_string();
            println!(
                "\n  {}",
                wechat_cli_string_with_args(
                    "cli-wechat-qr-login",
                    &[("attempt", &qr_attempt), ("max", &qr_max)],
                )
            );
            println!("  {}\n", wechat_cli_string("cli-wechat-scan-to-connect"));
            let qr_payload = if qrcode_img_url.is_empty() {
                qrcode.as_str()
            } else {
                qrcode_img_url
            };
            match render_login_qr(qr_payload) {
                Ok(qr) => println!("{qr}"),
                Err(err) => {
                    crate::record!(
                        WARN,
                        crate::chkit::log::Event::new(
                            module_path!(),
                            crate::chkit::log::Action::Note
                        )
                        .with_outcome(crate::chkit::log::EventOutcome::Unknown)
                        .with_attrs(::serde_json::json!({"error": format!("{}", err)})),
                        "failed to render terminal QR code"
                    )
                }
            }
            if !qrcode_img_url.is_empty() {
                println!(
                    "  {}",
                    wechat_cli_string_with_args("cli-wechat-qr-url", &[("url", qrcode_img_url)],)
                );
            }

            // Poll for scan status
            let deadline = std::time::Instant::now() + QR_SCAN_TIMEOUT;
            let mut scanned_printed = false;

            while std::time::Instant::now() < deadline {
                let status_url = format!(
                    "{}?qrcode={}",
                    self.api_url("get_qrcode_status"),
                    urlencoding::encode(&qrcode)
                );
                let mut headers = reqwest::header::HeaderMap::new();
                headers.insert(
                    "iLink-App-ClientVersion",
                    reqwest::header::HeaderValue::from_static("1"),
                );

                let poll_result = tokio::time::timeout(
                    QR_POLL_TIMEOUT + Duration::from_secs(5),
                    self.client
                        .get(&status_url)
                        .headers(headers)
                        .timeout(QR_POLL_TIMEOUT)
                        .send(),
                )
                .await;

                let resp = match poll_result {
                    Ok(Ok(r)) => r,
                    Ok(Err(e)) => {
                        crate::record!(
                            DEBUG,
                            crate::chkit::log::Event::new(
                                module_path!(),
                                crate::chkit::log::Action::Note
                            )
                            .with_attrs(::serde_json::json!({"error": format!("{}", e)})),
                            "QR poll error"
                        );
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        continue;
                    }
                    Err(_) => {
                        // Client-side timeout, normal for long-poll
                        continue;
                    }
                };

                let status: serde_json::Value = match resp.json().await {
                    Ok(v) => v,
                    Err(e) => {
                        crate::record!(
                            DEBUG,
                            crate::chkit::log::Event::new(
                                module_path!(),
                                crate::chkit::log::Action::Note
                            )
                            .with_attrs(::serde_json::json!({"error": format!("{}", e)})),
                            "QR poll parse error"
                        );
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        continue;
                    }
                };

                let status_str = status
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("wait");

                match status_str {
                    "wait" => {}
                    "scaned" => {
                        if !scanned_printed {
                            println!("  {}", wechat_cli_string("cli-wechat-scanned-confirm"));
                            scanned_printed = true;
                        }
                    }
                    "expired" => {
                        println!(
                            "  {}",
                            wechat_cli_string("cli-wechat-qr-expired-refreshing")
                        );
                        break; // Will loop back and get a new QR code
                    }
                    "confirmed" => {
                        let bot_token = status
                            .get("bot_token")
                            .and_then(|v| v.as_str())
                            .with_context(|| {
                                wechat_cli_string_with_args(
                                    "cli-wechat-login-confirmed-missing-field",
                                    &[("field", "bot_token")],
                                )
                            })?
                            .to_string();
                        let account_id = status
                            .get("ilink_bot_id")
                            .and_then(|v| v.as_str())
                            .with_context(|| {
                                wechat_cli_string_with_args(
                                    "cli-wechat-login-confirmed-missing-field",
                                    &[("field", "ilink_bot_id")],
                                )
                            })?
                            .to_string();
                        let user_id = status
                            .get("ilink_user_id")
                            .and_then(|v| v.as_str())
                            .map(String::from);

                        println!("  {}", wechat_cli_string("cli-wechat-connected"));
                        return Ok((bot_token, account_id, user_id));
                    }
                    other => {
                        crate::record!(
                            DEBUG,
                            crate::chkit::log::Event::new(
                                module_path!(),
                                crate::chkit::log::Action::Note
                            )
                            .with_attrs(::serde_json::json!({"other": other})),
                            "QR status"
                        );
                    }
                }

                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            // If we reach here without returning, the QR expired or timed out.
            // Loop will try again up to MAX_QR_REFRESH times.
        }
    }

    /// Ensure we have a valid bot token, performing QR login if needed.
    pub(crate) async fn ensure_logged_in(&self) -> anyhow::Result<()> {
        if self.has_token() {
            return Ok(());
        }

        crate::record!(
            INFO,
            crate::chkit::log::Event::new(module_path!(), crate::chkit::log::Action::Note),
            "no persisted token, starting QR login..."
        );
        let (token, account_id, user_id) = self.qr_login().await?;

        // Save to memory
        if let Ok(mut t) = self.bot_token.write() {
            *t = Some(token.clone());
        }
        if let Ok(mut a) = self.account_id.write() {
            *a = Some(account_id.clone());
        }

        // If a user scanned, persist them as an allowed peer
        if let Some(ref uid) = user_id
            && let Err(e) = self.persist_allowed_identity(uid).await
        {
            crate::record!(
                WARN,
                crate::chkit::log::Event::new(module_path!(), crate::chkit::log::Action::Note)
                    .with_outcome(crate::chkit::log::EventOutcome::Unknown)
                    .with_attrs(::serde_json::json!({"error": format!("{}", e), "uid": uid})),
                "failed to persist scanned identity"
            );
        }

        // Persist to disk
        self.save_account_data(&token, &account_id, user_id.as_deref());

        Ok(())
    }

    pub(crate) async fn send_message_items(
        &self,
        to: &str,
        item_list: Vec<serde_json::Value>,
        context_token: Option<&str>,
    ) -> anyhow::Result<()> {
        let token = self.get_token().context("not logged in, cannot send")?;

        let client_id = format!("clarity-{}", uuid::Uuid::new_v4());
        let body = serde_json::json!({
            "msg": {
                "from_user_id": "",
                "to_user_id": to,
                "client_id": client_id,
                "message_type": crate::chkit::wechat::types::MESSAGE_TYPE_BOT,
                "message_state": crate::chkit::wechat::types::MESSAGE_STATE_FINISH,
                "item_list": item_list,
                "context_token": context_token.unwrap_or("")
            },
            "base_info": build_base_info()
        });

        let resp = self
            .client
            .post(self.api_url("sendmessage"))
            .headers(build_headers(Some(&token)))
            .json(&body)
            .timeout(API_TIMEOUT)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let err = resp.text().await.unwrap_or_default();
            anyhow::bail!("sendMessage failed ({status}): {err}");
        }

        Ok(())
    }

    /// Send a text message via iLink API.
    pub(crate) async fn send_text(
        &self,
        to: &str,
        text: &str,
        context_token: Option<&str>,
    ) -> anyhow::Result<()> {
        self.send_message_items(
            to,
            vec![serde_json::json!({
                "type": ITEM_TYPE_TEXT,
                "text_item": { "text": markdown_to_plain_text(text) }
            })],
            context_token,
        )
        .await
    }

    pub(crate) async fn send_attachment(
        &self,
        to: &str,
        attachment: &WeChatAttachment,
        context_token: Option<&str>,
    ) -> anyhow::Result<()> {
        let payload = self.load_attachment_payload(attachment).await?;
        let uploaded = self
            .upload_media_payload(to, attachment.kind, &payload)
            .await?;

        let item = match attachment.kind {
            WeChatAttachmentKind::Image => serde_json::json!({
                "type": ITEM_TYPE_IMAGE,
                "image_item": {
                    "media": {
                        "encrypt_query_param": uploaded.encrypted_query_param,
                        "aes_key": uploaded.aes_key_base64,
                        "encrypt_type": 1
                    },
                    "mid_size": uploaded.encrypted_size
                }
            }),
            WeChatAttachmentKind::Video => serde_json::json!({
                "type": ITEM_TYPE_VIDEO,
                "video_item": {
                    "media": {
                        "encrypt_query_param": uploaded.encrypted_query_param,
                        "aes_key": uploaded.aes_key_base64,
                        "encrypt_type": 1
                    },
                    "video_size": uploaded.encrypted_size
                }
            }),
            WeChatAttachmentKind::Document
            | WeChatAttachmentKind::Audio
            | WeChatAttachmentKind::Voice => serde_json::json!({
                "type": ITEM_TYPE_FILE,
                "file_item": {
                    "media": {
                        "encrypt_query_param": uploaded.encrypted_query_param,
                        "aes_key": uploaded.aes_key_base64,
                        "encrypt_type": 1
                    },
                    "file_name": payload.file_name,
                    "len": uploaded.raw_size.to_string()
                }
            }),
        };

        self.send_message_items(to, vec![item], context_token).await
    }

    /// Fetch typing_ticket for a user via getconfig.
    pub(crate) async fn fetch_typing_ticket(&self, user_id: &str) -> Option<String> {
        let token = self.get_token()?;
        let context_token = self.get_context_token(user_id);

        let body = serde_json::json!({
            "ilink_user_id": user_id,
            "context_token": context_token.unwrap_or_default(),
            "base_info": build_base_info()
        });

        let resp = self
            .client
            .post(self.api_url("getconfig"))
            .headers(build_headers(Some(&token)))
            .json(&body)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .ok()?;

        let data: serde_json::Value = resp.json().await.ok()?;
        data.get("typing_ticket")
            .and_then(|v| v.as_str())
            .map(String::from)
    }

    /// Get or fetch typing_ticket for a user.
    pub(crate) async fn get_typing_ticket(&self, user_id: &str) -> Option<String> {
        // Check cache first
        if let Some(ticket) = self.typing_tickets.lock().get(user_id).cloned() {
            return Some(ticket);
        }

        // Fetch and cache
        let ticket = self.fetch_typing_ticket(user_id).await?;
        self.typing_tickets
            .lock()
            .insert(user_id.to_string(), ticket.clone());
        Some(ticket)
    }

    /// Handle an unauthorized message (check for /bind command).
    pub(crate) async fn handle_unauthorized_message(&self, from_user_id: &str, text: &str) {
        if let Some(code) = Self::extract_bind_code(text) {
            if let Some(pairing) = self.pairing.as_ref() {
                match pairing.try_pair(code, from_user_id).await {
                    Ok(Some(_token)) => {
                        if let Err(e) = self.persist_allowed_identity(from_user_id).await {
                            crate::record!(WARN, crate::chkit::log::Event::new(module_path!(), crate::chkit::log::Action::Note).with_outcome(crate::chkit::log::EventOutcome::Unknown).with_attrs(::serde_json::json!({"from_user_id": from_user_id, "e": e.to_string()})), "failed to persist bound identity");
                        }
                        let ctx = self.get_context_token(from_user_id);
                        let reply = wechat_cli_string("cli-wechat-bound-success");
                        let _ = self.send_text(from_user_id, &reply, ctx.as_deref()).await;
                        crate::record!(
                            INFO,
                            crate::chkit::log::Event::new(
                                module_path!(),
                                crate::chkit::log::Action::Note
                            )
                            .with_attrs(::serde_json::json!({"from_user_id": from_user_id})),
                            "user bound via pairing code"
                        );
                    }
                    Ok(None) => {
                        let ctx = self.get_context_token(from_user_id);
                        let reply = wechat_cli_string("cli-wechat-invalid-bind-code");
                        let _ = self.send_text(from_user_id, &reply, ctx.as_deref()).await;
                    }
                    Err(e) => {
                        crate::record!(
                            WARN,
                            crate::chkit::log::Event::new(
                                module_path!(),
                                crate::chkit::log::Action::Note
                            )
                            .with_outcome(crate::chkit::log::EventOutcome::Unknown)
                            .with_attrs(::serde_json::json!({"error": format!("{}", e)})),
                            "pairing error"
                        );
                    }
                }
            }
        } else {
            crate::record!(
                DEBUG,
                crate::chkit::log::Event::new(module_path!(), crate::chkit::log::Action::Note)
                    .with_attrs(::serde_json::json!({"from_user_id": from_user_id})),
                "ignoring unauthorized message from"
            );
        }
    }

    pub(crate) async fn health_check(&self) -> bool {
        let token = match self.get_token() {
            Some(t) => t,
            None => return false,
        };

        // Use getconfig with a dummy user as a health check
        let body = serde_json::json!({
            "ilink_user_id": "",
            "context_token": "",
            "base_info": build_base_info()
        });

        match tokio::time::timeout(
            Duration::from_secs(5),
            self.client
                .post(self.api_url("getconfig"))
                .headers(build_headers(Some(&token)))
                .json(&body)
                .send(),
        )
        .await
        {
            Ok(Ok(resp)) => resp.status().is_success(),
            _ => false,
        }
    }

    pub(crate) async fn start_typing(&self, recipient: &str) -> anyhow::Result<()> {
        self.stop_typing(recipient).await?;

        let token = match self.get_token() {
            Some(t) => t,
            None => return Ok(()),
        };

        let typing_ticket = match self.get_typing_ticket(recipient).await {
            Some(t) => t,
            None => return Ok(()),
        };

        let client = self.client.clone();
        let url = self.api_url("sendtyping");
        let user_id = recipient.to_string();

        let handle = crate::spawn!(async move {
            loop {
                let body = serde_json::json!({
                    "ilink_user_id": &user_id,
                    "typing_ticket": &typing_ticket,
                    "status": 1,
                    "base_info": build_base_info()
                });
                let _ = client
                    .post(&url)
                    .headers(build_headers(Some(&token)))
                    .json(&body)
                    .timeout(Duration::from_secs(10))
                    .send()
                    .await;
                // Refresh typing indicator every 4 seconds
                tokio::time::sleep(Duration::from_secs(4)).await;
            }
        });

        *self.typing_handle.lock() = Some(handle);
        Ok(())
    }

    pub(crate) async fn stop_typing(&self, _recipient: &str) -> anyhow::Result<()> {
        let mut guard = self.typing_handle.lock();
        if let Some(handle) = guard.take() {
            handle.abort();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_wechat_uin_is_base64() {
        let uin = random_wechat_uin();
        assert!(!uin.is_empty());
        // Should be valid base64
        assert!(base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &uin).is_ok());
    }
}
