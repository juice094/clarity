//! Example: scan a directory and search the resulting knowledge index.
//!
//! Run with:
//!
//! ```bash
//! cargo run -p clarity-knowledge --example scan_vault -- /path/to/vault "query"
//! ```

use clarity_knowledge::{InMemoryIndex, KnowledgeIndex, SearchQuery, SourceConfig};
use std::env;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: scan_vault <vault-path> <query>");
        std::process::exit(1);
    }

    let vault_path = &args[1];
    let query = &args[2];

    let index = InMemoryIndex::new()?;
    index.add_source(SourceConfig::new(vault_path)).await?;

    let events = index.scan_all()?;
    println!("Indexed {} files", events.len());

    let graph = index.graph()?;
    println!(
        "Graph: {} nodes, {} edges",
        graph.node_count(),
        graph.edge_count()
    );

    let results = index.search(SearchQuery::new(query).with_limit(5)).await?;
    println!("\nSearch results for {:?}:", query);
    for (i, result) in results.iter().enumerate() {
        println!(
            "{}. {} (score: {:.2})\n   {}\n",
            i + 1,
            result.title.as_deref().unwrap_or("(untitled)"),
            result.score,
            result.snippet.replace('\n', " ")
        );
    }

    Ok(())
}
