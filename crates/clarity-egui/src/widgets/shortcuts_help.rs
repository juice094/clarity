//! Keyboard shortcuts reference overlay (Ctrl+/).
//!
//! Extracted from `main.rs` per the egui panel render limit (300 lines).
//! Rendered via `clarity_ui::widgets::overlay::Overlay` per the Clarity Design
//! Protocol v1.0.

use clarity_ui::design_system::{Space, TextStyle, gap, text};
use clarity_ui::theme::ICON_X;
use clarity_ui::widgets::icon_button::icon_button_toolbar;
use clarity_ui::widgets::overlay::Overlay;

use crate::App;

impl App {
    /// Render the keyboard shortcuts reference overlay.
    pub(crate) fn render_shortcuts_help(&mut self, ctx: &egui::Context) {
        if !self.shortcuts_help_open {
            return;
        }

        let mut close_requested = false;

        Overlay::new("shortcuts_help")
            .width(520.0)
            .max_height(480.0)
            .show(ctx, |ui| {
                let t = clarity_ui::design_system::theme(ui.ctx());

                // Title row with close button (replaces `Window::open` title-bar X).
                ui.horizontal(|ui| {
                    text(ui, "Keyboard Shortcuts", TextStyle::Title);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if icon_button_toolbar(ui, ICON_X, 16.0, &t).clicked() {
                            close_requested = true;
                        }
                    });
                });

                gap(ui, Space::S2);

                let actions: &[(&str, &[crate::shortcuts::ShortcutAction])] = &[
                    (
                        "General",
                        &[
                            crate::shortcuts::ShortcutAction::NewSession,
                            crate::shortcuts::ShortcutAction::SendMessage,
                            crate::shortcuts::ShortcutAction::StopGeneration,
                            crate::shortcuts::ShortcutAction::CloseModal,
                            crate::shortcuts::ShortcutAction::ShowShortcuts,
                        ],
                    ),
                    (
                        "Panels",
                        &[
                            crate::shortcuts::ShortcutAction::ToggleCommandPalette,
                            crate::shortcuts::ShortcutAction::FocusInput,
                            crate::shortcuts::ShortcutAction::ToggleConsole,
                            crate::shortcuts::ShortcutAction::ToggleFiles,
                            crate::shortcuts::ShortcutAction::ToggleShare,
                            crate::shortcuts::ShortcutAction::ToggleSkillPanel,
                            crate::shortcuts::ShortcutAction::ToggleTeamPanel,
                            crate::shortcuts::ShortcutAction::ToggleDashboardPanel,
                        ],
                    ),
                    (
                        "View",
                        &[
                            crate::shortcuts::ShortcutAction::IncreaseFontScale,
                            crate::shortcuts::ShortcutAction::DecreaseFontScale,
                            crate::shortcuts::ShortcutAction::ToggleLayoutDebug,
                        ],
                    ),
                    (
                        "Chat Messages",
                        &[
                            crate::shortcuts::ShortcutAction::NavigateMessageUp,
                            crate::shortcuts::ShortcutAction::NavigateMessageDown,
                            crate::shortcuts::ShortcutAction::CopySelectedMessage,
                            crate::shortcuts::ShortcutAction::EditSelectedMessage,
                            crate::shortcuts::ShortcutAction::RegenerateSelectedMessage,
                            crate::shortcuts::ShortcutAction::ClearMessageSelection,
                            crate::shortcuts::ShortcutAction::ScrollToBottom,
                        ],
                    ),
                ];

                for (group, items) in actions {
                    gap(ui, Space::S1);
                    text(ui, *group, TextStyle::CaptionStrong);
                    for action in items.iter() {
                        ui.horizontal(|ui| {
                            // ponytail: custom colour/size label until TextStyle covers
                            // accent monospace at small size.
                            ui.add_sized(
                                [140.0, t.text_base],
                                egui::Label::new(
                                    egui::RichText::new(action.keybinding())
                                        .size(t.text_sm)
                                        .monospace()
                                        .color(t.accent),
                                ),
                            );
                            // ponytail: custom colour/size label until TextStyle covers
                            // body text at small size.
                            ui.label(
                                egui::RichText::new(action.description())
                                    .size(t.text_sm)
                                    .color(t.text),
                            );
                        });
                    }
                }
            });

        if close_requested {
            self.shortcuts_help_open = false;
        }
    }
}
