//! System tray integration for Clarity Claw.
//!
//! Provides the tao event loop, tray icon, menu, OS notifications,
//! Gateway task polling, and wire message listening.

use crate::{TaskListPayload, TaskSummary, POLL_INTERVAL_SECS};
use clarity_wire::{Wire, WireMessage};
use notify::Watcher;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tao::{
    event::Event,
    event_loop::{ControlFlow, EventLoopBuilder},
    window::WindowBuilder,
};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    MouseButton, TrayIconBuilder, TrayIconEvent,
};

/// Custom events for the Tao event loop.
#[derive(Clone, Debug)]
pub enum UserEvent {
    /// A message arrived from the backend wire.
    WireMsg(WireMessage),
    /// Task list update from Gateway polling.
    TaskUpdate(Vec<TaskSummary>),
    /// Show quick input dialog.
    QuickInput,
    /// Show task creation dialog.
    CreateTask,
    /// Cancel a specific task.
    CancelTask(String),
}

/// Run the tray event loop.
///
/// This function blocks until the user selects "Quit" from the tray menu.
pub fn run() -> anyhow::Result<()> {
    let gateway_url = crate::resolve_gateway_url();

    // ------------------------------------------------------------------
    // Backend communication channel (wire)
    // ------------------------------------------------------------------
    let wire = Wire::new();
    let _soul = wire.soul_side().clone();
    let mut ui_side = wire.ui_side(true);

    // ------------------------------------------------------------------
    // Tray menu
    // ------------------------------------------------------------------
    let menu = Menu::new();
    let quick_ask_item = MenuItem::new("Quick Ask...", true, None);
    let new_chat_item = MenuItem::new("New Chat", true, None);
    let separator1 = PredefinedMenuItem::separator();
    let create_task_item = MenuItem::new("Create Task...", true, None);
    let refresh_tasks_item = MenuItem::new("Refresh Tasks", true, None);
    let view_tasks_item = MenuItem::new("View Tasks", true, None);
    let separator2 = PredefinedMenuItem::separator();
    let quit_item = MenuItem::new("Quit", true, None);

    let _ = menu.append(&quick_ask_item);
    let _ = menu.append(&new_chat_item);
    let _ = menu.append(&separator1);
    let _ = menu.append(&create_task_item);
    let _ = menu.append(&refresh_tasks_item);
    let _ = menu.append(&view_tasks_item);
    let _ = menu.append(&separator2);
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
    // Event loop
    // ------------------------------------------------------------------
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    // ------------------------------------------------------------------
    // Background: wire listener → OS notifications
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
    // Background: Gateway task polling
    // ------------------------------------------------------------------
    let poll_proxy = proxy.clone();
    let poll_url = format!("{}/v1/tasks", gateway_url);
    let tasks_dir = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join(".clarity")
        .join("tasks");
    let task_cache: Arc<Mutex<Vec<TaskSummary>>> = Arc::new(Mutex::new(Vec::new()));
    let task_cache_bg = task_cache.clone();

    tokio::spawn(async move {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        let mut last_running: Vec<String> = Vec::new();

        let (fs_tx, mut fs_rx) = tokio::sync::mpsc::channel::<notify::Result<notify::Event>>(10);
        let _watcher = if tasks_dir.exists() {
            match notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
                let _ = fs_tx.try_send(res);
            }) {
                Ok(mut w) => {
                    let _ = w.watch(&tasks_dir, notify::RecursiveMode::NonRecursive);
                    tracing::info!("Filesystem watcher active on {:?}", tasks_dir);
                    Some(w)
                }
                Err(e) => {
                    tracing::warn!("Failed to create filesystem watcher: {}", e);
                    None
                }
            }
        } else {
            None
        };

        loop {
            let timeout = tokio::time::sleep(Duration::from_secs(POLL_INTERVAL_SECS));
            tokio::select! {
                _ = timeout => {}
                _ = fs_rx.recv() => {
                    tracing::debug!("Filesystem change detected, refreshing tasks immediately");
                }
            }

            match client.get(&poll_url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    if let Ok(payload) = resp.json::<TaskListPayload>().await {
                        let tasks: Vec<TaskSummary> = payload
                            .tasks
                            .iter()
                            .map(|t| TaskSummary {
                                task_id: t.task_id.clone(),
                                name: t.name.clone(),
                                status: t.status.clone(),
                            })
                            .collect();
                        *task_cache_bg.lock().unwrap() = tasks.clone();

                        let running_now: Vec<String> = tasks
                            .iter()
                            .filter(|t| t.status == "Running")
                            .map(|t| t.task_id.clone())
                            .collect();

                        for old_id in &last_running {
                            if !running_now.iter().any(|id| id == old_id) {
                                if let Some(task) = tasks.iter().find(|t| &t.task_id == old_id) {
                                    let (summary, _urgency) =
                                        crate::classify_task_status(&task.status);
                                    let mut notif = notify_rust::Notification::new();
                                    notif
                                        .summary(&format!("Clarity — {}", task.name))
                                        .body(summary);
                                    #[cfg(target_os = "linux")]
                                    if let Some(u) = urgency {
                                        notif.urgency(u);
                                    }
                                    let _ = notif.show();
                                }
                            }
                        }

                        last_running = running_now;
                        let _ = poll_proxy.send_event(UserEvent::TaskUpdate(tasks.clone()));
                    }
                }
                _ => {
                    let _ = poll_proxy.send_event(UserEvent::TaskUpdate(Vec::new()));
                }
            }
        }
    });

    // ------------------------------------------------------------------
    // Hidden window (required for event loop)
    // ------------------------------------------------------------------
    let _window = WindowBuilder::new()
        .with_visible(false)
        .with_title("Clarity Claw")
        .with_inner_size(tao::dpi::LogicalSize::new(1, 1))
        .build(&event_loop)
        .ok();

    tracing::info!("Claw tray icon active.");

    // ------------------------------------------------------------------
    // Event loop body
    // ------------------------------------------------------------------
    event_loop.run(move |event, _event_loop, control_flow| {
        *control_flow = ControlFlow::Wait;

        if let Event::UserEvent(user_event) = &event {
            let gw_url = gateway_url.clone();
            let _cache = task_cache.clone();
            let proxy = proxy.clone();

            match user_event {
                UserEvent::QuickInput => {
                    tracing::info!("Quick Ask triggered");
                    let _proxy = proxy.clone();
                    std::thread::spawn(move || {
                        if let Some(input) =
                            prompt_input("Clarity Quick Ask", "Enter your message:")
                        {
                            let input = input.trim().to_string();
                            if !input.is_empty() {
                                let gw = gw_url.clone();
                                let rt = tokio::runtime::Runtime::new().unwrap();
                                match rt.block_on(crate::quick_chat(&gw, &input)) {
                                    Ok(reply) => {
                                        let _ = notify_rust::Notification::new()
                                            .summary("Clarity Reply")
                                            .body(&truncate_notification(&reply, 200))
                                            .show();
                                    }
                                    Err(e) => {
                                        let mut n = notify_rust::Notification::new();
                                        n.summary("Clarity Error");
                                        n.body(&format!("Failed: {}", e));
                                        #[cfg(target_os = "linux")]
                                        {
                                            n.urgency(notify_rust::Urgency::Critical);
                                        }
                                        let _ = n.show();
                                    }
                                }
                            }
                        }
                    });
                }
                UserEvent::CreateTask => {
                    tracing::info!("Create Task triggered");
                    let _proxy = proxy.clone();
                    std::thread::spawn(move || {
                        let name = prompt_input("New Task", "Task name:");
                        if let Some(name) = name {
                            let name = name.trim().to_string();
                            if !name.is_empty() {
                                let prompt =
                                    prompt_input("New Task", &format!("Prompt for '{}':", name));
                                if let Some(prompt) = prompt {
                                    let prompt = prompt.trim().to_string();
                                    if !prompt.is_empty() {
                                        let gw = gw_url.clone();
                                        let rt = tokio::runtime::Runtime::new().unwrap();
                                        match rt.block_on(crate::create_remote_task(
                                            &gw, &name, &prompt,
                                        )) {
                                            Ok(task_id) => {
                                                let _ = notify_rust::Notification::new()
                                                    .summary("Clarity Task Created")
                                                    .body(&format!("{} ({})", name, task_id))
                                                    .show();
                                            }
                                            Err(e) => {
                                                let mut n = notify_rust::Notification::new();
                                                n.summary("Clarity Error");
                                                n.body(&format!("Failed to create task: {}", e));
                                                #[cfg(target_os = "linux")]
                                                {
                                                    n.urgency(notify_rust::Urgency::Critical);
                                                }
                                                let _ = n.show();
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    });
                }
                UserEvent::CancelTask(task_id) => {
                    tracing::info!("Cancel task: {}", task_id);
                    let task_id = task_id.clone();
                    let gw = gw_url.clone();
                    std::thread::spawn(move || {
                        let rt = tokio::runtime::Runtime::new().unwrap();
                        match rt.block_on(crate::cancel_remote_task(&gw, &task_id)) {
                            Ok(()) => {
                                let _ = notify_rust::Notification::new()
                                    .summary("Clarity")
                                    .body(&format!("Task {} cancelled", task_id))
                                    .show();
                            }
                            Err(e) => {
                                tracing::warn!("Failed to cancel task {}: {}", task_id, e);
                            }
                        }
                    });
                }
                UserEvent::WireMsg(msg) => {
                    if let Some(text) = match msg {
                        WireMessage::ContentPart { text } => Some(text.clone()),
                        WireMessage::TurnBegin { user_input } => {
                            Some(format!("You: {}", user_input))
                        }
                        _ => None,
                    } {
                        let _ = notify_rust::Notification::new()
                            .summary("Clarity")
                            .body(&text)
                            .show();
                    }
                }
                UserEvent::TaskUpdate(tasks) => {
                    let running = tasks.iter().filter(|t| t.status == "Running").count();
                    let pending = tasks.iter().filter(|t| t.status == "Pending").count();
                    let tooltip = crate::format_tooltip(running, pending, tasks.len());
                    let _ = tray_icon.set_tooltip(Some(&tooltip));
                }
            }
        }

        // Tray icon click (left → quick ask)
        if let Ok(TrayIconEvent::Click {
            button: MouseButton::Left,
            ..
        }) = tray_channel.try_recv()
        {
            let _ = proxy.send_event(UserEvent::QuickInput);
        }

        // Tray menu events
        if let Ok(menu_event) = menu_channel.try_recv() {
            let id = menu_event.id;

            if id == quick_ask_item.id() {
                let _ = proxy.send_event(UserEvent::QuickInput);
            } else if id == new_chat_item.id() {
                let url = format!("{}/chat.html", gateway_url);
                let _ = open_url(&url);
            } else if id == create_task_item.id() {
                let _ = proxy.send_event(UserEvent::CreateTask);
            } else if id == refresh_tasks_item.id() {
                let _ = proxy.send_event(UserEvent::TaskUpdate(task_cache.lock().unwrap().clone()));
            } else if id == view_tasks_item.id() {
                let url = format!("{}/chat.html", gateway_url);
                let _ = open_url(&url);
            } else if id == quit_item.id() {
                tracing::info!("Menu: Quit");
                *control_flow = ControlFlow::Exit;
            } else {
                let id_str = id.0.as_str();
                if let Some(task_id) = id_str.strip_prefix("cancel-") {
                    let _ = proxy.send_event(UserEvent::CancelTask(task_id.to_string()));
                }
            }
        }
    });
}

/// Prompt for user input via native OS dialog.
fn prompt_input(title: &str, prompt: &str) -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        let ps_script = format!(
            "Add-Type -AssemblyName Microsoft.VisualBasic; $input = [Microsoft.VisualBasic.Interaction]::InputBox('{}', '{}'); if ($input) {{ Write-Output $input }}",
            prompt.replace('\'', "''"),
            title.replace('\'', "''")
        );
        let output = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", &ps_script])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .output()
            .ok()?;

        if output.status.success() {
            let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !s.is_empty() {
                return Some(s);
            }
        }
        None
    }

    #[cfg(target_os = "macos")]
    {
        let script = format!(
            r#"display dialog "{}" default answer "" with title "{}" buttons {{"Cancel", "OK"}} default button "OK""#,
            prompt.replace('"', "\\\""),
            title.replace('"', "\\\"")
        );
        let output = std::process::Command::new("osascript")
            .args(["-e", &script])
            .output()
            .ok()?;
        if output.status.success() {
            let s = String::from_utf8_lossy(&output.stdout);
            if let Some(text) = s.split("text returned:").nth(1) {
                let val = text.split(',').next().unwrap_or("").trim().to_string();
                if !val.is_empty() {
                    return Some(val);
                }
            }
        }
        None
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let output = std::process::Command::new("zenity")
            .args(["--entry", "--title", title, "--text", prompt])
            .output()
            .ok()?;
        if output.status.success() {
            let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !s.is_empty() {
                return Some(s);
            }
        }
        None
    }
}

/// Open a URL in the default browser (cross-platform).
fn open_url(url: &str) -> std::io::Result<std::process::Child> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(url).spawn()
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open").arg(url).spawn()
    }
}

/// Truncate a string to `max` chars, appending "...".
fn truncate_notification(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max])
    }
}
