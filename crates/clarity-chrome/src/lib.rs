//! Clarity chrome shell — generic window chrome orchestration.
//!
//! The chrome is the outermost frame around every Clarity surface: titlebar,
//! left/right rails, status bar, input panel, overlays, modals, onboarding, and
//! window resize handles. It intentionally knows nothing about the concrete
//! application state; rendering is delegated to a [`ChromeRenderer`] implementation.
//!
//! The active sub-application (a [`ClarityApp`] from `clarity-shell`) is rendered
//! into the main stage by the renderer each frame. The concrete renderer selects
//! the active app from the supplied state.

/// Renderer callback that draws the full chrome around the active app.
///
/// The renderer receives the full window `Ui` and is responsible for laying out
/// the chrome regions. This keeps the generic shell agnostic of egui layout
/// details while giving the concrete implementation full control over sizing and
/// animation.
pub trait ChromeRenderer<State> {
    /// Draw the entire chrome: titlebar, rails, main stage, status/input panels,
    /// overlays, modals, onboarding, resize handles, etc.
    fn render(&mut self, state: &mut State, ui: &mut egui::Ui, ctx: &egui::Context);
}

/// Generic chrome shell.
///
/// `State` is the application-specific shared state (e.g. the egui `App`).
/// `Renderer` supplies the concrete chrome drawing callback.
pub struct Chrome<State, Renderer: ChromeRenderer<State>> {
    renderer: Renderer,
    active: usize,
    _phantom: std::marker::PhantomData<State>,
}

impl<State, Renderer: ChromeRenderer<State>> Chrome<State, Renderer> {
    /// Create a new chrome with the given renderer.
    pub fn new(renderer: Renderer) -> Self {
        Self {
            renderer,
            active: 0,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Switch to the app at `idx`. The renderer may use this index or derive
    /// the active app directly from `state`.
    pub fn set_active(&mut self, idx: usize) {
        self.active = idx;
    }

    /// Render the full chrome and the active sub-application.
    pub fn render(&mut self, state: &mut State, ui: &mut egui::Ui, ctx: &egui::Context) {
        self.renderer.render(state, ui, ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clarity_shell::ClarityAppResponse;

    /// Renderer that records that the render callback was invoked and forwards
    /// the active app render so its response can be asserted.
    struct MockRenderer {
        render_calls: usize,
        last_response: Option<ClarityAppResponse>,
    }

    impl MockRenderer {
        fn new() -> Self {
            Self {
                render_calls: 0,
                last_response: None,
            }
        }
    }

    impl ChromeRenderer<()> for MockRenderer {
        fn render(&mut self, _state: &mut (), ui: &mut egui::Ui, ctx: &egui::Context) {
            self.render_calls += 1;
            // Simulate a minimal main-stage response so the test can assert the
            // renderer ran through the active app path.
            let _ = ui.label("mock chrome");
            self.last_response = Some(ClarityAppResponse::Toast("main_stage".into()));
            ctx.request_repaint();
        }
    }

    #[test]
    fn set_active_stores_index() {
        let mut chrome: Chrome<(), MockRenderer> = Chrome::new(MockRenderer::new());
        assert_eq!(chrome.active, 0);

        chrome.set_active(2);
        assert_eq!(chrome.active, 2);
    }

    #[test]
    fn render_schedules_main_stage() {
        let renderer = MockRenderer::new();
        let mut chrome: Chrome<(), MockRenderer> = Chrome::new(renderer);

        let egui_ctx = egui::Context::default();
        let _output = egui_ctx.run_ui(egui::RawInput::default(), |egui_ctx| {
            egui::Area::new("chrome_test".into()).show(egui_ctx, |ui| {
                chrome.render(&mut (), ui, egui_ctx);
            });
        });

        assert!(chrome.renderer.render_calls > 0);
        assert_eq!(
            chrome.renderer.last_response,
            Some(ClarityAppResponse::Toast("main_stage".into()))
        );
    }
}
