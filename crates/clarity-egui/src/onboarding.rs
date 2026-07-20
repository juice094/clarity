//! First-run onboarding overlay for egui.
//!
//! Detects unconfigured state on startup and guides the user to:
//! 1. Enter a cloud API key, 2. Download a local GGUF model, or 3. Skip for now.
//!
//! IS-1 Sprint 31: auto-trigger download on first launch, auto-configure on complete,
//! true cancellation via CancellationToken.

use crate::App;
use crate::settings::GuiSettings;
use clarity_ui::design_system::{Space, TextStyle, gap, text};
use clarity_ui::widgets::button::Button;
use clarity_ui::widgets::modal::{Modal, modal_scrim};

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
    let state = app.context.onboarding_store.onboarding_state.clone();
    match state {
        OnboardingState::Hidden => (),
        OnboardingState::ChooseProvider => {
            // Sprint 31: auto-trigger download on first encounter
            if !app.context.onboarding_store.downloading_auto {
                app.context.onboarding_store.downloading_auto = true;
                start_model_download(app);
                app.context.onboarding_store.onboarding_state = OnboardingState::Downloading {
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
    modal_scrim(ctx);
    let mut close_requested = false;

    Modal::new("onboarding_welcome")
        .width(420.0)
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                text(ui, "Welcome to Clarity", TextStyle::Heading);
                gap(ui, Space::S1);
                text(ui, "Local-first AI agent runtime", TextStyle::Small);
                gap(ui, Space::S5);
                text(
                    ui,
                    "Get started by choosing how you'd like to run Clarity:",
                    TextStyle::Body,
                );
                gap(ui, Space::S3);

                if ui
                    .add_sized([280.0, 36.0], Button::new("Enter API Key (Cloud Provider)"))
                    .clicked()
                {
                    app.navigate(clarity_core::ui::AppView::Settings.into());
                    close_requested = true;
                }

                gap(ui, Space::S1);

                if ui
                    .add_sized(
                        [280.0, 36.0],
                        Button::new("Download Local Model (~1 GB)").primary(),
                    )
                    .clicked()
                {
                    start_model_download(app);
                }

                gap(ui, Space::S1);

                if ui
                    .add_sized([280.0, 36.0], Button::new("Skip for Now").ghost())
                    .clicked()
                {
                    close_requested = true;
                }

                gap(ui, Space::S3);
                text(
                    ui,
                    "Note: Local models use the Qwen2 architecture. \
                     Download other architectures manually via Settings.",
                    TextStyle::Small,
                );
            });
        });

    if close_requested {
        app.context.onboarding_store.onboarding_state = OnboardingState::Hidden;
    }
}

fn render_downloading(
    app: &mut App,
    ctx: &egui::Context,
    bytes_downloaded: u64,
    total_bytes: Option<u64>,
) {
    // Poll for progress updates from the download task
    let mut channel_disconnected = false;
    if let Some(ref mut rx) = app.context.onboarding_store.onboarding_progress_rx {
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
                    app.context.onboarding_store.onboarding_state = OnboardingState::Downloading {
                        bytes_downloaded,
                        total_bytes,
                    };
                }
                Ok(ModelDownloadProgress::Complete) => {
                    let dest = clarity_core::model_download::default_model_dir()
                        .join(clarity_core::model_download::PRECONFIGURED_MODELS[0].filename);
                    app.context.onboarding_store.onboarding_state =
                        OnboardingState::DownloadComplete { model_path: dest };
                    break;
                }
                Ok(ModelDownloadProgress::Cancelled) => {
                    app.context.onboarding_store.onboarding_state = OnboardingState::Hidden;
                    break;
                }
                Ok(ModelDownloadProgress::Failed(err)) => {
                    app.context.onboarding_store.onboarding_state =
                        OnboardingState::DownloadFailed(err);
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
            app.context.onboarding_store.onboarding_state,
            OnboardingState::Downloading { .. }
        )
    {
        app.context.onboarding_store.onboarding_state =
            OnboardingState::DownloadFailed("Download interrupted".into());
    }

    modal_scrim(ctx);
    let mut cancel = false;

    Modal::new("onboarding_downloading")
        .width(420.0)
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                text(ui, "Preparing Local Model", TextStyle::Heading);
                gap(ui, Space::S1);

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

                text(ui, label.as_str(), TextStyle::Body);
                gap(ui, Space::S1);
                // ponytail: ProgressBar is not yet wrapped in clarity-ui.
                ui.add(
                    egui::ProgressBar::new(fraction.min(1.0))
                        .show_percentage()
                        .desired_width(280.0),
                );
                gap(ui, Space::S3);

                if ui
                    .add_sized([140.0, 28.0], Button::new("Cancel and Skip").ghost())
                    .clicked()
                {
                    cancel = true;
                }
            });
        });

    if cancel {
        // Sprint 31: true cancellation
        if let Some(ref token) = app.context.onboarding_store.cancel_token {
            token.cancel();
        }
        app.context.onboarding_store.cancel_token = None;
        app.context.onboarding_store.onboarding_progress_rx = None;
        app.context.onboarding_store.onboarding_state = OnboardingState::Hidden;
    }
}

fn render_download_failed(app: &mut App, ctx: &egui::Context, err: &str) {
    modal_scrim(ctx);
    let mut try_again = false;
    let mut use_api_key = false;
    let mut skip = false;

    Modal::new("onboarding_download_failed")
        .width(420.0)
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                text(ui, "Download Failed", TextStyle::Heading);
                gap(ui, Space::S1);
                clarity_ui::design_system::text_with_color(
                    ui,
                    err,
                    clarity_ui::design_system::TextStyle::Body,
                    app.context.ui_store.theme.danger,
                );
                gap(ui, Space::S3);

                if ui
                    .add_sized([140.0, 28.0], Button::new("Try Again").primary())
                    .clicked()
                {
                    try_again = true;
                }
                gap(ui, Space::S1);
                if ui
                    .add_sized([140.0, 28.0], Button::new("Enter API Key Instead"))
                    .clicked()
                {
                    use_api_key = true;
                }
                gap(ui, Space::S1);
                if ui
                    .add_sized([140.0, 28.0], Button::new("Skip").ghost())
                    .clicked()
                {
                    skip = true;
                }
            });
        });

    if try_again {
        start_model_download(app);
    } else if use_api_key {
        app.navigate(clarity_core::ui::AppView::Settings.into());
        app.context.onboarding_store.onboarding_state = OnboardingState::Hidden;
    } else if skip {
        app.context.onboarding_store.onboarding_state = OnboardingState::Hidden;
    }
}

fn auto_configure_and_hide(app: &mut App, model_path: &std::path::Path) {
    // Auto-configure settings to local provider
    let settings_store = app.settings_store_mut();
    settings_store.settings_edit.provider = "local".to_string();
    settings_store.settings_edit.model = model_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "local".to_string());
    settings_store.settings_edit.local_model_path = Some(model_path.display().to_string());

    // S3.2: use centralized commit + reload helpers instead of inline mirror.
    if let Err(e) = app.commit_settings() {
        tracing::error!("Onboarding: failed to save settings: {}", e);
        return;
    }
    app.trigger_llm_reload();

    app.context.onboarding_store.onboarding_state = crate::onboarding::OnboardingState::Hidden;
    app.context.onboarding_store.cancel_token = None;
}

fn start_model_download(app: &mut App) {
    use clarity_core::model_download::{
        ModelDownloadProgress, PRECONFIGURED_MODELS, default_model_dir, download_model_files,
    };
    use tokio_util::sync::CancellationToken;

    let model = &PRECONFIGURED_MODELS[0];
    let dest = default_model_dir();

    let cancel_token = CancellationToken::new();
    app.context.onboarding_store.cancel_token = Some(cancel_token.clone());

    let (tx, rx) = std::sync::mpsc::channel::<ModelDownloadProgress>();
    app.context.onboarding_store.onboarding_progress_rx = Some(rx);
    app.context.onboarding_store.onboarding_state = OnboardingState::Downloading {
        bytes_downloaded: 0,
        total_bytes: None,
    };

    let model_clone = *model;
    let handle = app.context.runtime.handle().clone();
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
