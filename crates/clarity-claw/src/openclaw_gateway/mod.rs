//! OpenClaw Gateway client support.
//!
//! This module implements the client side of the OpenClaw JSON-RPC WebSocket
//! protocol spoken by local Kimi Desktop Gateways (`127.0.0.1:18679`). It is
//! kept separate from the native `clarity_gateway` protocol because the frame
//! formats are different.

pub mod chat;
pub mod client;
pub mod device;
pub mod kimi_file;
pub mod protocol;
pub mod session;
pub mod types;

pub use chat::{OpenClawChatApi, blocks_to_text};
pub use client::{OpenClawClientError, OpenClawEvent, OpenClawGatewayClient};
pub use device::{OpenClawDeviceApi, PairRequestResult};
pub use kimi_file::{
    KimiFileDownload, KimiFileMetadata, ensure_downloaded, parse_kimi_file_uri, resolve_metadata,
    sanitize_kimi_file_name,
};
pub use protocol::{
    ConnectParams, HelloOk, OpenClawAuth, OpenClawClientInfo, OpenClawDeviceProof,
    OpenClawErrorShape, OpenClawFeatures, OpenClawFrame, OpenClawPolicy, OpenClawServerInfo,
    build_cli_connect_params, build_device_connect_params,
};
pub use session::OpenClawSessionApi;
pub use types::{
    ChatBlock, ChatEvent, ChatHistory, ChatHistoryParams, ChatSendParams, OpenClawMessage,
    OpenClawSession, SessionList, SessionListParams,
};
