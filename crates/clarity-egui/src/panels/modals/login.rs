//! Kimi Code OAuth Device Flow login modal.

use crate::App;
use crate::stores::KimiCodeLoginState;
use crate::ui::types::UiEvent;
use clarity_ui::design_system::{Space, TextStyle, code_frame, gap, spinner, text};
use clarity_ui::widgets::button::Button;
use clarity_ui::widgets::modal::Modal;

/// Renders the oauth login modal UI using the Clarity Design Protocol.
///
/// The modal shell itself (scrim + frame + centering) is owned by
/// `clarity_ui::widgets::modal`; this function only renders the content.
pub fn render_oauth_login_modal(
    app: &mut App,
    ctx: &egui::Context,
    config: &clarity_llm::auth::OAuthDeviceFlowConfig,
) {
    if app.current_modal() != Some(&clarity_core::ui::ModalType::KimiCodeLogin) {
        return;
    }

    let mut close_requested = false;

    Modal::new("kimi_code_login")
        .width(420.0)
        .max_height(600.0)
        .show(ctx, |ui| {
            let state = app.settings_store().kimi_code_login_state.clone();
            match state {
                KimiCodeLoginState::Idle => {
                    ui.vertical_centered(|ui| {
                        gap(ui, Space::S2);
                        text(ui, "Connect to Kimi Code", TextStyle::Title);
                        gap(ui, Space::S1);
                        text(
                            ui,
                            "Click below to start OAuth Device Flow.\nA browser window will open for authorization.",
                            TextStyle::Body,
                        );
                        gap(ui, Space::S3);
                        if ui
                            .add(Button::new("Start Login").primary().width(80.0))
                            .clicked()
                        {
                            start_login(app, config.clone());
                        }
                    });
                }
                KimiCodeLoginState::Requesting => {
                    ui.vertical_centered(|ui| {
                        gap(ui, Space::S5);
                        text(ui, "Requesting device code...", TextStyle::Body);
                        gap(ui, Space::S2);
                        spinner(ui);
                    });
                }
                KimiCodeLoginState::Waiting {
                    user_code,
                    verification_uri_complete,
                    ..
                } => {
                    ui.vertical_centered(|ui| {
                        gap(ui, Space::S1);
                        text(ui, "Authorize this device", TextStyle::Title);
                        gap(ui, Space::S2);
                        text(ui, "Enter this code in your browser:", TextStyle::Body);
                        gap(ui, Space::S0);
                        code_frame(ui, |ui| {
                            text(ui, &user_code, TextStyle::Mono);
                        });
                        gap(ui, Space::S2);
                        let url = verification_uri_complete.clone();
                        // ponytail: hyperlink_to is not yet wrapped in clarity-ui.
                        if ui.hyperlink_to("Open verification page →", &url).clicked() {
                            open_browser(&url);
                        }
                        gap(ui, Space::S2);
                        text(ui, "Waiting for authorization...", TextStyle::Small);
                        spinner(ui);
                    });
                }
                KimiCodeLoginState::Polling => {
                    ui.vertical_centered(|ui| {
                        gap(ui, Space::S5);
                        text(ui, "Completing login...", TextStyle::Body);
                        gap(ui, Space::S2);
                        spinner(ui);
                    });
                }
                KimiCodeLoginState::Success => {
                    ui.vertical_centered(|ui| {
                        gap(ui, Space::S5);
                        text(ui, "Login successful", TextStyle::Title);
                        gap(ui, Space::S1);
                        text(ui, "You can now use Kimi Code.", TextStyle::Body);
                        gap(ui, Space::S3);
                        if ui.add(Button::new("Close").primary().width(80.0)).clicked() {
                            close_requested = true;
                        }
                    });
                }
                KimiCodeLoginState::Error(e) => {
                    ui.vertical_centered(|ui| {
                        gap(ui, Space::S5);
                        text(ui, "Login failed", TextStyle::Title);
                        gap(ui, Space::S1);
                        text(ui, &e, TextStyle::Body);
                        gap(ui, Space::S3);
                        if ui.add(Button::new("Retry").primary().width(80.0)).clicked() {
                            start_login(app, config.clone());
                        }
                    });
                }
            }
        });

    if close_requested {
        app.close_modal();
        app.settings_store_mut().kimi_code_login_state = KimiCodeLoginState::Idle;
    }
}

fn start_login(app: &mut App, config: clarity_llm::auth::OAuthDeviceFlowConfig) {
    app.settings_store_mut().kimi_code_login_state = KimiCodeLoginState::Requesting;
    let tx = app.context.ui_tx.clone();
    let runtime = app.context.runtime.handle().clone();

    runtime.spawn(async move {
        let client = clarity_llm::auth::OAuthDeviceFlowClient::with_config(config.clone());

        let auth = match client.request_device_authorization().await {
            Ok(a) => a,
            Err(e) => {
                let _ = tx.send(UiEvent::KimiCodeLoginStateUpdate {
                    state: "error".into(),
                    user_code: None,
                    url: None,
                    error: Some(format!("Failed to request authorization: {}", e)),
                });
                let _ = tx.send(UiEvent::KimiCodeLoginResult {
                    success: false,
                    message: format!("Failed to request authorization: {}", e),
                    provider_id: "kimi_code".to_string(),
                });
                return;
            }
        };

        let user_code = auth.user_code.clone();
        let verification_uri_complete = auth.verification_uri_complete.clone();

        // Notify UI to switch to Waiting state
        let _ = tx.send(UiEvent::KimiCodeLoginStateUpdate {
            state: "waiting".into(),
            user_code: Some(user_code.clone()),
            url: Some(verification_uri_complete.clone()),
            error: None,
        });

        // Open browser automatically
        open_browser(&verification_uri_complete);

        let interval = auth.interval.max(1);
        let max_attempts = auth.expires_in.map(|e| e / interval).unwrap_or(300);

        for _attempt in 1..=max_attempts {
            tokio::time::sleep(tokio::time::Duration::from_secs(interval)).await;

            // Notify UI that we're now polling
            let _ = tx.send(UiEvent::KimiCodeLoginStateUpdate {
                state: "polling".into(),
                user_code: Some(user_code.clone()),
                url: Some(verification_uri_complete.clone()),
                error: None,
            });

            match client.poll_device_token(&auth).await {
                Ok(token) => {
                    let store = clarity_llm::auth::TokenStore::default_kimi_code();
                    if let Err(e) = store.save(&token) {
                        let _ = tx.send(UiEvent::KimiCodeLoginStateUpdate {
                            state: "error".into(),
                            user_code: None,
                            url: None,
                            error: Some(format!("Failed to save token: {}", e)),
                        });
                        let _ = tx.send(UiEvent::KimiCodeLoginResult {
                            success: false,
                            message: format!("Failed to save token: {}", e),
                            provider_id: "kimi_code".to_string(),
                        });
                        return;
                    }
                    let _ = tx.send(UiEvent::KimiCodeLoginStateUpdate {
                        state: "success".into(),
                        user_code: None,
                        url: None,
                        error: None,
                    });
                    let _ = tx.send(UiEvent::KimiCodeLoginResult {
                        success: true,
                        message: "Kimi Code login successful.".into(),
                        provider_id: "kimi_code".to_string(),
                    });
                    return;
                }
                Err(clarity_llm::auth::AuthError::Expired) => {
                    let _ = tx.send(UiEvent::KimiCodeLoginStateUpdate {
                        state: "error".into(),
                        user_code: None,
                        url: None,
                        error: Some("Device authorization expired.".into()),
                    });
                    let _ = tx.send(UiEvent::KimiCodeLoginResult {
                        success: false,
                        message: "Device authorization expired. Please try again.".into(),
                        provider_id: "kimi_code".to_string(),
                    });
                    return;
                }
                Err(clarity_llm::auth::AuthError::Request(ref msg))
                    if msg.contains("authorization_pending") =>
                {
                    // Still waiting — send a heartbeat so UI knows we're alive
                    let _ = tx.send(UiEvent::KimiCodeLoginStateUpdate {
                        state: "waiting".into(),
                        user_code: Some(user_code.clone()),
                        url: Some(verification_uri_complete.clone()),
                        error: None,
                    });
                    continue;
                }
                Err(e) => {
                    let _ = tx.send(UiEvent::KimiCodeLoginStateUpdate {
                        state: "error".into(),
                        user_code: None,
                        url: None,
                        error: Some(format!("{}", e)),
                    });
                    let _ = tx.send(UiEvent::KimiCodeLoginResult {
                        success: false,
                        message: format!("Login failed: {}", e),
                        provider_id: "kimi_code".to_string(),
                    });
                    return;
                }
            }
        }

        let _ = tx.send(UiEvent::KimiCodeLoginStateUpdate {
            state: "error".into(),
            user_code: None,
            url: None,
            error: Some("Device authorization timed out.".into()),
        });
        let _ = tx.send(UiEvent::KimiCodeLoginResult {
            success: false,
            message: "Device authorization timed out. Please try again.".into(),
            provider_id: "kimi_code".to_string(),
        });
    });
}

#[cfg(target_os = "windows")]
fn open_browser(url: &str) {
    let _ = std::process::Command::new("cmd")
        .args(["/C", "start", "", url])
        .spawn();
}

#[cfg(target_os = "macos")]
fn open_browser(url: &str) {
    let _ = std::process::Command::new("open").arg(url).spawn();
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn open_browser(url: &str) {
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}
