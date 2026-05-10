//! System tray integration for Clarity egui desktop.
//!
//! Provides minimize-to-tray functionality with a context menu:
//!   - Show / Hide window
//!   - Copy session link
//!   - Pause / resume Agent
//!   - Notifications toggle
//!   - Open Settings
//!   - Quit
//!
//! The tray icon is generated procedurally as a 32×32 "node wave" (centre dot +
//! two pairs of outward arcs) and supports dynamic colour switching based on
//! runtime state (idle / active / error / message).

use tray_icon::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

/// Runtime state reflected in the tray icon colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayIconStatus {
    /// Background — no active work.
    Idle,
    /// Agent is running a task.
    Active,
    /// Gateway down or core error.
    Error,
    /// Pending user approval or completed task awaiting attention.
    Message,
}

/// Actions that can originate from the tray context menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayAction {
    Show,
    CopySessionLink,
    Pause,
    Settings,
    Quit,
}

/// Holds the tray icon and menu item references so we can match events.
pub struct TrayManager {
    tray_icon: TrayIcon,
    show_item: MenuItem,
    copy_link_item: MenuItem,
    pause_item: MenuItem,
    settings_item: MenuItem,
    quit_item: MenuItem,
    current_status: TrayIconStatus,
}

impl TrayManager {
    /// Create the tray icon with a procedurally-generated icon and context menu.
    ///
    /// Returns `None` if the tray subsystem fails to initialize.
    pub fn new() -> Option<Self> {
        let icon = Self::build_icon(TrayIconStatus::Idle);
        let menu = Menu::new();

        let show_item = MenuItem::new("Show Clarity", true, None);
        let copy_link_item = MenuItem::new("Copy Session Link", true, None);
        let pause_item = MenuItem::new("Pause Agent", true, None);
        let settings_item = MenuItem::new("Open Settings", true, None);
        let quit_item = MenuItem::new("Quit Clarity", true, None);

        menu.append(&show_item).ok()?;
        menu.append(&PredefinedMenuItem::separator()).ok()?;
        menu.append(&copy_link_item).ok()?;
        menu.append(&pause_item).ok()?;

        // Notification submenu
        let notif_sub = Submenu::new("Notifications", true);
        let _notif_all = MenuItem::new("All", true, None);
        let _notif_errors = MenuItem::new("Errors only", true, None);
        let _notif_silent = MenuItem::new("Silent", true, None);
        notif_sub.append(&_notif_all).ok()?;
        notif_sub.append(&_notif_errors).ok()?;
        notif_sub.append(&_notif_silent).ok()?;
        menu.append(&notif_sub).ok()?;
        menu.append(&PredefinedMenuItem::separator()).ok()?;
        menu.append(&settings_item).ok()?;
        menu.append(&quit_item).ok()?;

        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("Clarity AI Runtime")
            .with_icon(icon)
            .build()
            .ok()?;

        Some(Self {
            tray_icon,
            show_item,
            copy_link_item,
            pause_item,
            settings_item,
            quit_item,
            current_status: TrayIconStatus::Idle,
        })
    }

    /// Update the tray icon colour to reflect the current runtime state.
    pub fn set_status(&mut self, status: TrayIconStatus) {
        if self.current_status == status {
            return;
        }
        self.current_status = status;
        let new_icon = Self::build_icon(status);
        if let Err(e) = self.tray_icon.set_icon(Some(new_icon)) {
            tracing::warn!("Failed to update tray icon: {}", e);
        }
        let tooltip = match status {
            TrayIconStatus::Idle => "Clarity — Idle",
            TrayIconStatus::Active => "Clarity — Agent running",
            TrayIconStatus::Error => "Clarity — Gateway offline",
            TrayIconStatus::Message => "Clarity — Attention needed",
        };
        if let Err(e) = self.tray_icon.set_tooltip(Some(tooltip)) {
            tracing::warn!("Failed to update tray tooltip: {}", e);
        }
    }

    /// Poll tray icon click events.
    pub fn poll_tray_events(&self) -> Vec<tray_icon::TrayIconEvent> {
        let rx = tray_icon::TrayIconEvent::receiver();
        let mut events = Vec::new();
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }
        events
    }

    /// Poll menu item click events and map them to [`TrayAction`].
    pub fn poll_menu_events(&self) -> Vec<TrayAction> {
        let rx = tray_icon::menu::MenuEvent::receiver();
        let mut actions = Vec::new();
        while let Ok(event) = rx.try_recv() {
            let id = event.id;
            if id == *self.show_item.id() {
                actions.push(TrayAction::Show);
            } else if id == *self.copy_link_item.id() {
                actions.push(TrayAction::CopySessionLink);
            } else if id == *self.pause_item.id() {
                actions.push(TrayAction::Pause);
            } else if id == *self.settings_item.id() {
                actions.push(TrayAction::Settings);
            } else if id == *self.quit_item.id() {
                actions.push(TrayAction::Quit);
            }
        }
        actions
    }

    /// Build a 32×32 icon for the given status.
    fn build_icon(status: TrayIconStatus) -> Icon {
        let (color, with_dot) = match status {
            TrayIconStatus::Idle => ([0xc1, 0xc2, 0xc5, 0xff], false),
            TrayIconStatus::Active => ([0x4d, 0xab, 0xf7, 0xff], false),
            TrayIconStatus::Error => ([0xff, 0x6b, 0x6b, 0xff], false),
            TrayIconStatus::Message => ([0x4d, 0xab, 0xf7, 0xff], true),
        };
        let rgba = generate_node_wave_rgba(32, color, with_dot);
        Icon::from_rgba(rgba, 32, 32).expect("valid 32×32 icon")
    }
}

/// Generate a "node wave" icon: a filled centre circle + two pairs of outward
/// arcs. When `with_dot` is true, a 4px red badge is drawn in the top-right
/// corner for the "message" state.
fn generate_node_wave_rgba(size: u32, color: [u8; 4], with_dot: bool) -> Vec<u8> {
    let mut pixels = vec![0u8; (size * size * 4) as usize];
    let cx = size as f32 / 2.0;
    let cy = size as f32 / 2.0 - 1.0; // nudge up slightly for visual balance
    let stroke = 1.6_f32;

    for y in 0..size {
        for x in 0..size {
            let idx = ((y * size + x) * 4) as usize;
            let px = x as f32;
            let py = y as f32;

            let mut alpha = 0.0_f32;

            // Centre solid circle
            let dist_center = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
            if dist_center < size as f32 / 7.0 {
                alpha = 1.0;
            }

            // Inner arcs — closer to the centre, steeper curve
            for (arc_cx, arc_cy, r) in [
                (cx - 3.5, cy, size as f32 / 5.0),
                (cx + 3.5, cy, size as f32 / 5.0),
            ] {
                let d = ((px - arc_cx).powi(2) + (py - arc_cy).powi(2)).sqrt();
                if (d - r).abs() < stroke && px > arc_cx.min(cx) && px < arc_cx.max(cx) && py > cy {
                    alpha = 1.0;
                }
            }

            // Outer arcs — wider, gentler curve
            for (arc_cx, arc_cy, r) in [
                (cx - 6.5, cy, size as f32 / 3.3),
                (cx + 6.5, cy, size as f32 / 3.3),
            ] {
                let d = ((px - arc_cx).powi(2) + (py - arc_cy).powi(2)).sqrt();
                if (d - r).abs() < stroke && px > arc_cx.min(cx) && px < arc_cx.max(cx) && py > cy {
                    alpha = 1.0;
                }
            }

            if alpha > 0.0 {
                pixels[idx] = color[0];
                pixels[idx + 1] = color[1];
                pixels[idx + 2] = color[2];
                pixels[idx + 3] = (color[3] as f32 * alpha) as u8;
            }
        }
    }

    // Red badge (4px) in top-right corner for Message state
    if with_dot {
        let dot_color = [0xff, 0x6b, 0x6b, 0xff];
        let dot_r = 3.0_f32;
        let dot_cx = size as f32 - dot_r - 1.0;
        let dot_cy = dot_r + 1.0;
        for y in 0..size {
            for x in 0..size {
                let idx = ((y * size + x) * 4) as usize;
                let d = ((x as f32 - dot_cx).powi(2) + (y as f32 - dot_cy).powi(2)).sqrt();
                if d < dot_r {
                    let a = (1.0 - d / dot_r).clamp(0.0, 1.0);
                    pixels[idx] = dot_color[0];
                    pixels[idx + 1] = dot_color[1];
                    pixels[idx + 2] = dot_color[2];
                    pixels[idx + 3] = (dot_color[3] as f32 * a) as u8;
                }
            }
        }
    }

    pixels
}
