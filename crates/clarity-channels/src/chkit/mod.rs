//! Shared channel primitives and utilities for Clarity channels.
//!
//! This module contains reusable building blocks used by the WeChat iLink
//! implementation and other channel adapters.

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
