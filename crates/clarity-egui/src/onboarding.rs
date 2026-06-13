//! First-run onboarding overlay for egui.
//!
//! Detects unconfigured state on startup and guides the user to:
//! 1. Enter a cloud API key, 2. Download a local GGUF model, or 3. Skip for now.
//!
//! IS-1 Sprint 31: auto-trigger download on first launch, auto-configure on complete,
//! true cancellation via CancellationToken.

use crate::App;
use crate::settings::GuiSettings;

/// State machine for the onboarding flow.
#[derive(Debug, Clone)]
pub enum OnboardingState {
    /// Not first-run or already completed.
    Hidden,
    /// Show the provider-selection screen.
    ChooseProvider,
    /// Downloading a pre-configured model.
    Downloading {
        bytes_downloaded: u64,
        total_bytes: Option<u64>,
    },
    /// Download finished successfully.
    DownloadComplete { model_path: std::path::PathBuf },
    /// Download failed.
    DownloadFailed(String),
}

/// Detect whether onboarding should be shown.
///
/// Returns `false` if any of the following is true:
/// - A GUI settings file already exists
/// - A cloud API key is set via environment variable
/// - A local `.gguf` model already exists in the default model directory
pub fn should_show_onboarding() -> bool {
    if GuiSettings::config_path().exists() {
        return false;
    }
    // Skip if a local model is already present (e.g. user manually downloaded)
    let model_dir = clarity_core::model_download::default_model_dir();
    if let Ok(entries) = std::fs::read_dir(&model_dir) {
        for entry in entries.flatten() {
            if let Some(ext) = entry.path().extension() {
                if ext == "gguf" {
                    return false;
                }
            }
        }
    }
    if std::env::var("KIMI_API_KEY").is_ok()
        || std::env::var("OPENAI_API_KEY").is_ok()
        || std::env::var("ANTHROPIC_AUTH_TOKEN").is_ok()
        || std::env::var("DEEPSEEK_API_KEY").is_ok()
    {
        return false;
    }
    true
}

/// Render the onboarding overlay (full-screen modal).
pub fn render_onboarding(app: &mut App, ctx: &egui::Context) {
    let state = app.onboarding_store.onboarding_state.clone();
    match state {
        OnboardingState::Hidden => (),
        OnboardingState::ChooseProvider => {
            // Sprint 31: auto-trigger download on first encounter
            if !app.onboarding_store.downloading_auto {
                app.onboarding_store.downloading_auto = true;
                start_model_download(app);
                app.onboarding_store.onboarding_state = OnboardingState::Downloading {
                    bytes_downloaded: 0,
                    total_bytes: None,
                };
            }
            render_choose_provider(app, ctx);
        }
        OnboardingState::Downloading {
            bytes_downloaded,
            total_bytes,
        } => render_downloading(app, ctx, bytes_downloaded, total_bytes),
        OnboardingState::DownloadComplete { model_path } => {
            // Sprint 31: auto-configure and hide without user click
            auto_configure_and_hide(app, &model_path);
        }
        OnboardingState::DownloadFailed(ref err) => render_download_failed(app, ctx, err),
    }
}

fn render_choose_provider(app: &mut App, ctx: &egui::Context) {
    let screen = ctx.screen_rect();

    // Dim background
    ctx.layer_painter(egui::LayerId::new(
        egui::Order::Background,
        egui::Id::new("onboarding_bg"),
    ))
    .rect_filled(screen, 0.0, egui::Color32::from_black_alpha(180));

    egui::Window::new("Welcome to Clarity")
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .frame(egui::Frame::window(&ctx.style()).fill(app.ui_store.theme.bg_elevated))
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.heading("Welcome to Clarity");
                ui.add_space(app.ui_store.theme.space_8);
                ui.label(
                    egui::RichText::new("Local-first AI agent runtime")
                        .color(app.ui_store.theme.text_dim)
                        .size(app.ui_store.theme.text_base),
                );
                ui.add_space(app.ui_store.theme.space_24);
                ui.label("Get started by choosing how you'd like to run Clarity:");
                ui.add_space(app.ui_store.theme.space_16);

                if ui
                    .add_sized(
                        [280.0, 36.0],
                        egui::Button::new("Enter API Key (Cloud Provider)"),
                    )
                    .clicked()
                {
                    app.view_state.main = clarity_core::ui::AppView::Settings;
                    app.onboarding_store.onboarding_state = OnboardingState::Hidden;
                }

                ui.add_space(app.ui_store.theme.space_8);

                if ui
                    .add_sized(
                        [280.0, 36.0],
                        egui::Button::new("Download Local Model (~1 GB)"),
                    )
                    .clicked()
                {
                    start_model_download(app);
                }

                ui.add_space(app.ui_store.theme.space_8);

                if ui
                    .add_sized([280.0, 36.0], egui::Button::new("Skip for Now"))
                    .clicked()
                {
                    app.onboarding_store.onboarding_state = OnboardingState::Hidden;
                }

                ui.add_space(app.ui_store.theme.space_16);
                ui.label(
                    egui::RichText::new(
                        "Note: Local models use the Qwen2 architecture. \
                         Download other architectures manually via Settings.",
                    )
                    .color(app.ui_store.theme.text_dim)
                    .size(app.ui_store.theme.text_sm),
                );
            });
        });
}

fn render_downloading(
    app: &mut App,
    ctx: &egui::Context,
    bytes_downloaded: u64,
    total_bytes: Option<u64>,
) {
    let screen = ctx.screen_rect();
    ctx.layer_painter(egui::LayerId::new(
        egui::Order::Background,
        egui::Id::new("onboarding_bg"),
    ))
    .rect_filled(screen, 0.0, egui::Color32::from_black_alpha(180));

    // Poll for progress updates from the download task
    let mut channel_disconnected = false;
    if let Some(ref mut rx) = app.onboarding_store.onboarding_progress_rx {
        let rx: &mut std::sync::mpsc::Receiver<
            clarity_core::model_download::ModelDownloadProgress,
        > = rx;
        loop {
            use clarity_core::model_download::ModelDownloadProgress;
            match rx.try_recv() {
                Ok(ModelDownloadProgress::Started) => {
                    // Download task has started; keep current UI state.
                }
                Ok(ModelDownloadProgress::Progress {
                    bytes_downloaded,
                    total_bytes,
                }) => {
                    app.onboarding_store.onboarding_state = OnboardingState::Downloading {
                        bytes_downloaded,
                        total_bytes,
                    };
                }
                Ok(ModelDownloadProgress::Complete) => {
                    let dest = clarity_core::model_download::default_model_dir()
                        .join(clarity_core::model_download::PRECONFIGURED_MODELS[0].filename);
                    app.onboarding_store.onboarding_state =
                        OnboardingState::DownloadComplete { model_path: dest };
                    break;
                }
                Ok(ModelDownloadProgress::Cancelled) => {
                    app.onboarding_store.onboarding_state = OnboardingState::Hidden;
                    break;
                }
                Ok(ModelDownloadProgress::Failed(err)) => {
                    app.onboarding_store.onboarding_state = OnboardingState::DownloadFailed(err);
                    break;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    channel_disconnected = true;
                    break;
                }
            }
        }
    }

    // Fallback: if channel disconnected without explicit Complete/Failed, treat as interrupted.
    if channel_disconnected
        && matches!(
            app.onboarding_store.onboarding_state,
            OnboardingState::Downloading { .. }
        )
    {
        app.onboarding_store.onboarding_state =
            OnboardingState::DownloadFailed("Download interrupted".into());
    }

    egui::Window::new("Downloading Model")
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .frame(egui::Frame::window(&ctx.style()).fill(app.ui_store.theme.bg_elevated))
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.heading("Preparing Local Model");
                ui.add_space(app.ui_store.theme.space_8);

                let (fraction, label) = if let Some(total) = total_bytes {
                    let pct = if total > 0 {
                        bytes_downloaded as f32 / total as f32
                    } else {
                        0.0
                    };
                    let mb = bytes_downloaded as f32 / 1_048_576.0;
                    let total_mb = total as f32 / 1_048_576.0;
                    (
                        pct,
                        format!("{:.1} / {:.1} MB ({:.0}%)", mb, total_mb, pct * 100.0),
                    )
                } else {
                    let mb = bytes_downloaded as f32 / 1_048_576.0;
                    (0.0, format!("{:.1} MB downloaded (unknown total)", mb))
                };

                ui.label(&label);
                ui.add_space(app.ui_store.theme.space_8);
                ui.add(
                    egui::ProgressBar::new(fraction.min(1.0))
                        .show_percentage()
                        .desired_width(280.0),
                );
                ui.add_space(app.ui_store.theme.space_16);

                if ui
                    .add_sized([140.0, 28.0], egui::Button::new("Cancel and Skip"))
                    .clicked()
                {
                    // Sprint 31: true cancellation
                    if let Some(ref token) = app.onboarding_store.cancel_token {
                        token.cancel();
                    }
                    app.onboarding_store.cancel_token = None;
                    app.onboarding_store.onboarding_progress_rx = None;
                    app.onboarding_store.onboarding_state = OnboardingState::Hidden;
                }
            });
        });
}

fn render_download_failed(app: &mut App, ctx: &egui::Context, err: &str) {
    let screen = ctx.screen_rect();
    ctx.layer_painter(egui::LayerId::new(
        egui::Order::Background,
        egui::Id::new("onboarding_bg"),
    ))
    .rect_filled(screen, 0.0, egui::Color32::from_black_alpha(180));

    egui::Window::new("Download Failed")
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .frame(egui::Frame::window(&ctx.style()).fill(app.ui_store.theme.bg_elevated))
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.heading("Download Failed");
                ui.add_space(app.ui_store.theme.space_8);
                ui.label(egui::RichText::new(err).color(egui::Color32::LIGHT_RED));
                ui.add_space(app.ui_store.theme.space_16);

                if ui
                    .add_sized([140.0, 28.0], egui::Button::new("Try Again"))
                    .clicked()
                {
                    start_model_download(app);
                }
                ui.add_space(app.ui_store.theme.space_8);
                if ui
                    .add_sized([140.0, 28.0], egui::Button::new("Enter API Key Instead"))
                    .clicked()
                {
                    app.view_state.main = clarity_core::ui::AppView::Settings;
                    app.onboarding_store.onboarding_state = OnboardingState::Hidden;
                }
                ui.add_space(app.ui_store.theme.space_8);
                if ui
                    .add_sized([140.0, 28.0], egui::Button::new("Skip"))
                    .clicked()
                {
                    app.onboarding_store.onboarding_state = OnboardingState::Hidden;
                }
            });
        });
}

fn auto_configure_and_hide(app: &mut App, model_path: &std::path::Path) {
    // Auto-configure settings to local provider
    app.settings_store.settings_edit.provider = "local".to_string();
    app.settings_store.settings_edit.model = model_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "local".to_string());
    app.settings_store.settings_edit.local_model_path = Some(model_path.display().to_string());

    // S3.2: use centralized commit + reload helpers instead of inline mirror.
    if let Err(e) = app.commit_settings() {
        tracing::error!("Onboarding: failed to save settings: {}", e);
        return;
    }
    app.trigger_llm_reload();

    app.onboarding_store.onboarding_state = crate::onboarding::OnboardingState::Hidden;
    app.onboarding_store.cancel_token = None;
}

fn start_model_download(app: &mut App) {
    use clarity_core::model_download::{
        ModelDownloadProgress, PRECONFIGURED_MODELS, default_model_dir, download_model_files,
    };
    use tokio_util::sync::CancellationToken;

    let model = &PRECONFIGURED_MODELS[0];
    let dest = default_model_dir();

    let cancel_token = CancellationToken::new();
    app.onboarding_store.cancel_token = Some(cancel_token.clone());

    let (tx, rx) = std::sync::mpsc::channel::<ModelDownloadProgress>();
    app.onboarding_store.onboarding_progress_rx = Some(rx);
    app.onboarding_store.onboarding_state = OnboardingState::Downloading {
        bytes_downloaded: 0,
        total_bytes: None,
    };

    let model_clone = *model;
    let handle = app.runtime.handle().clone();
    handle.clone().spawn(async move {
        let handle2 = handle.clone();
        // Bridge tokio mpsc -> std mpsc because App uses std::sync::mpsc for UI events
        let (tokio_tx, mut tokio_rx) = tokio::sync::mpsc::channel::<ModelDownloadProgress>(16);
        let download_handle = handle2.spawn(async move {
            download_model_files(&model_clone, dest, tokio_tx, cancel_token).await
        });

        // Forward progress from tokio channel to std channel
        let forward_handle = handle2.spawn(async move {
            while let Some(progress) = tokio_rx.recv().await {
                if tx.send(progress).is_err() {
                    break;
                }
            }
        });

        let result: Result<Result<std::path::PathBuf, String>, tokio::task::JoinError> =
            download_handle.await;
        let _ = forward_handle.await;

        if let Ok(Ok(path)) = result {
            tracing::info!("Model download complete: {}", path.display());
        } else if let Ok(Err(e)) = result {
            tracing::error!("Model download failed: {}", e);
        }
    });
}
