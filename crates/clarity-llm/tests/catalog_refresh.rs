#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        missing_docs,
        unsafe_code
    )
)]

//! Manual integration test: refresh model catalogs against real providers.
//!
//! This test is ignored by default because it makes real HTTP requests and
//! requires provider API keys in environment variables.
//!
//! Run with:
//!
//! ```bash
//! cargo test -p clarity-llm --test catalog_refresh -- --ignored --nocapture
//! ```

use clarity_llm::catalog::service::ModelCatalogService;

#[tokio::test]
#[ignore = "manual: requires network and provider API keys"]
async fn manual_refresh_against_real_providers() {
    let service = ModelCatalogService::with_defaults().expect("build service");
    let refreshed = service.refresh_all().await.expect("refresh");

    let mut total = 0usize;
    for (family, entries) in &refreshed {
        println!("{family}: {} models", entries.len());
        for entry in entries.iter().take(10) {
            println!("  - {}", entry.model_id);
        }
        if entries.len() > 10 {
            println!("  ... and {} more", entries.len() - 10);
        }
        total += entries.len();
    }

    println!("\nTotal models fetched: {total}");

    // We intentionally do not assert non-empty results: API keys may be missing
    // or a local Ollama instance may be offline. The test is informational.
}
