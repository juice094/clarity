//! OKF bundle and concept loaders.

use super::{OkfBundle, OkfConcept, OkfError, OkfFrontmatter, OkfResult};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Loads an OKF knowledge bundle from a directory.
#[derive(Debug, Clone, Default)]
pub struct BundleLoader;

impl BundleLoader {
    /// Load all `.md` files recursively from `root` into an [`OkfBundle`].
    ///
    /// Files that do not satisfy OKF v0.1 structural rules are skipped with a
    /// warning rather than failing the whole bundle. This makes it practical to
    /// load real-world directories that contain a mix of OKF concepts and
    /// plain Markdown notes.
    ///
    /// # Errors
    ///
    /// Returns an error only if the directory cannot be read.
    pub fn load(root: impl AsRef<Path>) -> OkfResult<OkfBundle> {
        let root = root.as_ref().canonicalize()?;
        let mut concepts = HashMap::new();
        let mut warnings = Vec::new();
        Self::load_recursive(&root, &root, &mut concepts, &mut warnings)?;
        Ok(OkfBundle {
            root,
            concepts,
            warnings,
        })
    }

    fn load_recursive(
        bundle_root: &Path,
        dir: &Path,
        concepts: &mut HashMap<String, OkfConcept>,
        warnings: &mut Vec<String>,
    ) -> OkfResult<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                Self::load_recursive(bundle_root, &path, concepts, warnings)?;
            } else if path.extension().and_then(|s| s.to_str()) == Some("md") {
                match ConceptLoader::load(bundle_root, &path) {
                    Ok(concept) => {
                        concepts.insert(concept.id.clone(), concept);
                    }
                    Err(e) => {
                        let msg =
                            format!("Skipping non-compliant OKF file {}: {}", path.display(), e);
                        tracing::warn!("{}", msg);
                        warnings.push(msg);
                    }
                }
            }
        }
        Ok(())
    }
}

/// Loads a single OKF concept from a markdown file.
#[derive(Debug, Clone, Default)]
pub struct ConceptLoader;

impl ConceptLoader {
    /// Parse a single markdown file into an [`OkfConcept`].
    ///
    /// `bundle_root` is used to compute the concept id relative to the bundle.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read, the frontmatter is
    /// malformed, or a non-reserved concept is missing the required `type`
    /// field.
    pub fn load(bundle_root: &Path, path: &Path) -> OkfResult<OkfConcept> {
        let content = std::fs::read_to_string(path)?;
        let rel_path = path.strip_prefix(bundle_root).unwrap_or(path);
        let id = normalize_concept_id(
            &rel_path
                .with_extension("")
                .to_string_lossy()
                .replace('\\', "/"),
        );
        let file_name = rel_path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        let is_reserved = is_reserved_file(file_name);

        let (frontmatter, body) = parse_content(&content, is_reserved, &id)?;

        Ok(OkfConcept {
            id,
            path: path.to_path_buf(),
            frontmatter,
            body,
            is_reserved,
        })
    }

    /// Parse a markdown string into an [`OkfConcept`] without filesystem
    /// context.
    ///
    /// This is primarily useful for tests and for in-memory previews.
    /// `id` should follow OKF concept id conventions (`path/without/.md`).
    pub fn parse(id: impl Into<String>, content: &str) -> OkfResult<OkfConcept> {
        let id = id.into();
        let is_reserved = is_reserved_file(&format!("{id}.md"));
        let (frontmatter, body) = parse_content(content, is_reserved, &id)?;
        Ok(OkfConcept {
            id,
            path: PathBuf::new(),
            frontmatter,
            body,
            is_reserved,
        })
    }
}

/// Returns `true` if `name` is a reserved OKF filename.
fn is_reserved_file(name: &str) -> bool {
    name == "index.md" || name == "log.md"
}

/// Normalize a concept id by collapsing `.` and `..` path segments.
pub(crate) fn normalize_concept_id(id: &str) -> String {
    let mut stack = Vec::new();
    for part in id.split('/') {
        match part {
            "" | "." => continue,
            ".." => {
                stack.pop();
            }
            _ => stack.push(part),
        }
    }
    stack.join("/")
}

/// Split content into YAML frontmatter and Markdown body.
///
/// Reserved files may omit frontmatter entirely. Non-reserved files must
/// begin with a delimited YAML block and contain a non-empty `type`.
fn parse_content(
    content: &str,
    is_reserved: bool,
    id: &str,
) -> OkfResult<(OkfFrontmatter, String)> {
    let trimmed = content.trim_start();

    if !trimmed.starts_with("---") {
        if is_reserved {
            return Ok((OkfFrontmatter::default(), content.to_string()));
        }
        return Err(OkfError::InvalidFrontmatter {
            path: id.to_string(),
            reason: "Concept must start with YAML frontmatter delimited by '---'".to_string(),
        });
    }

    let after_open = &trimmed[3..];
    let Some(close_idx) = after_open.find("\n---") else {
        return Err(OkfError::InvalidFrontmatter {
            path: id.to_string(),
            reason: "Missing closing '---' for YAML frontmatter".to_string(),
        });
    };

    let yaml_str = &after_open[..close_idx];
    let body = after_open[close_idx + 4..].trim_start().to_string();

    let frontmatter: OkfFrontmatter =
        serde_yaml::from_str(yaml_str).map_err(|e| OkfError::InvalidFrontmatter {
            path: id.to_string(),
            reason: e.to_string(),
        })?;

    if !is_reserved && frontmatter.r#type.is_empty() {
        return Err(OkfError::MissingType(id.to_string()));
    }

    Ok((frontmatter, body))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_concept_valid() {
        let content = r#"---
type: Metric
title: WAU
---

# Weekly Active Users

Computed as count(distinct user_id) over 7 days.
"#;
        let concept = ConceptLoader::parse("metrics/wau", content).unwrap();
        assert_eq!(concept.id, "metrics/wau");
        assert_eq!(concept.frontmatter.r#type, "Metric");
        assert_eq!(concept.frontmatter.title.as_deref().unwrap(), "WAU");
        assert!(concept.body.contains("Weekly Active Users"));
        assert!(!concept.is_reserved);
    }

    #[test]
    fn test_parse_reserved_index_without_frontmatter() {
        let content = "# Index\n\n- [Metric](metrics/wau.md)\n";
        let concept = ConceptLoader::parse("index", content).unwrap();
        assert!(concept.is_reserved);
        assert!(concept.frontmatter.r#type.is_empty());
        assert_eq!(concept.body, content);
    }

    #[test]
    fn test_parse_missing_frontmatter_fails() {
        let content = "# Just markdown\nNo frontmatter.";
        let result = ConceptLoader::parse("concept", content);
        assert!(matches!(result, Err(OkfError::InvalidFrontmatter { .. })));
    }

    #[test]
    fn test_parse_missing_type_fails() {
        let content = r#"---
title: Untyped
---

body
"#;
        let result = ConceptLoader::parse("concept", content);
        assert!(matches!(result, Err(OkfError::MissingType(_))));
    }

    #[test]
    fn test_load_bundle_from_dir() {
        use std::io::Write;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let root = dir.path();

        let mut index = std::fs::File::create(root.join("index.md")).unwrap();
        index
            .write_all(b"# Index\n\n- [WAU](metrics/wau.md)\n")
            .unwrap();

        let metrics = root.join("metrics");
        std::fs::create_dir(&metrics).unwrap();
        let mut wau = std::fs::File::create(metrics.join("wau.md")).unwrap();
        wau.write_all(b"---\ntype: Metric\ntitle: WAU\n---\n\n# Weekly Active Users\n")
            .unwrap();

        let bundle = BundleLoader::load(root).unwrap();
        assert_eq!(bundle.len(), 2);
        assert!(bundle.get("index").is_some());
        assert!(bundle.get("metrics/wau").is_some());
        assert_eq!(bundle.by_type("Metric").len(), 1);

        let graph = bundle.into_graph();
        assert_eq!(graph.outgoing("index").len(), 1);
        assert_eq!(graph.outgoing("index")[0].target, "metrics/wau");
    }

    #[test]
    fn test_load_bundle_skips_noncompliant_files() {
        use std::io::Write;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let root = dir.path();

        let mut good = std::fs::File::create(root.join("good.md")).unwrap();
        good.write_all(b"---\ntype: Concept\n---\n\nGood.").unwrap();

        let mut bad = std::fs::File::create(root.join("bad.md")).unwrap();
        bad.write_all(b"# Missing frontmatter\n").unwrap();

        let bundle = BundleLoader::load(root).unwrap();
        assert_eq!(bundle.len(), 1);
        assert!(bundle.get("good").is_some());
        assert!(bundle.get("bad").is_none());
    }
}
