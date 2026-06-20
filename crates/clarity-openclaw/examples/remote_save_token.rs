//! Persist an OpenClaw pairing record for automatic reconnection.
//!
//! The saved record is read by `clarity_openclaw::load_paired_token()` and
//! surfaced in egui as a discovered bot instance.
//!
//! Usage:
//!   OPENCLAW_REMOTE_URL=ws://host:18789 \
//!   OPENCLAW_ADMIN_TOKEN=<admin-token> \
//!   OPENCLAW_DEVICE_TOKEN=<device-token> \
//!     cargo run -p clarity-openclaw --example remote_save_token

#![allow(clippy::unwrap_used, clippy::expect_used)]

use clarity_openclaw::PairedToken;

fn main() {
    let gateway_url = std::env::var("OPENCLAW_REMOTE_URL")
        .expect("set OPENCLAW_REMOTE_URL env var, e.g. ws://host:18789");
    let admin_token =
        std::env::var("OPENCLAW_ADMIN_TOKEN").expect("set OPENCLAW_ADMIN_TOKEN env var");
    let device_token =
        std::env::var("OPENCLAW_DEVICE_TOKEN").expect("set OPENCLAW_DEVICE_TOKEN env var");

    let record = PairedToken {
        gateway_url,
        token: admin_token,
        device_token: Some(device_token),
        role: "operator".into(),
        scopes: vec![
            "operator.admin".into(),
            "operator.read".into(),
            "operator.write".into(),
            "operator.approvals".into(),
            "operator.pairing".into(),
            "operator.talk.secrets".into(),
        ],
        paired_at_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64,
    };

    clarity_openclaw::save_paired_token(&record).expect("save paired token");
    println!("Saved OpenClaw pairing record.");
    println!("egui will auto-discover it on next launch.");
}
