#![cfg_attr(
    test,
    allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)
)]
//! `clarity-slint` 阶段 1 入口 — 手绘图布局验证。
//!
//! 布局：左侧树形导航 / 中央 Bot+输入区 / 右侧可收起抽屉。

use clarity_slint::app_state::AppState;
use slint::ComponentHandle;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

mod window_chrome;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let ui = clarity_slint::ui::AppWindow::new()?;
    let state = Rc::new(RefCell::new(AppState::mock()));

    sync_ui(&ui, &state.borrow());

    // 左侧核心入口回调
    let ui_weak = ui.as_weak();
    let state_ref = state.clone();
    ui.on_core_action_clicked(move |action| {
        tracing::info!("核心入口: {}", action);
        state_ref
            .borrow_mut()
            .set_input_text(format!("/{}", action));
        if let Some(ui) = ui_weak.upgrade() {
            ui.set_input_text(state_ref.borrow().input_text());
        }
    });

    // 外链回调
    ui.on_external_link_clicked(move |link| {
        tracing::info!("外链: {}", link);
    });

    // 扩展功能回调
    ui.on_extra_feature_clicked(move |feature| {
        tracing::info!("扩展功能: {}", feature);
    });

    // Claw 设备回调
    ui.on_claw_device_clicked(move |device| {
        tracing::info!("Claw 设备: {}", device);
    });

    // 树节点点击回调：有子节点则展开/收起，叶子则选中
    let ui_weak = ui.as_weak();
    let state_ref = state.clone();
    ui.on_tree_item_clicked(move |item| {
        let id = item.id.to_string();
        let has_children = item.has_children;
        tracing::info!("树节点: {} (has_children={})", id, has_children);

        if has_children {
            state_ref.borrow_mut().toggle_tree_item(&id);
            if let Some(ui) = ui_weak.upgrade() {
                sync_ui(&ui, &state_ref.borrow());
            }
        }
    });

    // 用户菜单回调
    ui.on_user_menu_clicked(move || {
        tracing::info!("用户菜单");
    });

    // 输入框文本变化
    let state_ref = state.clone();
    ui.on_input_text_changed(move |text| {
        state_ref.borrow_mut().set_input_text(text.to_string());
    });

    // 发送回调
    ui.on_send_clicked(move || {
        tracing::info!("发送: {}", state.borrow().input_text());
    });

    // 上传回调
    ui.on_upload_clicked(move || {
        tracing::info!("上传文件");
    });

    // Pretext 渲染回调：PretextLabel 组件通过该回调把文本渲染成 Image。
    let backend = Arc::new(pretext_fontdb::FontdbBackend::new());
    let backend_for_callback = backend.clone();
    ui.on_pretext_render(
        move |text: slint::SharedString,
              width_px: f32,
              font: slint::SharedString,
              color: slint::Color|
              -> slint::Image {
            pretext_slint::render_text(&text, width_px, &font, color, &backend_for_callback)
        },
    );

    // 自定义标题栏控制
    ui.on_window_minimize(move || {
        window_chrome::minimize();
    });
    let ui_weak = ui.as_weak();
    ui.on_window_maximize_toggle(move || {
        if let Some(ui) = ui_weak.upgrade() {
            window_chrome::toggle_maximize(&ui);
        }
    });
    ui.on_window_close(move || {
        window_chrome::close();
    });

    // 窗口显示后初始化无边框样式（句柄此时才可用）
    let ui_weak = ui.as_weak();
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(ui) = ui_weak.upgrade() {
            window_chrome::setup(&ui);
        }
    });

    ui.run()?;
    Ok(())
}

/// 将 Rust 状态同步到 Slint UI。
fn sync_ui(ui: &clarity_slint::ui::AppWindow, state: &AppState) {
    ui.set_core_actions(state.core_actions_model().into());
    ui.set_external_links(state.external_links_model().into());
    ui.set_extra_features(state.extra_features_model().into());
    ui.set_claw_devices(state.claw_devices_model().into());
    ui.set_tree_items(state.tree_items_model().into());
    ui.set_user_name(state.user_name());
    ui.set_bot_name(state.bot_name());
    ui.set_input_text(state.input_text());
}
