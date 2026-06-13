//! ZeroClaw-compatible channel primitives and utilities.
//!
//! This module ports the battle-tested channel infrastructure from
//! ZeroClaw into Clarity without depending on the transitional
//! `zeroclaw-runtime` crate.

pub mod allowlist;
pub mod channel;
#[doc(hidden)]
pub mod i18n;
pub mod log;
pub mod pairing;
#[doc(hidden)]
pub mod spawn;
#[doc(hidden)]
pub mod util;
pub mod wechat;

pub use channel::{
    Channel, ChannelApprovalRequest, ChannelApprovalResponse, ChannelMessage, MediaAttachment,
    SendMessage,
};
pub use pairing::PairingGuard;
