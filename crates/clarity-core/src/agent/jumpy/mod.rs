//! # Jumpy Workflow Orchestration
//!
//! An experimental adaptation of **"Compositional Planning with Jumpy World Models"**
//! (Farebrother et al., 2026 / arXiv:2602.19634) for Agent workflow orchestration.
//!
//! ## Core Concepts (RL → Workflow Mapping)
//!
//! | RL Concept | Workflow Analog |
//! |------------|-----------------|
//! | State `s` | `JumpyState` — compact workflow context snapshot |
//! | Action `a` | Tool call or message |
//! | Policy `π_z` | `Skill` parameterized by intent / subgoal `z` |
//! | GHM `m^π_γ` | `OutcomePredictor` — predicts state after skill execution |
//! | Discount `γ` | `commitment` — how long to stay in a skill before reconsidering |
//! | Switching `α` | `1 - commitment` — probability of handing off to next skill |
//! | CompPlan | `HierarchicalPlanner` — plans skill sequences at test time |
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    SkillComposer (Runtime)                   │
//! │  ┌───────────────────────────────────────────────────────┐  │
//! │  │              HierarchicalPlanner (CompPlan)            │  │
//! │  │  - Random shooting over skill sequences                │  │
//! │  │  - Evaluates via OutcomePredictor (Lemma 1 estimator)  │  │
//! │  └───────────────────────────────────────────────────────┘  │
//! │                          │                                   │
//! │                          ▼                                   │
//! │  ┌───────────────────────────────────────────────────────┐  │
//! │  │              OutcomePredictor (GHM)                    │  │
//! │  │  - HistoricalPredictor: nearest-neighbor over logs     │  │
//! │  │  - ConsistentPredictor: horizon consistency wrapper    │  │
//! │  └───────────────────────────────────────────────────────┘  │
//! │                          │                                   │
//! │                          ▼                                   │
//! │  ┌───────────────────────────────────────────────────────┐  │
//! │  │              JumpyState (Observation Space)            │  │
//! │  │  - tags, memory, active_files, progress                │  │
//! │  └───────────────────────────────────────────────────────┘  │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Usage
//!
//! ```rust,no_run
//! use clarity_core::agent::jumpy::*;
//!
//! let predictor = Arc::new(HistoricalPredictor::new());
//! let composer = ComposerBuilder::new()
//!     .with_predictor(predictor.clone())
//!     .with_skills(vec![
//!         ("explore".into(), "default".into()),
//!         ("coder".into(), "default".into()),
//!     ])
//!     .build()?;
//! ```

pub mod composer;
pub mod planner;
pub mod predictor;
pub mod state;

#[cfg(test)]
mod tests;

pub use composer::{ComposerBuilder, CompositionResult, ExecutionStep, SkillComposer};
pub use planner::{Goal, HierarchicalPlanner, PlannerConfig, SkillProposal, SkillSequence};
pub use predictor::{ConsistentPredictor, HistoricalPredictor, OutcomePredictor, SkillObservation};
pub use state::JumpyState;
