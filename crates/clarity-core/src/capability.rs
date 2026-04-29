//! Capability Discovery — surface-aware feature availability
//!
//! Temporarily hard-coded per surface. Will be replaced by dynamic
//! registration once egui gains Interactive/Plan approval UIs.

/// Query which approval modes are supported by a given UI surface.
///
/// # Arguments
/// * `surface` — frontend identifier: `"egui"`, `"tui"`, `"gateway"`, `"headless"`
///
/// # Returns
/// Slice of mode strings (e.g. `["yolo"]`, `["interactive", "yolo", "plan"]`).
///
/// # Note
/// This is a temporary hard-coded mapping. Once egui implements
/// Interactive/Plan approval UIs, this will be replaced by a
/// `register_surface()` + runtime lookup mechanism.
pub struct CapabilityRegistry;

impl CapabilityRegistry {
    pub fn supported_approval_modes(surface: &str) -> Vec<&'static str> {
        match surface {
            "egui" => vec!["yolo"],
            "tui" => vec!["interactive", "yolo", "plan"],
            "gateway" => vec!["yolo"],
            "headless" => vec!["yolo", "plan"],
            _ => vec!["yolo"],
        }
    }
}

// ============================================================================
// Unit tests
// ============================================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_egui_only_yolo() {
        let modes = CapabilityRegistry::supported_approval_modes("egui");
        assert_eq!(modes, vec!["yolo"]);
    }

    #[test]
    fn test_tui_all_modes() {
        let modes = CapabilityRegistry::supported_approval_modes("tui");
        assert_eq!(modes, vec!["interactive", "yolo", "plan"]);
    }

    #[test]
    fn test_gateway_yolo_only() {
        let modes = CapabilityRegistry::supported_approval_modes("gateway");
        assert_eq!(modes, vec!["yolo"]);
    }

    #[test]
    fn test_headless_yolo_and_plan() {
        let modes = CapabilityRegistry::supported_approval_modes("headless");
        assert_eq!(modes, vec!["yolo", "plan"]);
    }

    #[test]
    fn test_unknown_surface_fallback() {
        let modes = CapabilityRegistry::supported_approval_modes("unknown");
        assert_eq!(modes, vec!["yolo"]);
    }
}
