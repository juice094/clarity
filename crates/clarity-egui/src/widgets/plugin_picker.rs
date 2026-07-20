//! Plugin picker — `/` command palette for invoking skills, MCP tools,
//! web tabs, and built-in actions from the composer.

use crate::design_system::{self, TextStyle};
use crate::theme::Theme;

/// State for the `/` plugin picker popup.
#[derive(Clone, Default)]
pub struct PluginPickerState {
    /// Whether the picker is currently visible.
    pub open: bool,
    /// The raw text after `/` that the user is typing (filter query).
    pub filter: String,
}

/// A plugin entry exposed by the picker.
///
/// This mirrors `crate::stores::PluginItem` but keeps the widget dependency-free
/// from the stores module so it can be rendered from tests and modals.
#[derive(Clone, Debug)]
pub struct PluginEntry {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub category: &'static str,
}

impl From<crate::stores::PluginItem> for PluginEntry {
    fn from(item: crate::stores::PluginItem) -> Self {
        let category = match item.source {
            crate::stores::PluginSource::Builtin { .. } => "Action",
            crate::stores::PluginSource::Skill { .. } => "Skill",
            crate::stores::PluginSource::Mcp { .. } => "Tool",
            crate::stores::PluginSource::WebTab { .. } => "Web",
        };
        Self {
            id: item.id,
            name: item.name,
            icon: item.icon,
            category,
        }
    }
}

/// Render the plugin picker popup. Returns `Some(PluginEntry)` when the user
/// confirms a selection, or `None` while the picker is open.
///
/// Callers should invoke this immediately after the chat input widget when
/// `state.open` is true, positioning it as an anchored popup.
pub fn render_plugin_picker(
    ui: &mut egui::Ui,
    state: &mut PluginPickerState,
    theme: &Theme,
    plugins: &[PluginEntry],
) -> Option<PluginEntry> {
    let mut result = None;

    egui::Frame::popup(ui.style())
        .fill(theme.surface)
        .stroke(egui::Stroke::new(1.0, theme.border))
        .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
        .inner_margin(egui::Margin::same(theme.space_8 as i8))
        .show(ui, |ui| {
            ui.set_min_width(240.0);
            ui.set_max_width(360.0);
            ui.set_max_height(320.0);

            // Filter input.
            let filter_resp = ui.add_sized(
                [ui.available_width(), 24.0],
                egui::TextEdit::singleline(&mut state.filter)
                    .hint_text("Search plugins...")
                    .text_color(theme.text),
            );
            if state.filter.is_empty() {
                filter_resp.request_focus();
            }
            crate::design_system::gap(ui, crate::design_system::Space::S0);

            let filtered: Vec<_> = plugins
                .iter()
                .filter(|p| {
                    let q = state.filter.to_lowercase();
                    p.name.to_lowercase().contains(&q) || p.category.to_lowercase().contains(&q)
                })
                .collect();

            egui::ScrollArea::vertical()
                .id_salt("plugin_picker_scroll")
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    if filtered.is_empty() {
                        design_system::text(ui, "No matching plugins", TextStyle::Small);
                    } else {
                        for plugin in filtered {
                            let row_response = ui.horizontal(|ui| {
                                ui.set_min_width(ui.available_width());
                                ui.spacing_mut().item_spacing.x = theme.space_8;

                                let glyph = plugin_icon_glyph(&plugin.icon, theme);
                                design_system::icon(ui, glyph, theme.text_sm);

                                ui.vertical(|ui| {
                                    design_system::text(ui, &plugin.name, TextStyle::Body);
                                    design_system::text_with_color(
                                        ui,
                                        plugin.category,
                                        TextStyle::Small,
                                        theme.text_dim,
                                    );
                                });
                            });
                            if row_response.response.clicked() {
                                result = Some(plugin.clone());
                            }
                            if row_response.response.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                        }
                    }
                });
        });

    result
}

/// Convert a persisted plugin icon key into an icon-font glyph.
fn plugin_icon_glyph<'a>(icon: &str, _theme: &'a Theme) -> &'a str {
    match icon {
        "globe" => crate::theme::ICON_GLOBE,
        "file" | "file_text" => crate::theme::ICON_FILE,
        "table" => crate::theme::ICON_LIST,
        "presentation" => crate::theme::ICON_FILE_CODE,
        "book" => crate::theme::ICON_BOOK,
        "wrench" => crate::theme::ICON_WRENCH,
        "flow" | "bot" => crate::theme::ICON_BOT,
        "layout_template" => crate::theme::ICON_LAYOUT_TEMPLATE,
        "terminal" => crate::theme::ICON_TERMINAL,
        "message_square" | "chat" => crate::theme::ICON_CHAT,
        "file_code" | "code" => crate::theme::ICON_FILE_CODE,
        "briefcase" | "work" => crate::theme::ICON_LAYERS,
        _ => crate::theme::ICON_LAYERS,
    }
}
