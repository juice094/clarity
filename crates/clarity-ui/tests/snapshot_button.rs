//! Snapshot tests for Clarity UI primitives.
//!
//! Run with `UPDATE_SNAPSHOTS=true cargo test -p clarity-ui --test snapshot_button`
//! to regenerate reference images.

use clarity_ui::widgets::button::Button;
use egui_kittest::Harness;

#[test]
fn snapshot_primary_button() {
    let mut harness = Harness::new_ui(|ui| {
        ui.add(Button::new("Save").primary());
    });

    harness.fit_contents();
    // This will write to tests/snapshots/snapshot_button__primary_button.png on
    // first run (if UPDATE_SNAPSHOTS=true) and compare on subsequent runs.
    harness.snapshot("primary_button");
}

#[test]
fn snapshot_button_variants() {
    let mut harness = Harness::new_ui(|ui| {
        ui.horizontal(|ui| {
            ui.add(Button::new("Primary").primary());
            ui.add(Button::new("Secondary").secondary());
            ui.add(Button::new("Ghost").ghost());
            ui.add(Button::new("Danger").danger());
        });
    });

    harness.fit_contents();
    harness.snapshot("button_variants");
}
