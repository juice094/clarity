use crate::ui;
use crate::ui::types::{AgentStatus, Role, UiEvent};
use crate::App;

fn format_thousands(n: u32) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result
}

pub fn render_chat_area(app: &mut App, ctx: &egui::Context) {
    egui::CentralPanel::default()
        .frame(egui::Frame::central_panel(&ctx.style()).fill(app.ui_store.theme.bg))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                if app.ui_store.sidebar_collapsed
                    && ui
                        .add(
                            egui::Button::new(egui::RichText::new("➡").size(14.0))
                                .fill(egui::Color32::TRANSPARENT)
                                .corner_radius(egui::CornerRadius::same(app.ui_store.theme.radius_sm as u8)),
                        )
                        .clicked()
                {
                    app.ui_store.sidebar_collapsed = false;
                }
                // ── Category instance tabs (hidden for emotion) ──
                if app.session_store.active_category != "emotion" {
                    let category_sessions: Vec<(String, String, bool)> = app
                        .session_store.sessions
                        .iter()
                        .filter(|s| s.category == app.session_store.active_category)
                        .map(|s| (s.id.clone(), s.title.clone(), s.id == app.session_store.active_session_id))
                        .collect();
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        for (id, title, is_active) in &category_sessions {
                            let bg = if *is_active {
                                app.ui_store.theme.surface
                            } else {
                                app.ui_store.theme.bg_elevated
                            };
                            let text_color = if *is_active {
                                app.ui_store.theme.text_strong
                            } else {
                                app.ui_store.theme.text_dim
                            };
                            let stroke = if *is_active {
                                egui::Stroke::new(1.5, app.ui_store.theme.accent)
                            } else {
                                egui::Stroke::new(1.0, app.ui_store.theme.border)
                            };
                            let tab_id = id.clone();
                            if ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new(title).size(12.0).color(text_color),
                                    )
                                    .fill(bg)
                                    .corner_radius(egui::CornerRadius::same(app.ui_store.theme.radius_sm as u8))
                                    .stroke(stroke)
                                    .min_size(egui::vec2(60.0, 28.0)),
                                )
                                .clicked()
                            {
                                app.save_current_session();
                                let old_id = app.session_store.active_session_id.clone();
                                if !app.chat_store.input.trim().is_empty() {
                                    app.session_store.drafts.insert(old_id, app.chat_store.input.clone());
                                } else {
                                    app.session_store.drafts.remove(&old_id);
                                }
                                app.session_store.active_session_id = tab_id.clone();
                                app.chat_store.input = app.session_store.drafts.remove(&tab_id).unwrap_or_default();
                            }
                        }
                        // New-tab button (browser style)
                        if ui
                            .add(
                                egui::Button::new(egui::RichText::new("+").size(14.0))
                                    .fill(egui::Color32::TRANSPARENT)
                                    .corner_radius(egui::CornerRadius::same(app.ui_store.theme.radius_sm as u8)),
                            )
                            .clicked()
                        {
                            app.new_session();
                        }
                    });
                } else {
                    // Emotion: show a static title instead of tabs
                    ui.label(
                        egui::RichText::new("情感")
                            .size(16.0)
                            .strong()
                            .color(app.ui_store.theme.text),
                    );
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    // Settings
                    if ui
                        .add(
                            egui::Button::new(egui::RichText::new("⚙").size(14.0))
                                .fill(egui::Color32::TRANSPARENT)
                                .corner_radius(egui::CornerRadius::same(app.ui_store.theme.radius_sm as u8)),
                        )
                        .clicked()
                    {
                        app.settings_store.settings_open = true;
                        app.settings_store.settings_edit = {
                            let guard = app.state.cached_settings.lock();
                            guard.clone()
                        };
                    }
                    // Tasks
                    let active_tasks = app.task_store.tasks.iter().filter(|t| !t.status.is_terminal()).count();
                    let task_btn = if active_tasks > 0 {
                        format!("📝 {}", active_tasks)
                    } else {
                        "📝".to_string()
                    };
                    if ui
                        .add(
                            egui::Button::new(egui::RichText::new(&task_btn).size(12.0))
                                .fill(egui::Color32::TRANSPARENT)
                                .corner_radius(egui::CornerRadius::same(app.ui_store.theme.radius_sm as u8)),
                        )
                        .clicked()
                    {
                        app.task_store.task_panel_open = !app.task_store.task_panel_open;
                        if app.task_store.task_panel_open {
                            app.refresh_tasks();
                        }
                    }
                    // MCP
                    let mcp_count = app.mcp_store.mcp_config.as_ref().map_or(0, |c| c.servers.len());
                    let mcp_btn = if mcp_count > 0 {
                        format!("🔌 {}", mcp_count)
                    } else {
                        "🔌".to_string()
                    };
                    if ui
                        .add(
                            egui::Button::new(egui::RichText::new(&mcp_btn).size(12.0))
                                .fill(egui::Color32::TRANSPARENT)
                                .corner_radius(egui::CornerRadius::same(app.ui_store.theme.radius_sm as u8)),
                        )
                        .clicked()
                    {
                        app.mcp_store.mcp_panel_open = !app.mcp_store.mcp_panel_open;
                    }
                    // Status
                    let (status_color, status_label) = match app.chat_store.agent_status {
                        AgentStatus::Online => (app.ui_store.theme.status_online, "Online"),
                        AgentStatus::Busy => (app.ui_store.theme.status_busy, "Busy"),
                        AgentStatus::Unconfigured => (app.ui_store.theme.status_offline, "Unconfigured"),
                        AgentStatus::Offline => (app.ui_store.theme.status_offline, "Offline"),
                    };
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
                    ui.painter().circle_filled(rect.center(), 4.0, status_color);
                    ui.label(
                        egui::RichText::new(status_label)
                            .size(12.0)
                            .color(app.ui_store.theme.text_dim),
                    );
                    // Token usage (session cumulative)
                    if let Some((p, c, t)) = app.chat_store.last_usage {
                        ui.label(
                            egui::RichText::new(format!(
                                "Session: {}↑ {}↓ {}∑",
                                format_thousands(p),
                                format_thousands(c),
                                format_thousands(t)
                            ))
                            .size(11.0)
                            .color(app.ui_store.theme.text_dim)
                            .monospace(),
                        );
                    }
                });
            });
            ui.add_space(app.ui_store.theme.space_4);
            ui.separator();

            let banner_text = app.ui_store.network_banner.clone();
            if let Some(banner) = banner_text {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(&banner)
                            .size(12.0)
                            .color(app.ui_store.theme.status_busy),
                    );
                    if ui.button("×").clicked() {
                        app.ui_store.network_banner = None;
                    }
                });
                ui.separator();
            }

            if app.chat_store.compacting {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Compacting conversation history…")
                            .size(12.0)
                            .color(app.ui_store.theme.text_dim),
                    );
                });
                ui.separator();
            }

            let available_height = ui.available_height() - 70.0;
            let is_loading = app.chat_store.is_loading;
            let theme = app.ui_store.theme.clone();
            let active_id = app.session_store.active_session_id.clone();
            let tool_calls = app.chat_store.tool_calls.clone();
            let scroll_y = app.ui_store.last_scroll_offset;
            let mut configure_clicked = false;

            let mut scroll_up = false;
            let output = egui::ScrollArea::vertical()
                .id_salt("chat_scroll")
                .stick_to_bottom(app.chat_store.stick_to_bottom)
                .auto_shrink([false; 2])
                .max_height(available_height)
                .show(ui, |ui| {
                    if let Some(session) = app.session_store.sessions.iter_mut().find(|s| s.id == active_id) {
                        if session.messages.is_empty() && !is_loading {
                            ui.vertical_centered(|ui| {
                                ui.add_space(120.0);
                                ui.label(
                                    egui::RichText::new("Clarity")
                                        .size(32.0)
                                        .strong()
                                        .color(theme.text_dim),
                                );
                                ui.add_space(app.ui_store.theme.space_8);
                                ui.label(
                                    egui::RichText::new("Local-first AI agent runtime")
                                        .size(14.0)
                                        .color(theme.text_dim),
                                );
                                ui.add_space(app.ui_store.theme.space_24);
                                if ui
                                    .add(
                                        egui::Button::new(
                                            egui::RichText::new("Configure Settings")
                                                .size(13.0)
                                                .color(theme.text),
                                        )
                                        .fill(theme.surface)
                                        .corner_radius(egui::CornerRadius::same(
                                            theme.radius_sm as u8,
                                        ))
                                        .min_size(egui::vec2(180.0, 40.0)),
                                    )
                                    .clicked()
                                {
                                    configure_clicked = true;
                                }
                            });
                        } else {
                            // --- Virtualized message list ---
                            let estimates: Vec<f32> = session
                                .messages
                                .iter()
                                .map(|m| {
                                    m.cached_height
                                        .unwrap_or_else(|| ui::render::estimate_height(m))
                                })
                                .collect();

                            let mut cumulative = 0.0;
                            let mut start_idx = 0;
                            let mut end_idx = session.messages.len();

                            for (i, h) in estimates.iter().enumerate() {
                                if cumulative + h >= scroll_y && start_idx == 0 {
                                    start_idx = i.saturating_sub(3);
                                }
                                cumulative += h;
                                if cumulative >= scroll_y + available_height
                                    && end_idx == session.messages.len()
                                {
                                    end_idx = (i + 3).min(session.messages.len());
                                    break;
                                }
                            }

                            if start_idx > 0 {
                                let top = estimates[..start_idx].iter().sum::<f32>();
                                ui.allocate_space(egui::vec2(ui.available_width(), top));
                            }

                            for i in start_idx..end_idx {
                                let actual =
                                    ui::render::message_bubble(ui, &session.messages[i], &theme);
                                session.messages[i].cached_height = Some(actual);
                            }

                            if end_idx < session.messages.len() {
                                let bottom = estimates[end_idx..].iter().sum::<f32>();
                                ui.allocate_space(egui::vec2(ui.available_width(), bottom));
                            }

                            // Tool calls & typing indicator (few items, always rendered)
                            for tc in &tool_calls {
                                ui::render::tool_call_bubble(ui, tc, &theme);
                            }
                            if is_loading
                                && session.messages.last().is_none_or(|m| m.role == Role::User)
                                && tool_calls.is_empty()
                            {
                                ui::render::typing_indicator(ui, &theme);
                            }
                        }
                    }
                    // Detect user scroll-up intent to release stick-to-bottom.
                    ui.input(|i| {
                        for event in &i.events {
                            if let egui::Event::MouseWheel { delta, .. } = event {
                                if delta.y > 0.0 {
                                    scroll_up = true;
                                }
                            }
                        }
                    });
                });

            if scroll_up {
                app.chat_store.stick_to_bottom = false;
            }
            app.ui_store.last_scroll_offset = output.state.offset.y;
            if configure_clicked {
                app.settings_store.settings_open = true;
                app.settings_store.settings_edit = {
                    let guard = app.state.cached_settings.lock();
                    guard.clone()
                };
            }

            ui.separator();

            // Plan review card above input bar
            if let Some(ref plan) = app.chat_store.pending_plan {
                let mut execute = false;
                let mut cancel = false;
                egui::Frame::group(ui.style())
                    .fill(app.ui_store.theme.surface)
                    .corner_radius(egui::CornerRadius::same(app.ui_store.theme.radius_md as u8))
                    .stroke(egui::Stroke::new(1.0, app.ui_store.theme.accent))
                    .inner_margin(egui::Margin::same(10))
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.label(
                            egui::RichText::new(format!("📋 Plan Review: {}", plan.title))
                                .size(13.0)
                                .strong()
                                .color(app.ui_store.theme.text),
                        );
                        ui.add_space(app.ui_store.theme.space_8);
                        for step in &plan.steps {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(format!("{}.", step.id))
                                        .size(11.0)
                                        .strong()
                                        .color(app.ui_store.theme.text),
                                );
                                ui.label(
                                    egui::RichText::new(&step.description)
                                        .size(11.0)
                                        .color(app.ui_store.theme.text),
                                );
                            });
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new("→")
                                        .size(10.0)
                                        .color(app.ui_store.theme.text_dim),
                                );
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{}({})",
                                        step.tool_name, step.tool_params
                                    ))
                                    .size(10.0)
                                    .color(app.ui_store.theme.text_dim)
                                    .monospace(),
                                );
                            });
                            ui.add_space(2.0);
                        }
                        ui.add_space(app.ui_store.theme.space_8);
                        ui.horizontal(|ui| {
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui
                                        .add_sized(
                                            egui::vec2(80.0, 32.0),
                                            egui::Button::new(
                                                egui::RichText::new("Cancel")
                                                    .size(12.0)
                                                    .color(app.ui_store.theme.text),
                                            )
                                            .fill(app.ui_store.theme.border),
                                        )
                                        .clicked()
                                    {
                                        cancel = true;
                                    }
                                    if ui
                                        .add_sized(
                                            egui::vec2(80.0, 32.0),
                                            egui::Button::new(
                                                egui::RichText::new("Execute")
                                                    .size(12.0)
                                                    .color(app.ui_store.theme.text),
                                            )
                                            .fill(app.ui_store.theme.accent),
                                        )
                                        .clicked()
                                    {
                                        execute = true;
                                    }
                                },
                            );
                        });
                    });
                if execute {
                    let plan = app.chat_store.pending_plan.take().unwrap();
                    // Initialize live execution tracker.
                    app.chat_store.plan_tracker = Some(crate::ui::types::PlanExecutionTracker {
                        title: plan.title.clone(),
                        steps: plan
                            .steps
                            .iter()
                            .map(|s| crate::ui::types::PlanStepTracker {
                                id: s.id.clone(),
                                description: s.description.clone(),
                                tool_name: s.tool_name.clone(),
                                status: crate::ui::types::PlanStepStatus::Pending,
                            })
                            .collect(),
                    });
                    let state = app.state.clone();
                    let tx = app.ui_tx.clone();
                    app.chat_store.is_loading = true;
                    app.chat_store.agent_status = AgentStatus::Busy;
                    app.runtime.spawn(async move {
                        match state.agent.execute_plan(&plan).await {
                            Ok(results) => {
                                let mut text = String::new();
                                for r in &results {
                                    text.push_str(&format!(
                                        "**Step {}**: {}\n```\n{}\n```\n\n",
                                        r.step_id,
                                        if r.success { "✅" } else { "❌" },
                                        r.output
                                    ));
                                }
                                if let Err(e) = tx.send(UiEvent::Chunk(text)) {
                                    tracing::warn!("Failed to send plan results: {}", e);
                                }
                            }
                            Err(e) => {
                                if let Err(err) =
                                    tx.send(UiEvent::Error(format!("Plan execution failed: {}", e)))
                                {
                                    tracing::warn!("Failed to send Error: {}", err);
                                }
                            }
                        }
                        if let Err(e) = tx.send(UiEvent::Done) {
                            tracing::warn!("Failed to send Done: {}", e);
                        }
                    });
                } else if cancel {
                    app.chat_store.pending_plan = None;
                }
                ui.separator();
            }

            // Plan execution tracker panel
            if let Some(ref tracker) = app.chat_store.plan_tracker {
                let mut dismiss = false;
                egui::Frame::group(ui.style())
                    .fill(app.ui_store.theme.surface)
                    .corner_radius(egui::CornerRadius::same(app.ui_store.theme.radius_md as u8))
                    .stroke(egui::Stroke::new(1.0, app.ui_store.theme.accent))
                    .inner_margin(egui::Margin::same(10))
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(format!("📋 {}", tracker.title))
                                    .size(13.0)
                                    .strong()
                                    .color(app.ui_store.theme.text),
                            );
                            if ui
                                .button(
                                    egui::RichText::new("✕")
                                        .size(12.0)
                                        .color(app.ui_store.theme.text_dim),
                                )
                                .clicked()
                            {
                                dismiss = true;
                            }
                        });
                        ui.add_space(app.ui_store.theme.space_8);
                        for step in &tracker.steps {
                            let (icon, color) = match step.status {
                                crate::ui::types::PlanStepStatus::Pending => {
                                    ("⏳", app.ui_store.theme.text_dim)
                                }
                                crate::ui::types::PlanStepStatus::Running => {
                                    ("▶️", app.ui_store.theme.accent)
                                }
                                crate::ui::types::PlanStepStatus::Success => ("✅", app.ui_store.theme.ok),
                                crate::ui::types::PlanStepStatus::Failed => {
                                    ("❌", app.ui_store.theme.danger)
                                }
                            };
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(icon).size(12.0));
                                ui.label(
                                    egui::RichText::new(format!("{}.", step.id))
                                        .size(11.0)
                                        .strong()
                                        .color(app.ui_store.theme.text),
                                );
                                ui.label(
                                    egui::RichText::new(&step.description)
                                        .size(11.0)
                                        .color(app.ui_store.theme.text),
                                );
                            });
                            ui.horizontal(|ui| {
                                ui.add_space(app.ui_store.theme.space_20);
                                ui.label(
                                    egui::RichText::new(format!("→ {}", step.tool_name))
                                        .size(10.0)
                                        .color(color)
                                        .monospace(),
                                );
                            });
                            ui.add_space(2.0);
                        }
                    });
                if dismiss {
                    app.chat_store.plan_tracker = None;
                }
                ui.separator();
            }

            // Attachment chips above input bar
            if !app.chat_store.attachments.is_empty() {
                let mut to_remove: Option<usize> = None;
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        egui::RichText::new("Attachments:")
                            .size(11.0)
                            .color(app.ui_store.theme.text_dim),
                    );
                    for (i, att) in app.chat_store.attachments.iter().enumerate() {
                        egui::Frame::group(ui.style())
                            .fill(app.ui_store.theme.surface)
                            .corner_radius(egui::CornerRadius::same(app.ui_store.theme.radius_full as u8))
                            .stroke(egui::Stroke::new(1.0, app.ui_store.theme.border))
                            .inner_margin(egui::Margin::symmetric(8, 4))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new("📎").size(11.0));
                                    ui.label(
                                        egui::RichText::new(&att.name)
                                            .size(11.0)
                                            .color(app.ui_store.theme.text)
                                            .monospace(),
                                    );
                                    if ui.small_button("×").clicked() {
                                        to_remove = Some(i);
                                    }
                                });
                            });
                    }
                });
                if let Some(i) = to_remove {
                    app.chat_store.attachments.remove(i);
                }
                ui.separator();
            }

            // Input bar card
            egui::Frame::group(ui.style())
                .fill(app.ui_store.theme.input_bg)
                .corner_radius(egui::CornerRadius::same(app.ui_store.theme.radius_lg as u8))
                .stroke(egui::Stroke::new(1.0, app.ui_store.theme.border))
                .inner_margin(egui::Margin::same(6))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        let available_width = ui.available_width();
                        let btn_area_width = if app.chat_store.is_loading { 100.0 } else { 52.0 };
                        let input_width = available_width - btn_area_width;
                        ui.allocate_ui_with_layout(
                            egui::vec2(input_width, 44.0),
                            egui::Layout::top_down(egui::Align::LEFT),
                            |ui| {
                                let hint = if app.chat_store.pending_send.is_some() {
                                    "Steer message queued — will send when current response stops..."
                                } else if !app.chat_store.attachments.is_empty() {
                                    "Type a message (files attached)..."
                                } else {
                                    "Type a message..."
                                };
                                let prev_input = app.chat_store.input.clone();
                                let line_count = app.chat_store.input.matches('\n').count() + 1;
                                let input_height =
                                    (line_count as f32 * 20.0 + 24.0).clamp(44.0, 120.0);
                                let text_edit = egui::TextEdit::multiline(&mut app.chat_store.input)
                                    .desired_rows(line_count.max(1))
                                    .hint_text(hint)
                                    .margin(egui::vec2(8.0, 8.0));
                                ui.add_sized(egui::vec2(input_width, input_height), text_edit);

                                // Track input modifications for IME suppression heuristic.
                                if app.chat_store.input != prev_input {
                                    app.ui_store.last_input_modified = std::time::Instant::now();
                                }

                                // Enter sends; Shift+Enter inserts newline.
                                // IME safeguard: detect IMECommit event instead of
                                // relying on a fragile time threshold.
                                let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));
                                let ime_commit = ui.input(|i| {
                                    i.events.iter().any(|e| matches!(e, egui::Event::Ime(egui::ImeEvent::Commit(_))))
                                });
                                if enter_pressed && !ui.input(|i| i.modifiers.shift) && !ime_commit {
                                    while app.chat_store.input.ends_with('\n') {
                                        app.chat_store.input.pop();
                                    }
                                    if app.chat_store.input == prev_input
                                        && !app.chat_store.input.trim().is_empty()
                                    {
                                        app.chat_store.stick_to_bottom = true;
                                        app.send();
                                    }
                                }
                            },
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if app.chat_store.is_loading {
                                // Queue-send button (rightmost) — enabled only if input has content.
                                let can_queue =
                                    !app.chat_store.input.trim().is_empty() || !app.chat_store.attachments.is_empty();
                                let queue_color = if can_queue {
                                    app.ui_store.theme.accent
                                } else {
                                    app.ui_store.theme.bg_elevated
                                };
                                let queue_text = if can_queue {
                                    app.ui_store.theme.text
                                } else {
                                    app.ui_store.theme.text_dim
                                };
                                let queue_btn = ui.add_sized(
                                    egui::vec2(44.0, 44.0),
                                    egui::Button::new(
                                        egui::RichText::new("▶").size(16.0).color(queue_text),
                                    )
                                    .fill(queue_color)
                                    .corner_radius(
                                        egui::CornerRadius::same(app.ui_store.theme.radius_full as u8),
                                    ),
                                );
                                if queue_btn.clicked() && can_queue {
                                    app.chat_store.stick_to_bottom = true;
                                    app.send();
                                }
                                if can_queue {
                                    queue_btn.on_hover_text(
                                        "Steer — cancel current response and send immediately",
                                    );
                                } else {
                                    queue_btn.on_hover_text("Type a message to steer");
                                }

                                // Stop button (left of queue).
                                let stop_btn = ui.add_sized(
                                    egui::vec2(44.0, 44.0),
                                    egui::Button::new(
                                        egui::RichText::new("■").size(16.0).color(app.ui_store.theme.text),
                                    )
                                    .fill(app.ui_store.theme.danger)
                                    .corner_radius(
                                        egui::CornerRadius::same(app.ui_store.theme.radius_full as u8),
                                    ),
                                );
                                if stop_btn.clicked() {
                                    app.stop();
                                }
                                stop_btn.on_hover_text("Stop generating (Ctrl+C)");
                            } else {
                                // Send button.
                                let btn = ui.add_sized(
                                    egui::vec2(44.0, 44.0),
                                    egui::Button::new(
                                        egui::RichText::new("▶").size(16.0).color(app.ui_store.theme.text),
                                    )
                                    .fill(app.ui_store.theme.accent)
                                    .corner_radius(
                                        egui::CornerRadius::same(app.ui_store.theme.radius_full as u8),
                                    ),
                                );
                                if btn.clicked() {
                                    app.chat_store.stick_to_bottom = true;
                                    app.send();
                                }
                                btn.on_hover_text("Send message");
                            }
                        });
                    });
                });
        });
}
