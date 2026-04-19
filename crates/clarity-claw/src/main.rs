//! clarity-claw —— 系统托盘常驻应用
//!
//! 格雷的物理居所。
//! 永存且唯一的对话入口，多模态感知，生活/陪伴。

use std::sync::{Arc, Mutex};

use clarity_wire::{Wire, WireMessage};
use tao::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    window::WindowBuilder,
};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    MouseButton, TrayIconBuilder, TrayIconEvent,
};

/// Custom events sent into the Tao event loop from other threads.
#[derive(Clone, Debug)]
enum UserEvent {
    /// A message arrived from the backend wire.
    WireMsg(WireMessage),
    /// The user submitted text via the quick-input dialog.
    InputResult(String),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("clarity_claw=info")
        .init();

    tracing::info!("🐾 Clarity Claw starting...");

    // ------------------------------------------------------------------
    // Backend communication channel
    // ------------------------------------------------------------------
    let wire = Wire::new();
    let soul = wire.soul_side().clone();
    let mut ui_side = wire.ui_side(true);

    // ------------------------------------------------------------------
    // Tray menu
    // ------------------------------------------------------------------
    let menu = Menu::new();
    let open_item = MenuItem::new("打开", true, None);
    let separator = PredefinedMenuItem::separator();
    let quit_item = MenuItem::new("退出", true, None);
    let _ = menu.append(&open_item);
    let _ = menu.append(&separator);
    let _ = menu.append(&quit_item);

    // ------------------------------------------------------------------
    // Tray icon
    // ------------------------------------------------------------------
    let _tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Clarity Claw")
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to create tray icon: {}", e))?;

    let menu_channel = MenuEvent::receiver();
    let tray_channel = TrayIconEvent::receiver();

    // ------------------------------------------------------------------
    // Event loop (with user events so background tasks can wake us)
    // ------------------------------------------------------------------
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    // ------------------------------------------------------------------
    // Background task: listen for wire messages and show OS notifications
    // ------------------------------------------------------------------
    let notify_proxy = proxy.clone();
    tokio::spawn(async move {
        while let Some(msg) = ui_side.recv().await {
            // Forward to the main loop
            let _ = notify_proxy.send_event(UserEvent::WireMsg(msg.clone()));

            // Show a native notification for content the user cares about
            let body = match &msg {
                WireMessage::StatusUpdate { message } => Some(message.clone()),
                WireMessage::ContentPart { text } => Some(text.clone()),
                WireMessage::TurnBegin { user_input } => {
                    Some(format!("You: {}", user_input))
                }
                _ => None,
            };

            if let Some(text) = body {
                let _ = notify_rust::Notification::new()
                    .summary("Clarity")
                    .body(&text)
                    .show();
            }
        }
    });

    // ------------------------------------------------------------------
    // Main window (hidden by default)
    // ------------------------------------------------------------------
    let window = WindowBuilder::new()
        .with_visible(false)
        .with_title("Clarity Claw")
        .with_inner_size(tao::dpi::LogicalSize::new(420.0, 200.0))
        .build(&event_loop)
        .ok();

    // Shared state for recent messages
    let recent_messages: Arc<Mutex<Vec<(String, String)>>> = Arc::new(Mutex::new(Vec::new()));
    let is_connected = Arc::new(Mutex::new(false));

    tracing::info!("Claw tray icon active. Right-click to interact.");

    // ------------------------------------------------------------------
    // Event loop
    // ------------------------------------------------------------------
    event_loop.run(move |event, _event_loop, control_flow| {
        *control_flow = ControlFlow::Wait;

        // --------------------------------------------------------------
        // 1. Custom user events (from background tasks)
        // --------------------------------------------------------------
        if let Event::UserEvent(user_event) = &event {
            match user_event {
                UserEvent::WireMsg(msg) => {
                    let mut msgs = recent_messages.lock().unwrap();
                    match msg {
                        WireMessage::StatusUpdate { message } => {
                            if message.to_lowercase().contains("connected")
                                || message.contains("在线")
                            {
                                *is_connected.lock().unwrap() = true;
                            } else if message.to_lowercase().contains("disconnected")
                                || message.contains("离线")
                            {
                                *is_connected.lock().unwrap() = false;
                            }
                            msgs.push(("System".to_string(), message.clone()));
                        }
                        WireMessage::ContentPart { text } => {
                            msgs.push(("Clarity".to_string(), text.clone()));
                        }
                        WireMessage::TurnBegin { user_input } => {
                            msgs.push(("You".to_string(), user_input.clone()));
                        }
                        _ => {}
                    }
                    while msgs.len() > 5 {
                        msgs.remove(0);
                    }
                }
                UserEvent::InputResult(text) => {
                    soul.send(WireMessage::TurnBegin {
                        user_input: text.clone(),
                    });
                }
            }
        }

        // --------------------------------------------------------------
        // 2. Tray icon events (left click → quick input)
        // --------------------------------------------------------------
        if let Ok(tray_event) = tray_channel.try_recv() {
            match tray_event {
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    ..
                } => {
                    let proxy = proxy.clone();
                    tokio::task::spawn_blocking(move || {
                        let result = inputbox::InputBox::new()
                            .title("Clarity")
                            .prompt("Enter command or question:")
                            .show();
                        if let Ok(Some(text)) = result {
                            let _ = proxy.send_event(UserEvent::InputResult(text));
                        }
                    });
                }
                _ => {}
            }
        }

        // --------------------------------------------------------------
        // 3. Tray menu events
        // --------------------------------------------------------------
        if let Ok(menu_event) = menu_channel.try_recv() {
            match menu_event.id {
                id if id == open_item.id() => {
                    tracing::info!("Menu: Open clicked");
                    if let Some(ref win) = window {
                        win.set_visible(true);
                        win.set_focus();
                    }
                }
                id if id == quit_item.id() => {
                    tracing::info!("Menu: Quit clicked");
                    *control_flow = ControlFlow::Exit;
                }
                _ => {}
            }
        }

        // --------------------------------------------------------------
        // 4. Window events
        // --------------------------------------------------------------
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                // Hide window instead of quitting
                if let Some(ref win) = window {
                    win.set_visible(false);
                }
            }
            _ => {}
        }
    });
}
