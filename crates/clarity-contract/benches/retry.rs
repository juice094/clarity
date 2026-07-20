#![allow(missing_docs, clippy::unwrap_used, clippy::expect_used)]

use clarity_contract::{ExponentialBackoff, sanitize_path_str};
use criterion::{Criterion, criterion_group, criterion_main};

// ── ExponentialBackoff::next_delay ──────────────────────────────────────

fn bench_next_delay(c: &mut Criterion) {
    let mut group = c.benchmark_group("ExponentialBackoff::next_delay");

    let backoff = ExponentialBackoff {
        initial_delay: std::time::Duration::from_secs(1),
        max_delay: std::time::Duration::from_secs(60),
        reset_after: std::time::Duration::from_secs(300),
    };

    // Normal range — no saturation.
    group.bench_function("attempt_0", |bench| {
        bench.iter(|| backoff.next_delay(0));
    });
    group.bench_function("attempt_5", |bench| {
        bench.iter(|| backoff.next_delay(5));
    });
    group.bench_function("attempt_10", |bench| {
        bench.iter(|| backoff.next_delay(10));
    });
    // Saturation boundary.
    group.bench_function("attempt_20", |bench| {
        bench.iter(|| backoff.next_delay(20));
    });
    group.bench_function("attempt_31 (saturating)", |bench| {
        bench.iter(|| backoff.next_delay(31));
    });

    // Custom short max_delay — early saturation.
    let short = ExponentialBackoff {
        initial_delay: std::time::Duration::from_millis(100),
        max_delay: std::time::Duration::from_secs(1),
        reset_after: std::time::Duration::from_secs(60),
    };
    group.bench_function("short_max_saturating", |bench| {
        bench.iter(|| short.next_delay(10));
    });

    group.finish();
}

// ── sanitize_path_str ───────────────────────────────────────────────────

fn bench_sanitize_path_str(c: &mut Criterion) {
    let mut group = c.benchmark_group("sanitize_path_str");

    // Short input, no paths.
    group.bench_function("no_paths", |bench| {
        bench.iter(|| sanitize_path_str("An error occurred during execution"));
    });

    // Contains absolute paths.
    group.bench_function("with_absolute_paths", |bench| {
        bench.iter(|| {
            sanitize_path_str("Error reading C:\\Users\\test\\config.json: permission denied")
        });
    });

    // Long input with multiple paths.
    group.bench_function("multi_path_long", |bench| {
        bench.iter(|| {
            sanitize_path_str(
                "Failed at C:\\projects\\src\\main.rs:42 while reading /home/user/data.txt and /etc/config.toml",
            )
        });
    });

    group.finish();
}

criterion_group!(benches, bench_next_delay, bench_sanitize_path_str);
criterion_main!(benches);
