//! Adaptive engine loop test: verify ModelRouter learns from historical data.

use clarity_core::adaptive::{
    AdaptiveModelRouter, CompressionOptimizer, ProviderProfile, TaskDescriptor, TaskType,
};

/// Router should select the provider with best historical latency for coding tasks.
#[test]
fn test_router_learns_from_latency_history() {
    let router = AdaptiveModelRouter::new();

    // Register two providers
    let mut fast = ProviderProfile::new("fast-local");
    fast.observe_latency(50.0);
    fast.observe_latency(55.0);
    fast.observe_latency(48.0);

    let mut slow = ProviderProfile::new("slow-cloud");
    slow.observe_latency(800.0);
    slow.observe_latency(850.0);
    slow.observe_latency(820.0);

    router.register_provider(fast);
    router.register_provider(slow);

    let task = TaskDescriptor::new(TaskType::Coding);
    let chosen = router.route(&task).unwrap();

    assert_eq!(
        chosen, "fast-local",
        "coding task should prefer low-latency provider"
    );
}

/// Router should exclude unhealthy providers (high error rate).
#[test]
fn test_router_excludes_unhealthy_providers() {
    let router = AdaptiveModelRouter::new();

    let mut healthy = ProviderProfile::new("healthy");
    healthy.observe_error(false);
    healthy.observe_error(false);
    healthy.observe_error(false);

    let mut unhealthy = ProviderProfile::new("unhealthy");
    for _ in 0..10 {
        unhealthy.observe_error(true);
    }

    router.register_provider(healthy);
    router.register_provider(unhealthy);

    let task = TaskDescriptor::new(TaskType::Plan);
    let chosen = router.route(&task).unwrap();

    assert_eq!(chosen, "healthy", "unhealthy provider must be excluded");
}

/// Router should weight quality higher for Plan tasks.
#[test]
fn test_router_plan_prefers_quality() {
    let router = AdaptiveModelRouter::new();

    let mut low_quality_fast = ProviderProfile::new("fast-but-dumb");
    low_quality_fast.observe_latency(10.0);
    low_quality_fast.observe_feedback(-0.8); // poor quality

    let mut high_quality_slow = ProviderProfile::new("slow-but-smart");
    high_quality_slow.observe_latency(500.0);
    high_quality_slow.observe_feedback(0.9); // excellent quality

    router.register_provider(low_quality_fast);
    router.register_provider(high_quality_slow);

    let task = TaskDescriptor::new(TaskType::Plan);
    let chosen = router.route(&task).unwrap();

    assert_eq!(
        chosen, "slow-but-smart",
        "plan task should prefer quality over speed"
    );
}

/// Router should weight cost higher for Background tasks.
#[test]
fn test_router_background_prefers_cheap() {
    let router = AdaptiveModelRouter::new();

    let mut expensive = ProviderProfile::new("expensive");
    expensive.cost_per_1k_tokens = 0.02;
    expensive.observe_latency(100.0);

    let mut cheap = ProviderProfile::new("cheap");
    cheap.cost_per_1k_tokens = 0.001;
    cheap.observe_latency(200.0);

    router.register_provider(expensive);
    router.register_provider(cheap);

    // Must set estimated tokens for cost scoring to have effect.
    let task = TaskDescriptor::new(TaskType::Background).with_estimated_tokens(5000);
    let chosen = router.route(&task).unwrap();

    assert_eq!(
        chosen, "cheap",
        "background task should prefer cheapest provider"
    );
}

/// CompressionOptimizer should adapt to exploratory sessions.
#[test]
fn test_compression_exploratory_pattern() {
    let optimizer = CompressionOptimizer::new();

    let stats = clarity_core::adaptive::compression::SessionStats {
        message_count: 20,
        current_tokens: 3000,
        max_tokens: 8192,
        avg_message_tokens: 50.0,
        session_pattern: clarity_core::adaptive::compression::SessionPattern::Exploratory,
    };

    let params = optimizer.optimize(&stats);

    // Exploratory: high trigger ratio (short messages don't fill fast)
    assert!(
        params.trigger_ratio > 0.8,
        "exploratory should have high trigger_ratio"
    );
    // Low reserve (short queries need less headroom)
    assert!(
        params.reserved_tokens < 1500,
        "exploratory should have low reserved_tokens"
    );
    // Don't enable LLM summary for short queries
    assert!(!params.enable_llm_summary);
}

/// CompressionOptimizer should adapt to deep-dive sessions.
#[test]
fn test_compression_deep_dive_pattern() {
    let optimizer = CompressionOptimizer::new();

    let stats = clarity_core::adaptive::compression::SessionStats {
        message_count: 5,
        current_tokens: 6000,
        max_tokens: 8192,
        avg_message_tokens: 800.0,
        session_pattern: clarity_core::adaptive::compression::SessionPattern::DeepDive,
    };

    let params = optimizer.optimize(&stats);

    // DeepDive: low trigger ratio (long messages fill fast)
    assert!(
        params.trigger_ratio < 0.7,
        "deep-dive should have low trigger_ratio"
    );
    // High reserve (need room for deep responses)
    assert!(
        params.reserved_tokens > 2000,
        "deep-dive should have high reserved_tokens"
    );
    // Enable LLM summary (worth it for complex context)
    assert!(params.enable_llm_summary);
}

/// Router should fail when no providers are registered.
#[test]
fn test_router_no_providers() {
    let router = AdaptiveModelRouter::new();
    let task = TaskDescriptor::new(TaskType::General);

    let result = router.route(&task);
    assert!(
        result.is_err(),
        "route must fail when no providers registered"
    );
}

/// Router should fail when all providers are unhealthy.
#[test]
fn test_router_all_unhealthy() {
    let router = AdaptiveModelRouter::new();

    let mut dead = ProviderProfile::new("dead");
    for _ in 0..20 {
        dead.observe_error(true);
    }
    router.register_provider(dead);

    let task = TaskDescriptor::new(TaskType::Coding);
    let result = router.route(&task);
    assert!(
        result.is_err(),
        "route must fail when all providers are unhealthy"
    );
}
