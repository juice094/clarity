//! egui-specific chrome renderer.
//!
//! Bridge between the generic `clarity-chrome` shell and the concrete egui
//! `App`. The renderer owns the full layout orchestration: it builds the
//! top/bottom panels and the central three-column strip in a single pass.

use crate::App;
use clarity_chrome::ChromeRenderer;
use clarity_core::ui::{AppView, ModalType};
use clarity_shell::{ClarityApp, ClarityAppContext, ClarityAppResponse};

/// Concrete renderer that implements the chrome trait for the egui `App`.
///
/// The renderer is stateless; all layout decisions come from the supplied
/// [`App`] and the egui context. Rendering is wrapped in panic isolation via
/// [`App::render_safe`] so a failure in one chrome region does not crash the
/// whole frame.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct AppChromeRenderer;

impl ChromeRenderer<App> for AppChromeRenderer {
    fn render(&mut self, state: &mut App, ui: &mut egui::Ui, ctx: &egui::Context) {
        // Layer 0: full-screen background and outer border. Drawn first so
        // every later panel sits on top of the unified surface.
        state.render_safe(ui, "main_frame", |app, _ui| {
            app.render_main_stage_border(ctx);
        });

        let transparent = clarity_ui::design_system::Elevation::Base
            .frame(&state.context.ui_store.theme)
            .fill(egui::Color32::TRANSPARENT);

        // Layer 1: titlebar at the very top.
        egui::Panel::top("chrome_titlebar")
            .exact_size(state.context.ui_store.theme.size_titlebar)
            .resizable(false)
            .show_separator_line(false)
            .frame(transparent)
            .show(ui, |ui| {
                state.render_safe(ui, "titlebar", |app, ui| app.render_titlebar(ui));
            });

        // Layer 2: central area split into left rail / main stage / right rail.
        // Status metadata is rendered inside the chat bot bar at the top of the
        // main stage, so there is no separate bottom status bar panel.
        // The input composer is rendered inside the main-stage cell so it is
        // constrained to the chat column width rather than spanning the entire
        // window above the side rails.
        // StripBuilder allocates the three columns in one pass instead of
        // letting nested side panels fight over the remaining width.
        egui::CentralPanel::default()
            .frame(transparent)
            .show(ui, |ui| {
                self.render_central_strip(state, ui, ctx);
            });

        // Layer 3: overlays, modals, onboarding and resize handles. These are
        // drawn last so they float above every chrome panel.
        state.render_safe(ui, "skill", |app, _ui| app.render_skill_panel(ctx));
        state.render_safe(ui, "mcp", |app, _ui| app.render_mcp_panel(ctx));
        state.render_safe(ui, "toast", |app, _ui| app.render_toasts(ctx));

        // Only draw the shared modal scrim for modal types that do not render
        // their own overlay scrim. Skill/Mcp panels render their own dimmer +
        // close-on-outside-click behaviour; drawing the shared scrim on top would
        // block interaction with their contents.
        let current_modal = state.current_modal().copied();
        let needs_shared_scrim = current_modal
            .map(|m| !matches!(m, ModalType::Skill | ModalType::Mcp))
            .unwrap_or(false);
        let scrim_clicked = if needs_shared_scrim {
            state.render_modal_scrim(ctx).clicked()
        } else {
            false
        };
        // Clicking the scrim dismisses the modal, except for approval prompts
        // where an explicit decision is required for safety.
        if scrim_clicked && current_modal != Some(ModalType::Approval) {
            state.close_modal();
        }

        if let Some(modal) = current_modal {
            state.render_safe(ui, "modal", |app, _ui| {
                match modal {
                    ModalType::CronCreate => app.render_cron_create_modal(ctx),
                    ModalType::Approval => app.render_approval_modal(ctx),
                    ModalType::Snapshot => app.render_snapshot_modal(ctx),
                    ModalType::TaskCreate => app.render_task_create_modal(ctx),
                    ModalType::TaskView => app.render_task_view_modal(ctx),
                    ModalType::SubAgentView => app.render_subagent_view_modal(ctx),
                    ModalType::TeamCreate => app.render_team_create_modal(ctx),
                    ModalType::KimiCodeLogin => {
                        crate::panels::modals::login::render_oauth_login_modal(
                            app,
                            ctx,
                            &clarity_llm::auth::OAuthDeviceFlowConfig::default(),
                        );
                    }
                    ModalType::ManageWebLinks => {
                        crate::panels::modals::manage_web_links::render_manage_web_links_modal(
                            app, ctx,
                        );
                    }
                    ModalType::ManageWorkTemplates => {
                        crate::panels::modals::manage_work_templates::render_manage_work_templates_modal(app, ctx);
                    }
                    ModalType::Login | ModalType::AddProvider => {}
                    // Skill and Mcp use full-screen scrim overlays rather than the
                    // modal render path; they are handled above.
                    ModalType::Skill | ModalType::Mcp => {}
                }
            });
        }

        state.render_safe(ui, "onboarding", |app, _ui| {
            crate::onboarding::render_onboarding(app, ctx);
        });

        state.render_safe(ui, "resize", |app, _ui| app.handle_window_resize(ctx));
    }
}

impl AppChromeRenderer {
    /// Render the central three-column strip: left rail, main stage, right rail.
    ///
    /// The rails are still rendered through their existing panel-based helpers
    /// (which themselves create `Panel::left/right`). Nesting them inside
    /// StripBuilder cells lets us size all three columns in a single layout
    /// pass; the inner panels fill their allocated cells.
    fn render_central_strip(&mut self, state: &mut App, ui: &mut egui::Ui, ctx: &egui::Context) {
        let left_w = state.effective_left_rail_width(ctx).max(0.0);
        let right_w = state.effective_right_rail_width(ctx).max(0.0);

        egui_extras::StripBuilder::new(ui)
            .size(egui_extras::Size::exact(left_w))
            .size(egui_extras::Size::remainder())
            .size(egui_extras::Size::exact(right_w))
            .horizontal(|mut strip| {
                strip.cell(|ui| {
                    state.render_safe(ui, "left_rail", |app, ui| app.render_left_rail(ui));
                });
                strip.cell(|ui| {
                    self.render_main_stage(state, ui, ctx);
                });
                strip.cell(|ui| {
                    state.render_safe(ui, "right_rail", |app, ui| app.render_right_rail(ui));
                });
            });
    }

    /// Render the active sub-application into the main stage cell.
    ///
    /// Includes the route-change slide transition. When a transition is active
    /// the outgoing view is translated horizontally and clipped to the main
    /// stage rectangle.
    fn render_main_stage(&mut self, state: &mut App, ui: &mut egui::Ui, ctx: &egui::Context) {
        // The StripBuilder cell already gives us the exact rectangle of the
        // central column in logical points. Use it to split chat and input; do
        // not rely on `ui.available_height()` which is infinite here.
        let main_rect = ui.max_rect();
        let (offset, finish_transition) = if let Some(trans) = state.main_stage_transition.as_ref()
        {
            let elapsed = trans.started.elapsed().as_secs_f32();
            let progress = (elapsed / trans.duration.as_secs_f32()).min(1.0);
            if progress >= 1.0 {
                (0.0, true)
            } else {
                let t = 1.0 - crate::animation::ease_out_cubic(progress);
                (t * main_rect.width() * trans.direction, false)
            }
        } else {
            (0.0, false)
        };

        let render_view =
            |app_state: &mut App, ui: &mut egui::Ui, view: AppView| -> ClarityAppResponse {
                // P1d: dispatch through `ClarityAppEnum`. The active app is
                // temporarily moved out and restored after render so the
                // `ClarityAppContext::state` (`&mut dyn AppState` backed by
                // `App`) and the app being rendered do not alias.
                //
                // ponytail: `std::mem::take` (via `take_*`) is kept as a
                // transition because `ChatRenderer::render_chat` still needs
                // `&mut App` alongside `&mut ChatApp`. Once chat rendering is
                // fully inside `clarity-apps` or receives a detached host-state
                // struct, the render path can borrow the enum variant in place
                // and these take helpers can be removed.
                let idx = match view {
                    AppView::Chat => 0,
                    AppView::Settings => 1,
                    AppView::Dashboard => 2,
                };
                // Move the active variant out first so `app_state` is free to
                // be borrowed as `&mut dyn AppState` while the app renders.
                let mut app = match view {
                    AppView::Chat => {
                        clarity_apps::ClarityAppEnum::Chat(app_state.apps[idx].take_chat())
                    }
                    AppView::Settings => {
                        clarity_apps::ClarityAppEnum::Settings(app_state.apps[idx].take_settings())
                    }
                    AppView::Dashboard => clarity_apps::ClarityAppEnum::Dashboard(
                        app_state.apps[idx].take_dashboard(),
                    ),
                };
                let mut theme = app_state.context.ui_store.theme.clone();
                let mut app_ctx = ClarityAppContext {
                    theme: &mut theme,
                    app_name: "Clarity",
                    app_version: env!("CARGO_PKG_VERSION"),
                    app_description: env!("CARGO_PKG_DESCRIPTION"),
                    app_license: "AGPL-3.0-or-later",
                    state: app_state as &mut dyn clarity_shell::AppState,
                };
                let response = app.render(&mut app_ctx, ui, ctx);
                // Restore the rendered app before returning.
                app_state.apps[idx] = app;
                // Mirror the theme back in case a sub-app mutated it.
                app_state.context.ui_store.theme = theme;
                response
            };

        state.render_safe(ui, "main_stage", |app_state, ui| {
            if finish_transition {
                app_state.main_stage_transition = None;
            }

            let view = *app_state.current_main();
            let show_input = matches!(view, AppView::Chat);

            // P6h: split the main-stage cell vertically. The chat/app view
            // occupies the remaining space above; the input composer sits in
            // a bottom strip constrained to the chat column width. This keeps
            // the composer from visually/physically spanning the left/right
            // rails and lets the message list ScrollArea own its own region.
            let chat_response = if show_input {
                // P6i-hotfix: the StripBuilder remainder cell reports an infinite
                // available_height, so top-down allocation cannot split the chat
                // view from the input composer. Carve the main-stage rectangle
                // into two fixed child UIs: the conversation above and the
                // composer at the bottom. Both use viewport coordinates so they
                // align exactly with the central column and leave the side rails
                // untouched.
                let input_h = crate::panels::chat::input::estimate_height(app_state);
                let (chat_rect, input_rect) = split_main_stage(main_rect, input_h);
                // Render the conversation into a child UI clipped to the chat
                // rectangle. The child UI does not advance the parent cursor.
                let mut chat_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(chat_rect)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                );
                chat_ui.set_clip_rect(chat_rect);
                let response = render_main_stage_view(
                    app_state,
                    &mut chat_ui,
                    ctx,
                    &render_view,
                    main_rect,
                    offset,
                    view,
                );
                // Reserve the chat rectangle so the parent layout accounts for
                // the space we carved out.
                let _slot = ui.allocate_space(chat_rect.size());

                // Render the input composer into a second child UI clipped to
                // the bottom strip. Using a child UI keeps the composer inside
                // the main-stage column instead of floating over the side rails.
                let mut input_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(input_rect)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                );
                input_ui.set_clip_rect(input_rect);
                crate::panels::chat::render_input_panel(app_state, &mut input_ui);

                response
            } else {
                render_main_stage_view(app_state, ui, ctx, &render_view, main_rect, offset, view)
            };

            match chat_response {
                ClarityAppResponse::Navigate(route) => app_state.navigate(route),
                ClarityAppResponse::FocusChatInput => {
                    app_state.context.ui_store.focus_target =
                        Some(crate::stores::FocusTarget::ChatInput);
                }
                _ => {}
            }
        });
    }
}

/// Render the active view into the main-stage cell, applying the route-change
/// slide transition when `offset` is non-zero.
fn render_main_stage_view(
    app_state: &mut App,
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    render_view: &dyn Fn(&mut App, &mut egui::Ui, AppView) -> ClarityAppResponse,
    main_rect: egui::Rect,
    offset: f32,
    view: AppView,
) -> ClarityAppResponse {
    if offset != 0.0 {
        ctx.request_repaint();
        egui::Area::new(egui::Id::new("main_stage_transition"))
            .fixed_pos(main_rect.min + egui::vec2(offset, 0.0))
            .show(ctx, |ui| {
                ui.set_clip_rect(main_rect);
                render_view(app_state, ui, view)
            })
            .inner
    } else {
        render_view(app_state, ui, view)
    }
}

/// Split the main-stage rectangle into a chat area above and an input strip
/// below. Guarantees the two rectangles are non-overlapping, fill `main_rect`,
/// and never extend above it. Input height is clamped to the main-stage height.
fn split_main_stage(main_rect: egui::Rect, input_h: f32) -> (egui::Rect, egui::Rect) {
    let input_h = input_h.max(0.0).min(main_rect.height());
    let chat_rect = egui::Rect::from_min_max(
        main_rect.min,
        egui::pos2(main_rect.max.x, main_rect.max.y - input_h),
    );
    let input_rect =
        egui::Rect::from_min_max(egui::pos2(main_rect.min.x, chat_rect.max.y), main_rect.max);
    (chat_rect, input_rect)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(min: [f32; 2], max: [f32; 2]) -> egui::Rect {
        egui::Rect::from_min_max(egui::pos2(min[0], min[1]), egui::pos2(max[0], max[1]))
    }

    #[test]
    fn split_main_stage_partitions_rect() {
        let main = rect([0.0, 0.0], [1000.0, 800.0]);
        let (chat, input) = split_main_stage(main, 140.0);
        assert_eq!(chat.min, main.min);
        assert_eq!(input.max, main.max);
        assert_eq!(chat.max.y, input.min.y);
        assert!(
            chat.max.y <= input.min.y,
            "chat and input should meet at the boundary"
        );
    }

    #[test]
    fn split_main_stage_clamps_negative_input_height() {
        let main = rect([10.0, 20.0], [200.0, 300.0]);
        let (chat, input) = split_main_stage(main, -50.0);
        assert_eq!(chat, main);
        assert_eq!(input.height(), 0.0);
        assert_eq!(input.min.y, main.max.y);
    }

    #[test]
    fn split_main_stage_clamps_oversized_input_height() {
        let main = rect([0.0, 0.0], [100.0, 100.0]);
        let (chat, input) = split_main_stage(main, 200.0);
        assert_eq!(chat.height(), 0.0);
        assert_eq!(input, main);
    }
}
