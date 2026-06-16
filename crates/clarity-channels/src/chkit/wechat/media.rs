//! Media upload, download, and attachment handling for WeChat iLink.

use std::path::PathBuf;

use anyhow::Context;
use base64::Engine;

use crate::chkit::wechat::{
    WeChatChannel,
    crypto::{self, aes_ecb_padded_size, encrypt_aes_ecb},
    parsing::{find_inbound_attachment, format_attachment_content},
    types::{
        API_TIMEOUT, InboundAttachmentSpec, UploadedWeChatMedia, WECHAT_MEDIA_MAX_BYTES,
        WeChatAttachment, WeChatAttachmentKind, WeChatMediaPayload, is_remote_url,
        sanitize_attachment_filename,
    },
};

impl WeChatChannel {
    pub(crate) fn cdn_download_url(&self, encrypted_query_param: &str) -> String {
        let base = self.cdn_base_url.trim_end_matches('/');
        format!(
            "{base}/download?encrypted_query_param={}",
            urlencoding::encode(encrypted_query_param)
        )
    }

    pub(crate) fn cdn_upload_url(&self, upload_param: &str, filekey: &str) -> String {
        let base = self.cdn_base_url.trim_end_matches('/');
        format!(
            "{base}/upload?encrypted_query_param={}&filekey={}",
            urlencoding::encode(upload_param),
            urlencoding::encode(filekey)
        )
    }

    pub(crate) fn resolve_local_attachment_path(&self, target: &str) -> PathBuf {
        let target = target.trim();
        let target = target.strip_prefix("file://").unwrap_or(target);

        let resolved = if let Some(rel) = target.strip_prefix("/workspace/") {
            if let Some(workspace_dir) = &self.workspace_dir {
                workspace_dir.join(rel)
            } else {
                PathBuf::from(target)
            }
        } else {
            let path = PathBuf::from(target);
            if path.is_absolute() {
                path
            } else {
                std::env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .join(path)
            }
        };

        // Prevent path traversal outside workspace when workspace_dir is set
        if let Some(workspace_dir) = &self.workspace_dir
            && let (Ok(canonical), Ok(allowed)) =
                (resolved.canonicalize(), workspace_dir.canonicalize())
            && !canonical.starts_with(&allowed)
        {
            crate::record!(
                WARN,
                crate::chkit::log::Event::new(module_path!(), crate::chkit::log::Action::Note)
                    .with_outcome(crate::chkit::log::EventOutcome::Unknown),
                &format!(
                    "attachment path {} escapes workspace {}, rejected",
                    canonical.display(),
                    allowed.display()
                )
            );
            return PathBuf::from(format!(
                "/nonexistent/blocked_path_traversal_{}",
                uuid::Uuid::new_v4()
            ));
        }

        resolved
    }

    pub(crate) fn remote_file_name(
        &self,
        url: &str,
        content_type: Option<&str>,
        kind: WeChatAttachmentKind,
    ) -> String {
        let cleaned_url = url
            .split('?')
            .next()
            .unwrap_or(url)
            .split('#')
            .next()
            .unwrap_or(url);

        if let Some(last_segment) = cleaned_url.rsplit('/').next()
            && let Some(name) = sanitize_attachment_filename(last_segment)
            && std::path::Path::new(&name).extension().is_some()
        {
            return name;
        }

        let ext = content_type
            .and_then(|value| value.split(';').next())
            .and_then(mime_guess::get_mime_extensions_str)
            .and_then(|exts: &[&str]| exts.first().copied())
            .unwrap_or(kind.default_extension());

        format!(
            "wechat_attachment_{}.{}",
            uuid::Uuid::new_v4().simple(),
            ext
        )
    }

    pub(crate) async fn download_remote_attachment(
        &self,
        url: &str,
        kind: WeChatAttachmentKind,
    ) -> anyhow::Result<WeChatMediaPayload> {
        if !url.starts_with("https://") {
            anyhow::bail!("refusing non-HTTPS attachment URL: {url}");
        }
        let resp = self
            .client
            .get(url)
            .timeout(API_TIMEOUT)
            .send()
            .await
            .with_context(|| format!("attachment download failed: {url}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("attachment download failed ({status}): {body}");
        }

        if let Some(len) = resp.content_length()
            && len > WECHAT_MEDIA_MAX_BYTES
        {
            anyhow::bail!(
                "attachment Content-Length ({len} bytes) exceeds {} MB limit",
                WECHAT_MEDIA_MAX_BYTES / (1024 * 1024)
            );
        }

        let content_type = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let bytes = resp.bytes().await?.to_vec();

        if bytes.len() as u64 > WECHAT_MEDIA_MAX_BYTES {
            anyhow::bail!(
                "attachment exceeds {} MB limit",
                WECHAT_MEDIA_MAX_BYTES / (1024 * 1024)
            );
        }

        Ok(WeChatMediaPayload {
            file_name: self.remote_file_name(url, content_type.as_deref(), kind),
            bytes,
        })
    }

    pub(crate) async fn load_attachment_payload(
        &self,
        attachment: &WeChatAttachment,
    ) -> anyhow::Result<WeChatMediaPayload> {
        let target = attachment.target.trim();
        if is_remote_url(target) {
            return self
                .download_remote_attachment(target, attachment.kind)
                .await;
        }

        let path = self.resolve_local_attachment_path(target);
        if !path.exists() {
            anyhow::bail!("attachment path not found: {}", path.display());
        }

        let file_name = sanitize_attachment_filename(
            path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("attachment.bin"),
        )
        .unwrap_or_else(|| {
            format!(
                "wechat_attachment_{}.{}",
                uuid::Uuid::new_v4().simple(),
                attachment.kind.default_extension()
            )
        });

        let bytes = tokio::fs::read(&path)
            .await
            .with_context(|| format!("attachment read failed: {}", path.display()))?;
        if bytes.len() as u64 > WECHAT_MEDIA_MAX_BYTES {
            anyhow::bail!(
                "attachment exceeds {} MB limit",
                WECHAT_MEDIA_MAX_BYTES / (1024 * 1024)
            );
        }

        Ok(WeChatMediaPayload { bytes, file_name })
    }

    pub(crate) async fn request_upload_param(
        &self,
        to: &str,
        kind: WeChatAttachmentKind,
        payload: &WeChatMediaPayload,
        aes_key: &[u8; 16],
        filekey: &str,
    ) -> anyhow::Result<String> {
        let token = self
            .get_token()
            .context("not logged in, cannot upload attachment")?;
        let body = serde_json::json!({
            "filekey": filekey,
            "media_type": kind.upload_media_type(),
            "to_user_id": to,
            "rawsize": payload.bytes.len(),
            "rawfilemd5": format!("{:x}", md5::compute(&payload.bytes)),
            "filesize": aes_ecb_padded_size(payload.bytes.len()),
            "no_need_thumb": true,
            "aeskey": hex::encode(aes_key),
            "base_info": crate::chkit::wechat::types::build_base_info()
        });

        let resp = self
            .client
            .post(self.api_url("getuploadurl"))
            .headers(crate::chkit::wechat::api::build_headers(Some(&token)))
            .json(&body)
            .timeout(API_TIMEOUT)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("getUploadUrl failed ({status}): {body}");
        }

        let data: serde_json::Value = resp.json().await?;
        data.get("upload_param")
            .and_then(|value| value.as_str())
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .context("getUploadUrl returned no upload_param")
    }

    pub(crate) async fn upload_to_cdn(
        &self,
        upload_param: &str,
        filekey: &str,
        ciphertext: &[u8],
    ) -> anyhow::Result<String> {
        let url = self.cdn_upload_url(upload_param, filekey);
        let mut last_error: Option<anyhow::Error> = None;

        for attempt in 1..=3 {
            let resp = self
                .client
                .post(&url)
                .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
                .body(ciphertext.to_vec())
                .timeout(API_TIMEOUT)
                .send()
                .await;

            match resp {
                Ok(resp) if resp.status().is_success() => {
                    let encrypted_param = resp
                        .headers()
                        .get("x-encrypted-param")
                        .and_then(|value| value.to_str().ok())
                        .filter(|value| !value.is_empty())
                        .map(str::to_string)
                        .context("CDN upload missing x-encrypted-param header")?;
                    return Ok(encrypted_param);
                }
                Ok(resp) => {
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();
                    crate::record!(
                        WARN,
                        crate::chkit::log::Event::new(
                            module_path!(),
                            crate::chkit::log::Action::Fail
                        )
                        .with_outcome(crate::chkit::log::EventOutcome::Failure)
                        .with_attrs(::serde_json::json!({
                            "attempt": attempt,
                            "status": status.as_u16(),
                            "body": body,
                            "phase": "cdn_upload",
                        })),
                        "wechat: CDN upload failed (non-success status)"
                    );
                    let error = anyhow::Error::msg(format!(
                        "CDN upload failed on attempt {attempt} ({status}): {body}"
                    ));
                    if status.is_client_error() {
                        return Err(error);
                    }
                    last_error = Some(error);
                }
                Err(err) => {
                    crate::record!(
                        WARN,
                        crate::chkit::log::Event::new(
                            module_path!(),
                            crate::chkit::log::Action::Fail
                        )
                        .with_outcome(crate::chkit::log::EventOutcome::Failure)
                        .with_attrs(::serde_json::json!({
                            "attempt": attempt,
                            "phase": "cdn_upload",
                            "error": format!("{}", err),
                        })),
                        "wechat: CDN upload request failed"
                    );
                    last_error = Some(anyhow::Error::msg(format!(
                        "CDN upload request failed on attempt {attempt}: {err}"
                    )));
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            crate::record!(
                ERROR,
                crate::chkit::log::Event::new(module_path!(), crate::chkit::log::Action::Fail)
                    .with_outcome(crate::chkit::log::EventOutcome::Failure)
                    .with_attrs(::serde_json::json!({"phase": "cdn_upload"})),
                "wechat: CDN upload exhausted retries"
            );
            anyhow::Error::msg("CDN upload failed")
        }))
    }

    pub(crate) async fn upload_media_payload(
        &self,
        to: &str,
        kind: WeChatAttachmentKind,
        payload: &WeChatMediaPayload,
    ) -> anyhow::Result<UploadedWeChatMedia> {
        let filekey = uuid::Uuid::new_v4().simple().to_string();
        let aes_key: [u8; 16] = rand::random();
        let upload_param = self
            .request_upload_param(to, kind, payload, &aes_key, &filekey)
            .await?;
        let ciphertext = encrypt_aes_ecb(&payload.bytes, &aes_key)?;
        let encrypted_query_param = self
            .upload_to_cdn(&upload_param, &filekey, &ciphertext)
            .await?;

        Ok(UploadedWeChatMedia {
            encrypted_query_param,
            aes_key_base64: base64::engine::general_purpose::STANDARD.encode(aes_key),
            raw_size: payload.bytes.len(),
            encrypted_size: ciphertext.len(),
        })
    }

    pub(crate) async fn download_inbound_attachment(
        &self,
        spec: &InboundAttachmentSpec,
    ) -> anyhow::Result<Vec<u8>> {
        let resp = self
            .client
            .get(self.cdn_download_url(&spec.encrypted_query_param))
            .timeout(API_TIMEOUT)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("attachment download failed ({status}): {body}");
        }

        let bytes = resp.bytes().await?.to_vec();
        if bytes.len() as u64 > WECHAT_MEDIA_MAX_BYTES {
            anyhow::bail!(
                "inbound attachment exceeds {} MB limit",
                WECHAT_MEDIA_MAX_BYTES / (1024 * 1024)
            );
        }

        match spec.aes_key.as_deref() {
            Some(aes_key) if !aes_key.is_empty() => {
                let key = crypto::parse_aes_key(aes_key)?;
                crypto::decrypt_aes_ecb(&bytes, &key)
            }
            _ => Ok(bytes),
        }
    }

    pub(crate) async fn try_build_attachment_content(
        &self,
        items: &[serde_json::Value],
        message_id: &str,
    ) -> Option<String> {
        let workspace_dir = self.workspace_dir.as_ref()?;
        let spec = find_inbound_attachment(items, message_id)?;
        let bytes = match self.download_inbound_attachment(&spec).await {
            Ok(bytes) => bytes,
            Err(err) => {
                crate::record!(
                    WARN,
                    crate::chkit::log::Event::new(module_path!(), crate::chkit::log::Action::Note)
                        .with_outcome(crate::chkit::log::EventOutcome::Unknown)
                        .with_attrs(::serde_json::json!({"error": format!("{}", err)})),
                    "attachment download skipped"
                );
                return None;
            }
        };

        let save_dir = workspace_dir.join("wechat_files");
        if let Err(err) = tokio::fs::create_dir_all(&save_dir).await {
            crate::record!(
                WARN,
                crate::chkit::log::Event::new(module_path!(), crate::chkit::log::Action::Note)
                    .with_outcome(crate::chkit::log::EventOutcome::Unknown)
                    .with_attrs(::serde_json::json!({"error": format!("{}", err)})),
                "Failed to create WeChat attachment dir"
            );
            return None;
        }

        let local_path = save_dir.join(&spec.file_name);
        if let Err(err) = tokio::fs::write(&local_path, bytes).await {
            crate::record!(
                WARN,
                crate::chkit::log::Event::new(module_path!(), crate::chkit::log::Action::Note)
                    .with_outcome(crate::chkit::log::EventOutcome::Unknown),
                &format!(
                    "Failed to save WeChat attachment to {}: {err}",
                    local_path.display()
                )
            );
            return None;
        }

        Some(format_attachment_content(
            spec.kind,
            &spec.file_name,
            &local_path,
        ))
    }
}
