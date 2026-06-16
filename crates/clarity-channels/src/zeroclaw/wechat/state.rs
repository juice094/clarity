//! Persistence and allowlist state management for the WeChat channel.

use std::path::Path;

use crate::zeroclaw::wechat::WeChatChannel;
use crate::zeroclaw::wechat::types::{AccountData, SyncData};

impl WeChatChannel {
    /// Load persisted token and cursor from state_dir.
    pub(crate) fn load_persisted_state(&mut self) {
        let account_path = self.state_dir.join("account.json");
        if let Ok(data) = std::fs::read_to_string(&account_path)
            && let Ok(account) = serde_json::from_str::<AccountData>(&data)
        {
            if let Some(ref token) = account.token
                && !token.is_empty()
            {
                if let Ok(mut t) = self.bot_token.write() {
                    *t = Some(token.clone());
                    crate::record!(
                        INFO,
                        crate::zeroclaw::log::Event::new(
                            module_path!(),
                            crate::zeroclaw::log::Action::Note
                        ),
                        "loaded persisted bot token"
                    );
                }
            }
            if let Some(ref id) = account.account_id {
                if let Ok(mut a) = self.account_id.write() {
                    *a = Some(id.clone());
                }
            }
        }

        let sync_path = self.state_dir.join("sync.json");
        if let Ok(data) = std::fs::read_to_string(&sync_path)
            && let Ok(sync) = serde_json::from_str::<SyncData>(&data)
        {
            if !sync.get_updates_buf.is_empty() {
                *self.cursor.lock() = sync.get_updates_buf;
                crate::record!(
                    INFO,
                    crate::zeroclaw::log::Event::new(
                        module_path!(),
                        crate::zeroclaw::log::Action::Note
                    ),
                    "loaded persisted sync cursor"
                );
            }
            if !sync.context_tokens.is_empty() {
                *self.context_tokens.lock() = sync.context_tokens;
                crate::record!(
                    INFO,
                    crate::zeroclaw::log::Event::new(
                        module_path!(),
                        crate::zeroclaw::log::Action::Note
                    ),
                    "loaded persisted context tokens"
                );
            }
        }
    }

    /// Save account data to disk.
    pub(crate) fn save_account_data(&self, token: &str, account_id: &str, user_id: Option<&str>) {
        if let Err(e) = std::fs::create_dir_all(&self.state_dir) {
            crate::record!(
                WARN,
                crate::zeroclaw::log::Event::new(
                    module_path!(),
                    crate::zeroclaw::log::Action::Note
                )
                .with_outcome(crate::zeroclaw::log::EventOutcome::Unknown)
                .with_attrs(::serde_json::json!({"error": format!("{}", e)})),
                "failed to create state dir"
            );
            return;
        }
        let data = AccountData {
            token: Some(token.to_string()),
            account_id: Some(account_id.to_string()),
            base_url: Some(self.api_base_url.clone()),
            user_id: user_id.map(String::from),
            saved_at: Some(chrono::Utc::now().to_rfc3339()),
        };
        let path = self.state_dir.join("account.json");
        match serde_json::to_string_pretty(&data) {
            Ok(json) => {
                if let Err(e) = write_private(&path, json.as_bytes()) {
                    crate::record!(
                        WARN,
                        crate::zeroclaw::log::Event::new(
                            module_path!(),
                            crate::zeroclaw::log::Action::Note
                        )
                        .with_outcome(crate::zeroclaw::log::EventOutcome::Unknown)
                        .with_attrs(::serde_json::json!({"error": format!("{}", e)})),
                        "failed to write account data"
                    );
                }
            }
            Err(e) => crate::record!(
                WARN,
                crate::zeroclaw::log::Event::new(
                    module_path!(),
                    crate::zeroclaw::log::Action::Note
                )
                .with_outcome(crate::zeroclaw::log::EventOutcome::Unknown)
                .with_attrs(::serde_json::json!({"error": format!("{}", e)})),
                "failed to serialize account data"
            ),
        }
    }

    /// Save sync cursor to disk.
    pub(crate) fn save_sync_data(&self) {
        if let Err(e) = std::fs::create_dir_all(&self.state_dir) {
            crate::record!(
                WARN,
                crate::zeroclaw::log::Event::new(
                    module_path!(),
                    crate::zeroclaw::log::Action::Note
                )
                .with_outcome(crate::zeroclaw::log::EventOutcome::Unknown)
                .with_attrs(::serde_json::json!({"error": format!("{}", e)})),
                "failed to create state dir"
            );
            return;
        }
        let data = SyncData {
            get_updates_buf: self.cursor.lock().clone(),
            context_tokens: self.context_tokens.lock().clone(),
        };
        let path = self.state_dir.join("sync.json");
        match serde_json::to_string(&data) {
            Ok(json) => {
                if let Err(e) = write_private(&path, json.as_bytes()) {
                    crate::record!(
                        WARN,
                        crate::zeroclaw::log::Event::new(
                            module_path!(),
                            crate::zeroclaw::log::Action::Note
                        )
                        .with_outcome(crate::zeroclaw::log::EventOutcome::Unknown)
                        .with_attrs(::serde_json::json!({"error": format!("{}", e)})),
                        "failed to write sync data"
                    );
                }
            }
            Err(e) => crate::record!(
                WARN,
                crate::zeroclaw::log::Event::new(
                    module_path!(),
                    crate::zeroclaw::log::Action::Note
                )
                .with_outcome(crate::zeroclaw::log::EventOutcome::Unknown)
                .with_attrs(::serde_json::json!({"error": format!("{}", e)})),
                "failed to serialize sync data"
            ),
        }
    }

    pub(crate) async fn persist_allowed_identity(&self, identity: &str) -> anyhow::Result<()> {
        let normalized = identity.trim().to_string();
        if normalized.is_empty() {
            anyhow::bail!("Cannot persist empty WeChat identity");
        }

        if let Err(e) = std::fs::create_dir_all(&self.state_dir) {
            crate::record!(
                WARN,
                crate::zeroclaw::log::Event::new(
                    module_path!(),
                    crate::zeroclaw::log::Action::Note
                )
                .with_outcome(crate::zeroclaw::log::EventOutcome::Unknown)
                .with_attrs(::serde_json::json!({"identity": identity, "error": format!("{}", e)})),
                "failed to create state dir for allowed users"
            );
            anyhow::bail!("failed to create state dir: {e}");
        }

        let path = self.state_dir.join("allowed_users.json");
        let mut list: Vec<String> = match std::fs::read_to_string(&path) {
            Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
            Err(_) => Vec::new(),
        };

        if list.iter().any(|u| u == &normalized) {
            return Ok(());
        }

        list.push(normalized);
        let json = serde_json::to_string_pretty(&list)?;
        write_private(&path, json.as_bytes())?;
        Ok(())
    }
}

/// Write bytes to a file with owner-only permissions (0o600) on Unix.
pub(crate) fn write_private(path: &Path, data: &[u8]) -> std::io::Result<()> {
    std::fs::write(path, data)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tempfile::tempdir;

    #[test]
    fn sync_data_round_trip_preserves_context_tokens() {
        let temp = tempdir().unwrap();
        let state_dir = temp.path().to_path_buf();

        let mut context_tokens = HashMap::new();
        context_tokens.insert("user123".to_string(), "token_abc".to_string());
        context_tokens.insert("user456".to_string(), "token_xyz".to_string());

        let original_data = SyncData {
            get_updates_buf: "cursor_value".to_string(),
            context_tokens: context_tokens.clone(),
        };

        let sync_path = state_dir.join("sync.json");
        let json = serde_json::to_string(&original_data).unwrap();
        write_private(&sync_path, json.as_bytes()).unwrap();

        let loaded_json = std::fs::read_to_string(&sync_path).unwrap();
        let loaded_data: SyncData = serde_json::from_str(&loaded_json).unwrap();

        assert_eq!(loaded_data.get_updates_buf, "cursor_value");
        assert_eq!(loaded_data.context_tokens.len(), 2);
        assert_eq!(
            loaded_data.context_tokens.get("user123"),
            Some(&"token_abc".to_string())
        );
        assert_eq!(
            loaded_data.context_tokens.get("user456"),
            Some(&"token_xyz".to_string())
        );
    }

    #[test]
    fn sync_data_backward_compatible_with_missing_context_tokens() {
        let old_json = r#"{"get_updates_buf":"old_cursor"}"#;
        let data: SyncData = serde_json::from_str(old_json).unwrap();

        assert_eq!(data.get_updates_buf, "old_cursor");
        assert!(data.context_tokens.is_empty());
    }

    #[tokio::test]
    async fn persist_allowed_identity_without_handle_warns_and_returns_ok() {
        let ch = WeChatChannel::new(
            "wechat_test_alias",
            Arc::new(Vec::new),
            None,
            None,
            Some("/tmp/test-wechat".into()),
        )
        .unwrap();
        // No `.with_persistence(...)` wired — should not panic, returns Ok(()).
        let result = ch.persist_allowed_identity("user_xyz@im.wechat").await;
        assert!(result.is_ok());
    }

    #[test]
    fn context_tokens_survive_channel_restart() {
        let temp = tempdir().unwrap();
        let state_dir = temp.path().to_path_buf();

        {
            let ch = WeChatChannel::new(
                "test",
                Arc::new(|| vec!["*".to_string()]),
                None,
                None,
                Some(state_dir.clone()),
            )
            .unwrap();
            ch.set_context_token("acct1:userA", "tok_A");
            ch.set_context_token("acct1:userB", "tok_B");
            *ch.cursor.lock() = "cursor_123".to_string();
            ch.save_sync_data();
        }

        let ch2 = WeChatChannel::new(
            "test",
            Arc::new(|| vec!["*".to_string()]),
            None,
            None,
            Some(state_dir),
        )
        .unwrap();

        assert_eq!(
            ch2.get_context_token("acct1:userA"),
            Some("tok_A".to_string())
        );
        assert_eq!(
            ch2.get_context_token("acct1:userB"),
            Some("tok_B".to_string())
        );
        assert_eq!(ch2.get_context_token("nonexistent"), None);
        assert_eq!(*ch2.cursor.lock(), "cursor_123");
    }

    #[test]
    fn set_context_token_persists_immediately() {
        let temp = tempdir().unwrap();
        let state_dir = temp.path().to_path_buf();

        let ch = WeChatChannel::new(
            "test",
            Arc::new(|| vec!["*".to_string()]),
            None,
            None,
            Some(state_dir.clone()),
        )
        .unwrap();
        ch.set_context_token("acct:user1", "immediate_tok");

        let ch2 = WeChatChannel::new(
            "test",
            Arc::new(|| vec!["*".to_string()]),
            None,
            None,
            Some(state_dir),
        )
        .unwrap();
        assert_eq!(
            ch2.get_context_token("acct:user1"),
            Some("immediate_tok".to_string())
        );
    }

    #[test]
    fn save_sync_data_preserves_context_tokens() {
        let temp = tempdir().unwrap();
        let state_dir = temp.path().to_path_buf();

        let ch = WeChatChannel::new(
            "test",
            Arc::new(|| vec!["*".to_string()]),
            None,
            None,
            Some(state_dir.clone()),
        )
        .unwrap();
        ch.set_context_token("acct:user1", "my_token");
        *ch.cursor.lock() = "new_cursor_value".to_string();
        ch.save_sync_data();

        let ch2 = WeChatChannel::new(
            "test",
            Arc::new(|| vec!["*".to_string()]),
            None,
            None,
            Some(state_dir),
        )
        .unwrap();
        assert_eq!(*ch2.cursor.lock(), "new_cursor_value");
        assert_eq!(
            ch2.get_context_token("acct:user1"),
            Some("my_token".to_string())
        );
    }

    #[test]
    fn load_from_empty_state_dir_produces_defaults() {
        let temp = tempdir().unwrap();
        let state_dir = temp.path().to_path_buf();

        let ch = WeChatChannel::new(
            "test",
            Arc::new(|| vec!["*".to_string()]),
            None,
            None,
            Some(state_dir),
        )
        .unwrap();

        assert_eq!(ch.get_context_token("anything"), None);
        assert_eq!(*ch.cursor.lock(), "");
    }

    #[test]
    fn context_token_overwrite_persists_latest() {
        let temp = tempdir().unwrap();
        let state_dir = temp.path().to_path_buf();

        let ch = WeChatChannel::new(
            "test",
            Arc::new(|| vec!["*".to_string()]),
            None,
            None,
            Some(state_dir.clone()),
        )
        .unwrap();
        ch.set_context_token("acct:user1", "old_token");
        ch.set_context_token("acct:user1", "new_token");

        let ch2 = WeChatChannel::new(
            "test",
            Arc::new(|| vec!["*".to_string()]),
            None,
            None,
            Some(state_dir),
        )
        .unwrap();
        assert_eq!(
            ch2.get_context_token("acct:user1"),
            Some("new_token".to_string())
        );
    }
}
