//! Snapshot Store
//!
//! workspace snapshot list + modal state

use std::time::Instant;

/// Holds snapshot UI state.
pub struct SnapshotStore {
    /// Whether the snapshot history modal is open.
    pub modal_open: bool,
    /// Cached snapshot list loaded from the core service.
    pub snapshots: Vec<clarity_core::agent::snapshot::SnapshotInfo>,
    /// Snapshot currently selected for diff preview.
    pub selected_id: Option<usize>,
    /// Snapshot ID awaiting restore confirmation.
    pub confirm_restore_id: Option<usize>,
    /// Whether a restore operation is in-flight.
    pub restoring: bool,
    /// Cached diff preview text for the selected snapshot.
    pub preview: Option<String>,
    /// Last time the list was refreshed.
    pub last_refresh: Instant,
}

impl Default for SnapshotStore {
    fn default() -> Self {
        Self {
            modal_open: false,
            snapshots: Vec::new(),
            selected_id: None,
            confirm_restore_id: None,
            restoring: false,
            preview: None,
            last_refresh: Instant::now(),
        }
    }
}
