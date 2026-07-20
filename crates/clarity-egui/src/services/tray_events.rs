//! System tray event handling — icon clicks and context menu actions.
//!
//! Extracted from `main.rs` per the file-size modularization effort.

use crate::App;
use crate::ToastLevel;

impl App {
    pub(crate) fn handle_tray_events(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let Some(tray) = self.context.tray_manager.as_ref() else {
            return;
        };

        // Tray icon clicks (double-click → show)
        for event in tray.poll_tray_events() {
            use tray_icon::TrayIconEvent;
            match event {
                TrayIconEvent::DoubleClick { .. } | TrayIconEvent::Click { .. } => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                }
                _ => {}
            }
        }

        // Context menu actions
        for action in tray.poll_menu_events() {
            use crate::services::tray::TrayAction;
            match action {
                TrayAction::Show => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                }
                TrayAction::CopySessionLink => {
                    if let Some(session) = self.context.session_store.active_session() {
                        let link = format!("clarity://session/{}", session.id);
                        ctx.copy_text(link);
                        self.push_toast("Session link copied".to_string(), ToastLevel::Info);
                    }
                }
                TrayAction::Pause => {
                    self.stop();
                    self.push_toast("Agent paused".to_string(), ToastLevel::Info);
                }
                TrayAction::Settings => {
                    self.view_state.main = clarity_core::ui::AppView::Settings;
                }
                TrayAction::Quit => {
                    self.tray_quit_requested = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }
}
