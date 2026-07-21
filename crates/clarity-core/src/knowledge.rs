//! Bridge between agent turns and the knowledge field.
//!
//! This module translates conversation text into activation signals on the
//! knowledge graph. It is intentionally thin: the heavy lifting (parsing,
//! indexing, spreading activation) lives in `clarity-knowledge`.

use clarity_contract::Message;
use clarity_knowledge::{KnowledgeField, MarkdownExtractor, NodeId, SearchQuery};
use regex::Regex;
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;
use std::sync::LazyLock;

/// Number of top knowledge-field results injected into the turn context.
///
/// Kept small on purpose: this section shares the dynamic-prompt budget with
/// memories and compiled memory, and each entry already carries a snippet.
const RECALL_TOP_K: usize = 3;

#[allow(clippy::expect_used)]
static WIKILINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"!?\[\[([^\]|#^]+)").expect("static wikilink regex is valid"));

#[allow(clippy::expect_used)]
static MD_LINK_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[([^\]]*)\]\(([^)]+\.md)(?:#[^)]*)?\)").expect("static md link regex is valid")
});

/// Update the knowledge field after a completed turn.
///
/// Extracts file references and wikilinks from the user query and assistant
/// messages, then injects activation energy into the corresponding graph nodes.
///
/// This is a best-effort operation: missing nodes are silently ignored so that
/// transient references to unindexed files do not break the turn.
pub fn update_on_turn(field: &Arc<KnowledgeField>, query: &str, messages: &[Message]) {
    let references = extract_references(query, messages);
    for reference in references {
        field.inject_activation(&NodeId::new(reference), 0.6);
    }
}

/// Index compiled memory `.md` files into the knowledge field.
///
/// Called after `MemoryCompiler::compile_all` finishes successfully. Every
/// `.md` file in the output directory is parsed and added to the field so
/// that future searches and spreading activation can reach the compiled
/// memories.
///
/// Failures for individual files are logged and skipped; the function only
/// returns an error if the directory cannot be read or the extractor cannot
/// be created.
pub fn index_compiled_memories(
    field: &Arc<KnowledgeField>,
    output_dir: &Path,
) -> anyhow::Result<()> {
    let extractor = MarkdownExtractor::new()?;
    let mut indexed = 0;

    for entry in std::fs::read_dir(output_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("Failed to read compiled memory {:?}: {}", path, e);
                continue;
            }
        };

        match extractor.extract(&path, &content) {
            Ok(doc) => {
                if let Err(e) = field.index_document(doc) {
                    tracing::warn!("Failed to index compiled memory {:?}: {}", path, e);
                } else {
                    indexed += 1;
                }
            }
            Err(e) => {
                tracing::warn!("Failed to extract compiled memory {:?}: {}", path, e);
            }
        }
    }

    tracing::info!(
        "Indexed {} compiled memory files into knowledge field",
        indexed
    );
    Ok(())
}

/// Recall relevant knowledge for the current turn.
///
/// Runs a hybrid field search over `query` and formats the top hits as a
/// markdown section for the dynamic system prompt. When the field's local
/// embedding branch is enabled (feature `local-embedding`), the ranking is
/// already RRF-fused at the retriever level, so this path transparently
/// upgrades to fused results.
///
/// Returns `None` when the query is blank or the field has no relevant
/// entries, so callers can skip the section entirely. Best-effort: retrieval
/// failures are logged and yield `None` so a broken index never breaks a
/// turn.
pub fn recall_context(field: &Arc<KnowledgeField>, query: &str) -> Option<String> {
    if query.trim().is_empty() {
        return None;
    }
    let results = match field.search(&SearchQuery::new(query).with_limit(RECALL_TOP_K)) {
        Ok(results) => results,
        Err(e) => {
            tracing::warn!("Knowledge field recall failed: {}", e);
            return None;
        }
    };
    if results.is_empty() {
        return None;
    }

    let mut section = String::from("\n\n# Relevant Knowledge\n");
    for result in &results {
        let title = result.title.as_deref().unwrap_or("untitled");
        // Collapse whitespace so multi-line snippets stay on one prompt line.
        let snippet: String = result
            .snippet
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        section.push_str(&format!(
            "- **{}** ({}): {}\n",
            title,
            result.path.display(),
            snippet
        ));
    }
    Some(section)
}

/// Extract candidate node ids from query and messages.
///
/// Currently recognizes:
/// - Wikilinks `[[target]]` and `![[target]]`
/// - Markdown links to `.md` files `[text](path.md)`
fn extract_references(query: &str, messages: &[Message]) -> HashSet<String> {
    let mut combined = String::from(query);
    for msg in messages {
        combined.push('\n');
        combined.push_str(&msg.content);
    }

    let mut refs = HashSet::new();

    for cap in WIKILINK_RE.captures_iter(&combined) {
        refs.insert(cap[1].trim().to_string());
    }

    for cap in MD_LINK_RE.captures_iter(&combined) {
        let target = cap[2].trim();
        let clean = target.trim_end_matches(".md");
        refs.insert(clean.to_string());
    }

    refs
}

#[cfg(test)]
mod tests {
    use super::*;
    use clarity_knowledge::{FieldConfig, SearchQuery};

    #[test]
    fn extracts_wikilinks_and_md_links() {
        let query = "See [[rust]] and [[async|async programming]].";
        let messages = vec![
            Message::assistant("Read [intro](notes/intro.md) first."),
            Message::user("Also ![[diagram.png]]."),
        ];

        let refs = extract_references(query, &messages);
        assert!(refs.contains("rust"));
        assert!(refs.contains("async"));
        assert!(refs.contains("notes/intro"));
        assert!(refs.contains("diagram.png"));
    }

    #[test]
    fn indexes_compiled_memories() {
        let temp = tempfile::tempdir().unwrap();
        let memory_path = temp.path().join("memory.md");
        std::fs::write(
            &memory_path,
            "# Memory\n\n## 1. Key Facts\n\n- **User prefers Rust**\n",
        )
        .unwrap();

        let field = Arc::new(KnowledgeField::new(FieldConfig::default()));
        index_compiled_memories(&field, temp.path()).unwrap();

        let results = field
            .search(&SearchQuery::new("Rust").with_limit(5))
            .unwrap();
        assert!(!results.is_empty(), "compiled memory should be searchable");
        assert_eq!(results[0].path, memory_path);
    }

    #[test]
    fn recall_context_includes_indexed_hits() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("rust.md"),
            "# Rust\nRust ownership model explained.",
        )
        .unwrap();

        let field = Arc::new(KnowledgeField::new(FieldConfig::default()));
        index_compiled_memories(&field, temp.path()).unwrap();

        let section = recall_context(&field, "rust ownership").unwrap();
        assert!(section.contains("# Relevant Knowledge"));
        assert!(section.contains("rust.md"));
        assert!(section.contains("Rust"));
    }

    #[test]
    fn recall_context_none_when_nothing_relevant() {
        let field = Arc::new(KnowledgeField::new(FieldConfig::default()));
        assert!(recall_context(&field, "anything").is_none());
        assert!(recall_context(&field, "   ").is_none());
    }
}
