use crate::App;
use crate::ui;
use crate::ui::types::{AgentStatus, Role, UiEvent};

pub fn render_chat_area(app: &mut App, ctx: &egui::Context) {
    egui::CentralPanel::default()
        .frame(egui::Frame::central_panel(&ctx.style()).fill(app.theme.bg))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                if app.sidebar_collapsed
                    && ui.add(egui::Button::new(egui::RichText::new("➡").size(14.0)).fill(egui::Color32::TRANSPARENT).corner_radius(egui::CornerRadius::same(app.theme.radius_sm as u8))).clicked()
                {
                    app.sidebar_collapsed = false;
                }
                ui.label(egui::RichText::new("Chat").size(16.0).strong().color(app.theme.text));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    // Settings
                    if ui.add(egui::Button::new(egui::RichText::new("⚙").size(14.0)).fill(egui::Color32::TRANSPARENT).corner_radius(egui::CornerRadius::same(app.theme.radius_sm as u8))).clicked() {
                        app.settings_open = true;
                        app.settings_edit = {
                            let guard = app.state.cached_settings.lock();
                            guard.clone()
                        };
                    }
                    // Tasks
                    let active_tasks = app.tasks.iter().filter(|t| !t.status.is_terminal()).count();
                    let task_btn = if active_tasks > 0 { format!("📝 {}", active_tasks) } else { "📝".to_string() };
                    if ui.add(egui::Button::new(egui::RichText::new(&task_btn).size(12.0)).fill(egui::Color32::TRANSPARENT).corner_radius(egui::CornerRadius::same(app.theme.radius_sm as u8))).clicked() {
                        app.task_panel_open = !app.task_panel_open;
                        if app.task_panel_open { app.refresh_tasks(); }
                    }
                    // MCP
                    let mcp_count = app.mcp_config.as_ref().map_or(0, |c| c.servers.len());
                    let mcp_btn = if mcp_count > 0 { format!("🔌 {}", mcp_count) } else { "🔌".to_string() };
                    if ui.add(egui::Button::new(egui::RichText::new(&mcp_btn).size(12.0)).fill(egui::Color32::TRANSPARENT).corner_radius(egui::CornerRadius::same(app.theme.radius_sm as u8))).clicked() {
                        app.mcp_panel_open = !app.mcp_panel_open;
                    }
                    // Status
                    let (status_color, status_label) = match app.agent_status {
                        AgentStatus::Online => (app.theme.status_online, "Online"),
                        AgentStatus::Busy => (app.theme.status_busy, "Busy"),
                        AgentStatus::Unconfigured => (app.theme.status_offline, "Unconfigured"),
                        AgentStatus::Offline => (app.theme.status_offline, "Offline"),
                    };
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
                    ui.painter().circle_filled(rect.center(), 4.0, status_color);
                    ui.label(egui::RichText::new(status_label).size(12.0).color(app.theme.text_dim));
                    // Token usage
                    if let Some((p, c, t)) = app.last_usage {
                        ui.label(egui::RichText::new(format!("Tokens: {}↑ {}↓ {}∑", p, c, t)).size(11.0).color(app.theme.text_dim).monospace());
                    }
                });
            });
            ui.add_space(4.0);
            ui.separator();

            let banner_text = app.network_banner.clone();
            if let Some(banner) = banner_text {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(&banner).size(12.0).color(app.theme.status_busy));
                    if ui.button("×").clicked() { app.network_banner = None; }
                });
                ui.separator();
            }

            if app.compacting {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Compacting conversation history…").size(12.0).color(app.theme.text_dim));
                });
                ui.separator();
            }

            let available_height = ui.available_height() - 70.0;
            let is_loading = app.is_loading;
            let theme = app.theme.clone();
            let active_id = app.active_session_id.clone();
            let tool_calls = app.tool_calls.clone();
            let scroll_y = app.last_scroll_offset;
            let mut configure_clicked = false;

            let output = egui::ScrollArea::vertical()
                .id_salt("chat_scroll")
                .stick_to_bottom(true)
                .auto_shrink([false; 2])
                .max_height(available_height)
                .show(ui, |ui| {
                    if let Some(session) = app.sessions.iter_mut().find(|s| s.id == active_id) {
                        if session.messages.is_empty() && !is_loading {
                            ui.vertical_centered(|ui| {
                                ui.add_space(120.0);
                                ui.label(egui::RichText::new("Clarity").size(32.0).strong().color(theme.text_dim));
                                ui.add_space(8.0);
                                ui.label(egui::RichText::new("Local-first AI agent runtime").size(14.0).color(theme.text_dim));
                                ui.add_space(24.0);
                                if ui.add(egui::Button::new(egui::RichText::new("Configure Settings").size(13.0).color(theme.text))
                                    .fill(theme.surface).corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
                                    .min_size(egui::vec2(180.0, 40.0))).clicked() { configure_clicked = true; }
                            });
                        } else {
                            // --- Virtualized message list ---
                            let estimates: Vec<f32> = session.messages.iter()
                                .map(|m| m.cached_height.unwrap_or_else(|| ui::render::estimate_height(m)))
                                .collect();

                            let mut cumulative = 0.0;
                            let mut start_idx = 0;
                            let mut end_idx = session.messages.len();

                            for (i, h) in estimates.iter().enumerate() {
                                if cumulative + h >= scroll_y && start_idx == 0 {
                                    start_idx = i.saturating_sub(3);
                                }
                                cumulative += h;
                                if cumulative >= scroll_y + available_height && end_idx == session.messages.len() {
                                    end_idx = (i + 3).min(session.messages.len());
                                    break;
                                }
                            }

                            if start_idx > 0 {
                                let top = estimates[..start_idx].iter().sum::<f32>();
                                ui.allocate_space(egui::vec2(ui.available_width(), top));
                            }

                            for i in start_idx..end_idx {
                                let actual = ui::render::message_bubble(ui, &session.messages[i], &theme);
                                session.messages[i].cached_height = Some(actual);
                            }

                            if end_idx < session.messages.len() {
                                let bottom = estimates[end_idx..].iter().sum::<f32>();
                                ui.allocate_space(egui::vec2(ui.available_width(), bottom));
                            }

                            // Tool calls & typing indicator (few items, always rendered)
                            for tc in &tool_calls { ui::render::tool_call_bubble(ui, tc, &theme); }
                            if is_loading && session.messages.last().is_none_or(|m| m.role == Role::User) && tool_calls.is_empty() {
                                ui::render::typing_indicator(ui, &theme);
                            }
                        }
                    }
                });

            app.last_scroll_offset = output.state.offset.y;
            if configure_clicked {
                app.settings_open = true;
                app.settings_edit = {
                    let guard = app.state.cached_settings.lock();
                    guard.clone()
                };
            }

            ui.separator();

            // Plan review card above input bar
            if let Some(ref plan) = app.pending_plan {
                let mut execute = false;
                let mut cancel = false;
                egui::Frame::group(ui.style())
                    .fill(app.theme.surface)
                    .corner_radius(egui::CornerRadius::same(app.theme.radius_md as u8))
                    .stroke(egui::Stroke::new(1.0, app.theme.accent))
                    .inner_margin(egui::Margin::same(10))
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.label(egui::RichText::new(format!("📋 Plan Review: {}", plan.title)).size(13.0).strong().color(app.theme.text));
                        ui.add_space(6.0);
                        for step in &plan.steps {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(format!("{}.", step.id)).size(11.0).strong().color(app.theme.text));
                                ui.label(egui::RichText::new(&step.description).size(11.0).color(app.theme.text));
                            });
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("→").size(10.0).color(app.theme.text_dim));
                                ui.label(egui::RichText::new(format!("{}({})", step.tool_name, step.tool_params)).size(10.0).color(app.theme.text_dim).monospace());
                            });
                            ui.add_space(2.0);
                        }
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.add_sized(egui::vec2(80.0, 32.0), egui::Button::new(egui::RichText::new("Cancel").size(12.0).color(app.theme.text)).fill(app.theme.border)).clicked() {
                                    cancel = true;
                                }
                                if ui.add_sized(egui::vec2(80.0, 32.0), egui::Button::new(egui::RichText::new("Execute").size(12.0).color(app.theme.text)).fill(app.theme.accent)).clicked() {
                                    execute = true;
                                }
                            });
                        });
                    });
                if execute {
                    let plan = app.pending_plan.take().unwrap();
                    let state = app.state.clone();
                    let tx = app.ui_tx.clone();
                    app.is_loading = true;
                    app.agent_status = AgentStatus::Busy;
                    app.runtime.spawn(async move {
                        match state.agent.execute_plan(&plan).await {
                            Ok(results) => {
                                let mut text = String::new();
                                for r in &results {
                                    text.push_str(&format!("**Step {}**: {}\n```\n{}\n```\n\n", r.step_id, if r.success { "✅" } else { "❌" }, r.output));
                                }
                                if let Err(e) = tx.send(UiEvent::Chunk(text)) {
                                    tracing::warn!("Failed to send plan results: {}", e);
                                }
                            }
                            Err(e) => {
                                if let Err(err) = tx.send(UiEvent::Error(format!("Plan execution failed: {}", e))) {
                                    tracing::warn!("Failed to send Error: {}", err);
                                }
                            }
                        }
                        if let Err(e) = tx.send(UiEvent::Done) {
                            tracing::warn!("Failed to send Done: {}", e);
                        }
                    });
                } else if cancel {
                    app.pending_plan = None;
                }
                ui.separator();
            }

            // Attachment chips above input bar
            if !app.attachments.is_empty() {
                let mut to_remove: Option<usize> = None;
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new("Attachments:").size(11.0).color(app.theme.text_dim));
                    for (i, att) in app.attachments.iter().enumerate() {
                        egui::Frame::group(ui.style())
                            .fill(app.theme.surface)
                            .corner_radius(egui::CornerRadius::same(app.theme.radius_full as u8))
                            .stroke(egui::Stroke::new(1.0, app.theme.border))
                            .inner_margin(egui::Margin::symmetric(8, 4))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new("📎").size(11.0));
                                    ui.label(egui::RichText::new(&att.name).size(11.0).color(app.theme.text).monospace());
                                    if ui.small_button("×").clicked() { to_remove = Some(i); }
                                });
                            });
                    }
                });
                if let Some(i) = to_remove {
                    app.attachments.remove(i);
                }
                ui.separator();
            }

            // Input bar card
            egui::Frame::group(ui.style())
                .fill(app.theme.input_bg)
                .corner_radius(egui::CornerRadius::same(app.theme.radius_lg as u8))
                .stroke(egui::Stroke::new(1.0, app.theme.border))
                .inner_margin(egui::Margin::same(6))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        let available_width = ui.available_width();
                        let btn_area_width = if app.is_loading { 100.0 } else { 52.0 };
                        let input_width = available_width - btn_area_width;
                        ui.allocate_ui_with_layout(
                            egui::vec2(input_width, 44.0),
                            egui::Layout::top_down(egui::Align::LEFT),
                            |ui| {
                                let hint = if app.pending_send.is_some() {
                                    "Message queued — will send when reply finishes..."
                                } else if !app.attachments.is_empty() {
                                    "Type a message (files attached)..."
                                } else {
                                    "Type a message..."
                                };
                                let prev_input = app.input.clone();
                                let line_count = app.input.matches('\n').count() + 1;
                                let input_height = (line_count as f32 * 20.0 + 24.0).clamp(44.0, 120.0);
                                let text_edit = egui::TextEdit::multiline(&mut app.input)
                                    .desired_rows(line_count.max(1))
                                    .hint_text(hint)
                                    .margin(egui::vec2(8.0, 8.0));
                                ui.add_sized(egui::vec2(input_width, input_height), text_edit);
                                // Enter sends; Shift+Enter keeps newline from TextEdit.
                                // We compare prev_input to detect IME commits (which add chars
                                // other than newline) vs plain Enter (which only adds newline).
                                let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));
                                if enter_pressed && !ui.input(|i| i.modifiers.shift) {
                                    while app.input.ends_with('\n') {
                                        app.input.pop();
                                    }
                                    if app.input == prev_input && !app.input.trim().is_empty() {
                                        app.send();
                                    }
                                }
                            },
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if app.is_loading {
                                // Queue-send button (rightmost) — enabled only if input has content.
                                let can_queue = !app.input.trim().is_empty() || !app.attachments.is_empty();
                                let queue_color = if can_queue { app.theme.accent } else { app.theme.bg_elevated };
                                let queue_text = if can_queue { app.theme.text } else { app.theme.text_dim };
                                let queue_btn = ui.add_sized(
                                    egui::vec2(44.0, 44.0),
                                    egui::Button::new(egui::RichText::new("▶").size(16.0).color(queue_text))
                                        .fill(queue_color).corner_radius(egui::CornerRadius::same(app.theme.radius_full as u8)),
                                );
                                if queue_btn.clicked() && can_queue {
                                    app.send();
                                }
                                if can_queue {
                                    queue_btn.on_hover_text("Queue message — auto-sends when current reply finishes");
                                } else {
                                    queue_btn.on_hover_text("Type a message to queue");
                                }

                                // Stop button (left of queue).
                                let stop_btn = ui.add_sized(
                                    egui::vec2(44.0, 44.0),
                                    egui::Button::new(egui::RichText::new("■").size(16.0).color(app.theme.text))
                                        .fill(app.theme.danger).corner_radius(egui::CornerRadius::same(app.theme.radius_full as u8)),
                                );
                                if stop_btn.clicked() { app.stop(); }
                                stop_btn.on_hover_text("Stop generating (Ctrl+C)");
                            } else {
                                // Send button.
                                let btn = ui.add_sized(
                                    egui::vec2(44.0, 44.0),
                                    egui::Button::new(egui::RichText::new("▶").size(16.0).color(app.theme.text))
                                        .fill(app.theme.accent).corner_radius(egui::CornerRadius::same(app.theme.radius_full as u8)),
                                );
                                if btn.clicked() { app.send(); }
                                btn.on_hover_text("Send message");
                            }
                        });
                    });
                });
        });
}
