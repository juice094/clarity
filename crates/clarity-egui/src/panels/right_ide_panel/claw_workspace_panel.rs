//! Claw workspace file-tree panel in the right IDE rail.
//!
//! Renders the selected device's workspace directory as a navigable file
//! tree. Uses `ui::file_browser::render_file_tree()` for the actual
//! rendering and wires file clicks to the inline preview system.

use crate::App;
use std::path::PathBuf;

/// Render the Claw workspace file-tree panel.
pub fn render(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.ui_store.theme.clone();

    // Resolve the active bot's workspace root. In the future this will
    // come from the device registry; for now use a sensible default.
    let bot = app
        .ui_store
        .bot_instances
        .iter()
        .find(|b| b.id == app.ui_store.active_bot_id)
        .or_else(|| app.ui_store.bot_instances.first());

    let bot_name = bot.map(|b| b.name.clone()).unwrap_or_default();

    // Resolve workspace from per-device connection, falling back to the
    // legacy path resolution for ZeroClaw devices.
    let workspace_root = app
        .device_state
        .active_connection(&app.ui_store.active_bot_id)
        .map(|c| c.workspace_root)
        .filter(|p| p != std::path::Path::new(".") && p.exists())
        .unwrap_or_else(|| resolve_workspace_root(&bot_name));

    // Show connection type badge.
    let conn_type = app
        .device_state
        .active_connection(&app.ui_store.active_bot_id)
        .map(|c| match c.claw_type {
            crate::claw::ClawType::ZeroClaw => "ZeroClaw",
            crate::claw::ClawType::OpenClaw => "OpenClaw",
        })
        .unwrap_or("");

    egui::ScrollArea::vertical()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());

            // Header with bot name, type badge, and root path.
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(crate::theme::ICON_FOLDER_OPEN)
                        .size(theme.text_sm)
                        .color(theme.text_dim),
                );
                ui.label(
                    egui::RichText::new(&bot_name)
                        .size(theme.text_sm)
                        .color(theme.text_strong),
                );
                if !conn_type.is_empty() {
                    ui.add_space(theme.space_4);
                    ui.label(
                        egui::RichText::new(conn_type)
                            .size(theme.text_xs)
                            .color(theme.accent)
                            .strong(),
                    );
                }
            });
            ui.label(
                egui::RichText::new(workspace_root.display().to_string())
                    .size(theme.text_xs)
                    .color(theme.text_muted),
            );

            ui.add_space(theme.space_8);

            if workspace_root.exists() && workspace_root.is_dir() {
                crate::ui::file_browser::render_file_tree(
                    ui,
                    &workspace_root,
                    &theme,
                    0,    // depth
                    None, // selected_path
                    &mut |path: &std::path::Path| {
                        app.ui_store.active_project = Some(path.to_string_lossy().into_owned());
                        app.view_state
                            .set_right_rail_panel(clarity_core::ui::RightRailPanel::ClawWebBridge);
                    },
                    false, // compact
                );
            } else {
                ui.add_space(theme.space_16);
                ui.label(
                    egui::RichText::new(app.t("Workspace directory not accessible"))
                        .size(theme.text_sm)
                        .color(theme.text_muted),
                );
                ui.add_space(theme.space_8);
                ui.label(
                    egui::RichText::new(format!(
                        "{}: {}",
                        app.t("Expected path"),
                        workspace_root.display()
                    ))
                    .size(theme.text_xs)
                    .color(theme.text_dim),
                );
            }
        });
}

/// Resolve the workspace root directory for a given bot.
///
/// On Windows, defaults to `%USERPROFILE%\.claw\<bot_name>`.
/// On other platforms, defaults to `~/.claw/<bot_name>`.
fn resolve_workspace_root(bot_name: &str) -> PathBuf {
    let home = if cfg!(target_os = "windows") {
        std::env::var("USERPROFILE").unwrap_or_else(|_| "C:".into())
    } else {
        std::env::var("HOME").unwrap_or_else(|_| "/".into())
    };
    PathBuf::from(home).join(".claw").join(bot_name)
}
