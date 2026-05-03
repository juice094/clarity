//! Kimi Code OAuth Device Flow login modal.

use crate::stores::KimiCodeLoginState;
use crate::ui::types::UiEvent;
use crate::App;

pub fn render_oauth_login_modal(
    app: &mut App,
    ctx: &egui::Context,
    config: &clarity_core::auth::OAuthDeviceFlowConfig,
) {
    if !app.settings_store.kimi_code_login_open {
        return;
    }

    let screen = ctx.screen_rect();
    let scrim = egui::Color32::from_rgba_premultiplied(0, 0, 0, 180);
    ctx.layer_painter(egui::LayerId::background()).rect_filled(
        screen,
        egui::CornerRadius::same(0),
        scrim,
    );

    let mut close_requested = false;
    egui::Area::new("kimi_login_scrim".into())
        .interactable(true)
        .order(egui::Order::Background)
        .show(ctx, |ui| {
            ui.set_min_size(screen.size());
            if ui.allocate_response(screen.size(), egui::Sense::click()).clicked()
                || ctx.input(|i| i.key_pressed(egui::Key::Escape))
            {
                close_requested = true;
            }
        });

    let theme = app.ui_store.theme.clone();

    egui::Window::new("Kimi Code Login")
        .collapsible(false)
        .resizable(false)
        .fixed_size(egui::vec2(440.0, 320.0))
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .frame(
            egui::Frame::window(&ctx.style())
                .fill(theme.bg)
                .corner_radius(egui::CornerRadius::same(theme.radius_lg as u8))
                .inner_margin(egui::Margin::same(16)),
        )
        .show(ctx, |ui| {
            let state = app.settings_store.kimi_code_login_state.clone();
            match state {
                KimiCodeLoginState::Idle => {
                    ui.vertical_centered(|ui| {
                        ui.add_space(theme.space_12);
                        ui.label(
                            egui::RichText::new("Connect to Kimi Code")
                                .size(theme.text_lg)
                                .strong()
                                .color(theme.text),
                        );
                        ui.add_space(theme.space_8);
                        ui.label(
                            egui::RichText::new(
                                "Click below to start OAuth Device Flow.\nA browser window will open for authorization.",
                            )
                            .size(theme.text_sm)
                            .color(theme.text_dim),
                        );
                        ui.add_space(theme.space_16);
                        if ui.add(theme.primary_button("Start Login")).clicked() {
                            start_login(app, config.clone());
                        }
                    });
                }
                KimiCodeLoginState::Requesting => {
                    ui.vertical_centered(|ui| {
                        ui.add_space(theme.space_24);
                        ui.label(
                            egui::RichText::new("Requesting device code...")
                                .size(theme.text_base)
                                .color(theme.text),
                        );
                        ui.add_space(theme.space_12);
                        ui.spinner();
                    });
                }
                KimiCodeLoginState::Waiting {
                    user_code,
                    verification_uri_complete,
                    ..
                } => {
                    ui.vertical_centered(|ui| {
                        ui.add_space(theme.space_8);
                        ui.label(
                            egui::RichText::new("Authorize this device")
                                .size(theme.text_lg)
                                .strong()
                                .color(theme.text),
                        );
                        ui.add_space(theme.space_12);
                        ui.label(
                            egui::RichText::new("Enter this code in your browser:")
                                .size(theme.text_sm)
                                .color(theme.text_dim),
                        );
                        ui.add_space(theme.space_4);
                        egui::Frame::new()
                            .fill(theme.bg_accent)
                            .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
                            .inner_margin(egui::Margin::symmetric(16, 10))
                            .show(ui, |ui| {
                                ui.label(
                                    egui::RichText::new(user_code)
                                        .font(theme.font(theme.text_xl))
                                        .color(theme.accent)
                                        .monospace(),
                                );
                            });
                        ui.add_space(theme.space_12);
                        let url = verification_uri_complete.clone();
                        if ui
                            .hyperlink_to("Open verification page →", &url)
                            .clicked()
                        {
                            open_browser(&url);
                        }
                        ui.add_space(theme.space_12);
                        ui.label(
                            egui::RichText::new("Waiting for authorization...")
                                .size(theme.text_sm)
                                .color(theme.text_muted),
                        );
                        ui.spinner();
                    });
                }
                KimiCodeLoginState::Polling => {
                    ui.vertical_centered(|ui| {
                        ui.add_space(theme.space_24);
                        ui.label(
                            egui::RichText::new("Completing login...")
                                .size(theme.text_base)
                                .color(theme.text),
                        );
                        ui.add_space(theme.space_12);
                        ui.spinner();
                    });
                }
                KimiCodeLoginState::Success => {
                    ui.vertical_centered(|ui| {
                        ui.add_space(theme.space_24);
                        ui.label(
                            egui::RichText::new("✓ Login successful")
                                .size(theme.text_lg)
                                .strong()
                                .color(theme.ok),
                        );
                        ui.add_space(theme.space_8);
                        ui.label(
                            egui::RichText::new("You can now use Kimi Code.")
                                .size(theme.text_sm)
                                .color(theme.text_dim),
                        );
                        ui.add_space(theme.space_16);
                        if ui.add(theme.primary_button("Close")).clicked() {
                            close_requested = true;
                        }
                    });
                }
                KimiCodeLoginState::Error(e) => {
                    ui.vertical_centered(|ui| {
                        ui.add_space(theme.space_24);
                        ui.label(
                            egui::RichText::new("Login failed")
                                .size(theme.text_lg)
                                .strong()
                                .color(theme.danger),
                        );
                        ui.add_space(theme.space_8);
                        ui.label(
                            egui::RichText::new(e)
                                .size(theme.text_sm)
                                .color(theme.text_dim),
                        );
                        ui.add_space(theme.space_16);
                        if ui.add(theme.primary_button("Retry")).clicked() {
                            start_login(app, config.clone());
                        }
                    });
                }
            }
        });

    if close_requested {
        app.settings_store.kimi_code_login_open = false;
        app.settings_store.kimi_code_login_state = KimiCodeLoginState::Idle;
    }
}

fn start_login(app: &mut App, config: clarity_core::auth::OAuthDeviceFlowConfig) {
    app.settings_store.kimi_code_login_state = KimiCodeLoginState::Requesting;
    let tx = app.ui_tx.clone();
    let runtime = app.runtime.handle().clone();

    runtime.spawn(async move {
        let client = clarity_core::auth::OAuthDeviceFlowClient::with_config(config.clone());

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
                    let store = clarity_core::auth::TokenStore::default_kimi_code();
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
                Err(clarity_core::auth::AuthError::Expired) => {
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
                Err(clarity_core::auth::AuthError::Request(ref msg))
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
