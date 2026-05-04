//! Integration tests for Jumpy Workflow Orchestration.
//!
//! These tests simulate the full pipeline:
//!   1. Collect historical observations (offline learning)
//!   2. Plan a skill sequence for a goal
//!   3. Execute with deviation detection and replanning

use super::composer::ComposerBuilder;
use super::planner::Goal;
use super::predictor::OutcomePredictor;
use super::predictor::{ConsistentPredictor, HistoricalPredictor, SkillObservation};
use super::state::JumpyState;

#[tokio::test]
async fn test_full_pipeline_with_replanning() {
    // --- Phase 1: Offline learning ---
    // Simulate historical logs showing that:
    //   - "explore" skill typically adds "explored" tag and progresses 0.3
    //   - "coder" skill typically adds "coded" tag and progresses 0.5
    let mut predictor = HistoricalPredictor::new().with_threshold(0.5);

    predictor.observe(SkillObservation {
        skill_id: "explore".to_string(),
        params: "default".to_string(),
        before: JumpyState::from_query("start"),
        after: {
            let mut s = JumpyState::from_query("start");
            s.tags.push("explored".to_string());
            s.progress = 0.3;
            s
        },
    });

    predictor.observe(SkillObservation {
        skill_id: "coder".to_string(),
        params: "default".to_string(),
        before: {
            let mut s = JumpyState::from_query("start");
            s.tags.push("explored".to_string());
            s.progress = 0.3;
            s
        },
        after: {
            let mut s = JumpyState::from_query("start");
            s.tags.push("explored".to_string());
            s.tags.push("coded".to_string());
            s.progress = 0.8;
            s
        },
    });

    // Wrap with horizon consistency
    let predictor = ConsistentPredictor::new(predictor).with_horizons(0.5, 0.9);

    // --- Phase 2: Plan ---
    let goal = Goal::new(vec!["coded".to_string()]);
    let initial = JumpyState::from_query("Implement feature X");

    let mut composer = ComposerBuilder::new()
        .with_predictor(std::sync::Arc::new(predictor))
        .with_skills(vec![
            ("explore".to_string(), "default".to_string()),
            ("coder".to_string(), "default".to_string()),
        ])
        .with_replan_threshold(0.25)
        .with_num_candidates(64)
        .with_rng_seed(42)
        .build()
        .unwrap();

    // --- Phase 3: Execute ---
    // First execution: "explore" returns as predicted.
    // Second execution: "coder" deviates (only reaches 0.6 instead of 0.8),
    //                   which triggers replanning.
    let explore_outcome = {
        let mut s = JumpyState::from_query("Implement feature X");
        s.tags.push("explored".to_string());
        s.progress = 0.3;
        s
    };

    let coder_outcome_deviated = {
        let mut s = JumpyState::from_query("Implement feature X");
        s.tags.push("explored".to_string());
        // Missing "coded" tag + much lower progress = large deviation
        s.progress = 0.35;
        s
    };

    let coder_outcome_final = {
        let mut s = JumpyState::from_query("Implement feature X");
        s.tags.push("explored".to_string());
        s.tags.push("coded".to_string());
        s.tags.push("tested".to_string());
        s.progress = 0.95;
        s
    };

    // Build a stateful executor that returns different outcomes per call
    let call_counter = std::sync::atomic::AtomicUsize::new(0);
    let result = composer
        .compose(&goal, &initial, |skill_id, _params| {
            let count = call_counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let outcome = match (skill_id, count) {
                ("explore", 0) => explore_outcome.clone(),
                ("coder", 1) => coder_outcome_deviated.clone(),
                ("coder", 2) => coder_outcome_final.clone(),
                _ => JumpyState::default(),
            };
            async move { Ok(outcome) }
        })
        .await
        .unwrap();

    // --- Phase 4: Verify ---
    assert!(result.success, "Should reach the goal");
    assert!(result.total_steps >= 2, "Should take at least 2 steps");

    // The deviated step should have triggered replanning
    let replanned_steps: Vec<_> = result.steps.iter().filter(|s| s.replanned).collect();
    assert!(
        !replanned_steps.is_empty(),
        "Deviation should trigger at least one replan"
    );
}

#[tokio::test]
async fn test_consistency_wrapper_bootstraps_long_horizon() {
    let base = HistoricalPredictor::new();
    let consistent = ConsistentPredictor::new(base).with_horizons(0.3, 0.8);

    // With no observations, both short and long predictions should fail,
    // but the wrapper should still route correctly.
    let state = JumpyState::from_query("test");
    let result = consistent.predict("unknown", "p", &state, 0.9).await;
    assert!(result.is_err(), "Should fail when no history exists");
}
