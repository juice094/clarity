//! OpenClaw / KimiClaw Gateway client and device identity support.
//!
//! This crate is UI-agnostic: it exposes a WebSocket JSON-RPC client, Ed25519
//! device-paired authentication, and Gateway discovery, without depending on
//! any frontend crate.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod client;
pub mod device;
pub mod discovery;
pub mod types;

pub use client::ClawClient;
pub use client::ClawClient as OpenClawClient;
pub use device::{DeviceIdentity, PairedToken, load_paired_token, save_paired_token};
pub use discovery::discover_openclaw_devices;
pub use types::{ClawConnection, ClawType, DeviceInfo, DeviceRecord, DeviceStatus};
