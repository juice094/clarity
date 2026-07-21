//! Clarity sub-applications hosted inside the egui binary.
//!
//! Dashboard and Settings now live in `clarity-apps`. Chat has moved; the egui
//! render body remains in `chat.rs` as a `clarity_shell::ChatRenderer`
//! implementation.

pub mod chat;

#[allow(unused_imports)]
pub use clarity_apps::SettingsApp;

#[cfg(test)]
pub(crate) fn test_app(ctx: &egui::Context) -> crate::App {
    use std::collections::HashMap;
    use std::sync::{Arc, mpsc::channel};
    use std::time::Instant;

    crate::theme::setup_fonts(ctx);

    let (ui_tx, ui_rx) = channel::<crate::ui::types::UiEvent>();
    let now = Instant::now();

    let context = crate::app_context::AppContext {
        state: Arc::new(crate::app_state::AppState::default()),
        runtime: tokio::runtime::Runtime::new().expect("tokio runtime"),
        ui_tx,
        session_store: crate::stores::SessionStore {
            sessions: Vec::new(),
            active_session_id: String::new(),
            drafts: HashMap::new(),
            turn_cache: HashMap::new(),
        },
        ui_store: crate::stores::UiStore::default(),
        mcp_store: crate::stores::McpStore {
            mcp_config: None,
            mcp_changed: false,
            connected_tools: Vec::new(),
            last_mcp_poll: now,
            last_mcp_mtime: None,
        },
        onboarding_store: crate::stores::OnboardingStore {
            onboarding_state: crate::onboarding::OnboardingState::Hidden,
            onboarding_progress_rx: None,
            downloading_auto: false,
            cancel_token: None,
        },
        project_store: crate::stores::ProjectStore::default(),
        snapshot_store: crate::stores::SnapshotStore::default(),
        knowledge_store: {
            let mut store = crate::stores::KnowledgeStore::default();
            store.set_field(Arc::new(clarity_knowledge::KnowledgeField::new(
                clarity_knowledge::FieldConfig::default(),
            )));
            store
        },
        console_store: crate::stores::ConsoleStore::default(),
        files_store: crate::stores::FilesStore::default(),
        share_store: crate::stores::ShareStore::default(),
        template_store: crate::stores::TemplateStore::default(),
        gateway_manager: None,
        skill_watcher: None,
        tray_manager: None,
        device_state: crate::claw::DeviceState::default(),
        claw_ws: None,
        claw_ws_device_id: String::new(),
        claw_device_identity: None,
        claw_device_token: None,
        claw_pairing_client: None,
        claw_pairing_state: clarity_shell::PairingState::default(),
    };

    crate::App {
        context,
        ui_rx,
        view_state: clarity_core::ui::ViewState::default(),
        main_router: clarity_core::ui::Router::new(clarity_core::ui::AppView::Chat),
        modal_router: clarity_core::ui::Router::empty(),
        right_rail_router: clarity_core::ui::Router::empty(),
        shortcuts_help_open: false,
        command_palette: crate::widgets::command_palette::CommandPalette::new(),
        pretext_metrics: crate::pretext::EguiFontMetrics::new(ctx.clone()),
        panel_animation: crate::animation::PanelAnimationState::default(),
        main_stage_transition: None,
        prev_main_view: clarity_core::ui::AppView::Chat,
        apps: [
            clarity_apps::ClarityAppEnum::Chat(clarity_apps::ChatApp::new()),
            clarity_apps::ClarityAppEnum::Settings(clarity_apps::SettingsApp {
                store: clarity_apps::SettingsStore {
                    settings_edit: crate::settings::GuiSettings::default(),
                    settings_vm: clarity_core::view_models::settings::SettingsViewModel::default(),
                    settings_active_tab: 0,
                    show_add_provider: false,
                    add_provider_name: String::new(),
                    add_provider_url: String::new(),
                    add_provider_key: String::new(),
                    add_provider_format: String::new(),
                    provider_registry: crate::provider::ProviderRegistry::default(),
                    testing_provider: None,
                    kimi_code_login_state: clarity_apps::KimiCodeLoginState::Idle,
                    claw_editing_index: None,
                    claw_form: crate::settings::OpenClawConnection::default(),
                },
            }),
            clarity_apps::ClarityAppEnum::Dashboard(clarity_apps::DashboardApp {
                task_store: clarity_apps::dashboard::TaskStore {
                    tasks: Vec::new(),
                    last_task_refresh: now,
                    task_create_name: String::new(),
                    task_create_desc: String::new(),
                    task_create_prompt: String::new(),
                    task_create_priority: 0,
                    viewing_task_id: None,
                    viewing_task_result: None,
                },
                cron_store: clarity_apps::dashboard::CronStore {
                    tasks: Vec::new(),
                    last_refresh: now,
                    create_name: String::new(),
                    create_desc: String::new(),
                    create_prompt: String::new(),
                    create_expr: String::new(),
                    create_priority: 0,
                },
                team_store: clarity_apps::dashboard::TeamStore {
                    teams: Vec::new(),
                    create_name: String::new(),
                    create_goal: String::new(),
                    create_members: Vec::new(),
                    create_max_concurrency: 1,
                    create_timeout_secs: 60,
                },
                subagent_store: clarity_apps::dashboard::SubAgentStore::default(),
            }),
        ],
        chrome: None,
        tray_quit_requested: false,
        last_tray_status: None,
        last_frame_width: None,
    }
}
