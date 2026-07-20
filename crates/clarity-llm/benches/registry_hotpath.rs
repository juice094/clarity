#![allow(missing_docs)]

use std::collections::HashMap;

use clarity_llm::model_registry::{AuthType, ModelEntry, ProtocolType, ProviderConfig};
use clarity_llm::registry_table::family_defaults;
use clarity_llm::runtime_router::RouterHint;
use criterion::{Criterion, criterion_group, criterion_main};

// ── RouterHint::parse ──────────────────────────────────────────────────

fn bench_router_hint_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("RouterHint::parse");

    group.bench_function("cheapest alias", |b| {
        b.iter(|| RouterHint::parse("cheap"));
    });
    group.bench_function("coding alias", |b| {
        b.iter(|| RouterHint::parse("coding"));
    });
    group.bench_function("vision alias", |b| {
        b.iter(|| RouterHint::parse("vision"));
    });
    group.bench_function("tools alias", |b| {
        b.iter(|| RouterHint::parse("tools"));
    });
    group.bench_function("explicit alias (allocates)", |b| {
        b.iter(|| RouterHint::parse("some-custom-alias-name-here"));
    });
    group.bench_function("empty string → cheapest", |b| {
        b.iter(|| RouterHint::parse(""));
    });
    group.finish();
}

// ── family_defaults lookup ─────────────────────────────────────────────

fn bench_family_defaults(c: &mut Criterion) {
    let mut group = c.benchmark_group("family_defaults");

    group.bench_function("known (openai)", |b| {
        b.iter(|| family_defaults("openai"));
    });
    group.bench_function("known (anthropic)", |b| {
        b.iter(|| family_defaults("anthropic"));
    });
    group.bench_function("known (deepseek)", |b| {
        b.iter(|| family_defaults("deepseek"));
    });
    group.bench_function("unknown family", |b| {
        b.iter(|| family_defaults("nonexistent-provider-family"));
    });
    group.finish();
}

// ── select_provider policy ─────────────────────────────────────────────

fn bench_select_provider(c: &mut Criterion) {
    use clarity_llm::policy::select_provider;

    let mut group = c.benchmark_group("select_provider");

    group.bench_function("already bound", |b| {
        b.iter(|| select_provider("openai", true, Some("openai")));
    });
    group.bench_function("binding change", |b| {
        b.iter(|| select_provider("openai", true, Some("anthropic")));
    });
    group.bench_function("local only", |b| {
        b.iter(|| select_provider("local", true, None));
    });
    group.bench_function("network offline fallback", |b| {
        b.iter(|| select_provider("openai", false, None));
    });
    group.bench_function("first bind", |b| {
        b.iter(|| select_provider("deepseek", true, None));
    });
    group.finish();
}

// ── Helpers for ModelEntry benchmarks ──────────────────────────────────

fn base_provider_config() -> ProviderConfig {
    ProviderConfig {
        protocol: ProtocolType::OpenAiChat,
        base_url: Some("https://api.openai.com/v1".into()),
        api_key_env: Some("OPENAI_API_KEY".into()),
        auth_type: AuthType::ApiKey,
        auth_token_key: None,
        oauth: None,
        extra: HashMap::new(),
        pricing: None,
        tags: vec![],
    }
}

// ── ModelEntry::merge_into ─────────────────────────────────────────────

fn bench_model_entry_merge(c: &mut Criterion) {
    let mut group = c.benchmark_group("ModelEntry::merge_into");

    let base = base_provider_config();

    let minimal = ModelEntry {
        alias: "test-model".into(),
        provider: "test".into(),
        model_id: "test-1".into(),
        ..Default::default()
    };

    group.bench_function("minimal (no overrides)", |b| {
        b.iter(|| minimal.merge_into(&base));
    });

    let full = ModelEntry {
        alias: "full-model".into(),
        provider: "test".into(),
        model_id: "full-1".into(),
        api_key_env: Some("TEST_KEY".into()),
        base_url: Some("https://api.example.com/v1".into()),
        tags: vec![
            "coding".into(),
            "vision".into(),
            "cheap".into(),
            "tools".into(),
        ],
        pricing: Some(clarity_contract::llm::Pricing {
            input_per_1m: 0.5,
            output_per_1m: 0.5,
        }),
        extra: {
            let mut m = HashMap::new();
            m.insert("max_tokens".into(), "4096".into());
            m.insert("temperature".into(), "0.7".into());
            m
        },
        headers: {
            let mut m = HashMap::new();
            m.insert("X-Custom-Header".into(), "value".into());
            m
        },
        ..Default::default()
    };

    group.bench_function("full overrides", |b| {
        b.iter(|| full.merge_into(&base));
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_router_hint_parse,
    bench_family_defaults,
    bench_select_provider,
    bench_model_entry_merge,
);
criterion_main!(benches);
