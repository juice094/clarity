//! Diagnostic example: index one or more external vault directories and report stats.
//!
//! Usage:
//!   cargo run -p clarity-knowledge --example vault_index_qa -- \
//!     "C:\Users\22414\Documents\Obsidian Vault\00-Hub" \
//!     "C:\Users\22414\Documents\Obsidian Vault\10-Courses"

use clarity_knowledge::{FieldConfig, KnowledgeField};
use std::path::PathBuf;
use std::time::Instant;

fn main() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();

    let paths: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    if paths.is_empty() {
        eprintln!("Usage: vault_index_qa <vault-path> [<vault-path> ...]");
        std::process::exit(1);
    }

    println!("Vault index QA");
    println!("==============");

    let field = KnowledgeField::new(FieldConfig::default());
    let mut total_files = 0usize;
    let mut total_indexed = 0usize;

    for path in &paths {
        println!("\nVault: {}", path.display());
        let md_count = count_markdown_files(path);
        println!("  markdown files found: {}", md_count);

        let start = Instant::now();
        match field.index_directory(path) {
            Ok(indexed) => {
                let elapsed = start.elapsed();
                println!("  indexed: {}", indexed);
                println!(
                    "  elapsed: {:?} ({:.1} files/s)",
                    elapsed,
                    indexed as f64 / elapsed.as_secs_f64().max(0.001)
                );
                total_files += md_count;
                total_indexed += indexed;
            }
            Err(e) => {
                println!("  ERROR: {}", e);
            }
        }
    }

    println!("\nTotal");
    println!("  markdown files found: {}", total_files);
    println!("  indexed: {}", total_indexed);

    if total_indexed > 0 {
        println!("\nSearch recall check:");
        for q in ["Map", "map", "file:Map", "path:Map", "tag:Map", "obsidian"] {
            let query = clarity_knowledge::search::SearchQuery::new(q).with_limit(5);
            let start = Instant::now();
            match field.search(&query) {
                Ok(results) => {
                    println!(
                        "  query '{}': {} results ({:?})",
                        q,
                        results.len(),
                        start.elapsed()
                    );
                    for r in results.iter().take(5) {
                        // Only print file name, never content/snippet.
                        println!(
                            "    - {} (score={:.3})",
                            r.path.file_name().unwrap_or_default().to_string_lossy(),
                            r.semantic_score
                        );
                    }
                }
                Err(e) => println!("  query '{}': error {}", q, e),
            }
        }
    }
}

fn count_markdown_files(path: &std::path::Path) -> usize {
    walkdir::WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("md"))
        .count()
}
