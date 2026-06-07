//! Work mode panel — OpenClaw-style project orchestration.
//!
//! Layout (3-column):
//!   LEFT   : Project tree (multi-repo)
//!   CENTER : Task pipeline / Agent cluster status
//!   RIGHT  : Ops panel (Gateway, diagnostics, terminal)

use crate::App;

/// Hard-coded project list for MVP (configurable via settings later).
const PROJECTS: &[(&str, &str)] = &[
    (".kimi_openclaw", "~/.kimi_openclaw"),
    ("clarity", "~/dev/clarity"),
    ("devbase", "~/dev/devbase"),
    ("syncthing-rust", "~/dev/syncthing-rust"),
];

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
                for (name, _path) in PROJECTS {
                    let active = app.ui_store.active_project.as_deref() == Some(*name);
                    let (bg, fg) = if active {
                        (theme.surface_strong, theme.text)
                    } else {
                        (theme.bg_hover, theme.text_dim)
                    };
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new(*name)
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
            let right_w = (total_w * 0.26).clamp(200.0, 300.0);
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

                // ── RIGHT: Ops panel ──
                ui.allocate_ui_with_layout(
                    egui::vec2(right_w, ui.available_height()),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        render_ops_panel(app, ui, &theme);
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

    egui::ScrollArea::vertical()
        .id_salt(ui.id().with("work_tree_scroll"))
        .auto_shrink([false, true])
        .show(ui, |ui| {
            let active_project: String = app
                .ui_store
                .active_project
                .as_deref()
                .unwrap_or(PROJECTS[0].0)
                .to_string();

            for (name, _path) in PROJECTS {
                let is_active = *name == active_project;
                let text_color = if is_active { theme.accent } else { theme.text_dim };

                let mut rt = egui::RichText::new(format!("📁 {}", name))
                    .size(theme.text_sm)
                    .color(text_color);
                if is_active {
                    rt = rt.strong();
                }
                let resp = ui.selectable_label(is_active, rt);
                if resp.clicked() {
                    app.ui_store.active_project = Some(name.to_string());
                }

                if is_active {
                    ui.add_space(theme.space_4);
                    let subdirs = ["memory", "skills", "tools", "ontology"];
                    for sub in subdirs {
                        ui.horizontal(|ui| {
                            ui.add_space(theme.space_12);
                            ui.label(
                                egui::RichText::new(format!("  📂 {}", sub))
                                    .size(theme.text_xs)
                                    .color(theme.text_dim),
                            );
                        });
                    }
                    ui.add_space(theme.space_8);
                }
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

    // Placeholder: show a simple card per task stage
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

    // Agent cluster status
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

fn render_ops_panel(app: &mut App, ui: &mut egui::Ui, theme: &crate::theme::Theme) {
    ui.label(
        egui::RichText::new("运维")
            .size(theme.text_xs)
            .color(theme.text_dim)
            .strong(),
    );
    ui.add_space(theme.space_8);

    let ops = ["🔄 重启 Gateway", "🔧 修复配置", "🩺 AI 问题诊断", "💻 打开终端"];
    for label in ops {
        let btn = egui::Button::new(
            egui::RichText::new(label)
                .size(theme.text_sm)
                .color(theme.text),
        )
        .fill(theme.bg_hover)
        .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
        .min_size(egui::vec2(ui.available_width(), 36.0));
        if ui.add(btn).clicked() {
            if label.contains("Gateway") {
                app.push_toast("Gateway restart requested".to_string(), crate::ui::types::ToastLevel::Info);
            } else if label.contains("诊断") {
                app.push_toast("AI diagnostics running...".to_string(), crate::ui::types::ToastLevel::Info);
            }
        }
        ui.add_space(theme.space_4);
    }

    ui.add_space(theme.space_16);

    // Version info
    ui.label(
        egui::RichText::new("版本信息")
            .size(theme.text_xs)
            .color(theme.text_dim)
            .strong(),
    );
    ui.add_space(theme.space_8);
    ui.label(
        egui::RichText::new("Clarity v0.3.0")
            .size(theme.text_xs)
            .color(theme.text_dim),
    );
    ui.label(
        egui::RichText::new("OpenClaw 2026.4.14")
            .size(theme.text_xs)
            .color(theme.text_dim),
    );
}
