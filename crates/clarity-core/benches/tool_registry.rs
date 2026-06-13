use clarity_core::registry::ToolRegistry;
use criterion::{Criterion, black_box, criterion_group, criterion_main};

fn bench_tool_registry_creation(c: &mut Criterion) {
    c.bench_function("ToolRegistry::with_builtin_tools", |b| {
        b.iter(|| {
            let _ = ToolRegistry::with_builtin_tools();
        })
    });
}

fn bench_tool_schemas(c: &mut Criterion) {
    let registry = ToolRegistry::with_builtin_tools();
    c.bench_function("ToolRegistry::get_tool_schemas", |b| {
        b.iter(|| {
            let _ = registry.get_tool_schemas();
        })
    });
}

fn bench_tool_definitions(c: &mut Criterion) {
    let registry = ToolRegistry::with_builtin_tools();
    c.bench_function("ToolRegistry::get_tool_definitions", |b| {
        b.iter(|| {
            let _ = registry.get_tool_definitions();
        })
    });
}

criterion_group!(
    benches,
    bench_tool_registry_creation,
    bench_tool_schemas,
    bench_tool_definitions
);
criterion_main!(benches);
