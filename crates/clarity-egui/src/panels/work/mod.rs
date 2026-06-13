//! Work mode panel — OpenClaw-style project orchestration.
//!
//! Layout (3-column) aligned to Kimi Desktop drawer pattern:
//!   LEFT   : Project list (compact)
//!   CENTER : Task pipeline + Bot panel + File preview
//!   RIGHT  : Workspace file tree (drawer-style, real file system)

use crate::App;
use crate::stores::BotStatus;

/// Resolve a path string that may contain `~` into an absolute PathBuf.
fn resolve_project_path(raw: &str) -> std::path::PathBuf {
    if let Some(rest) = raw.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    std::path::PathBuf::from(raw)
}

/// Project list with resolved paths.
fn projects() -> Vec<(String, std::path::PathBuf)> {
    let raw = [
        (".kimi_openclaw", "~/.kimi_openclaw"),
        ("clarity", "~/dev/clarity"),
        ("devbase", "~/dev/devbase"),
        ("syncthing-rust", "~/dev/syncthing-rust"),
    ];
    raw.iter()
        .map(|(name, path)| (name.to_string(), resolve_project_path(path)))
        .collect()
}

/// Renders the work panel UI.
pub fn render_work_panel(app: &mut App, ctx: &egui::Context) {
    let theme = app.ui_store.theme.clone();

    egui::CentralPanel::default()
        .frame(
            egui::Frame::central_panel(&ctx.style())
                .fill(theme.bg)
                .inner_margin(egui::Margin::symmetric(
                    theme.space_16 as i8,
                    theme.space_16 as i8,
                )),
        )
        .show(ctx, |ui| {
            ui.set_min_size(egui::vec2(600.0, 400.0));

            // ── Three-column layout ──
            // Kimi-style: narrow left (project + bot), wide center (tasks), right drawer (files)
            let total_w = ui.available_width();
            let left_w = (total_w * 0.16).clamp(140.0, 200.0);
            let right_w = (total_w * 0.30).clamp(260.0, 380.0);
            let center_w = total_w - left_w - right_w - theme.space_16 * 2.0;

            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = theme.space_16;

                // ── LEFT: Project list + Bot card ──
                ui.allocate_ui_with_layout(
                    egui::vec2(left_w, ui.available_height()),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        render_project_list(app, ui, &theme);
                        ui.add_space(theme.space_16);
                        render_bot_compact(app, ui, &theme);
                    },
                );

                // ── CENTER: Task pipeline + inline file preview ──
                ui.allocate_ui_with_layout(
                    egui::vec2(center_w, ui.available_height()),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        render_center_zone(app, ui, &theme);
                    },
                );

                // ── RIGHT: Workspace file tree (Kimi drawer-style) ──
                ui.allocate_ui_with_layout(
                    egui::vec2(right_w, ui.available_height()),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        render_workspace_tree(app, ui, &theme);
                    },
                );
            });
        });
}

// ============================================================================
// LEFT: Project list
// ============================================================================

fn render_project_list(app: &mut App, ui: &mut egui::Ui, theme: &crate::theme::Theme) {
    ui.label(
        egui::RichText::new("项目")
            .size(theme.text_xs)
            .color(theme.text_dim)
            .strong(),
    );
    ui.add_space(theme.space_8);

    let projects = projects();
    for (name, _path) in &projects {
        let is_active = app.ui_store.active_project.as_deref() == Some(name.as_str());
        let text_color = if is_active {
            theme.accent
        } else {
            theme.text_dim
        };
        let mut rt = egui::RichText::new(format!("📁 {}", name))
            .size(theme.text_sm)
            .color(text_color);
        if is_active {
            rt = rt.strong();
        }
        let resp = ui.selectable_label(is_active, rt);
        if resp.clicked() {
            app.ui_store.active_project = Some(name.clone());
        }
    }
}

// ============================================================================
// CENTER: Task pipeline + Bot panel + File preview
// ============================================================================

fn render_center_zone(app: &mut App, ui: &mut egui::Ui, theme: &crate::theme::Theme) {
    egui::ScrollArea::vertical()
        .id_salt(ui.id().with("work_center_scroll"))
        .auto_shrink([false, true])
        .show(ui, |ui| {
            // 1. Task pipeline
            render_task_pipeline(app, ui, theme);
            ui.add_space(theme.space_16);

            // 2. File preview (inline, not drawer)
            if app.ui_store.preview_item.is_some() {
                ui.separator();
                ui.add_space(theme.space_8);
                render_inline_preview(app, ui, theme);
            }
        });
}

fn render_task_pipeline(_app: &mut App, ui: &mut egui::Ui, theme: &crate::theme::Theme) {
    ui.label(
        egui::RichText::new("任务管道")
            .size(theme.text_xs)
            .color(theme.text_dim)
            .strong(),
    );
    ui.add_space(theme.space_8);

    let stages = [
        ("🟡 待处理", "3 个任务等待执行"),
        ("🟢 运行中", "1 个 Agent 正在工作"),
        ("🔵 已完成", "12 个任务今日完成"),
    ];
    for (title, desc) in stages {
        egui::Frame::new()
            .fill(theme.bg_hover)
            .corner_radius(egui::CornerRadius::same(theme.radius_md as u8))
            .inner_margin(egui::Margin::symmetric(
                theme.space_12 as i8,
                theme.space_12 as i8,
            ))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.label(
                    egui::RichText::new(title)
                        .size(theme.text_base)
                        .strong()
                        .color(theme.text),
                );
                ui.add_space(theme.space_4);
                ui.label(
                    egui::RichText::new(desc)
                        .size(theme.text_sm)
                        .color(theme.text_dim),
                );
            });
        ui.add_space(theme.space_8);
    }

    ui.add_space(theme.space_8);

    // Agent cluster
    ui.label(
        egui::RichText::new("Agent 集群")
            .size(theme.text_xs)
            .color(theme.text_dim)
            .strong(),
    );
    ui.add_space(theme.space_8);

    let agents = [
        ("Gray-Cloud", "在线", theme.status_online),
        ("Gray-Desktop", "空闲", theme.text_dim),
    ];
    for (name, status, color) in agents {
        ui.horizontal(|ui| {
            ui.painter()
                .circle_filled(ui.cursor().min + egui::vec2(6.0, 8.0), 4.0, color);
            ui.add_space(theme.space_12);
            ui.label(
                egui::RichText::new(name)
                    .size(theme.text_sm)
                    .color(theme.text),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(status)
                        .size(theme.text_xs)
                        .color(theme.text_dim),
                );
            });
        });
        ui.add_space(theme.space_4);
    }
}

fn render_bot_compact(app: &mut App, ui: &mut egui::Ui, theme: &crate::theme::Theme) {
    let active_bot = app
        .ui_store
        .bot_instances
        .iter()
        .find(|b| b.id == app.ui_store.active_bot_id)
        .cloned();

    if let Some(bot) = active_bot {
        egui::Frame::new()
            .fill(theme.bg_hover)
            .corner_radius(egui::CornerRadius::same(theme.radius_lg as u8))
            .inner_margin(egui::Margin::symmetric(
                theme.space_12 as i8,
                theme.space_12 as i8,
            ))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());

                // Header: avatar + name + status dot
                ui.horizontal(|ui| {
                    let avatar_size = 36.0;
                    let (avatar_rect, _resp) = ui.allocate_exact_size(
                        egui::vec2(avatar_size, avatar_size),
                        egui::Sense::hover(),
                    );
                    ui.painter().circle_filled(
                        avatar_rect.center(),
                        avatar_size * 0.5,
                        theme.accent,
                    );
                    let label = ui.fonts(|f| {
                        f.layout(
                            "K".to_string(),
                            theme.font_bold(theme.text_base),
                            egui::Color32::WHITE,
                            f32::INFINITY,
                        )
                    });
                    let label_pos = avatar_rect.center() - label.rect.size() * 0.5;
                    ui.painter().galley(label_pos, label, egui::Color32::WHITE);

                    ui.add_space(theme.space_8);
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new(&bot.name)
                                .size(theme.text_sm)
                                .strong()
                                .color(theme.text),
                        );
                        ui.label(
                            egui::RichText::new(format!("ID: {}", bot.device_id))
                                .size(theme.text_xs)
                                .color(theme.text_dim)
                                .monospace(),
                        );
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let status_color = match bot.status {
                            BotStatus::Online => theme.status_online,
                            BotStatus::Syncing => theme.status_busy,
                            BotStatus::Offline => theme.text_dim,
                        };
                        ui.painter().circle_filled(
                            ui.cursor().min + egui::vec2(4.0, 8.0),
                            4.0,
                            status_color,
                        );
                    });
                });
                ui.add_space(theme.space_8);

                // Quick actions row
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = theme.space_8;

                    let join_btn = egui::Button::new(
                        egui::RichText::new("接入聊天")
                            .size(theme.text_xs)
                            .strong()
                            .color(theme.bg),
                    )
                    .fill(theme.accent)
                    .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8));
                    if ui.add(join_btn).clicked() {
                        app.view_state.main = clarity_core::ui::AppView::Chat;
                        app.push_toast(
                            format!("已切换到 {} 聊天频道", bot.name),
                            crate::ui::types::ToastLevel::Info,
                        );
                    }

                    let diag_btn = egui::Button::new(
                        egui::RichText::new("AI 诊断")
                            .size(theme.text_xs)
                            .color(theme.text),
                    )
                    .fill(theme.surface)
                    .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8));
                    if ui.add(diag_btn).clicked() {
                        app.push_toast(
                            "AI 诊断运行中...".to_string(),
                            crate::ui::types::ToastLevel::Info,
                        );
                    }

                    let restart_btn = egui::Button::new(
                        egui::RichText::new("重启 Gateway")
                            .size(theme.text_xs)
                            .color(theme.text),
                    )
                    .fill(theme.surface)
                    .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8));
                    if ui.add(restart_btn).clicked() {
                        app.push_toast(
                            "Gateway 重启请求已发送".to_string(),
                            crate::ui::types::ToastLevel::Info,
                        );
                    }
                });
            });
    }

    // Bot switcher
    ui.add_space(theme.space_8);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = theme.space_4;
        for bot in &app.ui_store.bot_instances {
            let is_active = bot.id == app.ui_store.active_bot_id;
            let dot_color = match bot.status {
                BotStatus::Online => theme.status_online,
                BotStatus::Syncing => theme.status_busy,
                BotStatus::Offline => theme.text_dim,
            };
            let (bg, fg) = if is_active {
                (theme.surface_strong, theme.text)
            } else {
                (theme.bg_hover, theme.text_dim)
            };
            let pill = egui::Button::new(
                egui::RichText::new(format!("● {}", bot.name))
                    .size(theme.text_xs)
                    .color(if is_active { fg } else { dot_color }),
            )
            .fill(bg)
            .corner_radius(egui::CornerRadius::same(theme.radius_full as u8));
            if ui.add(pill).clicked() {
                app.ui_store.active_bot_id = bot.id.clone();
            }
        }
    });
}

fn render_inline_preview(app: &mut App, ui: &mut egui::Ui, theme: &crate::theme::Theme) {
    // Extract owned copies upfront to release the borrow before closures.
    let (title, content) = match app.ui_store.preview_item.as_ref() {
        Some(crate::ui::types::PreviewItem::File { name, content, .. }) => {
            (name.clone(), content.clone())
        }
        Some(crate::ui::types::PreviewItem::WebPage { title, content, .. }) => {
            (title.clone(), content.clone())
        }
        None => return,
    };

    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(crate::theme::ICON_FILE)
                .font(theme.font_icon(theme.text_sm))
                .color(theme.text_muted),
        );
        ui.add_space(theme.space_4);
        ui.label(
            egui::RichText::new(&title)
                .size(theme.text_sm)
                .strong()
                .color(theme.text),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if crate::widgets::icon_button_toolbar(ui, crate::theme::ICON_X, theme.text_xs, theme)
                .clicked()
            {
                app.ui_store.preview_item = None;
            }
        });
    });
    ui.add_space(theme.space_4);

    egui::Frame::new()
        .fill(theme.code_block_bg)
        .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
        .inner_margin(egui::Margin::same(theme.space_8 as i8))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            let max_h = 200.0;
            egui::ScrollArea::vertical()
                .max_height(max_h)
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new(&content)
                            .font(theme.font_mono(theme.text_sm))
                            .color(theme.text),
                    );
                });
        });
}

// ============================================================================
// RIGHT: Workspace file tree (Kimi drawer-style)
// ============================================================================

fn render_workspace_tree(app: &mut App, ui: &mut egui::Ui, theme: &crate::theme::Theme) {
    ui.label(
        egui::RichText::new("工作区")
            .size(theme.text_xs)
            .color(theme.text_dim)
            .strong(),
    );
    ui.add_space(theme.space_8);

    let projects = projects();
    let active_project_name = app
        .ui_store
        .active_project
        .clone()
        .unwrap_or_else(|| projects[0].0.clone());

    let active_path = projects
        .iter()
        .find(|(n, _)| n == &active_project_name)
        .map(|(_, p)| p.clone())
        .unwrap_or_else(|| resolve_project_path("~"));

    if active_path.exists() {
        let selected: Option<String> = app.ui_store.preview_item.as_ref().and_then(|p| match p {
            crate::ui::types::PreviewItem::File { path, .. } => Some(path.clone()),
            _ => None,
        });

        egui::ScrollArea::vertical()
            .id_salt(ui.id().with("workspace_tree"))
            .auto_shrink([false, true])
            .show(ui, |ui| {
                crate::ui::file_browser::render_file_tree(
                    ui,
                    &active_path,
                    theme,
                    0,
                    selected.as_deref(),
                    &mut |path| {
                        if let Ok(content) = std::fs::read_to_string(path) {
                            app.ui_store.preview_item = Some(crate::ui::types::PreviewItem::File {
                                name: path
                                    .file_name()
                                    .map(|n| n.to_string_lossy().to_string())
                                    .unwrap_or_default(),
                                content,
                                path: path.to_string_lossy().to_string(),
                            });
                        }
                    },
                    false,
                );
            });
    } else {
        ui.label(
            egui::RichText::new(format!("路径不存在: {}", active_path.display()))
                .size(theme.text_sm)
                .color(theme.text_dim),
        );
    }
}
