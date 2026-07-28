//! Dashboard panel — read-only aggregate metrics for the right IDE rail.
//!
//! Reuses `clarity_apps::dashboard::render_metrics` so the metric cards stay
//! in sync with the standalone `DashboardApp`; only the container (dock tab
//! with its own scroll area) differs.

use crate::App;

/// Render the dashboard panel.
pub fn render(app: &mut App, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical()
        .id_salt("dashboard_rail_panel")
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            clarity_apps::dashboard::render_metrics(app, app.dashboard_app(), ui);
        });
}

// ── Panel trait implementation ──

/// Dashboard panel renderer.
pub struct DashboardPanel;

impl crate::design_system::Panel for DashboardPanel {
    fn title(&self, app: &crate::App) -> &str {
        app.t("Dashboard")
    }
    fn render(&mut self, app: &mut crate::App, ui: &mut egui::Ui) {
        render(app, ui);
    }
}
