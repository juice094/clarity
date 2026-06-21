//! Claw device settings panel in the right IDE rail.
//!
//! Shows the active device's info and provides action buttons for
//! management operations: diagnostics, rename, terminal, restart,
//! backup, update, and help.

use crate::App;
use crate::ui::types::ToastLevel;

/// Render the Claw device settings panel.
pub fn render(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.ui_store.theme.clone();

    let bot = app
        .ui_store
        .bot_instances
        .iter()
        .find(|b| b.id == app.ui_store.active_bot_id)
        .or_else(|| app.ui_store.bot_instances.first())
        .cloned();

    let (bot_name, bot_id, bot_version, bot_last_backup, role_for_passphrase) = match bot {
        Some(b) => (b.name, b.id, b.version, b.last_backup, b.role),
        None => {
            ui.label(
                egui::RichText::new(app.t("No devices"))
                    .size(theme.text_sm)
                    .color(theme.text_muted),
            );
            return;
        }
    };

    // Pre-translate strings that are used while `app.ui_store` is mutably borrowed.
    let hint_enter_passphrase = app.t("Enter passphrase…").to_string();

    egui::ScrollArea::vertical()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());

            // ── Device Identity ────────────────────────────────────────
            section_header(ui, &theme, app.t("Settings"), crate::theme::ICON_SETTINGS);
            ui.add_space(theme.space_8);
            ui.label(
                egui::RichText::new(&bot_name)
                    .size(theme.text_md)
                    .color(theme.text_strong),
            );
            ui.label(
                egui::RichText::new(format!("ID: {}", bot_id))
                    .size(theme.text_xs)
                    .color(theme.text_muted),
            );

            ui.add_space(theme.space_12);

            // ── Chat Channel ───────────────────────────────────────────
            section_header(ui, &theme, app.t("Chat channel"), crate::theme::ICON_CHAT);
            ui.add_space(theme.space_8);
            if action_button(ui, &theme, app.t("Connect chat channel")).clicked() {
                app.push_toast(app.t("Connecting to chat channel…"), ToastLevel::Info);
            }

            ui.add_space(theme.space_16);

            // ── Role Passphrase (E2EE) ─────────────────────────────────
            section_header(
                ui,
                &theme,
                app.t("Role passphrase"),
                crate::theme::ICON_LOCK,
            );
            ui.add_space(theme.space_8);
            ui.label(
                egui::RichText::new(app.t("Encrypts role-context events stored by Syncthing"))
                    .size(theme.text_xs)
                    .color(theme.text_muted),
            );
            ui.add_space(theme.space_4);

            if role_for_passphrase.is_empty() {
                ui.label(
                    egui::RichText::new(app.t("Select a Claw device to set a passphrase"))
                        .size(theme.text_sm)
                        .color(theme.text_muted),
                );
            } else {
                ui.label(
                    egui::RichText::new(format!("{}: {}", app.t("Role"), role_for_passphrase))
                        .size(theme.text_sm)
                        .color(theme.text_dim),
                );
                ui.add_space(theme.space_4);
                ui.add_sized(
                    egui::vec2(ui.available_width(), 28.0),
                    egui::TextEdit::singleline(&mut app.ui_store.claw_role_passphrase_input)
                        .password(true)
                        .hint_text(hint_enter_passphrase),
                );
                ui.add_space(theme.space_4);
                ui.horizontal(|ui| {
                    let has_connection = app.claw_ws.is_some();
                    ui.add_enabled_ui(has_connection, |ui| {
                        if action_button(ui, &theme, app.t("Apply passphrase")).clicked() {
                            let pw = app.ui_store.claw_role_passphrase_input.clone();
                            if let Some(ref claw) = app.claw_ws {
                                claw.set_role_passphrase(&role_for_passphrase, &pw);
                                app.push_toast(app.t("Passphrase applied"), ToastLevel::Info);
                            }
                        }
                    });
                    ui.add_space(theme.space_8);
                    if small_button(ui, &theme, app.t("Clear")).clicked() {
                        app.ui_store.claw_role_passphrase_input.clear();
                        if let Some(ref claw) = app.claw_ws {
                            claw.set_role_passphrase(&role_for_passphrase, "");
                            app.push_toast(app.t("Passphrase cleared"), ToastLevel::Info);
                        }
                    }
                });
            }

            ui.add_space(theme.space_16);

            // ── Actions ────────────────────────────────────────────────
            section_header(ui, &theme, app.t("Actions"), crate::theme::ICON_CPU);
            ui.add_space(theme.space_8);

            let actions: &[(&str, &str, bool)] = &[
                ("AI Diagnostics", "AI diagnosis running…", false),
                ("Edit bot name", "Rename not yet implemented", false),
                ("Open terminal", "", false),
                ("Restart Gateway", "Restart request sent", false),
                (
                    "Fix Kimi Claw config",
                    "Configuration check running…",
                    false,
                ),
                ("Subscribe quota", "Opening subscription page…", false),
                ("Upgrade Kimi Claw plugin", "Checking for updates…", false),
                ("Reset to defaults", "Reset complete", true),
                ("Delete", "Delete not yet implemented", true),
            ];

            for &(label, toast, _danger) in actions {
                let resp = if _danger {
                    action_button_danger(ui, &theme, app.t(label))
                } else {
                    action_button(ui, &theme, app.t(label))
                };

                if resp.clicked() {
                    if label == "Open terminal" {
                        app.view_state
                            .set_right_rail_panel(clarity_core::ui::RightRailPanel::ClawTerminal);
                    } else if !toast.is_empty() {
                        let level = if _danger {
                            ToastLevel::Warn
                        } else {
                            ToastLevel::Info
                        };
                        app.push_toast(app.t(toast), level);
                    }
                }
                ui.add_space(theme.space_4);
            }

            ui.add_space(theme.space_16);

            // ── Version ────────────────────────────────────────────────
            section_header(ui, &theme, app.t("Version"), crate::theme::ICON_INFO);
            ui.add_space(theme.space_8);
            ui.label(
                egui::RichText::new(format!("{} {}", app.t("Current version"), bot_version))
                    .size(theme.text_sm)
                    .color(theme.text),
            );
            ui.add_space(theme.space_4);
            if small_button(ui, &theme, app.t("Check for updates")).clicked() {
                app.push_toast(app.t("Already up to date"), ToastLevel::Info);
            }
            ui.add_space(theme.space_4);
            if small_button(ui, &theme, app.t("Release notes")).clicked() {
                app.view_state
                    .set_right_rail_panel(clarity_core::ui::RightRailPanel::ClawWebBridge);
            }

            ui.add_space(theme.space_16);

            // ── Data Backup ────────────────────────────────────────────
            section_header(ui, &theme, app.t("Data backup"), crate::theme::ICON_ARCHIVE);
            ui.add_space(theme.space_8);
            ui.label(
                egui::RichText::new(format!("{} {}", app.t("Last backup"), bot_last_backup))
                    .size(theme.text_sm)
                    .color(theme.text),
            );
            ui.add_space(theme.space_4);
            ui.horizontal(|ui| {
                if small_button(ui, &theme, app.t("Backup now")).clicked() {
                    app.push_toast(app.t("Backup started…"), ToastLevel::Info);
                }
                ui.add_space(theme.space_8);
                if small_button(ui, &theme, app.t("Restore")).clicked() {
                    app.push_toast(app.t("Restore started…"), ToastLevel::Info);
                }
            });

            ui.add_space(theme.space_16);

            // ── Help ───────────────────────────────────────────────────
            section_header(ui, &theme, app.t("Help"), crate::theme::ICON_BOOK_OPEN);
            ui.add_space(theme.space_8);
            if action_button(ui, &theme, app.t("User manual")).clicked() {
                app.view_state
                    .set_right_rail_panel(clarity_core::ui::RightRailPanel::ClawWebBridge);
            }
            ui.add_space(theme.space_4);
            if action_button(ui, &theme, app.t("Report issue")).clicked() {
                app.push_toast(app.t("Opening feedback form…"), ToastLevel::Info);
            }
            ui.add_space(theme.space_4);
        });
}

// ── Helpers ───────────────────────────────────────────────────────────

fn section_header(ui: &mut egui::Ui, theme: &crate::theme::Theme, text: &str, icon: &str) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = theme.space_8;
        ui.label(
            egui::RichText::new(icon)
                .size(theme.text_sm)
                .color(theme.text_dim),
        );
        ui.label(
            egui::RichText::new(text)
                .size(theme.text_sm)
                .color(theme.text_dim)
                .strong(),
        );
    });
}

fn action_button(ui: &mut egui::Ui, theme: &crate::theme::Theme, text: &str) -> egui::Response {
    ui.add_sized(
        egui::vec2(ui.available_width(), 28.0),
        egui::Button::new(egui::RichText::new(text).size(theme.text_sm))
            .fill(theme.surface)
            .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8)),
    )
}

fn action_button_danger(
    ui: &mut egui::Ui,
    theme: &crate::theme::Theme,
    text: &str,
) -> egui::Response {
    ui.add_sized(
        egui::vec2(ui.available_width(), 28.0),
        egui::Button::new(
            egui::RichText::new(text)
                .size(theme.text_sm)
                .color(theme.danger),
        )
        .fill(theme.surface)
        .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8)),
    )
}

fn small_button(ui: &mut egui::Ui, theme: &crate::theme::Theme, text: &str) -> egui::Response {
    ui.add_sized(
        egui::vec2(ui.available_width(), 24.0),
        egui::Button::new(egui::RichText::new(text).size(theme.text_sm)),
    )
}
