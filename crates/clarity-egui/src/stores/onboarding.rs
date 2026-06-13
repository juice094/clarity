//! Onboarding Store
//!
//! first-run wizard state

/// Holds onboarding UI state.
pub struct OnboardingStore {
    pub onboarding_state: crate::onboarding::OnboardingState,
    pub onboarding_progress_rx:
        Option<std::sync::mpsc::Receiver<clarity_core::model_download::ModelDownloadProgress>>,
    /// Set once when auto-download is triggered to prevent re-triggering every frame.
    pub downloading_auto: bool,
    /// Cancellation token for the active download task (IS-1 Sprint 31).
    pub cancel_token: Option<tokio_util::sync::CancellationToken>,
}
