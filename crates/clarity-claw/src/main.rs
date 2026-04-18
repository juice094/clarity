//! clarity-claw —— 系统托盘常驻应用
//!
//! 格雷的物理居所。
//! 永存且唯一的对话入口，多模态感知，生活/陪伴。

use tao::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    window::WindowBuilder,
};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    TrayIconBuilder,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("clarity_claw=info")
        .init();

    tracing::info!("🐾 Clarity Claw starting...");

    // 创建菜单
    let menu = Menu::new();
    let open_item = MenuItem::new("打开", true, None);
    let separator = PredefinedMenuItem::separator();
    let quit_item = MenuItem::new("退出", true, None);
    let _ = menu.append(&open_item);
    let _ = menu.append(&separator);
    let _ = menu.append(&quit_item);

    // 创建托盘图标
    let _tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Clarity Claw")
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to create tray icon: {}", e))?;

    // 菜单事件通道
    let menu_channel = MenuEvent::receiver();

    // 创建事件循环
    let event_loop = EventLoopBuilder::new().build();

    // 可选：创建隐藏窗口（tao 在 Windows 上需要窗口来处理消息）
    let window = WindowBuilder::new()
        .with_visible(false)
        .with_title("Clarity Claw")
        .build(&event_loop)
        .ok();

    tracing::info!("Claw tray icon active. Right-click to interact.");

    event_loop.run(move |event, _event_loop, control_flow| {
        *control_flow = ControlFlow::Wait;

        // 处理托盘菜单事件
        if let Ok(menu_event) = menu_channel.try_recv() {
            match menu_event.id {
                id if id == open_item.id() => {
                    tracing::info!("Menu: Open clicked");
                    // TODO: 显示对话窗口
                }
                id if id == quit_item.id() => {
                    tracing::info!("Menu: Quit clicked");
                    *control_flow = ControlFlow::Exit;
                }
                _ => {}
            }
        }

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                // 隐藏窗口而不是退出
                if let Some(ref win) = window {
                    win.set_visible(false);
                }
            }
            _ => {}
        }
    });
}
