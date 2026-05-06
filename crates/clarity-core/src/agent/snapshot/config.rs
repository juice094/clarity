//! Configuration for the workspace snapshot service.

/// Configuration for per-turn Git snapshots.
#[derive(Debug, Clone)]
pub struct SnapshotConfig {
    /// Whether snapshots are enabled.
    pub enabled: bool,
    /// Maximum number of snapshots to keep per workspace.
    pub max_snapshots: usize,
}

impl Default for SnapshotConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_snapshots: 10,
        }
    }
}

impl SnapshotConfig {
    /// Create a new config with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable or disable snapshots.
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Set the max snapshot count.
    pub fn with_max_snapshots(mut self, max: usize) -> Self {
        self.max_snapshots = max;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snapshot_config_defaults() {
        let cfg = SnapshotConfig::default();
        assert!(cfg.enabled);
        assert_eq!(cfg.max_snapshots, 10);
    }

    #[test]
    fn test_snapshot_config_builder() {
        let cfg = SnapshotConfig::new()
            .with_enabled(false)
            .with_max_snapshots(5);
        assert!(!cfg.enabled);
        assert_eq!(cfg.max_snapshots, 5);
    }
}
