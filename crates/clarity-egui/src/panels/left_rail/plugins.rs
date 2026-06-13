//! Left rail — Plugins panel.
//!
//! S6 Phase C: the Plugins expanded section shows user-customisable shortcuts.
//! In layout-edit mode the items can be drag-reordered; the resulting order is
//! persisted to `GuiSettings.plugin_order`.

use crate::App;
use crate::stores::{PluginItem, PluginSource};
use crate::ui::types::ToastLevel;

/// Render the Plugins expanded panel.
pub fn render_plugins_panel(app: &mut App, ctx: &egui::Context) {
    let theme = app.ui_store.theme.clone();
    let edit_mode = app.view_state.layout_edit_mode;

    // Collect plugins up-front so we can mutate `app` later without borrow issues.
    let items = {
        let all = crate::stores::all_plugins(
            &app.state.agent,
            &app.mcp_store,
            &app.settings_store.settings_edit,
        );
        let order = app.settings_store.settings_edit.plugin_order.clone();
        crate::stores::ordered_plugins(&all, &order)
    };

    let mut pending_action: Option<PluginSource> = None;
    let mut new_order: Option<Vec<String>> = None;

    egui::SidePanel::left("left_rail_plugins")
        .default_width(theme.size_sidebar)
        .min_width(180.0)
        .max_width(400.0)
        .resizable(true)
        .frame(
            egui::Frame::side_top_panel(&ctx.style())
                .fill(theme.bg)
                .stroke(egui::Stroke::NONE)
                .inner_margin(egui::Margin::symmetric(12, 16)),
        )
        .show(ctx, |ui| {
            ui.set_min_width(ui.available_width());

            // ── Header + edit-mode toggle ──
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Plugins")
                        .size(theme.text_base)
                        .strong()
                        .color(theme.text_strong),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let lock_icon = if edit_mode {
                        crate::theme::ICON_EDIT
                    } else {
                        crate::theme::ICON_SETTINGS
                    };
                    let lock_tooltip = if edit_mode {
                        "退出布局编辑"
                    } else {
                        "进入布局编辑"
                    };
                    if crate::widgets::icon_button_toolbar(ui, lock_icon, theme.text_base, &theme)
                        .on_hover_text(lock_tooltip)
                        .clicked()
                    {
                        app.view_state.toggle_layout_edit_mode();
                    }
                });
            });
            ui.add_space(theme.space_12);

            if items.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(theme.space_40);
                    ui.label(
                        egui::RichText::new("No plugins available")
                            .size(theme.text_sm)
                            .color(theme.text_dim),
                    );
                });
                return;
            }

            // ── Reorderable plugin list ──
            for (idx, item) in items.iter().enumerate() {
                let frame = egui::Frame::new()
                    .fill(theme.surface)
                    .stroke(egui::Stroke::new(1.0, theme.border))
                    .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
                    .inner_margin(egui::Margin::same(8));

                let (inner, dropped_id) = ui.dnd_drop_zone::<String, _>(frame, |ui| {
                    ui.set_min_width(ui.available_width());
                    if edit_mode {
                        let _ = ui.dnd_drag_source(ui.id().with(&item.id), item.id.clone(), |ui| {
                            render_plugin_row(ui, item, edit_mode, &theme)
                        });
                    } else {
                        let row_response = render_plugin_row(ui, item, edit_mode, &theme);
                        if row_response.clicked() {
                            pending_action = Some(item.source.clone());
                        }
                    }
                });

                // Keep the drop-zone response out of the inner borrow closure.
                let _ = inner;

                if let Some(dropped_id) = dropped_id {
                    let mut order: Vec<String> = items.iter().map(|p| p.id.clone()).collect();
                    if let Some(from) = order.iter().position(|id| id == dropped_id.as_ref()) {
                        let to = idx;
                        if from != to {
                            let id = order.remove(from);
                            order.insert(to, id);
                            new_order = Some(order);
                        }
                    }
                }

                ui.add_space(theme.space_4);
            }

            ui.add_space(theme.space_8);
            ui.separator();
            ui.add_space(theme.space_8);

            // Placeholder "add plugin" entry.
            ui.vertical_centered(|ui| {
                if ui
                    .button(
                        egui::RichText::new("+ Manage plugins")
                            .size(theme.text_sm)
                            .color(theme.text_dim),
                    )
                    .clicked()
                {
                    app.push_toast("Plugin manager coming soon".to_string(), ToastLevel::Info);
                }
            });
        });

    if let Some(order) = new_order {
        app.persist_plugin_order(order);
    }

    if let Some(source) = pending_action {
        match source {
            PluginSource::Builtin { id } => handle_builtin_action(app, &id),
            PluginSource::Skill { .. } => {
                app.view_state
                    .open_modal(clarity_core::ui::ModalType::Skill);
            }
            PluginSource::Mcp { .. } => {
                app.view_state.open_modal(clarity_core::ui::ModalType::Mcp);
            }
            PluginSource::WebTab { url } => {
                app.push_toast(format!("Open web tab: {}", url), ToastLevel::Info);
            }
        }
    }
}

fn render_plugin_row(
    ui: &mut egui::Ui,
    item: &PluginItem,
    edit_mode: bool,
    theme: &crate::theme::Theme,
) -> egui::Response {
    let icon = plugin_icon_glyph(&item.icon);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = theme.space_8;

        if edit_mode {
            ui.label(
                egui::RichText::new("⠿")
                    .size(theme.text_sm)
                    .color(theme.text_dim),
            );
        }

        ui.label(
            egui::RichText::new(icon)
                .size(theme.text_base)
                .color(theme.accent),
        );

        ui.label(
            egui::RichText::new(&item.name)
                .size(theme.text_sm)
                .color(theme.text),
        );
    })
    .response
}

fn plugin_icon_glyph(key: &str) -> &'static str {
    match key {
        "file_text" => crate::theme::ICON_FILE_TEXT,
        "file" => crate::theme::ICON_FILE,
        "globe" => crate::theme::ICON_GLOBE,
        "table" => crate::theme::ICON_TABLE,
        "presentation" => crate::theme::ICON_PRESENTATION,
        "wrench" => crate::theme::ICON_WRENCH,
        "book" => crate::theme::ICON_BOOK,
        "flow" => crate::theme::ICON_REFRESH,
        _ => crate::theme::ICON_FILE,
    }
}

fn handle_builtin_action(app: &mut App, id: &str) {
    match id {
        "doc" => app.push_toast(
            "文档插件: 打开文件上传（待实现）".to_string(),
            ToastLevel::Info,
        ),
        "web" => app.push_toast(
            "网站插件: 打开网页工具（待实现）".to_string(),
            ToastLevel::Info,
        ),
        "sheet" => app.push_toast(
            "表格插件: 打开表格工具（待实现）".to_string(),
            ToastLevel::Info,
        ),
        "ppt" => app.push_toast(
            "PPT 插件: 打开演示工具（待实现）".to_string(),
            ToastLevel::Info,
        ),
        _ => app.push_toast(format!("Builtin plugin: {}", id), ToastLevel::Info),
    }
}
