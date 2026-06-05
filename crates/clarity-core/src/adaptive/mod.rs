//! Adaptive Agent Engine — data-driven behavior optimization for Clarity agents.
//!
//! This module enables agents to learn from historical telemetry and autonomously
//! optimize their behavior across three dimensions:
//!
//! | Dimension | Component | Decisions |
//! |-----------|-----------|-----------|
//! | Model routing | [`AdaptiveModelRouter`] | Which provider/model to use per task |
//! | Context compression | [`CompressionOptimizer`] | When and how to compact conversation |
//! | Growth tracking | [`AgentGrowthProfile`] | Skill mastery, tool effectiveness, preference evolution |
//!
//! ## Architecture
//!
//! ```text
//! Telemetry events (clarity-telemetry)
//!          │
//!          ▼
//! ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
//! │ AdaptiveModel   │    │ Compression     │    │ AgentGrowth     │
//! │ Router          │    │ Optimizer       │    │ Profile         │
//! │                 │    │                 │    │                 │
//! │ - EWMA latency  │    │ - Token eff.    │    │ - Skill mastery │
//! │ - Error rate    │    │   curve         │    │ - Tool stats    │
//! │ - Cost tracking │    │ - Summary       │    │ - Model prefs   │
//! │ - Quality score │    │   retention     │    │ - User patterns │
//! └─────────────────┘    └─────────────────┘    └─────────────────┘
//! ```
//!
//! ## Design constraints
//!
//! - All adaptive decisions are **reversible** — the agent can always fall back
//!   to static defaults.
//! - No adaptive logic blocks the hot path — telemetry reads are cached, and
//!   profile updates are debounced.
//! - Profile data is **human-readable JSON** stored in `~/.clarity/profiles/`.

pub mod compression;
pub mod predictor;
pub mod profile;
pub mod router;

pub use compression::{CompressionOptimizer, CompressionParams};
pub use predictor::{BehaviorPredictor, TaskPattern, WindowedStats};
pub use profile::{AgentGrowthProfile, InteractionPatterns, MasteryLevel, ToolStats};
pub use router::{AdaptiveModelRouter, ProviderProfile, RouterError, TaskDescriptor, TaskType};
