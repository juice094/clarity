//! Shared runtime context for the egui desktop shell.
//!
//! `AppContext` holds all cross-cutting services and domain stores that panels
//! and service methods need. Keeping them in one place lets `App` stay focused
//! on chrome, routing, and sub-application orchestration, and makes it possible
//! to borrow just the context without dragging the whole UI tree into scope.

use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::time::Instant;

use crate::app_state::AppState;
use crate::ui::types::UiEvent;

/// Runtime services and shared domain state available to every panel.
///
/// Anything that outlives a single frame and is needed by more than one panel
/// belongs here. Transient chrome state (routers, animation, focus) stays in
/// [`crate::App`].
pub(crate) struct AppContext {
    // === Core Runtime ===
    pub(crate) state: Arc<AppState>,
    pub(crate) runtime: tokio::runtime::Runtime,
    pub(crate) ui_tx: Sender<UiEvent>,

    // === Domain Stores (Zustand-style slices) ===
    pub(crate) session_store: crate::stores::SessionStore,
    pub(crate) ui_store: crate::stores::UiStore,
    pub(crate) mcp_store: crate::stores::McpStore,
    pub(crate) onboarding_store: crate::stores::OnboardingStore,
    pub(crate) project_store: crate::stores::ProjectStore,
    pub(crate) snapshot_store: crate::stores::SnapshotStore,
    pub(crate) knowledge_store: crate::stores::KnowledgeStore,
    pub(crate) console_store: crate::stores::ConsoleStore,
    pub(crate) files_store: crate::stores::FilesStore,
    pub(crate) share_store: crate::stores::ShareStore,
    pub(crate) template_store: crate::stores::TemplateStore,

    // === Process / Device Services ===
    /// Gateway process manager (auto-start + manual control).
    #[allow(dead_code)]
    pub(crate) gateway_manager: Option<crate::services::gateway_manager::GatewayManager>,
    /// File-system watcher for live skill reloading.
    #[allow(dead_code)]
    pub(crate) skill_watcher: Option<clarity_core::skills::SkillWatcher>,
    /// System tray manager (minimize-to-tray + context menu).
    pub(crate) tray_manager: Option<crate::services::tray::TrayManager>,

    // === Claw / Peripheral State ===
    /// Live Claw device list polled from Gateway.
    pub(crate) device_state: crate::claw::DeviceState,
    /// Active WebSocket connection to the selected Claw Gateway.
    pub(crate) claw_ws: Option<crate::claw::ClawClientHandle>,
    /// Track which device the current WebSocket is connected to.
    pub(crate) claw_ws_device_id: String,
    /// Cached Clarity device identity for OpenClaw device-paired auth.
    pub(crate) claw_device_identity: Option<clarity_claw::DeviceIdentity>,
    /// Cached paired-device token for the OpenClaw Gateway.
    pub(crate) claw_device_token: Option<clarity_claw::PairedToken>,
    /// Temporary WebSocket client used only for in-app pairing.
    pub(crate) claw_pairing_client: Option<clarity_claw::ClawClient>,
    /// Current state of the in-app pairing flow.
    pub(crate) claw_pairing_state: clarity_shell::PairingState,
}

impl AppContext {
    /// Construct a context from its parts.
    ///
    /// Callers (`App::new`) build the stores and runtime locally, then hand them
    /// off so `App` only needs to hold this single bundle.
    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)] // used by App::new and test_app; rustc cannot see the call graph
    pub(crate) fn new(
        state: Arc<AppState>,
        runtime: tokio::runtime::Runtime,
        ui_tx: Sender<UiEvent>,
        session_store: crate::stores::SessionStore,
        ui_store: crate::stores::UiStore,
        mcp_store: crate::stores::McpStore,
        onboarding_store: crate::stores::OnboardingStore,
        project_store: crate::stores::ProjectStore,
        snapshot_store: crate::stores::SnapshotStore,
        knowledge_store: crate::stores::KnowledgeStore,
        console_store: crate::stores::ConsoleStore,
        files_store: crate::stores::FilesStore,
        share_store: crate::stores::ShareStore,
        template_store: crate::stores::TemplateStore,
        gateway_manager: Option<crate::services::gateway_manager::GatewayManager>,
        skill_watcher: Option<clarity_core::skills::SkillWatcher>,
        tray_manager: Option<crate::services::tray::TrayManager>,
        device_state: crate::claw::DeviceState,
        claw_ws: Option<crate::claw::ClawClientHandle>,
        claw_ws_device_id: String,
        claw_device_identity: Option<clarity_claw::DeviceIdentity>,
        claw_device_token: Option<clarity_claw::PairedToken>,
        claw_pairing_client: Option<clarity_claw::ClawClient>,
        claw_pairing_state: clarity_shell::PairingState,
    ) -> Self {
        Self {
            state,
            runtime,
            ui_tx,
            session_store,
            ui_store,
            mcp_store,
            onboarding_store,
            project_store,
            snapshot_store,
            knowledge_store,
            console_store,
            files_store,
            share_store,
            template_store,
            gateway_manager,
            skill_watcher,
            tray_manager,
            device_state,
            claw_ws,
            claw_ws_device_id,
            claw_device_identity,
            claw_device_token,
            claw_pairing_client,
            claw_pairing_state,
        }
    }

    /// Push a transient toast notification.
    pub(crate) fn push_toast(
        &mut self,
        message: impl Into<String>,
        level: crate::ui::types::ToastLevel,
    ) {
        crate::handlers::system::push_toast(&mut self.ui_store, message, level);
    }

    /// Refresh the cached shell prompt (cwd + git branch).
    pub(crate) fn refresh_shell_prompt(&mut self) {
        let cwd = std::env::current_dir()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_default();
        let branch = Self::detect_git_branch().unwrap_or_default();
        self.ui_store.shell_prompt = if branch.is_empty() {
            cwd
        } else {
            format!("{} {}", cwd, branch)
        };
    }

    /// Detect current git branch by reading `.git/HEAD`.
    fn detect_git_branch() -> Option<String> {
        let head = std::fs::read_to_string(".git/HEAD").ok()?;
        let line = head.trim();
        if let Some(prefix) = line.strip_prefix("ref: refs/heads/") {
            return Some(prefix.to_string());
        }
        if line.len() >= 7 {
            return Some(line[..7].to_string());
        }
        None
    }

    /// Poll `mcp.json` for external changes and hot-reload if modified.
    pub(crate) fn check_mcp_config_reload(&mut self) {
        if self.mcp_store.last_mcp_poll.elapsed() < std::time::Duration::from_secs(5) {
            return;
        }
        self.mcp_store.last_mcp_poll = Instant::now();

        let path = match clarity_core::mcp::config::default_config_path() {
            Ok(p) => p,
            Err(e) => {
                tracing::debug!("MCP default config path unavailable: {}", e);
                return;
            }
        };

        let mtime = match std::fs::metadata(&path).and_then(|m| m.modified()) {
            Ok(t) => Some(t),
            Err(e) => {
                tracing::debug!("Failed to read mcp.json metadata: {}", e);
                None
            }
        };

        if mtime == self.mcp_store.last_mcp_mtime {
            return;
        }
        self.mcp_store.last_mcp_mtime = mtime;

        let config = match clarity_core::mcp::config::McpConfig::load_default() {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("MCP config reload failed: {}", e);
                crate::handlers::system::push_toast(
                    &mut self.ui_store,
                    format!("MCP 配置加载失败: {}", e),
                    crate::ui::types::ToastLevel::Error,
                );
                return;
            }
        };

        self.hot_reload_mcp(config);
    }

    /// Disconnect old MCP tools and register new ones from the given config.
    pub(crate) fn hot_reload_mcp(&mut self, config: clarity_core::mcp::config::McpConfig) {
        let old_tools = self.mcp_store.connected_tools.clone();
        let agent = self.state.agent.clone();
        let tx = self.ui_tx.clone();
        self.runtime.spawn(async move {
            for name in &old_tools {
                let _ = agent.registry().unregister(name);
            }
            let manager = clarity_core::mcp::McpManager::from_config(&config).await;
            let tool_names: Vec<String> = manager
                .tools()
                .iter()
                .map(|t| t.name().to_string())
                .collect();
            manager.register_all(agent.registry());
            let _ = tx.send(crate::ui::types::UiEvent::McpReloaded {
                success: true,
                tools: tool_names,
                message: format!(
                    "MCP 配置已重新加载: {} 个服务器, {} 个工具",
                    manager.list_servers().len(),
                    manager.tools().len()
                ),
            });
        });
    }

    /// Persist the active session to disk.
    pub(crate) fn save_current_session(&mut self) {
        if let Some(session) = self.session_store.active_session_mut() {
            let now = crate::session::now_millis();
            match crate::session::save_session_internal(session) {
                Ok(()) => {
                    session.last_saved_at = now;
                }
                Err(e) => {
                    tracing::warn!("Failed to save session {}: {}", session.id, e);
                    crate::handlers::system::push_toast(
                        &mut self.ui_store,
                        format!("Failed to save session: {}", e),
                        crate::ui::types::ToastLevel::Error,
                    );
                }
            }
        }
    }
}
