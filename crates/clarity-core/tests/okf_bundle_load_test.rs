#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    missing_docs,
    unsafe_code
)]
//! OKF Bundle Load Test
//!
//! Verifies that the merged workspace OKF bundle loads without errors.

use clarity_core::okf::load_bundle;
use std::path::PathBuf;

#[test]
fn test_merged_workspace_okf_bundle_loads() {
    let home = std::env::var("USERPROFILE").expect("USERPROFILE not set");
    let bundle_root: PathBuf = [home.as_str(), ".kimi_openclaw", "workspace"]
        .iter()
        .collect();

    assert!(
        bundle_root.exists(),
        "workspace bundle root does not exist: {}",
        bundle_root.display()
    );

    let bundle = load_bundle(&bundle_root).expect("failed to load OKF bundle");

    assert!(
        !bundle.concepts.is_empty(),
        "bundle loaded but contains no concepts"
    );

    assert_eq!(
        bundle.warnings.len(),
        0,
        "bundle should have no skipped files after repair; skipped: {:?}",
        bundle.warnings
    );

    println!(
        "Loaded {} OKF concepts from {}",
        bundle.concepts.len(),
        bundle_root.display()
    );
}
