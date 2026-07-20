//! Claw WebSocket event processing — extracted from `main.rs` to keep the
//! hot-path `update()` function under control.
//!
//! Covers four concerns that previously lived inline in `update()`:
//!   1. Connection management (connect/reconnect to Claw devices)
//!   2. WebSocket response draining (Connected, StreamChunk, Done, History, …)
//!   3. Pairing client response draining
//!   4. Pairing request timeout

use crate::App;
use crate::PairingState;
use crate::claw::normalize_gateway_url;
use crate::ui::types::*;

impl App {
    /// Sync device list snapshot and manage Claw WebSocket connection lifecycle.
    ///
    /// Called every 30 frames (~0.5 s). Handles connect, reconnect, disconnect
    /// when switching away from a Claw session, and role-based device routing.
    pub(crate) fn manage_claw_connection(&mut self) {
        // Sync live device snapshot into the sidebar.
        let devices = self.context.device_state.snapshot();
        if !devices.is_empty() {
            self.context.ui_store.bot_instances = devices;
        }

        let active_session_is_claw = self
            .context
            .session_store
            .active_session()
            .map(|s| matches!(s.context, SessionContext::Claw { .. }))
            .unwrap_or(false);

        // Non-Claw sessions must not keep a Claw WebSocket alive — unreachable
        // remote OpenClaw devices can block the UI thread with synchronous TCP
        // connects and prevent normal chat.
        if !active_session_is_claw && self.context.claw_ws.is_some() {
            self.context.claw_ws = None;
            self.context.claw_ws_device_id.clear();
            self.context.ui_store.active_bot_id.clear();
        }

        let active_role = if active_session_is_claw {
            self.context
                .session_store
                .active_session()
                .and_then(|s| match &s.context {
                    SessionContext::Claw { role, .. } => Some(role.clone()),
                    _ => None,
                })
        } else {
            None
        };

        let picked_id = if let Some(ref role) = active_role {
            self.context
                .session_store
                .active_session()
                .and_then(|s| match &s.context {
                    SessionContext::Claw { affinity, .. } => self
                        .context
                        .device_state
                        .pick_instance(role, affinity)
                        .map(|b| b.id),
                    _ => None,
                })
                .unwrap_or_default()
        } else {
            String::new()
        };

        // Only reconnect when there is no handle or the selected device changed.
        let should_reconnect =
            self.context.claw_ws.is_none() || picked_id != self.context.claw_ws_device_id;
        if !should_reconnect || picked_id.is_empty() {
            return;
        }

        // Check backoff before attempting reconnect.
        if let Some(remaining) = self.context.ui_store.is_in_backoff(&picked_id) {
            if self.context.ui_store.frame_count % 300 == 0 {
                tracing::debug!(
                    device_id = %picked_id,
                    ?remaining,
                    "Claw device in reconnect backoff"
                );
            }
            return;
        }

        let Some(conn) = self.context.device_state.connection(&picked_id) else {
            self.context.ui_store.record_connect_failure(&picked_id);
            return;
        };

        let token_required = conn.protocol == crate::claw::ClawProtocol::OpenClawJsonRpc;
        if token_required && conn.gateway_token.is_empty() {
            return;
        }

        let ws_url =
            if conn.gateway_url.starts_with("ws://") || conn.gateway_url.starts_with("wss://") {
                conn.gateway_url.clone()
            } else {
                conn.gateway_url
                    .replace("http://", "ws://")
                    .replace("https://", "wss://")
            };

        let is_remote = !crate::is_localhost_host(&ws_url);

        // Prefer device-attested auth when a saved pairing record matches.
        let identity = self.context.claw_device_identity.clone().or_else(|| {
            match clarity_claw::DeviceIdentity::load_or_generate() {
                Ok(id) => {
                    self.context.claw_device_identity = Some(id.clone());
                    Some(id)
                }
                Err(e) => {
                    tracing::warn!("Failed to generate Claw device identity: {}", e);
                    None
                }
            }
        });
        let saved_pairing = self.context.claw_device_token.as_ref().and_then(|record| {
            if normalize_gateway_url(&record.gateway_url) == normalize_gateway_url(&ws_url) {
                Some(record)
            } else {
                None
            }
        });

        // Normalize the URL for the configured protocol family.
        let ws_url = if conn.protocol == crate::claw::ClawProtocol::GatewayWebSocket
            && !ws_url.ends_with("/ws")
        {
            format!("{}/ws", ws_url.trim_end_matches('/'))
        } else {
            ws_url
        };
        let ws_url = ws_url
            .replace("http://", "ws://")
            .replace("https://", "wss://");

        // Build auth for OpenClaw; Gateway dialect needs none.
        let auth = if conn.protocol == crate::claw::ClawProtocol::OpenClawJsonRpc {
            let token = match conn.auth_mode.as_deref() {
                Some("device_paired") => conn
                    .device_token
                    .clone()
                    .or_else(|| saved_pairing.map(|r| r.auth_token().to_string()))
                    .unwrap_or_else(|| conn.gateway_token.clone()),
                _ => conn.gateway_token.clone(),
            };
            if let Some(identity) = identity {
                if is_remote {
                    Some(clarity_claw::ClawAuth::TokenWithDevice {
                        token,
                        device: Box::new(identity),
                    })
                } else {
                    Some(clarity_claw::ClawAuth::DevicePaired {
                        device: Box::new(identity),
                        device_token: token,
                    })
                }
            } else {
                Some(clarity_claw::ClawAuth::TokenOnly { token })
            }
        } else {
            None
        };

        let manager = clarity_claw::ClawConnectionManager::connect_with_options(
            &ws_url,
            auth,
            conn.send_method,
            clarity_contract::retry::RetryConfig::default(),
        );
        self.context.claw_ws = Some(crate::claw::ClawClientHandle::new(manager));
        self.context.claw_ws_device_id = picked_id.clone();
        self.context.ui_store.clear_connect_backoff(&picked_id);

        // Preserve a user-selected bot as long as it still belongs to the
        // active Claw role.
        let selection_still_valid = active_role
            .as_ref()
            .map(|role| {
                self.context
                    .device_state
                    .has_device_in_role(role, &self.context.ui_store.active_bot_id)
            })
            .unwrap_or(false);
        if self.context.ui_store.active_bot_id.is_empty()
            || (!selection_still_valid && self.context.ui_store.active_bot_id != picked_id)
        {
            self.context.ui_store.active_bot_id = picked_id.clone();
        }
    }

    /// Drain WebSocket responses from the active Claw connection and dispatch
    /// them to the appropriate session / UI state.
    pub(crate) fn drain_claw_ws_responses(&mut self) {
        let responses = self
            .context
            .claw_ws
            .as_ref()
            .map(|ws| ws.drain())
            .unwrap_or_default();

        for event in responses {
            match event {
                crate::claw::ClawEvent::Connected {
                    gateway_url,
                    session_id,
                } => {
                    self.push_toast(
                        format!("Connected to Claw Gateway: {}", gateway_url),
                        ToastLevel::Info,
                    );
                    self.context
                        .device_state
                        .record_success(&self.context.claw_ws_device_id, 0);
                    if let Some(ref id) = session_id {
                        self.context.ui_store.claw_gateway_session_id = id.clone();
                        if let Some(ref ws) = self.context.claw_ws {
                            ws.get_history(&self.context.ui_store.claw_gateway_session_id);
                        }
                        // The Gateway assigns a concrete session id during
                        // handshake. Update the session_key in the Claw session
                        // context so `sessions.send` targets the correct key.
                        if let Some(session) = self.context.session_store.active_session_mut() {
                            if let SessionContext::Claw { session_key, .. } = &mut session.context {
                                *session_key = id.clone();
                            }
                        }
                        self.subscribe_claw_session(id);
                    }
                    // Auto-subscribe and fetch history after connect.
                    let session_key = self
                        .context
                        .session_store
                        .active_session()
                        .and_then(|s| match &s.context {
                            SessionContext::Claw { session_key, .. } => Some(session_key.clone()),
                            _ => None,
                        })
                        .unwrap_or_else(|| "agent:main:main".to_string());
                    self.subscribe_claw_session(&session_key);

                    // Trigger role-context sync when the active session is a
                    // Claw session. OpenClaw uses syncthing-rust for this, so
                    // skip the WebSocket sync for that dialect.
                    let is_openclaw = self.active_claw_protocol()
                        == Some(crate::claw::ClawProtocol::OpenClawJsonRpc);
                    if !is_openclaw {
                        if let Some(session) = self.context.session_store.active_session() {
                            if let SessionContext::Claw { role, .. } = &session.context {
                                let device_id = self
                                    .context
                                    .claw_device_identity
                                    .as_ref()
                                    .map(|id| id.device_id())
                                    .unwrap_or_else(|| self.context.claw_ws_device_id.clone());
                                if let Some(ref ws) = self.context.claw_ws {
                                    ws.sync_role_context(role, None, &device_id);
                                }
                            }
                        }
                    }
                    // Pin the active Claw session to this device.
                    if let Some(session) = self.context.session_store.active_session_mut() {
                        if let SessionContext::Claw { affinity, .. } = &mut session.context {
                            *affinity =
                                DeviceAffinity::Specific(self.context.claw_ws_device_id.clone());
                        }
                    }
                    // Remember this device as the last successful pick.
                    if let Some(session) = self.context.session_store.active_session() {
                        if let SessionContext::Claw { role, .. } = &session.context {
                            self.context
                                .device_state
                                .set_last_picked(role, &self.context.claw_ws_device_id);
                        }
                    }
                }
                crate::claw::ClawEvent::StreamChunk(text) => {
                    if !text.trim().is_empty() {
                        let session_id = self
                            .chat_store()
                            .claw_in_flight_session_id
                            .clone()
                            .unwrap_or_else(|| {
                                self.context.session_store.active_session_id.clone()
                            });
                        let _ = self.context.ui_tx.send(UiEvent::Chunk { session_id, text });
                    }
                }
                crate::claw::ClawEvent::Done => {
                    let session_id = self
                        .chat_store_mut()
                        .claw_in_flight_session_id
                        .take()
                        .unwrap_or_else(|| self.context.session_store.active_session_id.clone());
                    let _ = self.context.ui_tx.send(UiEvent::Done { session_id });
                }
                crate::claw::ClawEvent::WirePayload(payload) => {
                    let session_id = self
                        .chat_store()
                        .claw_in_flight_session_id
                        .clone()
                        .unwrap_or_else(|| self.context.session_store.active_session_id.clone());
                    crate::services::wire_dispatcher::dispatch_wire_payload(
                        &payload,
                        &session_id,
                        &self.context.ui_tx,
                    );
                }
                crate::claw::ClawEvent::History {
                    session_key,
                    messages,
                } => {
                    let count = messages.len();
                    self.push_toast(
                        format!("Loaded {} messages from session", count),
                        ToastLevel::Info,
                    );
                    self.context.ui_store.claw_history = messages
                        .iter()
                        .map(|m| format!("[{}] {}", m.role, m.content))
                        .collect();
                    // Merge Gateway history into the target Claw session.
                    let target_id = session_key
                        .as_deref()
                        .and_then(|key| self.claw_session_id_by_key(key))
                        .unwrap_or_else(|| self.context.session_store.active_session_id.clone());
                    if let Some(session) = self.context.session_store.session_mut(&target_id) {
                        let existing: std::collections::HashSet<String> =
                            session.messages.iter().map(|m| m.content.clone()).collect();
                        for gm in messages {
                            let role = if gm.role == "user" {
                                Role::User
                            } else {
                                Role::Agent
                            };
                            let content = gm.content.clone();
                            if content.is_empty() || existing.contains(&content) {
                                continue;
                            }
                            let mut msg = Message {
                                role,
                                content: content.clone(),
                                blocks: vec![ContentBlock::Text { text: content }],
                                timestamp: std::time::Instant::now(),
                                parsed: vec![],
                                cached_height: None,
                                is_error: false,
                                lines: Vec::new(),
                            };
                            msg.prepare();
                            session.messages.push(msg);
                        }
                        if !session.messages.is_empty() {
                            session.updated_at = crate::session::now_millis();
                            self.save_current_session();
                        }
                    }
                }
                crate::claw::ClawEvent::RoleContextSynced {
                    role_id,
                    session_key,
                    events,
                    online_devices,
                    ..
                } => {
                    let count = events.len();
                    if count > 0 {
                        tracing::info!(
                            role = %role_id,
                            count,
                            "Role context synced from Gateway"
                        );
                        let active_id = self.context.session_store.active_session_id.clone();
                        let target_ids: Vec<String> = session_key
                            .as_deref()
                            .and_then(|key| self.claw_session_id_by_key(key))
                            .map(|id| vec![id])
                            .unwrap_or_else(|| self.claw_session_ids_by_role(&role_id))
                            .into_iter()
                            .collect();
                        for id in target_ids {
                            if let Some(session) = self.context.session_store.session_mut(&id) {
                                if let SessionContext::Claw { session_key, .. } = &session.context {
                                    let role_ctx = clarity_claw::mesh::merge_events(
                                        clarity_contract::RoleContextId::new(session_key.clone()),
                                        &events,
                                    );
                                    let existing: std::collections::HashSet<String> = session
                                        .messages
                                        .iter()
                                        .map(|m| m.content.clone())
                                        .collect();
                                    let mut appended = false;
                                    for m in role_ctx.messages {
                                        if m.content.is_empty() || existing.contains(&m.content) {
                                            continue;
                                        }
                                        let role = if m.role == "user" {
                                            Role::User
                                        } else {
                                            Role::Agent
                                        };
                                        let content = m.content.clone();
                                        let mut msg = Message {
                                            role,
                                            content: content.clone(),
                                            blocks: vec![ContentBlock::Text { text: content }],
                                            timestamp: std::time::Instant::now(),
                                            parsed: vec![],
                                            cached_height: None,
                                            is_error: false,
                                            lines: Vec::new(),
                                        };
                                        msg.prepare();
                                        session.messages.push(msg);
                                        appended = true;
                                    }
                                    if appended {
                                        session.updated_at = crate::session::now_millis();
                                        if id == active_id {
                                            self.save_current_session();
                                        } else {
                                            let _ = crate::session::save_session_internal(session);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if !online_devices.is_empty() {
                        tracing::info!(
                            role = %role_id,
                            devices = ?online_devices,
                            "Online Claw devices"
                        );
                    }
                }
                crate::claw::ClawEvent::ReconnectPending { .. } => {
                    let failed_device = self.context.claw_ws_device_id.clone();
                    if !failed_device.is_empty() {
                        self.context.device_state.record_failure(&failed_device);
                        self.context
                            .device_state
                            .update_status(&failed_device, crate::stores::ui::BotStatus::Offline);
                        self.push_toast(
                            format!("Claw device {} went offline; failing over", failed_device),
                            ToastLevel::Warn,
                        );
                    }
                    self.chat_store_mut().claw_in_flight_session_id = None;
                    self.context.claw_ws = None;
                    self.context.claw_ws_device_id.clear();
                    continue;
                }
                crate::claw::ClawEvent::Error(e) => {
                    tracing::warn!("Claw WebSocket error: {}", e);
                    let surfaced_msg = format!("OpenClaw connection error: {}", e);
                    if self
                        .context
                        .ui_store
                        .should_surface_claw_error(&surfaced_msg)
                    {
                        let session_id = self
                            .chat_store_mut()
                            .claw_in_flight_session_id
                            .take()
                            .unwrap_or_else(|| {
                                self.context.session_store.active_session_id.clone()
                            });
                        let _ = self.context.ui_tx.send(UiEvent::Error {
                            session_id,
                            message: surfaced_msg,
                        });
                    }
                    let failed_device = self.context.claw_ws_device_id.clone();
                    if !failed_device.is_empty() {
                        self.context.device_state.record_failure(&failed_device);
                    }
                    self.context.claw_ws = None;
                    self.context.claw_ws_device_id.clear();
                    if !failed_device.is_empty() {
                        let (count, _next) =
                            self.context.ui_store.record_connect_failure(&failed_device);
                        if count >= 5 {
                            self.context.device_state.update_status(
                                &failed_device,
                                crate::stores::ui::BotStatus::Offline,
                            );
                            self.push_toast(
                                format!(
                                    "Claw device {} failed 5 times; marked offline",
                                    failed_device
                                ),
                                ToastLevel::Warn,
                            );
                        }
                    }
                }
                // PairingResult from the main WebSocket is logged but the
                // pairing flow uses a dedicated client.
                crate::claw::ClawEvent::PairingResult {
                    device_id,
                    approved,
                    token,
                    scopes,
                } => {
                    if approved {
                        self.push_toast(
                            format!(
                                "Claw device {} paired (scopes: {})",
                                &device_id[..device_id.len().min(8)],
                                scopes.join(",")
                            ),
                            ToastLevel::Info,
                        );
                        tracing::info!(
                            device_id = %device_id,
                            ?token,
                            ?scopes,
                            "OpenClaw pairing approved"
                        );
                    } else {
                        self.push_toast(
                            format!(
                                "Claw device {} pairing pending approval",
                                &device_id[..device_id.len().min(8)]
                            ),
                            ToastLevel::Info,
                        );
                        tracing::info!(device_id = %device_id, "OpenClaw pairing pending");
                    }
                }
            }
        }
    }

    /// Drain responses from the temporary pairing client (used during in-app
    /// device pairing flows).
    pub(crate) fn drain_pairing_responses(&mut self) {
        let Some(client) = self.context.claw_pairing_client.as_ref() else {
            return;
        };
        for resp in client.drain() {
            match resp {
                clarity_claw::client::ClawResponse::PairingResult {
                    device_id,
                    approved,
                    token,
                    scopes,
                } => {
                    if approved {
                        if let Some(t) = token {
                            self.finish_openclaw_pairing(&device_id, &t, &scopes);
                        } else {
                            self.context.claw_pairing_state = PairingState::Error(
                                "Pairing approved but no token returned".to_string(),
                            );
                            self.context.claw_pairing_client = None;
                        }
                    } else {
                        self.push_toast(
                            format!(
                                "Device {} pairing pending approval",
                                &device_id[..device_id.len().min(8)]
                            ),
                            ToastLevel::Info,
                        );
                    }
                }
                clarity_claw::client::ClawResponse::Error(e) => {
                    self.context.claw_pairing_state = PairingState::Error(e.clone());
                    self.context.claw_pairing_client = None;
                    self.push_toast(format!("Pairing failed: {}", e), ToastLevel::Error);
                }
                _ => {}
            }
        }
    }

    /// Time out pairing requests that have been waiting for more than 120 s.
    pub(crate) fn timeout_claw_pairing(&mut self) {
        if let PairingState::Waiting { since, .. } = self.context.claw_pairing_state {
            if since.elapsed() > std::time::Duration::from_secs(120) {
                self.context.claw_pairing_state =
                    PairingState::Error("Pairing timed out".to_string());
                self.context.claw_pairing_client = None;
                self.push_toast("Pairing timed out".to_string(), ToastLevel::Error);
            }
        }
    }
}
