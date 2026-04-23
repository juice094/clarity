use clarity_core::{Agent, ToolRegistry};
use criterion::{criterion_group, criterion_main, Criterion};

fn bench_build_system_prompt(c: &mut Criterion) {
    let registry = ToolRegistry::with_builtin_tools();
    let agent = Agent::new(registry);
    c.bench_function("Agent::build_system_prompt", |b| {
        b.iter(|| {
            let _ = agent.build_system_prompt();
        })
    });
}

criterion_group!(benches, bench_build_system_prompt);
criterion_main!(benches);
