//! LSP (Language Server Protocol) stdio client integration.
//!
//! Provides a lightweight, hook-driven LSP client that:
//! - Spawns a language server subprocess (e.g. rust-analyzer).
//! - Sends `textDocument/didOpen` / `didChange` after file edits.
//! - Receives `textDocument/publishDiagnostics` and injects them into the
//!   agent conversation as a `Message::system` block.
//!
//! No `tower-lsp` or `lsp-types` dependency — just `tokio::process` + `serde_json`.

pub mod client;
pub mod config;
pub mod hook;
mod protocol;

pub use client::LspClient;
pub use config::LspClientConfig;
pub use hook::LspHook;
