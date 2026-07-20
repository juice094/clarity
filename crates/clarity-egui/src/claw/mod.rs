//! Distributed Claw device integration.
//!
//! # Architecture
//!
//! Clarity manages a **distributed Claw network** with two protocol families:
//!
//! | Type | Runtime | Gateway | Typical location |
//! |------|---------|---------|-----------------|
//! | `ZeroClaw` | clarity-claw | clarity-gateway (:18790) | Local machine |
//! | `OpenClaw` | kimi-openclaw | OpenClaw Gateway (:18789) | Local + Cloud (Tailscale) |
//!
//! This module is the egui-specific adapter. The UI-agnostic OpenClaw client,
//! device identity and discovery live in the dedicated `clarity-claw` crate.
//! Per-device connection parameters drive the behaviour of the Settings /
//! Workspace / Terminal / WebBridge panels.
//!
//! # Config sources (priority order)
//!
//! 1. **ZeroClaw** — local clarity-claw daemon (always registered as fallback)
//! 2. **Local OpenClaw** — `~/.kimi_openclaw/openclaw.json` + paired devices
//! 3. **Remote OpenClaw (env)** — `OPENCLAW_REMOTE_URL` / `OPENCLAW_REMOTE_TOKEN`
//! 4. **Remote OpenClaw (settings)** — user-configured `GuiSettings::openclaw_connections`
//! 5. **Persisted pairing** — `~/.clarity/claw-device-token.json`

use crate::settings::{GuiSettings, OpenClawAuthMode, OpenClawConnection};
use crate::stores::ui::{BotInstance, BotStatus};
use crate::ui::types::DeviceAffinity;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

// Re-export UI-agnostic types from the shared crate so existing panels can keep
// using `crate::claw::ClawType` / `crate::claw::ClawConnection`.
pub use clarity_apps::settings_data::{normalize_gateway_url, to_ws_url};
pub use clarity_claw::types::{ClawConnection, ClawProtocol, ClawType, OpenClawSendMethod};

/// Source of a discovered Claw endpoint, ordered by user-intent priority.
/// Lower discriminants win when two sources describe the same endpoint.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum DeviceSource {
    /// User-configured in `GuiSettings::openclaw_connections`.
    Settings,
    /// `OPENCLAW_REMOTE_*` environment variables.
    Env,
    /// Persisted pairing record (`claw-device-token.json`).
    SavedPairing,
    /// Local OpenClaw discovery (`~/.kimi_openclaw`).
    LocalDiscovery,
    /// ZeroClaw fallback.
    ZeroClaw,
    /// Ultimate localhost fallback when nothing else is found.
    Fallback,
}

impl DeviceSource {
    fn label(self) -> &'static str {
        match self {
            DeviceSource::Settings => "settings",
            DeviceSource::Env => "env",
            DeviceSource::SavedPairing => "paired",
            DeviceSource::LocalDiscovery => "local",
            DeviceSource::ZeroClaw => "zeroclaw",
            DeviceSource::Fallback => "fallback",
        }
    }
}

/// Stable key for deduplicating discovered endpoints that point to the same
/// target session. Endpoints are considered identical when they share the same
/// role, protocol family, normalized Gateway URL, and *effective* session key.
/// Auth token differences are ignored because multiple config sources (admin
/// token, device token, env, pairing) may all route to the same session.
fn endpoint_key(role: &str, conn: &ClawConnection, session_key: &str) -> String {
    format!(
        "{}|{:?}|{:?}|{}|{}",
        role,
        conn.claw_type,
        conn.protocol,
        normalize_gateway_url(&conn.gateway_url),
        session_key,
    )
}

/// A single message entry returned as part of a Claw history response.
#[derive(Clone, Debug)]
pub struct ClawHistoryMessage {
    /// Role of the message author.
    pub role: String,
    /// Message content.
    pub content: String,
}

/// Unified event stream produced by either an OpenClaw or a native Gateway client.
#[derive(Clone, Debug)]
pub enum ClawEvent {
    /// Connection established.
    Connected {
        /// URL of the connected Gateway.
        gateway_url: String,
        /// Gateway-assigned session id, if any.
        session_id: Option<String>,
    },
    /// A chunk of assistant text.
    StreamChunk(String),
    /// End of the current assistant turn.
    Done,
    /// A streamed `clarity_wire::WireMessage` payload (Gateway native protocol).
    WirePayload(serde_json::Value),
    /// History response.
    History {
        /// Session key the history belongs to, when known.
        session_key: Option<String>,
        messages: Vec<ClawHistoryMessage>,
    },
    /// Pairing result (OpenClaw only).
    PairingResult {
        /// Paired device id.
        device_id: String,
        /// Whether pairing was approved.
        approved: bool,
        /// Auth token returned by the Gateway.
        token: Option<String>,
        /// Granted scopes.
        scopes: Vec<String>,
    },
    /// The connection is retrying after a transient failure (OpenClaw only).
    #[allow(dead_code)]
    ReconnectPending {
        /// Human-readable reason for the reconnect.
        reason: String,
        /// Seconds until the next retry attempt.
        seconds: u64,
    },
    /// Role-context sync response from the Gateway.
    RoleContextSynced {
        /// Role that was synchronized.
        role_id: String,
        /// Session key the events belong to, when known.
        session_key: Option<String>,
        /// Missing events.
        events: Vec<clarity_contract::ClawContextEvent>,
        /// Cursor for the next sync request.
        #[allow(dead_code)]
        next_cursor: Option<String>,
        /// Devices currently online for this role.
        online_devices: Vec<String>,
    },
    /// Terminal error or provider error.
    Error(String),
}

/// A protocol-agnostic handle for an active Claw connection.
///
/// Internally this wraps a `clarity_claw::ClawConnectionManager` that
/// auto-detects the remote dialect (OpenClaw JSON-RPC vs native Gateway
/// WebSocket) from the server's first message.
#[derive(Clone)]
pub struct ClawClientHandle {
    manager: clarity_claw::ClawConnectionManager,
}

impl ClawClientHandle {
    /// Wrap a pre-configured connection manager.
    pub fn new(manager: clarity_claw::ClawConnectionManager) -> Self {
        Self { manager }
    }

    /// Send a chat message. The wire method is chosen by the detected dialect:
    /// Gateway WebSocket uses `chat.send`; OpenClaw JSON-RPC uses `sessions.send`.
    pub fn send_chat(&self, session_key: &str, message: &str) {
        self.manager.send(clarity_claw::ProtocolCommand::Chat {
            session_key: session_key.into(),
            message: message.into(),
        });
    }

    /// Request conversation history for the given session key.
    pub fn get_history(&self, session_key: &str) {
        self.manager
            .send(clarity_claw::ProtocolCommand::GetHistory {
                session_key: session_key.into(),
            });
    }

    /// Subscribe to session-level events (OpenClaw only; no-op for Gateway).
    pub fn subscribe_session(&self, key: &str) {
        self.manager
            .send(clarity_claw::ProtocolCommand::SubscribeSession { key: key.into() });
    }

    /// Subscribe to message-level events (OpenClaw only; no-op for Gateway).
    pub fn subscribe_messages(&self, key: &str) {
        self.manager
            .send(clarity_claw::ProtocolCommand::SubscribeMessages { key: key.into() });
    }

    /// Request missing role-context events for the given role.
    pub fn sync_role_context(&self, role_id: &str, since_event_id: Option<&str>, device_id: &str) {
        self.manager
            .send(clarity_claw::ProtocolCommand::SyncRoleContext {
                role_id: role_id.into(),
                since_event_id: since_event_id.map(Into::into),
                device_id: device_id.into(),
            });
    }

    /// Set or clear the passphrase used to encrypt role-context events at rest.
    pub fn set_role_passphrase(&self, role_id: &str, passphrase: &str) {
        self.manager.set_role_passphrase(role_id, passphrase);
    }

    /// Drain all pending events from the underlying manager and normalize them to
    /// [`ClawEvent`].
    pub fn drain(&self) -> Vec<ClawEvent> {
        self.manager
            .drain()
            .into_iter()
            .flat_map(map_protocol_event)
            .collect()
    }
}

fn map_protocol_event(event: clarity_claw::ProtocolEvent) -> Vec<ClawEvent> {
    use clarity_claw::ProtocolEvent;
    let mut events = Vec::new();
    match event {
        ProtocolEvent::Connected {
            gateway_url,
            session_id,
        } => {
            events.push(ClawEvent::Connected {
                gateway_url,
                session_id,
            });
        }
        ProtocolEvent::ChatChunk(text) => {
            if !text.trim().is_empty() {
                events.push(ClawEvent::StreamChunk(text));
            }
        }
        ProtocolEvent::Done => {
            events.push(ClawEvent::Done);
        }
        ProtocolEvent::History(messages) => {
            events.push(ClawEvent::History {
                // ProtocolEvent currently does not carry the originating
                // session_key; the egui layer falls back to the active Claw
                // session when this is None.
                session_key: None,
                messages: messages
                    .into_iter()
                    .map(|m| ClawHistoryMessage {
                        role: m.role,
                        content: m.content,
                    })
                    .collect(),
            });
        }
        ProtocolEvent::PairingResult {
            device_id,
            approved,
            token,
            scopes,
        } => {
            events.push(ClawEvent::PairingResult {
                device_id,
                approved,
                token,
                scopes,
            });
        }
        ProtocolEvent::ReconnectPending { reason, seconds } => {
            events.push(ClawEvent::ReconnectPending { reason, seconds });
        }
        ProtocolEvent::Error(e) => {
            events.push(ClawEvent::Error(e));
        }
        ProtocolEvent::WireMessage(payload) => {
            events.push(ClawEvent::WirePayload(payload));
        }
        ProtocolEvent::RoleContextSynced {
            role_id,
            events: sync_events,
            next_cursor,
            online_devices,
        } => {
            events.push(ClawEvent::RoleContextSynced {
                role_id,
                // ProtocolEvent currently does not include a session_key; the
                // egui layer routes by role_id when this is None.
                session_key: None,
                events: sync_events,
                next_cursor,
                online_devices,
            });
        }
        ProtocolEvent::Unsupported { reason } => {
            events.push(ClawEvent::Error(reason));
        }
    }
    events
}

// ── DeviceState ────────────────────────────────────────────────────────

/// Per-device health metrics used to rank online role instances.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DeviceHealth {
    /// Number of successful interactions recorded for this device.
    pub success_count: u32,
    /// Number of failed interactions recorded for this device.
    pub failure_count: u32,
    /// Timestamp (ms since UNIX epoch) of the last recorded success.
    pub last_success_at_ms: u64,
    /// Timestamp (ms since UNIX epoch) of the last recorded failure.
    pub last_failure_at_ms: u64,
    /// EWMA latency in milliseconds; 0 means "unknown".
    pub latency_ewma_ms: u64,
}

/// Aggregated device list + per-device connection parameters.
///
/// Devices are stored grouped by their `role` so that role-based routing can
/// pick an instance without scanning a flat list on every frame.
#[derive(Clone)]
pub struct DeviceState {
    /// role → devices with that role
    roles: Arc<RwLock<HashMap<String, Vec<BotInstance>>>>,
    /// device_id → connection info
    connections: Arc<RwLock<HashMap<String, ClawConnection>>>,
    /// device_id → accumulated health metrics
    health: Arc<RwLock<HashMap<String, DeviceHealth>>>,
    /// role → most recently picked device_id
    last_picked: Arc<RwLock<HashMap<String, String>>>,
}

impl Default for DeviceState {
    fn default() -> Self {
        Self {
            roles: Arc::new(RwLock::new(HashMap::new())),
            connections: Arc::new(RwLock::new(HashMap::new())),
            health: Arc::new(RwLock::new(HashMap::new())),
            last_picked: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl DeviceState {
    /// Snapshot of the current device list for the UI thread.
    ///
    /// Returns a flat, deterministic ordering: sorted role name, then sorted
    /// device id. This preserves compatibility with panels that expect a
    /// one-dimensional list.
    pub fn snapshot(&self) -> Vec<BotInstance> {
        let guard = match self.roles.read() {
            Ok(g) => g,
            Err(_) => return Vec::new(),
        };
        let mut role_names: Vec<String> = guard.keys().cloned().collect();
        role_names.sort();
        let mut out = Vec::new();
        for role in role_names {
            if let Some(devices) = guard.get(&role) {
                let mut sorted = devices.clone();
                sorted.sort_by(|a, b| a.id.cmp(&b.id));
                out.extend(sorted);
            }
        }
        out
    }

    /// Snapshot grouped by role, useful for UI sections that render devices
    /// under role headings.
    #[allow(dead_code)]
    pub fn snapshot_grouped(&self) -> Vec<(String, Vec<BotInstance>)> {
        let guard = match self.roles.read() {
            Ok(g) => g,
            Err(_) => return Vec::new(),
        };
        let mut role_names: Vec<String> = guard.keys().cloned().collect();
        role_names.sort();
        role_names
            .into_iter()
            .filter_map(|role| {
                let mut devices = guard.get(&role)?.clone();
                devices.sort_by(|a, b| a.id.cmp(&b.id));
                Some((role, devices))
            })
            .collect()
    }

    /// A session-centric view of the discovered device list.
    ///
    /// Devices that share the same `(role, effective_session_key)` are grouped
    /// together because they all route to the same target Claw session. The UI
    /// renders one row per group and lets the user expand the device sub-list
    /// for pinning / failover.
    pub fn snapshot_by_session(&self) -> Vec<ClawSessionGroup> {
        let guard = match self.roles.read() {
            Ok(g) => g,
            Err(_) => return Vec::new(),
        };
        let mut groups: HashMap<(String, String), Vec<BotInstance>> = HashMap::new();
        for (role, devices) in guard.iter() {
            for device in devices {
                let session_key = device
                    .session_key
                    .clone()
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| crate::session::claw_session_key(role));
                groups
                    .entry((role.clone(), session_key))
                    .or_default()
                    .push(device.clone());
            }
        }
        let mut out: Vec<ClawSessionGroup> = groups
            .into_iter()
            .map(|((role, session_key), mut devices)| {
                devices.sort_by(|a, b| a.id.cmp(&b.id));
                ClawSessionGroup {
                    role,
                    session_key,
                    devices,
                }
            })
            .collect();
        out.sort_by(|a, b| (&a.role, &a.session_key).cmp(&(&b.role, &b.session_key)));
        out
    }

    /// Look up connection parameters for a device.
    pub fn connection(&self, device_id: &str) -> Option<ClawConnection> {
        self.connections.read().ok()?.get(device_id).cloned()
    }

    /// Connection for the active device, or the first available.
    pub fn active_connection(&self, active_bot_id: &str) -> Option<ClawConnection> {
        self.connection(active_bot_id).or_else(|| {
            let devs = self.snapshot();
            devs.first().and_then(|d| self.connection(&d.id))
        })
    }

    /// Pick a bot instance for a Claw session according to the requested role
    /// and affinity.
    ///
    /// - `Specific(device_id)`: returns the device with that id if it is online
    ///   or syncing, searching across roles. If the pinned device is offline or
    ///   missing, falls back to the healthiest online/syncing instance of the
    ///   requested role.
    /// - `AnyOnline`: returns the healthiest online/syncing device in the
    ///   requested role according to `best_in_role`.
    pub fn pick_instance(&self, role: &str, affinity: &DeviceAffinity) -> Option<BotInstance> {
        let guard = self.roles.read().ok()?;
        let health = self.health.read().ok()?;
        let last_picked = self.last_picked.read().ok()?;
        match affinity {
            DeviceAffinity::Specific(device_id) => {
                // Prefer the requested device if it is still alive.
                let preferred = guard.get(role).into_iter();
                let others = guard
                    .iter()
                    .filter(|(r, _)| r.as_str() != role)
                    .map(|(_, v)| v);
                if let Some(device) = preferred.chain(others).flat_map(|v| v.iter()).find(|b| {
                    b.id == *device_id && matches!(b.status, BotStatus::Online | BotStatus::Syncing)
                }) {
                    return Some(device.clone());
                }
                // Pinned device is offline or missing — failover to the best
                // online/syncing instance of the requested role.
                guard
                    .get(role)
                    .and_then(|devices| best_in_role(devices, &health, &last_picked, role))
            }
            DeviceAffinity::AnyOnline => guard
                .get(role)
                .and_then(|devices| best_in_role(devices, &health, &last_picked, role)),
        }
    }

    /// Record a successful interaction with a device and update EWMA latency.
    pub fn record_success(&self, device_id: &str, latency_ms: u64) {
        if let Ok(mut guard) = self.health.write() {
            let now = crate::session::now_millis();
            let h = guard.entry(device_id.into()).or_default();
            h.success_count = h.success_count.saturating_add(1);
            h.last_success_at_ms = now;
            if latency_ms > 0 {
                let alpha = 0.25;
                if h.latency_ewma_ms == 0 {
                    h.latency_ewma_ms = latency_ms;
                } else {
                    h.latency_ewma_ms = (alpha * latency_ms as f64
                        + (1.0 - alpha) * h.latency_ewma_ms as f64)
                        .round() as u64;
                }
            }
        }
    }

    /// Record a failed interaction with a device.
    pub fn record_failure(&self, device_id: &str) {
        if let Ok(mut guard) = self.health.write() {
            let now = crate::session::now_millis();
            let h = guard.entry(device_id.into()).or_default();
            h.failure_count = h.failure_count.saturating_add(1);
            h.last_failure_at_ms = now;
        }
    }

    /// Remember which device was most recently picked for a role.
    pub fn set_last_picked(&self, role: &str, device_id: &str) {
        if let Ok(mut guard) = self.last_picked.write() {
            guard.insert(role.into(), device_id.into());
        }
    }

    /// Update the status of a device by id.
    ///
    /// Called by the connection loop when a device goes offline or comes back
    /// online.
    pub fn update_status(&self, device_id: &str, status: BotStatus) {
        if let Ok(mut guard) = self.roles.write() {
            for devices in guard.values_mut() {
                if let Some(device) = devices.iter_mut().find(|d| d.id == device_id) {
                    device.status = status;
                    break;
                }
            }
        }
    }

    /// Returns true if `device_id` belongs to `role` in the current snapshot.
    ///
    /// Used by the UI to decide whether a user-selected device is still valid
    /// for the active Claw session's role, or whether to reset the selection to
    /// the best available instance.
    pub fn has_device_in_role(&self, role: &str, device_id: &str) -> bool {
        let guard = match self.roles.read() {
            Ok(g) => g,
            Err(_) => return false,
        };
        guard
            .get(role)
            .map(|devices| devices.iter().any(|d| d.id == device_id))
            .unwrap_or(false)
    }

    /// Return the first session-key override found for `role`, if any.
    ///
    /// Remote OpenClaw connections may specify a custom session key (e.g. the
    /// remote's existing main session UUID) that should be used instead of the
    /// default `agent:main:<role>` key.
    pub fn session_key_for_role(&self, role: &str) -> Option<String> {
        let guard = match self.roles.read() {
            Ok(g) => g,
            Err(_) => return None,
        };
        guard
            .get(role)
            .and_then(|devices| devices.iter().find_map(|d| d.session_key.clone()))
    }

    /// Set the session-key override for a device by id.
    #[allow(dead_code)]
    pub fn set_session_key(&self, device_id: &str, session_key: String) {
        if let Ok(mut guard) = self.roles.write() {
            for devices in guard.values_mut() {
                if let Some(device) = devices.iter_mut().find(|d| d.id == device_id) {
                    device.session_key = Some(session_key);
                    break;
                }
            }
        }
    }

    /// Add a device with its connection info.
    fn register(&self, device: BotInstance, conn: ClawConnection) {
        if let Ok(mut guard) = self.roles.write() {
            guard
                .entry(device.role.clone())
                .or_default()
                .push(device.clone());
        }
        if let Ok(mut conns) = self.connections.write() {
            conns.insert(device.id.clone(), conn);
        }
        if let Ok(mut health) = self.health.write() {
            health.entry(device.id).or_default();
        }
    }
}

/// Discovered devices grouped by the Claw session they target.
///
/// All devices in a group share the same `(role, session_key)` and therefore
/// the same Gateway-side session context. The UI uses this as the primary
/// navigation unit, with the individual devices shown as a collapsible
/// failover list.
#[derive(Clone, Debug)]
pub struct ClawSessionGroup {
    pub role: String,
    pub session_key: String,
    pub devices: Vec<BotInstance>,
}

/// Select the healthiest online/syncing device in a role.
///
/// Score ordering is ascending; lower is better:
///
/// 1. Fewer recorded failures.
/// 2. Lower EWMA latency (unknown latency is treated as `u64::MAX`).
/// 3. More recent last success.
/// 4. Device id matches the role's `last_picked` entry.
/// 5. Stable registration order as a final tie-breaker.
fn best_in_role(
    role_devices: &[BotInstance],
    health: &HashMap<String, DeviceHealth>,
    last_picked: &HashMap<String, String>,
    role: &str,
) -> Option<BotInstance> {
    let preferred = last_picked.get(role).cloned();
    let mut candidates: Vec<(usize, &BotInstance)> = role_devices
        .iter()
        .enumerate()
        .filter(|(_, b)| matches!(b.status, BotStatus::Online | BotStatus::Syncing))
        .collect();
    candidates.sort_by(|(idx_a, a), (idx_b, b)| {
        let ha = health.get(&a.id).copied().unwrap_or_default();
        let hb = health.get(&b.id).copied().unwrap_or_default();

        let latency_a = if ha.latency_ewma_ms == 0 {
            u64::MAX
        } else {
            ha.latency_ewma_ms
        };
        let latency_b = if hb.latency_ewma_ms == 0 {
            u64::MAX
        } else {
            hb.latency_ewma_ms
        };

        let score_a = (
            ha.failure_count,
            latency_a,
            u64::MAX - ha.last_success_at_ms,
            if preferred.as_ref() == Some(&a.id) {
                0
            } else {
                1
            },
            *idx_a,
        );
        let score_b = (
            hb.failure_count,
            latency_b,
            u64::MAX - hb.last_success_at_ms,
            if preferred.as_ref() == Some(&b.id) {
                0
            } else {
                1
            },
            *idx_b,
        );
        score_a.cmp(&score_b)
    });
    candidates.first().map(|(_, b)| (*b).clone())
}

// ── Discovery ──────────────────────────────────────────────────────────

/// Discover all Claw devices from all sources.
///
/// `settings_connections` comes from `GuiSettings::openclaw_connections` and is
/// re-evaluated on every app launch so users can add/remove remote Gateways
/// without recompiling.
pub fn discover(settings_connections: &[OpenClawConnection]) -> DeviceState {
    let state = DeviceState::default();
    let hostname = local_hostname();
    let mut endpoints: HashMap<String, (DeviceSource, BotInstance, ClawConnection)> =
        HashMap::new();

    // Source 1: ZeroClaw (local clarity-claw).
    discover_zeroclaw(&mut endpoints, &hostname);

    // Source 2 & 3: Local and remote OpenClaw via the shared crate
    // (local config + OPENCLAW_REMOTE_* env vars).
    discover_openclaw_crate(&mut endpoints, &hostname);

    // Source 4: User-configured remote OpenClaw connections from settings.
    discover_settings_openclaw(&mut endpoints, settings_connections);

    // Source 5: Persisted paired token (e.g. a remote private Claw Gateway).
    discover_saved_openclaw(&mut endpoints);

    // Ultimate fallback: only register if no endpoint at all was discovered.
    if endpoints.is_empty() {
        discover_fallback(&mut endpoints, &hostname);
    }

    for (_, (source, mut device, conn)) in endpoints {
        device.source = Some(source.label().to_string());
        state.register(device, conn);
    }

    state
}

/// Insert or merge a discovered endpoint into the canonical map.
///
/// Higher-priority sources (lower `DeviceSource` discriminant) win the display
/// name and connection parameters. `session_key` overrides are preserved from
/// any source.
fn merge_endpoint(
    endpoints: &mut HashMap<String, (DeviceSource, BotInstance, ClawConnection)>,
    source: DeviceSource,
    device: BotInstance,
    conn: ClawConnection,
) {
    let default_session_key = crate::session::claw_session_key(&device.role);
    let effective_session_key = device
        .session_key
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or(&default_session_key);
    let key = endpoint_key(&device.role, &conn, effective_session_key);
    match endpoints.entry(key) {
        std::collections::hash_map::Entry::Occupied(mut entry) => {
            let (existing_source, existing_device, existing_conn) = entry.get_mut();
            // Merge session_key override: keep any non-None value.
            let merged_session_key = existing_device
                .session_key
                .clone()
                .or_else(|| device.session_key.clone());
            if source < *existing_source {
                // Higher-priority source replaces the previous config.
                *existing_source = source;
                *existing_device = device;
                *existing_conn = conn;
            }
            existing_device.session_key = merged_session_key;
            // If the kept config has no session_key but the new one had a token
            // override we might care about, we intentionally do not merge tokens:
            // the higher-priority source's auth parameters are authoritative.
        }
        std::collections::hash_map::Entry::Vacant(entry) => {
            entry.insert((source, device, conn));
        }
    }
}

fn discover_settings_openclaw(
    endpoints: &mut HashMap<String, (DeviceSource, BotInstance, ClawConnection)>,
    connections: &[OpenClawConnection],
) {
    for conn in connections
        .iter()
        .filter(|c| c.enabled && !c.gateway_url.is_empty())
    {
        let name = if conn.name.is_empty() {
            format!("OpenClaw ({})", conn.gateway_url)
        } else {
            conn.name.clone()
        };
        let host = gateway_host(&conn.gateway_url).unwrap_or_else(|| "openclaw".into());
        let id = format!("openclaw-settings-{}", host);
        let auth_mode = Some(match conn.auth_mode {
            OpenClawAuthMode::TokenOnly => "token_only".into(),
            OpenClawAuthMode::TokenWithDevice => "token_with_device".into(),
            OpenClawAuthMode::DevicePaired => "device_paired".into(),
        });
        let send_method = match conn.send_method {
            crate::settings::OpenClawSendMethod::SessionsSend => {
                clarity_claw::types::OpenClawSendMethod::SessionsSend
            }
            crate::settings::OpenClawSendMethod::ChatSend => {
                clarity_claw::types::OpenClawSendMethod::ChatSend
            }
        };
        merge_endpoint(
            endpoints,
            DeviceSource::Settings,
            BotInstance {
                id: id.clone(),
                name,
                device_id: id,
                role: "operator".into(),
                status: BotStatus::Online,
                version: env!("CARGO_PKG_VERSION").into(),
                last_backup: String::new(),
                session_key: conn.session_key.clone(),
                source: None,
            },
            ClawConnection {
                claw_type: ClawType::OpenClaw,
                protocol: ClawProtocol::OpenClawJsonRpc,
                gateway_url: conn.gateway_url.clone(),
                gateway_token: GuiSettings::resolve_api_key(&conn.token).unwrap_or_default(),
                workspace_root: std::env::current_dir().unwrap_or_default(),
                host,
                auth_mode,
                device_token: GuiSettings::resolve_api_key(&conn.device_token),
                send_method,
            },
        );
    }
}

fn discover_saved_openclaw(
    endpoints: &mut HashMap<String, (DeviceSource, BotInstance, ClawConnection)>,
) {
    let paired = match clarity_claw::load_paired_token() {
        Ok(Some(p)) => p,
        Ok(None) => return,
        Err(e) => {
            tracing::warn!("Failed to load saved OpenClaw token: {}", e);
            return;
        }
    };

    let gateway_url = paired.gateway_url.clone();
    let host = gateway_host(&gateway_url).unwrap_or_else(|| "openclaw".into());
    let id = format!("openclaw-saved-{}", host);
    merge_endpoint(
        endpoints,
        DeviceSource::SavedPairing,
        BotInstance {
            id: id.clone(),
            name: format!("OpenClaw Saved ({host})"),
            device_id: id,
            role: "operator".into(),
            status: BotStatus::Online,
            version: env!("CARGO_PKG_VERSION").into(),
            last_backup: String::new(),
            session_key: None,
            source: None,
        },
        ClawConnection {
            claw_type: ClawType::OpenClaw,
            protocol: ClawProtocol::OpenClawJsonRpc,
            gateway_url,
            gateway_token: paired.auth_token().to_string(),
            workspace_root: std::env::current_dir().unwrap_or_default(),
            host,
            auth_mode: Some("device_paired".into()),
            device_token: None,
            send_method: OpenClawSendMethod::SessionsSend,
        },
    );
}

fn gateway_host(url: &str) -> Option<String> {
    url.trim_start_matches("ws://")
        .trim_start_matches("wss://")
        .trim_start_matches("http://")
        .trim_start_matches("https://")
        .split(':')
        .next()
        .map(String::from)
}

fn discover_zeroclaw(
    endpoints: &mut HashMap<String, (DeviceSource, BotInstance, ClawConnection)>,
    hostname: &str,
) {
    merge_endpoint(
        endpoints,
        DeviceSource::ZeroClaw,
        BotInstance {
            id: "zeroclaw-local".into(),
            name: format!("{} (ZeroClaw)", hostname),
            device_id: hostname.into(),
            role: "operator".into(),
            status: BotStatus::Online,
            session_key: None,
            version: env!("CARGO_PKG_VERSION").into(),
            last_backup: String::new(),
            source: None,
        },
        ClawConnection {
            claw_type: ClawType::ZeroClaw,
            protocol: ClawProtocol::GatewayWebSocket,
            gateway_url: "http://127.0.0.1:18790".to_string(),
            gateway_token: String::new(),
            workspace_root: std::env::current_dir().unwrap_or_default(),
            host: hostname.into(),
            auth_mode: None,
            device_token: None,
            send_method: OpenClawSendMethod::SessionsSend,
        },
    );
}

/// Ultimate fallback when no Claw endpoint was discovered at all.
fn discover_fallback(
    endpoints: &mut HashMap<String, (DeviceSource, BotInstance, ClawConnection)>,
    hostname: &str,
) {
    merge_endpoint(
        endpoints,
        DeviceSource::Fallback,
        BotInstance {
            id: "claw-fallback-local".into(),
            name: format!("{} (Fallback)", hostname),
            device_id: hostname.into(),
            role: "operator".into(),
            status: BotStatus::Offline,
            session_key: None,
            version: env!("CARGO_PKG_VERSION").into(),
            last_backup: String::new(),
            source: None,
        },
        ClawConnection {
            claw_type: ClawType::ZeroClaw,
            protocol: ClawProtocol::GatewayWebSocket,
            gateway_url: "http://127.0.0.1:18790".to_string(),
            gateway_token: String::new(),
            workspace_root: std::env::current_dir().unwrap_or_default(),
            host: hostname.into(),
            auth_mode: None,
            device_token: None,
            send_method: OpenClawSendMethod::SessionsSend,
        },
    );
}

fn discover_openclaw_crate(
    endpoints: &mut HashMap<String, (DeviceSource, BotInstance, ClawConnection)>,
    hostname: &str,
) {
    for record in clarity_claw::discovery::discover_openclaw_devices(hostname) {
        let mut device = BotInstance {
            id: record.info.id,
            name: record.info.name,
            device_id: record.info.device_id,
            role: "operator".into(),
            status: map_status(record.info.status),
            version: record.info.version,
            last_backup: String::new(),
            session_key: None,
            source: None,
        };
        // Env-var remote connections may override the target session key.
        if device.id == "openclaw-remote-env"
            && let Ok(remote_session_key) = std::env::var("OPENCLAW_REMOTE_SESSION_KEY")
            && !remote_session_key.is_empty()
        {
            device.session_key = Some(remote_session_key);
        }
        let source = if device.id == "openclaw-remote-env" {
            DeviceSource::Env
        } else {
            DeviceSource::LocalDiscovery
        };
        merge_endpoint(endpoints, source, device, record.connection);
    }
}

fn map_status(status: clarity_claw::types::DeviceStatus) -> BotStatus {
    match status {
        clarity_claw::types::DeviceStatus::Online => BotStatus::Online,
        clarity_claw::types::DeviceStatus::Offline => BotStatus::Offline,
        clarity_claw::types::DeviceStatus::Syncing => BotStatus::Syncing,
    }
}

fn local_hostname() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "unknown".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{OpenClawAuthMode, OpenClawSendMethod};

    fn sample_settings_connection() -> OpenClawConnection {
        OpenClawConnection {
            name: "Gray-Cloud".into(),
            gateway_url: "wss://gray-cloud.example:18789".into(),
            token: Some("token-with-device".into()),
            auth_mode: OpenClawAuthMode::TokenWithDevice,
            enabled: true,
            device_token: None,
            session_key: Some("custom:main:operator".into()),
            send_method: OpenClawSendMethod::SessionsSend,
        }
    }

    fn bot(id: &str, role: &str, status: BotStatus) -> BotInstance {
        BotInstance {
            id: id.into(),
            name: format!("Bot {}", id),
            device_id: id.into(),
            role: role.into(),
            status,
            version: "0.0.0".into(),
            last_backup: String::new(),
            session_key: None,
            source: None,
        }
    }

    fn conn(id: &str) -> ClawConnection {
        ClawConnection {
            claw_type: ClawType::ZeroClaw,
            protocol: ClawProtocol::OpenClawJsonRpc,
            gateway_url: format!("http://{}", id),
            gateway_token: String::new(),
            workspace_root: std::env::current_dir().unwrap_or_default(),
            host: id.into(),
            auth_mode: None,
            device_token: None,
            send_method: crate::claw::OpenClawSendMethod::SessionsSend,
        }
    }

    #[test]
    fn test_devices_grouped_by_role() {
        let state = DeviceState::default();
        state.register(bot("a-op-1", "operator", BotStatus::Online), conn("a-op-1"));
        state.register(
            bot("a-coder-1", "coder", BotStatus::Online),
            conn("a-coder-1"),
        );
        state.register(
            bot("a-op-2", "operator", BotStatus::Offline),
            conn("a-op-2"),
        );

        let grouped = state.snapshot_grouped();
        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped[0].0, "coder");
        assert_eq!(grouped[0].1.len(), 1);
        assert_eq!(grouped[1].0, "operator");
        assert_eq!(grouped[1].1.len(), 2);

        // Flat snapshot is sorted by role then id.
        let flat = state.snapshot();
        let ids: Vec<_> = flat.iter().map(|b| b.id.as_str()).collect();
        assert_eq!(ids, vec!["a-coder-1", "a-op-1", "a-op-2"]);
    }

    #[test]
    fn test_pick_instance_specific() {
        let state = DeviceState::default();
        state.register(bot("op-1", "operator", BotStatus::Online), conn("op-1"));
        state.register(bot("coder-1", "coder", BotStatus::Online), conn("coder-1"));

        let picked = state
            .pick_instance("operator", &DeviceAffinity::Specific("op-1".into()))
            .expect("finds specific operator device");
        assert_eq!(picked.id, "op-1");

        // Searching within a role that does not contain the id falls back to
        // other roles.
        let cross = state
            .pick_instance("operator", &DeviceAffinity::Specific("coder-1".into()))
            .expect("falls back across roles");
        assert_eq!(cross.id, "coder-1");

        // A missing pinned id falls back to the first online/syncing device in
        // the requested role.
        let missing = state
            .pick_instance("operator", &DeviceAffinity::Specific("missing".into()))
            .expect("falls back to online device in role");
        assert_eq!(missing.id, "op-1");
    }

    #[test]
    fn test_pick_instance_any_online() {
        let state = DeviceState::default();
        state.register(
            bot("op-off", "operator", BotStatus::Offline),
            conn("op-off"),
        );
        state.register(
            bot("op-sync", "operator", BotStatus::Syncing),
            conn("op-sync"),
        );
        state.register(bot("op-on", "operator", BotStatus::Online), conn("op-on"));

        let picked = state
            .pick_instance("operator", &DeviceAffinity::AnyOnline)
            .expect("finds online or syncing device");
        // Devices are kept in registration order; the first online/syncing
        // device is op-sync.
        assert_eq!(picked.id, "op-sync");
        assert!(matches!(
            picked.status,
            BotStatus::Online | BotStatus::Syncing
        ));
    }

    #[test]
    fn test_pick_instance_specific_failover_to_online() {
        let state = DeviceState::default();
        state.register(
            bot("op-off", "operator", BotStatus::Offline),
            conn("op-off"),
        );
        state.register(
            bot("op-sync", "operator", BotStatus::Syncing),
            conn("op-sync"),
        );
        state.register(bot("op-on", "operator", BotStatus::Online), conn("op-on"));

        // Pinned offline device fails over to the first online/syncing device
        // in the same role.
        let picked = state
            .pick_instance("operator", &DeviceAffinity::Specific("op-off".into()))
            .expect("fails over to online/syncing device");
        assert_eq!(picked.id, "op-sync");

        // A missing pinned id also falls back to the first online/syncing role
        // device.
        let missing = state
            .pick_instance("operator", &DeviceAffinity::Specific("missing".into()))
            .expect("fails over to online/syncing device");
        assert_eq!(missing.id, "op-sync");
    }

    #[test]
    fn test_update_status() {
        let state = DeviceState::default();
        state.register(bot("op-1", "operator", BotStatus::Online), conn("op-1"));

        state.update_status("op-1", BotStatus::Offline);
        let flat = state.snapshot();
        assert_eq!(flat[0].status, BotStatus::Offline);

        state.update_status("missing", BotStatus::Syncing);
        // No panic and existing device unchanged.
        assert_eq!(state.snapshot()[0].status, BotStatus::Offline);
    }

    #[test]
    fn test_discover_settings_preserves_auth_mode() {
        let conn = sample_settings_connection();
        let state = discover(&[conn]);
        let snapshot = state.snapshot();
        let bot = snapshot
            .iter()
            .find(|b| b.name == "Gray-Cloud")
            .expect("Gray-Cloud bot registered");
        let c = state.connection(&bot.id).expect("connection exists");
        assert_eq!(c.auth_mode.as_deref(), Some("token_with_device"));
        assert_eq!(c.gateway_token, "token-with-device");
    }

    #[test]
    fn test_discover_settings_device_paired() {
        let mut conn = sample_settings_connection();
        conn.auth_mode = OpenClawAuthMode::DevicePaired;
        conn.device_token = Some("paired-device-token".into());
        let state = discover(&[conn]);
        let snapshot = state.snapshot();
        let bot = snapshot
            .iter()
            .find(|b| b.name == "Gray-Cloud")
            .expect("Gray-Cloud bot registered");
        let c = state.connection(&bot.id).expect("connection exists");
        assert_eq!(c.auth_mode.as_deref(), Some("device_paired"));
        assert_eq!(c.device_token.as_deref(), Some("paired-device-token"));
    }

    #[test]
    fn test_discover_settings_propagates_session_key() {
        let mut conn = sample_settings_connection();
        conn.session_key = Some("467adc23-03cb-42b5-923e-4824c22ead4f".into());
        let state = discover(&[conn]);
        let snapshot = state.snapshot();
        let bot = snapshot
            .iter()
            .find(|b| b.name == "Gray-Cloud")
            .expect("Gray-Cloud bot registered");
        assert_eq!(
            bot.session_key.as_deref(),
            Some("467adc23-03cb-42b5-923e-4824c22ead4f")
        );
        assert_eq!(
            state.session_key_for_role("operator").as_deref(),
            Some("467adc23-03cb-42b5-923e-4824c22ead4f")
        );
    }

    #[test]
    fn test_health_score_prefers_lower_failures() {
        let state = DeviceState::default();
        state.register(bot("a", "operator", BotStatus::Online), conn("a"));
        state.register(bot("b", "operator", BotStatus::Online), conn("b"));
        state.record_failure("a");
        let picked = state
            .pick_instance("operator", &DeviceAffinity::AnyOnline)
            .expect("finds online device");
        assert_eq!(picked.id, "b");
    }

    #[test]
    fn test_health_score_prefers_lower_latency() {
        let state = DeviceState::default();
        state.register(bot("a", "operator", BotStatus::Online), conn("a"));
        state.register(bot("b", "operator", BotStatus::Online), conn("b"));
        state.record_success("a", 200);
        state.record_success("b", 50);
        let picked = state
            .pick_instance("operator", &DeviceAffinity::AnyOnline)
            .expect("finds online device");
        assert_eq!(picked.id, "b");
    }

    #[test]
    fn test_health_score_prefers_last_success() {
        let state = DeviceState::default();
        state.register(bot("a", "operator", BotStatus::Online), conn("a"));
        state.register(bot("b", "operator", BotStatus::Online), conn("b"));
        if let Ok(mut h) = state.health.write() {
            h.insert(
                "a".into(),
                DeviceHealth {
                    last_success_at_ms: 1000,
                    ..Default::default()
                },
            );
            h.insert(
                "b".into(),
                DeviceHealth {
                    last_success_at_ms: 2000,
                    ..Default::default()
                },
            );
        }
        let picked = state
            .pick_instance("operator", &DeviceAffinity::AnyOnline)
            .expect("finds online device");
        assert_eq!(picked.id, "b");
    }

    #[test]
    fn test_last_picked_tiebreaker() {
        let state = DeviceState::default();
        state.register(bot("a", "operator", BotStatus::Online), conn("a"));
        state.register(bot("b", "operator", BotStatus::Online), conn("b"));
        state.set_last_picked("operator", "b");
        let picked = state
            .pick_instance("operator", &DeviceAffinity::AnyOnline)
            .expect("finds online device");
        assert_eq!(picked.id, "b");
    }

    #[test]
    fn test_specific_failover_uses_health_score() {
        let state = DeviceState::default();
        state.register(
            bot("pinned", "operator", BotStatus::Offline),
            conn("pinned"),
        );
        state.register(bot("a", "operator", BotStatus::Online), conn("a"));
        state.register(bot("b", "operator", BotStatus::Online), conn("b"));
        state.record_failure("a");
        let picked = state
            .pick_instance("operator", &DeviceAffinity::Specific("pinned".into()))
            .expect("fails over to online device");
        assert_eq!(picked.id, "b");
    }

    #[test]
    fn test_claw_event_mapping_connected() {
        let events = map_protocol_event(clarity_claw::ProtocolEvent::Connected {
            gateway_url: "wss://gray-cloud.example:18789".into(),
            session_id: None,
        });
        assert_eq!(events.len(), 1);
        match &events[0] {
            ClawEvent::Connected {
                gateway_url,
                session_id,
            } => {
                assert_eq!(gateway_url, "wss://gray-cloud.example:18789");
                assert!(session_id.is_none());
            }
            _ => panic!("expected Connected event"),
        }
    }

    #[test]
    fn test_claw_event_mapping_wire_payload() {
        let payload = serde_json::json!({"foo": "bar"});
        let events = map_protocol_event(clarity_claw::ProtocolEvent::WireMessage(payload.clone()));
        assert_eq!(events.len(), 1);
        match &events[0] {
            ClawEvent::WirePayload(p) => assert_eq!(p, &payload),
            _ => panic!("expected WirePayload event"),
        }
    }

    #[test]
    fn test_claw_event_mapping_history() {
        let events = map_protocol_event(clarity_claw::ProtocolEvent::History(vec![
            clarity_claw::ProtocolHistoryMessage {
                role: "user".into(),
                content: "hello".into(),
            },
            clarity_claw::ProtocolHistoryMessage {
                role: "assistant".into(),
                content: "hi there".into(),
            },
        ]));
        assert_eq!(events.len(), 1);
        match &events[0] {
            ClawEvent::History {
                session_key,
                messages,
            } => {
                assert!(session_key.is_none());
                assert_eq!(messages.len(), 2);
                assert_eq!(messages[0].role, "user");
                assert_eq!(messages[0].content, "hello");
                assert_eq!(messages[1].role, "assistant");
                assert_eq!(messages[1].content, "hi there");
            }
            _ => panic!("expected History event"),
        }
    }

    #[test]
    fn test_claw_event_mapping_pairing_result() {
        let events = map_protocol_event(clarity_claw::ProtocolEvent::PairingResult {
            device_id: "dev-1".into(),
            approved: true,
            token: Some("tok".into()),
            scopes: vec!["operator.read".into()],
        });
        assert_eq!(events.len(), 1);
        match &events[0] {
            ClawEvent::PairingResult {
                device_id,
                approved,
                token,
                scopes,
            } => {
                assert_eq!(device_id, "dev-1");
                assert!(approved);
                assert_eq!(token.as_deref(), Some("tok"));
                assert_eq!(scopes, &["operator.read"]);
            }
            _ => panic!("expected PairingResult event"),
        }
    }

    #[test]
    fn test_claw_event_mapping_reconnect_pending() {
        let events = map_protocol_event(clarity_claw::ProtocolEvent::ReconnectPending {
            reason: "network flap".into(),
            seconds: 4,
        });
        assert_eq!(events.len(), 1);
        match &events[0] {
            ClawEvent::ReconnectPending { reason, seconds } => {
                assert_eq!(reason, "network flap");
                assert_eq!(*seconds, 4);
            }
            _ => panic!("expected ReconnectPending event"),
        }
    }

    #[test]
    fn test_snapshot_by_session_groups_by_effective_key() {
        let state = DeviceState::default();
        state.register(bot("op-1", "operator", BotStatus::Online), conn("op-1"));
        state.register(bot("op-2", "operator", BotStatus::Online), conn("op-2"));
        state.register(bot("op-3", "operator", BotStatus::Online), conn("op-3"));
        state.set_session_key("op-3", "custom:main:operator".into());
        state.register(bot("coder-1", "coder", BotStatus::Online), conn("coder-1"));

        let groups = state.snapshot_by_session();
        assert_eq!(groups.len(), 3);

        let operator_default = groups
            .iter()
            .find(|g| g.role == "operator" && g.session_key == "agent:main:operator")
            .expect("operator default group");
        assert_eq!(operator_default.devices.len(), 2);

        let operator_custom = groups
            .iter()
            .find(|g| g.role == "operator" && g.session_key == "custom:main:operator")
            .expect("operator custom group");
        assert_eq!(operator_custom.devices.len(), 1);
        assert_eq!(operator_custom.devices[0].id, "op-3");

        let coder_group = groups
            .iter()
            .find(|g| g.role == "coder")
            .expect("coder group");
        assert_eq!(coder_group.devices.len(), 1);
    }

    #[test]
    fn test_merge_endpoint_deduplicates_same_session() {
        let mut endpoints = HashMap::new();
        merge_endpoint(
            &mut endpoints,
            DeviceSource::Settings,
            BotInstance {
                id: "settings-op".into(),
                name: "Settings".into(),
                device_id: "settings-op".into(),
                role: "operator".into(),
                status: BotStatus::Online,
                version: "0.0.0".into(),
                last_backup: String::new(),
                session_key: None,
                source: None,
            },
            ClawConnection {
                claw_type: ClawType::OpenClaw,
                protocol: ClawProtocol::OpenClawJsonRpc,
                gateway_url: "ws://gray-cloud.example:18789".into(),
                gateway_token: "admin-token".into(),
                workspace_root: std::env::current_dir().unwrap_or_default(),
                host: "gray-cloud".into(),
                auth_mode: Some("token_with_device".into()),
                device_token: None,
                send_method: crate::claw::OpenClawSendMethod::SessionsSend,
            },
        );
        merge_endpoint(
            &mut endpoints,
            DeviceSource::SavedPairing,
            BotInstance {
                id: "paired-op".into(),
                name: "Paired".into(),
                device_id: "paired-op".into(),
                role: "operator".into(),
                status: BotStatus::Online,
                version: "0.0.0".into(),
                last_backup: String::new(),
                session_key: None,
                source: None,
            },
            ClawConnection {
                claw_type: ClawType::OpenClaw,
                protocol: ClawProtocol::OpenClawJsonRpc,
                gateway_url: "ws://gray-cloud.example:18789".into(),
                gateway_token: "device-token".into(),
                workspace_root: std::env::current_dir().unwrap_or_default(),
                host: "gray-cloud".into(),
                auth_mode: Some("device_paired".into()),
                device_token: None,
                send_method: crate::claw::OpenClawSendMethod::SessionsSend,
            },
        );

        assert_eq!(endpoints.len(), 1);
        let (_, (source, device, _)) = endpoints.into_iter().next().expect("one endpoint");
        assert_eq!(source, DeviceSource::Settings);
        assert_eq!(device.id, "settings-op");
    }
}
