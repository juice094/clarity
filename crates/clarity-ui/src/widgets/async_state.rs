//! Reusable async-state container and empty/loading/error placeholders.
//!
//! Replaces ad-hoc `loading: bool` + `error: Option<String>` pairs with a
//! single three-state enum that the UI can render consistently.

use crate::theme::Theme;

/// Generic state for an asynchronously loaded value.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum AsyncState<T> {
    /// Nothing has happened yet or a load is in progress.
    #[default]
    Loading,
    /// The value loaded successfully.
    Ready(T),
    /// The load failed.
    Error {
        /// Human-readable failure reason.
        message: String,
    },
}

impl<T> AsyncState<T> {
    /// Return `true` if the state is `Loading`.
    pub fn is_loading(&self) -> bool {
        matches!(self, AsyncState::Loading)
    }

    /// Return `true` if the state is `Error`.
    #[allow(dead_code)]
    pub fn is_error(&self) -> bool {
        matches!(self, AsyncState::Error { .. })
    }

    /// Return the ready value, if any.
    pub fn ready(&self) -> Option<&T> {
        match self {
            AsyncState::Ready(v) => Some(v),
            _ => None,
        }
    }

    /// Map the ready value to a different type, preserving Loading/Error.
    #[allow(dead_code)]
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> AsyncState<U> {
        match self {
            AsyncState::Loading => AsyncState::Loading,
            AsyncState::Ready(v) => AsyncState::Ready(f(v)),
            AsyncState::Error { message } => AsyncState::Error { message },
        }
    }
}

/// Render a centered placeholder for an empty list/state.
///
/// Returns `true` if anything was rendered.
pub fn render_empty_state(
    ui: &mut egui::Ui,
    theme: &Theme,
    icon: &str,
    title: impl Into<String>,
    message: impl Into<String>,
) {
    ui.vertical_centered(|ui| {
        ui.add_space(ui.available_height() * 0.25);
        ui.label(
            egui::RichText::new(icon)
                .size(theme.text_2xl)
                .color(theme.text_dim),
        );
        ui.add_space(theme.space_8);
        ui.label(
            egui::RichText::new(title.into())
                .size(theme.text_base)
                .color(theme.text)
                .strong(),
        );
        ui.add_space(theme.space_4);
        ui.label(
            egui::RichText::new(message.into())
                .size(theme.text_sm)
                .color(theme.text_muted),
        );
    });
}

/// Render a centered loading spinner with a message.
pub fn render_loading_state(ui: &mut egui::Ui, theme: &Theme, message: impl Into<String>) {
    ui.vertical_centered(|ui| {
        ui.add_space(ui.available_height() * 0.3);
        ui.spinner();
        ui.add_space(theme.space_8);
        ui.label(
            egui::RichText::new(message.into())
                .size(theme.text_sm)
                .color(theme.text_dim),
        );
    });
}

/// Render a centered error placeholder with an optional retry callback.
pub fn render_error_state(
    ui: &mut egui::Ui,
    theme: &Theme,
    title: impl Into<String>,
    message: impl Into<String>,
    retry: Option<&mut dyn FnMut()>,
) {
    ui.vertical_centered(|ui| {
        ui.add_space(ui.available_height() * 0.25);
        ui.label(
            egui::RichText::new(crate::theme::ICON_X)
                .size(theme.text_2xl)
                .color(theme.danger),
        );
        ui.add_space(theme.space_8);
        ui.label(
            egui::RichText::new(title.into())
                .size(theme.text_base)
                .color(theme.danger)
                .strong(),
        );
        ui.add_space(theme.space_4);
        ui.label(
            egui::RichText::new(message.into())
                .size(theme.text_sm)
                .color(theme.text_muted),
        );
        if let Some(on_retry) = retry {
            ui.add_space(theme.space_12);
            if ui
                .button(egui::RichText::new("Retry").color(theme.accent))
                .clicked()
            {
                on_retry();
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn async_state_helpers() {
        let state: AsyncState<i32> = AsyncState::Loading;
        assert!(state.is_loading());
        assert!(!state.is_error());
        assert!(state.ready().is_none());

        let state = AsyncState::Ready(42);
        assert_eq!(state.ready(), Some(&42));

        let state = AsyncState::<i32>::Error {
            message: "fail".into(),
        };
        assert!(state.is_error());
        assert_eq!(state.map(|v| v * 2).ready(), None);
    }

    #[test]
    fn async_state_map() {
        let state = AsyncState::Ready(5);
        let mapped = state.map(|v| v.to_string());
        assert_eq!(mapped.ready(), Some(&"5".to_string()));
    }
}
