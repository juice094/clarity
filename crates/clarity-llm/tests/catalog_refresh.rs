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
//! Run all families:
//!
//! ```bash
//! cargo test -p clarity-llm --test catalog_refresh -- --ignored --nocapture
//! ```
//!
//! Run a single family:
//!
//! ```bash
//! CATALOG_REFRESH_FAMILY=openai cargo test -p clarity-llm --test catalog_refresh -- --ignored --nocapture
//! ```

use clarity_llm::catalog::service::ModelCatalogService;
use clarity_llm::registry_table;

#[tokio::test]
#[ignore = "manual: requires network and provider API keys"]
async fn manual_refresh_against_real_providers() {
    let service = ModelCatalogService::with_defaults().expect("build service");

    let target = std::env::var("CATALOG_REFRESH_FAMILY").ok();
    let families: Vec<&str> = target
        .as_ref()
        .map(|f| vec![f.as_str()])
        .unwrap_or_else(|| registry_table::all_family_names().to_vec());

    let mut total_models = 0usize;
    let mut attempted = 0usize;
    let mut succeeded = 0usize;

    for family in families {
        attempted += 1;
        match service.refresh_family(family).await {
            Ok(entries) => {
                succeeded += 1;
                total_models += entries.len();
                println!("{family}: {} models", entries.len());
                for entry in entries.iter().take(10) {
                    println!("  - {}", entry.model_id);
                }
                if entries.len() > 10 {
                    println!("  ... and {} more", entries.len() - 10);
                }
            }
            Err(e) => {
                println!("{family}: FAILED ({e})");
            }
        }
    }

    println!("\nTotal: {succeeded}/{attempted} families reachable, {total_models} models fetched");
}
