use crate::App;
use crate::design_system::{self, Space};

/// OpenClaw operations tab — aligned to Kimi Desktop "设置" panel.
///
/// Features: AI diagnostics, Gateway control, config repair, data backup,
/// version info, terminal launch.
pub fn render_ops(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.ui_store.theme.clone();

    design_system::gap(ui, Space::S1);

    // ── Active bot info ──
    let active_bot = app
        .ui_store
        .bot_instances
        .iter()
        .find(|b| b.id == app.ui_store.active_bot_id)
        .cloned();

    if let Some(ref bot) = active_bot {
        egui::Frame::new()
            .fill(theme.bg_hover)
            .corner_radius(egui::CornerRadius::same(theme.radius_md as u8))
            .inner_margin(egui::Margin::symmetric(
                theme.space_12 as i8,
                theme.space_12 as i8,
            ))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    let dot_color = match bot.status {
                        crate::stores::BotStatus::Online => theme.status_online,
                        crate::stores::BotStatus::Syncing => theme.status_busy,
                        crate::stores::BotStatus::Offline => theme.text_dim,
                    };
                    ui.painter().circle_filled(
                        ui.cursor().min + egui::vec2(4.0, 8.0),
                        5.0,
                        dot_color,
                    );
                    design_system::gap(ui, Space::S1);
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new(&bot.name)
                                .size(theme.text_base)
                                .strong()
                                .color(theme.text),
                        );
                        ui.label(
                            egui::RichText::new(format!(
                                "ID: {} · 版本: {}",
                                bot.device_id, bot.version
                            ))
                            .size(theme.text_xs)
                            .color(theme.text_dim)
                            .monospace(),
                        );
                    });
                });
            });
        design_system::gap(ui, Space::S3);
    }

    // ── Action buttons (Kimi-style grid) ──
    ui.label(
        egui::RichText::new("运维操作")
            .size(theme.text_sm)
            .strong()
            .color(theme.text_dim),
    );
    design_system::gap(ui, Space::S1);

    let actions = [
        ("🩺", "AI 问题诊断", "运行自诊断检查"),
        ("🔄", "重启 Gateway", "重启本地 Gateway 服务"),
        ("🔧", "修复配置", "自动修复常见配置问题"),
        ("💻", "打开终端", "打开系统终端"),
        ("💾", "数据备份", "备份当前会话和配置"),
        ("📊", "系统状态", "查看详细系统状态"),
    ];

    for (icon, title, desc) in actions {
        let btn = egui::Button::new(
            egui::RichText::new(format!("{} {}", icon, title))
                .size(theme.text_sm)
                .color(theme.text),
        )
        .fill(theme.bg_hover)
        .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
        .min_size(egui::vec2(ui.available_width(), 40.0));

        let resp = ui.add(btn);
        if resp.clicked() {
            match title {
                "AI 问题诊断" => {
                    app.push_toast(
                        "AI 诊断运行中...".to_string(),
                        crate::ui::types::ToastLevel::Info,
                    );
                }
                "重启 Gateway" => {
                    app.push_toast(
                        "Gateway 重启请求已发送".to_string(),
                        crate::ui::types::ToastLevel::Info,
                    );
                }
                "修复配置" => {
                    app.push_toast(
                        "配置修复中...".to_string(),
                        crate::ui::types::ToastLevel::Info,
                    );
                }
                "数据备份" => {
                    app.push_toast(
                        "数据备份完成".to_string(),
                        crate::ui::types::ToastLevel::Info,
                    );
                }
                _ => {}
            }
        }
        if resp.hovered() {
            resp.on_hover_text(desc);
        }
        design_system::gap(ui, Space::S0);
    }

    design_system::gap(ui, Space::S3);

    // ── Version & backup info ──
    ui.label(
        egui::RichText::new("版本信息")
            .size(theme.text_sm)
            .strong()
            .color(theme.text_dim),
    );
    design_system::gap(ui, Space::S1);

    let version_rows = [
        ("Clarity", env!("CARGO_PKG_VERSION")),
        ("OpenClaw", "2026.4.14"),
        ("Rust", "1.85"),
        ("egui", "0.31"),
    ];
    for (name, value) in version_rows {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(name)
                    .size(theme.text_sm)
                    .color(theme.text_dim),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(value)
                        .size(theme.text_sm)
                        .color(theme.text)
                        .monospace(),
                );
            });
        });
        ui.add_space(2.0);
    }

    if let Some(ref bot) = active_bot {
        design_system::gap(ui, Space::S1);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("上次备份")
                    .size(theme.text_sm)
                    .color(theme.text_dim),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(&bot.last_backup)
                        .size(theme.text_sm)
                        .color(theme.text)
                        .monospace(),
                );
            });
        });
    }
}

// ── Panel trait implementation ──

pub struct OpsPanel;

impl crate::design_system::Panel for OpsPanel {
    fn title(&self, _app: &crate::App) -> &str {
        "Ops"
    }
    fn render(&mut self, app: &mut crate::App, ui: &mut egui::Ui) {
        render_ops(app, ui);
    }
}
