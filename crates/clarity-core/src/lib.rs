//! # Clarity Core
//! 
//! Core engine for Project Clarity - An AI agent framework with tool registry.
//! 
//! ## Architecture
//! 
//! ```text
//! ┌─────────────┐     ┌──────────────┐     ┌─────────────┐
//! │    Agent    │────▶│ ToolRegistry │────▶│    Tools    │
//! │   (Loop)    │◄────│  (Discover)  │◄────│  (Execute)  │
//! └─────────────┘     └──────────────┘     └─────────────┘
//!       │                                            
//!       ▼                                            
//! ┌─────────────┐                                   
//! │  LLM Client │                                   
//! └─────────────┘                                   
//! ```
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use clarity_core::{Agent, ToolRegistry};
//! use clarity_core::tools::{FileReadTool, BashTool, GlobTool};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Create registry and register tools
//!     let mut registry = ToolRegistry::new();
//!     registry.register(FileReadTool::new())?;
//!     registry.register(BashTool::new())?;
//!     registry.register(GlobTool::new())?;
//!
//!     // Create agent with registry
//!     let agent = Agent::new(registry);
//!     
//!     // Run agent loop
//!     agent.run("List all Rust files in the project").await?;
//!     
//!     Ok(())
//! }
//! ```

pub mod agent;
pub mod approval;
pub mod compaction;
pub mod config;
pub mod error;
pub mod llm;
pub mod memory;
pub mod mcp;
pub mod personality;
pub mod registry;
pub mod subagents;
pub mod tools;

// Re-export core types
pub use agent::Agent;
pub use error::{ToolError, AgentError};
pub use llm::{AnthropicLlm, KimiLlm, OpenAiCompatibleLlm, LlmFactory};
pub use registry::ToolRegistry;
pub use tools::{Tool, ToolContext, ToolResult};

/// Prelude module for convenient imports
pub mod prelude {
    pub use crate::{
        Agent, Tool, ToolContext, ToolRegistry, ToolResult, ToolError, AgentError,
    };
    pub use async_trait::async_trait;
    pub use serde_json::Value;
}

/// Version of the clarity-core crate
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
