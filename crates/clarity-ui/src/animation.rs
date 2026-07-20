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

/// Linear interpolation: constant speed.
pub fn linear(t: f32) -> f32 {
    t.clamp(0.0, 1.0)
}

/// Named easing curve.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Easing {
    #[default]
    EaseOutCubic,
    EaseInOutCubic,
    Linear,
}

impl Easing {
    /// Apply the curve to a normalized time `t` (0.0–1.0).
    pub fn eval(self, t: f32) -> f32 {
        match self {
            Easing::EaseOutCubic => ease_out_cubic(t),
            Easing::EaseInOutCubic => ease_in_out_cubic(t),
            Easing::Linear => linear(t),
        }
    }
}

/// Semantic animation duration.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AnimationSpeed {
    #[default]
    Normal,
    Fast,
    Slow,
}

/// Theme-aware animation configuration.
///
/// Used with [`crate::theme::Theme::animate_bool`] and friends so the whole
/// app shares the same timing and easing language.
#[derive(Clone, Copy, Debug)]
pub struct AnimationConfig {
    pub speed: AnimationSpeed,
    pub easing: Easing,
}

impl AnimationConfig {
    /// Normal-speed ease-out animation (the app default).
    pub fn normal() -> Self {
        Self {
            speed: AnimationSpeed::Normal,
            easing: Easing::EaseOutCubic,
        }
    }

    /// Fast micro-interaction (hover, focus rings).
    pub fn fast() -> Self {
        Self {
            speed: AnimationSpeed::Fast,
            easing: Easing::EaseOutCubic,
        }
    }

    /// Slow, prominent transition (modals, large panels).
    pub fn slow() -> Self {
        Self {
            speed: AnimationSpeed::Slow,
            easing: Easing::EaseInOutCubic,
        }
    }

    /// Override the easing curve.
    pub fn with_easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
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
    /// Easing curve used for interpolation.
    pub easing: Easing,
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
            easing: Easing::EaseOutCubic,
        }
    }

    /// Start with a specific easing curve.
    pub fn start_with_easing(from: f32, to: f32, duration_secs: f32, easing: Easing) -> Self {
        Self {
            from,
            to,
            started_at: Instant::now(),
            duration_secs,
            done: (from - to).abs() < 0.5,
            easing,
        }
    }

    /// Get the current interpolated value.
    pub fn current(&self) -> f32 {
        if self.done {
            return self.to;
        }
        let elapsed = self.started_at.elapsed().as_secs_f32();
        let t = self.easing.eval(elapsed / self.duration_secs);
        if t >= 1.0 {
            return self.to;
        }
        self.from + (self.to - self.from) * t
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
            easing: Easing::EaseOutCubic,
        }
    }
}
