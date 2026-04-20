//! Simple benchmark runner for key clarity-core operations.
//!
//! Run with: `cargo run --example benchmark -p clarity-core --release`

use clarity_core::registry::ToolRegistry;
use std::time::{Duration, Instant};

fn bench(name: &str, iterations: usize, f: impl Fn()) -> Duration {
    // Warmup
    for _ in 0..iterations.min(10) {
        f();
    }

    let start = Instant::now();
    for _ in 0..iterations {
        f();
    }
    let elapsed = start.elapsed();

    let avg = elapsed.as_secs_f64() * 1000.0 / iterations as f64;
    println!(
        "  {:<40} {:>10} iter  {:>10.3} ms/iter",
        name,
        iterations,
        avg
    );
    elapsed
}

fn main() {
    println!("Clarity Core Benchmarks");
    println!("========================\n");

    // Benchmark 1: ToolRegistry with builtin tools
    bench("ToolRegistry::with_builtin_tools", 100, || {
        let _ = ToolRegistry::with_builtin_tools();
    });

    // Benchmark 2: Get tool schemas
    let registry = ToolRegistry::with_builtin_tools();
    bench("ToolRegistry::get_tool_schemas", 1000, || {
        let _ = registry.get_tool_schemas();
    });

    // Benchmark 3: Build system prompt
    let agent = clarity_core::Agent::new(registry);
    bench("Agent::build_system_prompt", 1000, || {
        let _ = agent.build_system_prompt();
    });

    // Benchmark 4: Skill context builder
    let skill = clarity_core::skills::SkillLoader::parse(
        r#"---
id: bench-skill
name: Bench Skill
description: A benchmark skill
tools:
  - bash
  - file_read
---

# Instructions
Do something useful.
"#,
    )
    .unwrap();
    bench("Skill::build_context", 10000, || {
        let _ = skill.build_context();
    });

    // Benchmark 5: Chunking long text
    let long_text = "Paragraph one.\n\n".repeat(200);
    let chunk_config = clarity_core::memory::ChunkConfig::new().with_chunk_size(256);
    bench("Chunker::split (200 paras)", 1000, || {
        let _ = clarity_core::memory::Chunker::split(&long_text, &chunk_config);
    });

    println!("\nDone.");
}
