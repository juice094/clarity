//! Example: index a vault and export it as a read-only Obsidian vault.
//!
//! Run with:
//!
//! ```bash
//! cargo run -p clarity-knowledge --example obsidian_export -- \
//!   /path/to/input/vault /path/to/output/vault
//! ```

use clarity_knowledge::{FieldConfig, KnowledgeField, ObsidianExporter};
use std::path::PathBuf;
use std::process;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: obsidian_export <vault_input_path> <vault_output_path>");
        process::exit(1);
    }

    let input_path = PathBuf::from(&args[1]);
    let output_path = PathBuf::from(&args[2]);

    let field = KnowledgeField::new(FieldConfig::default());

    match field.index_directory(&input_path) {
        Ok(indexed) => println!("Indexed {indexed} markdown files from {:?}", input_path),
        Err(e) => {
            eprintln!("Failed to index input vault: {e}");
            process::exit(1);
        }
    }

    let exporter = ObsidianExporter::new(field, &input_path);
    match exporter.export(&output_path) {
        Ok(count) => println!("Exported {count} markdown files to {:?}", output_path),
        Err(e) => {
            eprintln!("Failed to export vault: {e}");
            process::exit(1);
        }
    }
}
