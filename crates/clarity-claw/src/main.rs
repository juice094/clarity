//! clarity-claw —— 系统托盘常驻应用（运行时监控器）
//!
//! 格雷的物理居所。
//! 常驻系统托盘，监控后台任务状态，提供快速入口。

use std::sync::{Arc, Mutex};
use std::time::Duration;

use clarity_wire::{Wire, WireMessage};
use tao::{
    event::Event,
    event_loop::{ControlFlow, EventLoopBuilder},
    window::WindowBuilder,
};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    MouseButton, TrayIconBuilder, TrayIconEvent,
};

const GATEWAY_URL: &str = "http://127.0.0.1:18790";
const POLL_INTERVAL_SECS: u64 = 5;

/// Custom events sent into the Tao event loop from other threads.
#[derive(Clone, Debug)]
enum UserEvent {
    /// A message arrived from the backend wire.
    WireMsg(WireMessage),
    /// The user submitted text via the quick-input dialog.
    InputResult(String),
    /// Task list update from Gateway polling.
    TaskUpdate(Vec<TaskSummary>),
}

/// Minimal task info deserialized from Gateway `/v1/tasks`.
#[derive(Clone, Debug, serde::Deserialize)]
struct TaskSummary {
    #[serde(rename = "task_id")]
    task_id: String,
    name: String,
    status: String,
}

/// Gateway task list payload.
#[derive(Clone, Debug, serde::Deserialize)]
struct TaskListPayload {
    tasks: Vec<TaskSummary>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("clarity_claw=info")
        .init();

    tracing::info!("🐾 Clarity Claw starting...");

    let gateway_url =
        std::env::var("CLARITY_GATEWAY_URL").unwrap_or_else(|_| GATEWAY_URL.to_string());

    // ------------------------------------------------------------------
    // Backend communication channel (wire — reserved for future Soul link)
    // ------------------------------------------------------------------
    let wire = Wire::new();
    let soul = wire.soul_side().clone();
    let mut ui_side = wire.ui_side(true);

    // ------------------------------------------------------------------
    // Tray menu
    // ------------------------------------------------------------------
    let menu = Menu::new();
    let new_chat_item = MenuItem::new("New Chat", true, None);
    let view_tasks_item = MenuItem::new("View Tasks", true, None);
    let open_window_item = MenuItem::new("Open Window", true, None);
    let separator = PredefinedMenuItem::separator();
    let quit_item = MenuItem::new("Quit", true, None);
    let _ = menu.append(&new_chat_item);
    let _ = menu.append(&view_tasks_item);
    let _ = menu.append(&open_window_item);
    let _ = menu.append(&separator);
    let _ = menu.append(&quit_item);

    // ------------------------------------------------------------------
    // Tray icon
    // ------------------------------------------------------------------
    let tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Clarity Claw — connecting...")
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
            let _ = notify_proxy.send_event(UserEvent::WireMsg(msg.clone()));

            let body = match &msg {
                WireMessage::StatusUpdate { message } => Some(message.clone()),
                WireMessage::ContentPart { text } => Some(text.clone()),
                WireMessage::TurnBegin { user_input } => Some(format!("You: {}", user_input)),
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
    // Background task: poll Gateway tasks and update tooltip / notify
    // ------------------------------------------------------------------
    let poll_proxy = proxy.clone();
    let poll_url = format!("{}/v1/tasks", gateway_url);
    tokio::spawn(async move {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        let mut last_running: Vec<String> = Vec::new();

        loop {
            tokio::time::sleep(Duration::from_secs(POLL_INTERVAL_SECS)).await;

            match client.get(&poll_url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    if let Ok(payload) = resp.json::<TaskListPayload>().await {
                        let running_now: Vec<String> = payload
                            .tasks
                            .iter()
                            .filter(|t| t.status == "Running")
                            .map(|t| t.task_id.clone())
                            .collect();

                        // Detect tasks that were running but no longer are
                        for old_id in &last_running {
                            if !running_now.iter().any(|id| id == old_id) {
                                if let Some(task) =
                                    payload.tasks.iter().find(|t| &t.task_id == old_id)
                                {
                                    let (summary, urgency) = match task.status.as_str() {
                                        "Completed" => ("✅ Task completed", None),
                                        "Failed" => ("❌ Task failed", Some(notify_rust::Urgency::Critical)),
                                        "Cancelled" => ("🚫 Task cancelled", None),
                                        _ => ("Task finished", None),
                                    };
                                    let mut notif = notify_rust::Notification::new();
                                    notif.summary(&format!("Clarity — {}", task.name))
                                        .body(summary);
                                    if let Some(u) = urgency {
                                        notif.urgency(u);
                                    }
                                    let _ = notif.show();
                                }
                            }
                        }

                        last_running = running_now;
                        let _ = poll_proxy.send_event(UserEvent::TaskUpdate(payload.tasks));
                    }
                }
                _ => {
                    // Gateway unavailable — silently degrade
                    let _ = poll_proxy.send_event(UserEvent::TaskUpdate(Vec::new()));
                }
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

    tracing::info!("Claw tray icon active. Right-click for menu, left-click for quick ask.");

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
                UserEvent::TaskUpdate(tasks) => {
                    let running = tasks.iter().filter(|t| t.status == "Running").count();
                    let pending = tasks.iter().filter(|t| t.status == "Pending").count();
                    let tooltip = if tasks.is_empty() {
                        "Clarity Claw — idle (no tasks)".to_string()
                    } else {
                        format!(
                            "Clarity Claw — {} running, {} pending ({} total)",
                            running,
                            pending,
                            tasks.len()
                        )
                    };
                    let _ = tray_icon.set_tooltip(Some(&tooltip));
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
                id if id == new_chat_item.id() => {
                    tracing::info!("Menu: New Chat");
                    let url = format!("{}/chat.html", gateway_url);
                    let _ = std::process::Command::new("cmd")
                        .args(&["/C", "start", "", &url])
                        .spawn();
                }
                id if id == view_tasks_item.id() => {
                    tracing::info!("Menu: View Tasks");
                    let url = format!("{}/chat.html", gateway_url);
                    let _ = std::process::Command::new("cmd")
                        .args(&["/C", "start", "", &url])
                        .spawn();
                }
                id if id == open_window_item.id() => {
                    tracing::info!("Menu: Open Window");
                    if let Some(ref win) = window {
                        win.set_visible(true);
                        win.set_focus();
                    }
                }
                id if id == quit_item.id() => {
                    tracing::info!("Menu: Quit");
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
                event: tao::event::WindowEvent::CloseRequested,
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
