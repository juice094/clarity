//! System tray integration for Clarity Claw.
//!
//! Provides the tao event loop, tray icon, menu, OS notifications,
//! Gateway task polling, and wire message listening.

use crate::{POLL_INTERVAL_SECS, TaskSummary, ThreadSummary};
use clarity_wire::{Wire, WireMessage};
use notify::Watcher;
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::Duration;
use tao::{
    event::Event,
    event_loop::{ControlFlow, EventLoopBuilder},
    window::WindowBuilder,
};
use tray_icon::{
    Icon, MouseButton, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu},
};

// ------------------------------------------------------------------
// Shared tokio runtime for tray callbacks (avoids spawning a new
// Runtime on every menu click).
// ------------------------------------------------------------------
fn tray_runtime() -> anyhow::Result<Arc<tokio::runtime::Runtime>> {
    static RUNTIME: Mutex<Option<Arc<tokio::runtime::Runtime>>> = Mutex::new(None);
    let mut guard = RUNTIME.lock();
    if guard.is_none() {
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| anyhow::anyhow!("Failed to create tray tokio runtime: {}", e))?;
        *guard = Some(Arc::new(rt));
    }
    match guard.as_ref() {
        Some(rt) => Ok(rt.clone()),
        None => Err(anyhow::anyhow!("tray runtime not initialized")),
    }
}

// ------------------------------------------------------------------
// Default icon (32×32 copper #c98a5e square)
// ------------------------------------------------------------------
fn default_icon() -> anyhow::Result<Icon> {
    const SIZE: usize = 32;
    let mut rgba = Vec::with_capacity(SIZE * SIZE * 4);
    for _ in 0..(SIZE * SIZE) {
        rgba.push(0xC9); // R
        rgba.push(0x8A); // G
        rgba.push(0x5E); // B
        rgba.push(0xFF); // A
    }
    Icon::from_rgba(rgba, SIZE as u32, SIZE as u32)
        .map_err(|e| anyhow::anyhow!("Failed to create tray icon: {}", e))
}

// ------------------------------------------------------------------
// Menu identifiers (stable across rebuilds)
// ------------------------------------------------------------------
const MENU_QUICK_ASK: &str = "quick-ask";
const MENU_NEW_CHAT: &str = "new-chat";
const MENU_CREATE_TASK: &str = "create-task";
const MENU_REFRESH_TASKS: &str = "refresh-tasks";
const MENU_VIEW_TASKS: &str = "view-tasks";
const MENU_RECENT_THREADS: &str = "recent-threads";
const MENU_THREAD_PREFIX: &str = "thread-";
const MENU_NO_THREADS: &str = "no-threads";
const MENU_QUIT: &str = "quit";

/// Build a thread label from a [`ThreadSummary`].
fn thread_label(thread: &ThreadSummary) -> String {
    thread.title.clone().unwrap_or_else(|| {
        let len = thread.thread_id.len();
        format!("Thread {}", &thread.thread_id[..8.min(len)])
    })
}

/// Build the tray menu, including a dynamic "Recent Threads" submenu.
fn build_tray_menu(threads: &[ThreadSummary]) -> Menu {
    let menu = Menu::new();
    let _ = menu.append(&MenuItem::with_id(
        MENU_QUICK_ASK,
        "Quick Ask...",
        true,
        None,
    ));
    let _ = menu.append(&MenuItem::with_id(MENU_NEW_CHAT, "New Chat", true, None));
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&MenuItem::with_id(
        MENU_CREATE_TASK,
        "Create Task...",
        true,
        None,
    ));
    let _ = menu.append(&MenuItem::with_id(
        MENU_REFRESH_TASKS,
        "Refresh Tasks",
        true,
        None,
    ));
    let _ = menu.append(&MenuItem::with_id(
        MENU_VIEW_TASKS,
        "View Tasks",
        true,
        None,
    ));

    let threads_menu = Submenu::with_id(MENU_RECENT_THREADS, "Recent Threads", true);
    if threads.is_empty() {
        let _ = threads_menu.append(&MenuItem::with_id(
            MENU_NO_THREADS,
            "No recent threads",
            false,
            None,
        ));
    } else {
        for thread in threads.iter().take(5) {
            let id = format!("{MENU_THREAD_PREFIX}{}", thread.thread_id);
            let label = thread_label(thread);
            let _ = threads_menu.append(&MenuItem::with_id(&id, &label, true, None));
        }
    }
    let _ = menu.append(&threads_menu);

    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&MenuItem::with_id(MENU_QUIT, "Quit", true, None));
    menu
}

// ------------------------------------------------------------------
// Single-instance guard (cross-platform: TCP port binding)
// ------------------------------------------------------------------

/// Ensure only one Clarity Claw process binds the local instance port.
///
/// Returns `true` if this is the first instance, `false` if another
/// instance is already running.
pub fn ensure_single_instance() -> bool {
    use std::net::TcpListener;

    static INSTANCE_LOCK: Mutex<Option<TcpListener>> = Mutex::new(None);
    let mut lock = INSTANCE_LOCK.lock();
    if lock.is_some() {
        return false;
    }
    match TcpListener::bind("127.0.0.1:51987") {
        Ok(listener) => {
            *lock = Some(listener);
            true
        }
        Err(_) => false,
    }
}

/// Custom events for the Tao event loop.
#[derive(Clone, Debug)]
pub enum UserEvent {
    /// A message arrived from the backend wire.
    WireMsg(WireMessage),
    /// Task list update from Gateway polling.
    TaskUpdate(Vec<TaskSummary>),
    /// Thread list update from Gateway polling.
    ThreadUpdate(Vec<ThreadSummary>),
    /// Show quick input dialog.
    QuickInput,
    /// Show task creation dialog.
    CreateTask,
    /// Request graceful shutdown (e.g. Ctrl+C).
    Quit,
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
    let mut ui_side = wire.ui_side(true);

    // ------------------------------------------------------------------
    // Tray menu and icon
    // ------------------------------------------------------------------
    let thread_cache: Arc<Mutex<Vec<ThreadSummary>>> = Arc::new(Mutex::new(Vec::new()));
    let initial_menu = build_tray_menu(&thread_cache.lock());
    let tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(initial_menu))
        .with_tooltip("Clarity Claw — connecting...")
        .with_icon(default_icon()?)
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
    // Background: Ctrl+C graceful shutdown
    // ------------------------------------------------------------------
    let ctrlc_proxy = proxy.clone();
    tokio::spawn(async move {
        if let Err(e) = tokio::signal::ctrl_c().await {
            tracing::warn!("Failed to listen for Ctrl+C: {}", e);
            return;
        }
        tracing::info!("Ctrl+C received — requesting graceful shutdown");
        let _ = ctrlc_proxy.send_event(UserEvent::Quit);
    });

    // ------------------------------------------------------------------
    // Background: wire listener → OS notifications
    // ------------------------------------------------------------------
    let notify_proxy = proxy.clone();
    tokio::spawn(async move {
        while let Some(msg) = ui_side.recv().await {
            let _ = notify_proxy.send_event(UserEvent::WireMsg(msg.clone()));

            let body = match &msg {
                WireMessage::StatusUpdate { message, .. } => Some(message.clone()),
                WireMessage::ContentPart { text, .. } => Some(text.clone()),
                WireMessage::TurnBegin { user_input, .. } => Some(format!("You: {}", user_input)),
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
    let poll_gateway_url = gateway_url.clone();
    let tasks_dir = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join(".clarity")
        .join("tasks");
    let task_cache: Arc<Mutex<Vec<TaskSummary>>> = Arc::new(Mutex::new(Vec::new()));
    let task_cache_bg = task_cache.clone();
    let thread_cache_bg = thread_cache.clone();

    tokio::spawn(async move {
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

            match crate::poll_tasks(&poll_gateway_url).await {
                Ok(tasks) => {
                    *task_cache_bg.lock() = tasks.clone();

                    let running_now: Vec<String> = tasks
                        .iter()
                        .filter(|t| t.status == "Running")
                        .map(|t| t.task_id.clone())
                        .collect();

                    for old_id in &last_running {
                        if !running_now.iter().any(|id| id == old_id) {
                            if let Some(task) = tasks.iter().find(|t| &t.task_id == old_id) {
                                let (summary, urgency) = crate::classify_task_status(&task.status);
                                let mut notif = notify_rust::Notification::new();
                                notif
                                    .summary(&format!("Clarity — {}", task.name))
                                    .body(summary);
                                #[cfg(target_os = "linux")]
                                if let Some(u) = urgency {
                                    notif.urgency(u);
                                }
                                #[cfg(not(target_os = "linux"))]
                                let _ = urgency;
                                let _ = notif.show();
                            }
                        }
                    }

                    last_running = running_now;
                    let _ = poll_proxy.send_event(UserEvent::TaskUpdate(tasks));
                }
                Err(e) => {
                    tracing::warn!("Failed to poll tasks: {}", e);
                    let _ = poll_proxy.send_event(UserEvent::TaskUpdate(Vec::new()));
                }
            }

            match crate::poll_threads(&poll_gateway_url).await {
                Ok(threads) => {
                    *thread_cache_bg.lock() = threads.clone();
                    let _ = poll_proxy.send_event(UserEvent::ThreadUpdate(threads));
                }
                Err(e) => {
                    tracing::warn!("Failed to poll threads: {}", e);
                    let _ = poll_proxy.send_event(UserEvent::ThreadUpdate(Vec::new()));
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

        if let Event::LoopDestroyed = event {
            tracing::info!("Tray event loop destroyed — cleaning up icon");
            let _ = tray_icon.set_visible(false);
            return;
        }

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
                                let rt = match tray_runtime() {
                                    Ok(rt) => rt,
                                    Err(e) => {
                                        tracing::error!("{}", e);
                                        let _ = notify_rust::Notification::new()
                                            .summary("Clarity Error")
                                            .body(&format!("Runtime initialization failed: {}", e))
                                            .show();
                                        return;
                                    }
                                };
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
                                        let rt = match tray_runtime() {
                                            Ok(rt) => rt,
                                            Err(e) => {
                                                tracing::error!("{}", e);
                                                let _ = notify_rust::Notification::new()
                                                    .summary("Clarity Error")
                                                    .body(&format!(
                                                        "Runtime initialization failed: {}",
                                                        e
                                                    ))
                                                    .show();
                                                return;
                                            }
                                        };
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
                UserEvent::WireMsg(msg) => {
                    if let Some(text) = match msg {
                        WireMessage::ContentPart { text, .. } => Some(text.clone()),
                        WireMessage::TurnBegin { user_input, .. } => {
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
                    let threads = thread_cache.lock();
                    let tooltip =
                        crate::format_tooltip(running, pending, tasks.len(), threads.len());
                    let _ = tray_icon.set_tooltip(Some(&tooltip));
                }
                UserEvent::ThreadUpdate(threads) => {
                    let tasks = task_cache.lock();
                    let running = tasks.iter().filter(|t| t.status == "Running").count();
                    let pending = tasks.iter().filter(|t| t.status == "Pending").count();
                    let tooltip =
                        crate::format_tooltip(running, pending, tasks.len(), threads.len());
                    let _ = tray_icon.set_tooltip(Some(&tooltip));
                    let new_menu = build_tray_menu(threads);
                    tray_icon.set_menu(Some(Box::new(new_menu)));
                }
                UserEvent::Quit => {
                    tracing::info!("Graceful shutdown requested");
                    let _ = tray_icon.set_visible(false);
                    *control_flow = ControlFlow::Exit;
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
            let id_str = menu_event.id.0.as_str();

            match id_str {
                MENU_QUICK_ASK => {
                    let _ = proxy.send_event(UserEvent::QuickInput);
                }
                MENU_NEW_CHAT => {
                    let gw = gateway_url.clone();
                    std::thread::spawn(move || {
                        let rt = match tray_runtime() {
                            Ok(rt) => rt,
                            Err(e) => {
                                tracing::error!("{}", e);
                                return;
                            }
                        };
                        match rt.block_on(crate::create_remote_thread(&gw, None)) {
                            Ok(thread_id) => {
                                let url = format!("{}/chat.html?thread_id={}", gw, thread_id);
                                let _ = open_url(&url);
                            }
                            Err(e) => {
                                tracing::warn!("Failed to create thread: {}", e);
                                let _ = notify_rust::Notification::new()
                                    .summary("Clarity")
                                    .body(&format!("Could not start a new chat: {}", e))
                                    .show();
                            }
                        }
                    });
                }
                MENU_CREATE_TASK => {
                    let _ = proxy.send_event(UserEvent::CreateTask);
                }
                MENU_REFRESH_TASKS => {
                    let _ = proxy.send_event(UserEvent::TaskUpdate(task_cache.lock().clone()));
                }
                MENU_VIEW_TASKS => {
                    let url = format!("{}/chat.html", gateway_url);
                    let _ = open_url(&url);
                }
                MENU_QUIT => {
                    tracing::info!("Menu: Quit");
                    *control_flow = ControlFlow::Exit;
                }
                _ => {
                    if let Some(thread_id) = id_str.strip_prefix(MENU_THREAD_PREFIX) {
                        let url = format!("{}/chat.html?thread_id={}", gateway_url, thread_id);
                        let _ = open_url(&url);
                    }
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
