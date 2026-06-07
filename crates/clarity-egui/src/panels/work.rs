//! Work mode panel — OpenClaw-style project orchestration.
//!
//! Layout (3-column):
//!   LEFT   : Project tree (multi-repo)
//!   CENTER : Task pipeline / Agent cluster status
//!   RIGHT  : Bot instance panel (aligned to Kimi Desktop v3.0.15)

use crate::App;
use crate::stores::BotStatus;

/// Resolve a path string that may contain `~` into an absolute PathBuf.
fn resolve_project_path(raw: &str) -> std::path::PathBuf {
    if raw.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(&raw[2..]);
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

            // ── Top bar: project selector pills ──
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = theme.space_8;
                ui.label(
                    egui::RichText::new("项目")
                        .size(theme.text_sm)
                        .strong()
                        .color(theme.text_dim),
                );
                ui.add_space(theme.space_8);
                let projects = projects();
                for (name, _path) in &projects {
                    let active = app.ui_store.active_project.as_deref() == Some(name.as_str());
                    let (bg, fg) = if active {
                        (theme.surface_strong, theme.text)
                    } else {
                        (theme.bg_hover, theme.text_dim)
                    };
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new(name.as_str())
                                    .size(theme.text_sm)
                                    .color(fg),
                            )
                            .fill(bg)
                            .corner_radius(egui::CornerRadius::same(theme.radius_full as u8)),
                        )
                        .clicked()
                    {
                        app.ui_store.active_project = Some(name.to_string());
                    }
                }
            });
            ui.add_space(theme.space_16);

            // ── Three-column layout ──
            let total_w = ui.available_width();
            let left_w = (total_w * 0.22).clamp(180.0, 280.0);
            let right_w = (total_w * 0.26).clamp(220.0, 320.0);
            let center_w = total_w - left_w - right_w - theme.space_16 * 2.0;

            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = theme.space_16;

                // ── LEFT: Project tree ──
                ui.allocate_ui_with_layout(
                    egui::vec2(left_w, ui.available_height()),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        render_project_tree(app, ui, &theme);
                    },
                );

                // ── CENTER: Task pipeline + Agent cluster ──
                ui.allocate_ui_with_layout(
                    egui::vec2(center_w, ui.available_height()),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        render_task_pipeline(app, ui, &theme);
                    },
                );

                // ── RIGHT: Bot instance panel (Kimi-style) ──
                ui.allocate_ui_with_layout(
                    egui::vec2(right_w, ui.available_height()),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        render_bot_panel(app, ui, &theme);
                    },
                );
            });
        });
}

fn render_project_tree(app: &mut App, ui: &mut egui::Ui, theme: &crate::theme::Theme) {
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

    // Project switcher (compact list)
    for (name, _path) in &projects {
        let is_active = name == &active_project_name;
        let text_color = if is_active { theme.accent } else { theme.text_dim };
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

    ui.add_space(theme.space_8);
    ui.separator();
    ui.add_space(theme.space_8);

    // Real file tree for active project
    let active_path = projects
        .iter()
        .find(|(n, _)| n == &active_project_name)
        .map(|(_, p)| p.clone())
        .unwrap_or_else(|| resolve_project_path("~"));

    if active_path.exists() {
        // Extract selected path *before* the closure to avoid borrow conflict
        let selected: Option<String> = app
            .ui_store
            .preview_item
            .as_ref()
            .and_then(|p| match p {
                crate::ui::types::PreviewItem::File { path, .. } => Some(path.clone()),
                _ => None,
            });
        egui::ScrollArea::vertical()
            .id_salt(ui.id().with("work_file_tree"))
            .auto_shrink([false, true])
            .show(ui, |ui| {
                let ctx = ui.ctx().clone();
                crate::ui::file_browser::render_file_tree(
                    ui,
                    &active_path,
                    theme,
                    0,
                    selected.as_deref(),
                    &mut |path| {
                        if let Ok(content) = std::fs::read_to_string(path) {
                            app.ui_store.preview_item =
                                Some(crate::ui::types::PreviewItem::File {
                                    name: path
                                        .file_name()
                                        .map(|n| n.to_string_lossy().to_string())
                                        .unwrap_or_default(),
                                    content,
                                    path: path.to_string_lossy().to_string(),
                                });
                            app.ui_store.preview_drawer_open = true;
                            ctx.request_repaint();
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
            .inner_margin(egui::Margin::symmetric(theme.space_12 as i8, theme.space_12 as i8))
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

    ui.add_space(theme.space_16);

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
            ui.painter().circle_filled(
                ui.cursor().min + egui::vec2(6.0, 8.0),
                4.0,
                color,
            );
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

fn render_bot_panel(app: &mut App, ui: &mut egui::Ui, theme: &crate::theme::Theme) {
    let active_bot = app
        .ui_store
        .bot_instances
        .iter()
        .find(|b| b.id == app.ui_store.active_bot_id)
        .cloned()
        .unwrap_or_else(|| crate::stores::BotInstance {
            id: "unknown".into(),
            name: "Unknown".into(),
            device_id: "-".into(),
            status: BotStatus::Offline,
            version: "-".into(),
            last_backup: "-".into(),
        });

    // ── Bot switcher pills ──
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
    ui.add_space(theme.space_16);

    // ── Bot identity card (Kimi-style) ──
    egui::Frame::new()
        .fill(theme.bg_hover)
        .corner_radius(egui::CornerRadius::same(theme.radius_lg as u8))
        .inner_margin(egui::Margin::symmetric(theme.space_16 as i8, theme.space_16 as i8))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());

            // Avatar + name + ID
            ui.horizontal(|ui| {
                let avatar_size = 48.0;
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
                        theme.font_bold(theme.text_lg),
                        egui::Color32::WHITE,
                        f32::INFINITY,
                    )
                });
                let label_pos = avatar_rect.center() - label.rect.size() * 0.5;
                ui.painter().galley(label_pos, label, egui::Color32::WHITE);

                ui.add_space(theme.space_12);
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new(&active_bot.name)
                            .size(theme.text_base)
                            .strong()
                            .color(theme.text),
                    );
                    ui.label(
                        egui::RichText::new(format!("ID: {}", active_bot.device_id))
                            .size(theme.text_xs)
                            .color(theme.text_dim)
                            .monospace(),
                    );
                });
            });
            ui.add_space(theme.space_12);

            // "接入聊天频道" primary button
            let join_btn = egui::Button::new(
                egui::RichText::new("接入聊天频道")
                    .size(theme.text_sm)
                    .strong()
                    .color(theme.bg),
            )
            .fill(theme.accent)
            .corner_radius(egui::CornerRadius::same(theme.radius_md as u8))
            .min_size(egui::vec2(ui.available_width(), 36.0));
            if ui.add(join_btn).clicked() {
                app.view_state.main = clarity_core::ui::AppView::Chat;
                app.push_toast(
                    format!("已切换到 {} 聊天频道", active_bot.name),
                    crate::ui::types::ToastLevel::Info,
                );
            }
        });

    ui.add_space(theme.space_16);

    // ── Function menu (Kimi-style vertical list) ──
    let menu_items = [
        ("🩺", "AI 问题诊断"),
        ("✏️", "编辑 ID 名称"),
        ("💻", "打开终端"),
        ("🔄", "重启 Gateway"),
        ("🔧", "修复 Claw 配置"),
        ("📦", "订阅模块"),
        ("⬆️", "升级 Kimi Claw"),
    ];

    for (icon, label) in menu_items {
        let is_primary = label == "重启 Gateway" || label == "修复 Claw 配置";
        let (bg, fg) = if is_primary {
            (theme.bg_hover, theme.text)
        } else {
            (egui::Color32::TRANSPARENT, theme.text_dim)
        };

        let btn = egui::Button::new(
            egui::RichText::new(format!("{} {}", icon, label))
                .size(theme.text_sm)
                .color(fg),
        )
        .fill(bg)
        .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
        .min_size(egui::vec2(ui.available_width(), 32.0));
        if ui.add(btn).clicked() {
            match label {
                "AI 问题诊断" => {
                    app.push_toast("AI 问题诊断运行中...".to_string(), crate::ui::types::ToastLevel::Info);
                }
                "重启 Gateway" => {
                    app.push_toast("Gateway 重启请求已发送".to_string(), crate::ui::types::ToastLevel::Info);
                }
                "修复 Claw 配置" => {
                    app.push_toast("配置修复中...".to_string(), crate::ui::types::ToastLevel::Info);
                }
                "打开终端" => {
                    app.push_toast("终端已打开".to_string(), crate::ui::types::ToastLevel::Info);
                }
                _ => {}
            }
        }
        ui.add_space(2.0);
    }

    ui.add_space(theme.space_16);

    // ── Status footer ──
    ui.horizontal(|ui| {
        let status_color = match active_bot.status {
            BotStatus::Online => theme.status_online,
            BotStatus::Syncing => theme.status_busy,
            BotStatus::Offline => theme.text_dim,
        };
        let status_text = match active_bot.status {
            BotStatus::Online => "在线",
            BotStatus::Syncing => "同步中",
            BotStatus::Offline => "离线",
        };
        ui.painter().circle_filled(
            ui.cursor().min + egui::vec2(4.0, 8.0),
            4.0,
            status_color,
        );
        ui.add_space(theme.space_8);
        ui.label(
            egui::RichText::new(status_text)
                .size(theme.text_xs)
                .color(theme.text_dim),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new(format!("版本: {}", active_bot.version))
                    .size(theme.text_xs)
                    .color(theme.text_dim)
                    .monospace(),
            );
        });
    });
    ui.add_space(theme.space_4);
    ui.label(
        egui::RichText::new(format!("上次备份: {}", active_bot.last_backup))
            .size(theme.text_xs)
            .color(theme.text_dim),
    );
}
