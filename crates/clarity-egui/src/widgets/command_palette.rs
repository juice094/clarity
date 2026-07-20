use crate::theme::Theme;
use clarity_ui::design_system::{Space, TextStyle, gap, text};
use clarity_ui::widgets::overlay::Overlay;
use clarity_ui::widgets::text_input::TextInput;

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
    /// Creates a new instance.
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

        Overlay::new("command_palette")
            .width(theme.palette_w)
            .top_center(theme.modal_offset_y)
            .show(ctx, |ui| {
                // ── Input ──
                let input_resp = ui.add(
                    TextInput::singleline(&mut self.query)
                        .hint_text("> type a command...")
                        .width(ui.available_width()),
                );
                if input_resp.changed() {
                    self.selected = 0;
                }

                // ponytail: raw separator; no semantic wrapper in clarity-ui yet.
                ui.separator();

                // ── List ──
                // ponytail: raw ScrollArea; no wrapper in clarity-ui yet.
                egui::ScrollArea::vertical()
                    .max_height(theme.palette_max_h)
                    .show(ui, |ui| {
                        for (idx, cmd) in filtered.iter().enumerate() {
                            let is_selected = idx == self.selected;
                            let row_bg = if is_selected {
                                theme.bg_hover
                            } else {
                                egui::Color32::TRANSPARENT
                            };

                            let row_resp = crate::design_system::interactive_row_frame(ui)
                                .fill(row_bg)
                                .inner_margin(egui::Margin::symmetric(
                                    theme.space_12 as i8,
                                    theme.space_8 as i8,
                                ))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        // ponytail: raw Label with custom icon font/color.
                                        ui.label(
                                            egui::RichText::new("›")
                                                .font(theme.font_icon(theme.text_sm))
                                                .color(if is_selected {
                                                    theme.accent
                                                } else {
                                                    theme.text_dim
                                                }),
                                        );
                                        gap(ui, Space::S1);
                                        text(ui, cmd.name.as_str(), TextStyle::Body);
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if let Some(ref sc) = cmd.shortcut {
                                                    // ponytail: raw Label for text_muted + monospace shortcut.
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
                if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) && !filtered.is_empty() {
                    self.selected = (self.selected + 1) % filtered.len();
                }
                if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) && !filtered.is_empty() {
                    self.selected = self.selected.saturating_sub(1);
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

    fn filter<'a>(&self, commands: &'a [CommandItem]) -> Vec<&'a CommandItem> {
        let q = self.query.to_lowercase();
        if q.is_empty() {
            return commands.iter().collect();
        }
        let mut scored: Vec<(i32, &CommandItem)> = commands
            .iter()
            .filter_map(|cmd| {
                let s = fuzzy_score(&cmd.name.to_lowercase(), &q);
                if s >= 0 { Some((s, cmd)) } else { None }
            })
            .collect();
        scored.sort_by_key(|(s, _)| -s);
        scored.into_iter().map(|(_, cmd)| cmd).collect()
    }
}

/// Simple fuzzy scorer: returns higher score for better matches,
/// -1 if query chars don't appear in order.
fn fuzzy_score(target: &str, query: &str) -> i32 {
    let mut score = 0i32;
    let mut prev = -1i32;
    let mut t_iter = target.chars().enumerate();
    for qc in query.chars() {
        let mut found = false;
        for (ti, tc) in &mut t_iter {
            if tc == qc {
                let consec = if ti as i32 == prev + 1 { 5 } else { 0 };
                let start = if ti == 0 || target.as_bytes().get(ti.saturating_sub(1)) == Some(&b' ')
                {
                    10
                } else {
                    0
                };
                score += 1 + consec + start;
                prev = ti as i32;
                found = true;
                break;
            }
        }
        if !found {
            return -1;
        }
    }
    score
}
