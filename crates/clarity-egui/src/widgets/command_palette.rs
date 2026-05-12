use crate::theme::Theme;

/// Re-export from shared core so the palette speaks the same language as TUI.
pub use clarity_core::ui::CommandItem;

/// Floating Command Palette — Pretext UI discovery layer.
///
/// Replaces the placeholder toast in `main.rs` (Ctrl+Shift+P)
/// with a real fuzzy-searchable command surface.
///
/// # Layout
/// - Width `theme.palette_w`, max scroll height `theme.palette_max_h`,
///   anchored CENTER_TOP + `theme.modal_offset_y`.
/// - Single-line input with `>` prefix semantics.
/// - List rows: icon + name (left) + shortcut (right, monospace).
/// - Selection via ↑/↓, execution via Enter, dismiss via Esc.
///
/// # Dispatch (P0.5.C.2)
/// `show()` returns `Some(command_id)` when the user activates a command via
/// click or Enter. The caller (`App::update`) is expected to forward that id
/// to [`App::dispatch_command`](crate::App::dispatch_command), which is the
/// single source of truth for both keyboard shortcuts and the palette.
pub struct CommandPalette {
    pub open: bool,
    pub query: String,
    pub selected: usize,
}

impl CommandPalette {
    pub fn new() -> Self {
        Self {
            open: false,
            query: String::new(),
            selected: 0,
        }
    }

    /// Render the palette. Sets `self.open = false` when the user dismisses it.
    ///
    /// Returns `Some(command_id)` when the user activates a command (click or
    /// Enter). The caller must dispatch that id via `App::dispatch_command`.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        theme: &Theme,
        commands: &[CommandItem],
    ) -> Option<String> {
        if !self.open {
            return None;
        }
        let mut keep_open = true;
        let mut activated: Option<String> = None;
        let filtered = self.filter(commands);

        // Clamp selection when filter results change.
        if self.selected >= filtered.len() && !filtered.is_empty() {
            self.selected = filtered.len() - 1;
        }

        egui::Window::new("command_palette")
            .title_bar(false)
            .resizable(false)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, theme.modal_offset_y))
            .frame(
                egui::Frame::new()
                    .fill(theme.bg_elevated)
                    .stroke(egui::Stroke::new(1.0, theme.border))
                    .corner_radius(egui::CornerRadius::same(theme.radius_md as u8))
                    .inner_margin(egui::Margin::symmetric(0, theme.space_12 as i8)),
            )
            .show(ctx, |ui| {
                ui.set_width(theme.palette_w);

                // ── Input ──
                let input_resp = ui.add(
                    egui::TextEdit::singleline(&mut self.query)
                        .hint_text("> type a command...")
                        .font(egui::FontId::monospace(theme.text_base))
                        .text_color(theme.text)
                        .frame(false),
                );
                if input_resp.changed() {
                    self.selected = 0;
                }

                ui.separator();

                // ── List ──
                egui::ScrollArea::vertical().max_height(theme.palette_max_h).show(ui, |ui| {
                    for (idx, cmd) in filtered.iter().enumerate() {
                        let is_selected = idx == self.selected;
                        let row_bg = if is_selected {
                            theme.bg_hover
                        } else {
                            egui::Color32::TRANSPARENT
                        };

                        let row_resp = egui::Frame::new()
                            .fill(row_bg)
                            .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
                            .inner_margin(egui::Margin::symmetric(theme.space_12 as i8, theme.space_8 as i8))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new("›")
                                            .font(theme.font_icon(theme.text_sm))
                                            .color(if is_selected { theme.accent } else { theme.text_dim }),
                                    );
                                    ui.add_space(theme.space_8);
                                    ui.label(
                                        egui::RichText::new(cmd.name.as_str())
                                            .color(theme.text)
                                            .size(theme.text_base),
                                    );
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if let Some(ref sc) = cmd.shortcut {
                                                ui.label(
                                                    egui::RichText::new(sc.as_str())
                                                        .color(theme.text_muted)
                                                        .size(theme.text_xs)
                                                        .monospace(),
                                                );
                                            }
                                        },
                                    );
                                });
                            })
                            .response
                            .interact(egui::Sense::click());

                        if row_resp.clicked() {
                            activated = Some(cmd.id.clone());
                            keep_open = false;
                        }
                    }
                });

                // ── Keyboard navigation ──
                if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                    if !filtered.is_empty() {
                        self.selected = (self.selected + 1) % filtered.len();
                    }
                }
                if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                    if !filtered.is_empty() {
                        self.selected = self.selected.saturating_sub(1);
                    }
                }
                if ui.input(|i| i.key_pressed(egui::Key::Enter)) && !filtered.is_empty() {
                    if let Some(cmd) = filtered.get(self.selected) {
                        activated = Some(cmd.id.clone());
                        keep_open = false;
                    }
                }
                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    keep_open = false;
                }
            });

        self.open = keep_open;
        activated
    }

    fn filter<'a>(
        &self,
        commands: &'a [CommandItem],
    ) -> Vec<&'a CommandItem> {
        let q = self.query.to_lowercase();
        if q.is_empty() {
            return commands.iter().collect();
        }
        commands
            .iter()
            .filter(|cmd| cmd.name.to_lowercase().contains(&q))
            .collect()
    }
}
