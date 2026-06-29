//! Animation helpers for panel transitions and UI micro-interactions.
//!
//! Provides frame-based interpolation with cubic easing suitable for
//! right-rail panel width transitions and content fade-in effects.

use std::time::Instant;

/// Cubic ease-out: fast start, smooth deceleration.
pub fn ease_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// Cubic ease-in-out: smooth start and end.
#[allow(dead_code)]
pub fn ease_in_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

/// State for a single float-valued animation (e.g. width, alpha).
#[derive(Clone)]
pub struct FloatAnimation {
    /// Where we started.
    pub from: f32,
    /// Where we're going.
    pub to: f32,
    /// When the animation started.
    pub started_at: Instant,
    /// Duration of the animation.
    pub duration_secs: f32,
    /// Whether the animation is complete (stops consuming CPU).
    pub done: bool,
}

impl FloatAnimation {
    /// Start a new animation from the current value to a target.
    pub fn start(from: f32, to: f32, duration_secs: f32) -> Self {
        Self {
            from,
            to,
            started_at: Instant::now(),
            duration_secs,
            done: (from - to).abs() < 0.5,
        }
    }

    /// Get the current interpolated value (cubic ease-out).
    pub fn current(&self) -> f32 {
        if self.done {
            return self.to;
        }
        let elapsed = self.started_at.elapsed().as_secs_f32();
        let t = (elapsed / self.duration_secs).clamp(0.0, 1.0);
        if t >= 1.0 {
            return self.to;
        }
        self.from + (self.to - self.from) * ease_out_cubic(t)
    }

    /// Mark the animation as complete (latch to target).
    #[allow(dead_code)]
    pub fn finish(&mut self) {
        self.done = true;
    }
}

impl Default for FloatAnimation {
    fn default() -> Self {
        Self {
            from: 0.0,
            to: 240.0,
            started_at: Instant::now(),
            duration_secs: 0.2,
            done: true,
        }
    }
}

/// Animation state used by the right-rail panel and the left-rail sidebar.
#[derive(Clone, Default)]
pub struct PanelAnimationState {
    /// Width animation for the right IDE panel.
    pub right_panel_width: FloatAnimation,
    /// Width animation for the left navigation rail.
    pub left_rail_width: FloatAnimation,
    /// Previous panel variant for detecting switches.
    pub prev_panel: Option<clarity_core::ui::RightRailPanel>,
}
