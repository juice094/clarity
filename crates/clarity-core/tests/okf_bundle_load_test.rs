#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    missing_docs,
    unsafe_code
)]
//! OKF Bundle Load Test
//!
//! Verifies that a workspace OKF bundle loads without errors.
//! Uses an isolated temporary directory so the test does not depend on the
//! contents of the developer's real `~/.kimi_openclaw/workspace`.

use clarity_core::okf::load_bundle;
use std::io::Write;
use tempfile::TempDir;

#[test]
fn test_merged_workspace_okf_bundle_loads() {
    let dir = TempDir::new().expect("failed to create temp dir");
    let root = dir.path();

    let mut index =
        std::fs::File::create(root.join("index.md")).expect("failed to create index.md");
    index
        .write_all(b"# Index\n\n- [WAU](metrics/wau.md)\n")
        .expect("failed to write index.md");

    let metrics = root.join("metrics");
    std::fs::create_dir(&metrics).expect("failed to create metrics dir");

    let mut wau = std::fs::File::create(metrics.join("wau.md")).expect("failed to create wau.md");
    wau.write_all(b"---\ntype: Metric\ntitle: WAU\n---\n\n# Weekly Active Users\n")
        .expect("failed to write wau.md");

    let bundle = load_bundle(root).expect("failed to load OKF bundle");

    assert!(
        !bundle.concepts.is_empty(),
        "bundle loaded but contains no concepts"
    );
    assert_eq!(
        bundle.warnings.len(),
        0,
        "bundle should have no skipped files; skipped: {:?}",
        bundle.warnings
    );

    println!(
        "Loaded {} OKF concepts from {}",
        bundle.concepts.len(),
        root.display()
    );
}
