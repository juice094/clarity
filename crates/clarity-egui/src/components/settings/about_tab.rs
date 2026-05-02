use crate::App;

pub fn render_about(app: &mut App, ui: &mut egui::Ui) {
    ui.vertical_centered(|ui| {
        ui.label(egui::RichText::new("Clarity").size(app.ui_store.theme.text_2xl).strong().color(app.ui_store.theme.text));
        ui.label(egui::RichText::new("Local-first AI agent runtime").size(app.ui_store.theme.text_base).color(app.ui_store.theme.text_muted));
        ui.add_space(12.0);
        ui.label(egui::RichText::new(format!("v{}", env!("CARGO_PKG_VERSION"))).size(app.ui_store.theme.text_sm).color(app.ui_store.theme.text_dim));
        ui.label(egui::RichText::new("egui 0.31 · glow").size(app.ui_store.theme.text_sm).color(app.ui_store.theme.text_dim));
        ui.add_space(8.0);
        ui.hyperlink_to(egui::RichText::new("github.com/juice094/clarity").size(app.ui_store.theme.text_sm).color(app.ui_store.theme.accent),
            "https://github.com/juice094/clarity");
    });
}
