#![allow(missing_docs, clippy::unwrap_used, clippy::expect_used)]

use clarity_mcp::{ToolCallResult, ToolContent, process_mcp_tool_result};
use criterion::{Criterion, criterion_group, criterion_main};

// ── Helpers ────────────────────────────────────────────────────────────

fn text_result(text: &str) -> ToolCallResult {
    ToolCallResult {
        content: vec![ToolContent::Text {
            text: text.to_string(),
        }],
        is_error: false,
    }
}

fn multi_text_result(texts: &[&str]) -> ToolCallResult {
    ToolCallResult {
        content: texts
            .iter()
            .map(|t| ToolContent::Text {
                text: t.to_string(),
            })
            .collect(),
        is_error: false,
    }
}

fn json_result(json: &str) -> ToolCallResult {
    ToolCallResult {
        content: vec![ToolContent::Text {
            text: json.to_string(),
        }],
        is_error: false,
    }
}

fn error_result(text: &str) -> ToolCallResult {
    ToolCallResult {
        content: vec![ToolContent::Text {
            text: text.to_string(),
        }],
        is_error: true,
    }
}

// ── Benchmarks ─────────────────────────────────────────────────────────

fn bench_process_mcp_tool_result(c: &mut Criterion) {
    let mut group = c.benchmark_group("process_mcp_tool_result");

    // Plain text — simplest path.
    let plain = text_result("The operation completed successfully.");
    group.bench_function("plain_text", |bench| {
        bench.iter(|| process_mcp_tool_result(plain.clone()));
    });

    // Multi-text — join path exercised.
    let multi = multi_text_result(&[
        "First part of output.",
        "Second part with more detail.",
        "Third concluding part.",
    ]);
    group.bench_function("multi_text(3)", |bench| {
        bench.iter(|| process_mcp_tool_result(multi.clone()));
    });

    // Error-flagged — error path exercised (error field detection).
    let err = error_result("{\"success\": false, \"error\": \"Something went wrong\"}");
    group.bench_function("error_json", |bench| {
        bench.iter(|| process_mcp_tool_result(err.clone()));
    });

    // JSON re-parse path — content with embedded JSON.
    let json = json_result("{\"result\": \"ok\", \"items\": [1,2,3]}");
    group.bench_function("json_reparse", |bench| {
        bench.iter(|| process_mcp_tool_result(json.clone()));
    });

    // Credential scrubbing path — contains API key patterns.
    let secrets = text_result(
        "API key: sk-1234567890abcdef and Bearer eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0",
    );
    group.bench_function("credential_scrub", |bench| {
        bench.iter(|| process_mcp_tool_result(secrets.clone()));
    });

    // Empty content — edge case.
    let empty = ToolCallResult {
        content: vec![],
        is_error: false,
    };
    group.bench_function("empty_content", |bench| {
        bench.iter(|| process_mcp_tool_result(empty.clone()));
    });

    group.finish();
}

criterion_group!(benches, bench_process_mcp_tool_result);
criterion_main!(benches);
