#![allow(dead_code)]

//! Window Manager — multi-window lifecycle for the Agent OS (egui path A).
//!
//! Path A: single process with multiple `eframe` windows.
//! Each window is bound to a Soul and communicates via the TierBus.
//!
//! # Architecture
//!
//! ```text
//! WindowManager
//!   ├── Window 0: 「格雷」常驻
//!   │     Soul: "grey", tools=[], model=local-gguf
//!   ├── Window 1: 「观察者」知识库
//!   │     Soul: "observer", tools=[file_read, web_search]
//!   ├── Window 2: 「专项 A」项目经理
//!   │     Soul: "pm-rust", tools=[file_write, shell, git]
//!   └── WmCommand channel (spawn / close / focus / inject)
//! ```

use std::collections::HashMap;

use clarity_core::soul::Soul;
use clarity_core::tier_bus::{TierBus, TierMessage};

// ============================================================================
// WindowState
// ============================================================================

/// State of a single managed window.
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub struct WindowState {
    /// Window identifier.
    pub id: String,
    /// Display title.
    pub title: String,
    /// Bound soul.
    pub soul_id: String,
    /// Whether this window is currently focused.
    pub focused: bool,
    /// Window position (x, y).
    pub position: (f32, f32),
    /// Window size (width, height).
    pub size: (f32, f32),
}

// ============================================================================
// WindowManager
// ============================================================================

/// Manages multiple egui windows in a single process.
///
/// This is the **Path A** implementation (single process, multiple windows).
/// Path B (gateway-as-hub thin client) and Path C (microkernel) are future
/// evolutions documented in `AGENT_OS_VISION.md`.
#[allow(dead_code)]
pub struct WindowManager {
    /// Known windows.
    windows: HashMap<String, WindowState>,
    /// soul_id → window_id (1:1 in path A).
    soul_bindings: HashMap<String, String>,
    /// Next window index for auto-naming.
    next_index: u32,
}

impl WindowManager {
    /// Create a new window manager.
    pub fn new() -> Self {
        Self {
            windows: HashMap::new(),
            soul_bindings: HashMap::new(),
            next_index: 0,
        }
    }

    /// Spawn a new window bound to a soul.
    ///
    /// Returns the window ID.
    pub fn spawn_window(&mut self, soul: &Soul) -> String {
        let id = format!("win-{}", self.next_index);
        self.next_index += 1;

        let state = WindowState {
            id: id.clone(),
            title: format!("{} — {}", soul.name, soul.id),
            soul_id: soul.id.clone(),
            focused: false,
            position: (100.0 + self.next_index as f32 * 50.0, 100.0),
            size: (1280.0, 800.0),
        };

        self.windows.insert(id.clone(), state);
        self.soul_bindings.insert(soul.id.clone(), id.clone());
        id
    }

    /// Close a window by ID.
    pub fn close_window(&mut self, window_id: &str) {
        if let Some(state) = self.windows.remove(window_id) {
            self.soul_bindings.remove(&state.soul_id);
        }
    }

    /// Focus a window.
    pub fn focus_window(&mut self, window_id: &str) {
        for (id, state) in &mut self.windows {
            state.focused = id == window_id;
        }
    }

    /// Get the window bound to a soul, if any.
    pub fn window_for_soul(&self, soul_id: &str) -> Option<&WindowState> {
        self.soul_bindings
            .get(soul_id)
            .and_then(|win_id| self.windows.get(win_id))
    }

    /// Inject a TierMessage into the window's soul context.
    ///
    /// In Path A this is a direct HashMap operation; in Path B/C
    /// this would serialize over IPC.
    pub fn inject_message(&self, _tier_bus: &TierBus, target_soul_id: &str, message: TierMessage) {
        // In Path A, we simply verify the window exists and
        // forward the message. The actual consumption happens
        // in the egui update loop.
        if self.soul_bindings.contains_key(target_soul_id) {
            // NOTE: Real implementation would push to a channel
            // or queue that the egui App reads each frame.
            let _ = message;
        }
    }

    /// List all managed windows.
    pub fn list_windows(&self) -> Vec<&WindowState> {
        self.windows.values().collect()
    }

    /// Currently focused window, if any.
    pub fn focused_window(&self) -> Option<&WindowState> {
        self.windows.values().find(|w| w.focused)
    }

    /// Total window count.
    pub fn len(&self) -> usize {
        self.windows.len()
    }

    /// Returns true when there are no windows.
    pub fn is_empty(&self) -> bool {
        self.windows.is_empty()
    }
}

impl Default for WindowManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spawn_and_close() {
        let mut wm = WindowManager::new();
        let soul = Soul::new("grey").with_name("Gray");

        let win_id = wm.spawn_window(&soul);
        assert_eq!(wm.len(), 1);
        assert_eq!(wm.window_for_soul("grey").unwrap().title, "Gray — grey");

        wm.close_window(&win_id);
        assert_eq!(wm.len(), 0);
        assert!(wm.window_for_soul("grey").is_none());
    }

    #[test]
    fn test_focus() {
        let mut wm = WindowManager::new();
        let w1 = wm.spawn_window(&Soul::new("a"));
        let w2 = wm.spawn_window(&Soul::new("b"));

        wm.focus_window(&w2);
        assert!(wm.focused_window().unwrap().id == w2);
        assert!(!wm.windows.get(&w1).unwrap().focused);
    }

    #[test]
    fn test_inject_message() {
        let wm = WindowManager::new();
        let mut wm = wm;
        wm.spawn_window(&Soul::new("target"));

        let bus = TierBus::new();
        let msg = TierMessage::ParentDirective {
            from: "parent".to_string(),
            to: "target".to_string(),
            payload: Default::default(),
            priority: clarity_core::tier_bus::Priority::Normal,
        };

        // Should not panic — target exists.
        wm.inject_message(&bus, "target", msg);

        // Non-existent target should be silently ignored.
        wm.inject_message(
            &bus,
            "missing",
            TierMessage::ChildQuery {
                from: "a".to_string(),
                to: "missing".to_string(),
                query: Default::default(),
            },
        );
    }
}
