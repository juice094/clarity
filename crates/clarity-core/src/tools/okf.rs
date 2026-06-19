//! Open Knowledge Format (OKF) tools.
//!
//! These tools let an agent load, search, and read OKF knowledge bundles.
//! OKF represents knowledge as a directory of Markdown files with YAML
//! frontmatter. See `crate::okf` for the consumer implementation.

use crate::okf::OkfBundle;
use crate::tools::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// Resolve a bundle path that may be absolute or relative to the working
/// directory.
fn resolve_bundle_path(path: &str, working_dir: &Path) -> PathBuf {
    let p = Path::new(path);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        working_dir.join(p)
    }
}

/// Load an OKF bundle, using the in-memory cache when available and running
/// the blocking load on a worker thread.
async fn load_bundle(path: PathBuf) -> ToolResult<OkfBundle> {
    tokio::task::spawn_blocking(move || {
        crate::okf::OkfBundleCache::global()
            .get_or_load(&path)
            .map_err(into_tool_error)
    })
    .await
    .map_err(|e| clarity_contract::ToolError::execution_failed(format!("Join error: {e}")))?
}

/// Convert an OKF consumer error into a tool error.
fn into_tool_error(e: crate::okf::OkfError) -> clarity_contract::ToolError {
    clarity_contract::ToolError::execution_failed(e.to_string())
}

/// Tool that loads an OKF bundle and returns a high-level summary.
pub struct OkfLoadTool;

impl OkfLoadTool {
    /// Create a new OKF load tool.
    pub fn new() -> Self {
        Self
    }
}

impl Default for OkfLoadTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for OkfLoadTool {
    fn name(&self) -> &str {
        "okf_load"
    }

    fn description(&self) -> &str {
        "Load an Open Knowledge Format (OKF) bundle from a directory path and \
         return a summary including concept count, available concept types, \
         and reserved files."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the OKF bundle directory. May be absolute or relative to the working directory."
                }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, args: Value, ctx: ToolContext) -> ToolResult<Value> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| clarity_contract::ToolError::invalid_params("missing path"))?;
        let resolved = resolve_bundle_path(path, &ctx.working_dir);
        let bundle = load_bundle(resolved).await?;

        let mut types: Vec<String> = bundle
            .iter()
            .map(|c| c.frontmatter.r#type.clone())
            .filter(|t| !t.is_empty())
            .collect();
        types.sort();
        types.dedup();

        let reserved_files: Vec<String> = bundle
            .iter()
            .filter(|c| c.is_reserved)
            .map(|c| c.id.clone())
            .collect();

        let skipped_count = bundle.warnings.len();
        let skipped_files: Vec<&str> = bundle
            .warnings
            .iter()
            .filter_map(|w| w.strip_prefix("Skipping non-compliant OKF file "))
            .filter_map(|rest| rest.split_once(": "))
            .map(|(path, _reason)| path)
            .collect();

        Ok(json!({
            "root": bundle.root.to_string_lossy(),
            "concept_count": bundle.len(),
            "types": types,
            "reserved_files": reserved_files,
            "skipped_count": skipped_count,
            "skipped_files": skipped_files,
            "warnings": bundle.warnings,
        }))
    }
}

/// Tool that searches concepts within an OKF bundle.
pub struct OkfSearchTool;

impl OkfSearchTool {
    /// Create a new OKF search tool.
    pub fn new() -> Self {
        Self
    }
}

impl Default for OkfSearchTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for OkfSearchTool {
    fn name(&self) -> &str {
        "okf_search"
    }

    fn description(&self) -> &str {
        "Search concepts in an OKF bundle by keyword and optional concept type. \
         Returns matching concept ids, titles, and descriptions."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the OKF bundle directory. May be absolute or relative to the working directory."
                },
                "query": {
                    "type": "string",
                    "description": "Search query matched against id, title, description, tags, and body."
                },
                "type": {
                    "type": "string",
                    "description": "Optional filter: only return concepts with this OKF type."
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of results to return",
                    "minimum": 1,
                    "maximum": 100,
                    "default": 20
                }
            },
            "required": ["path", "query"]
        })
    }

    async fn execute(&self, args: Value, ctx: ToolContext) -> ToolResult<Value> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| clarity_contract::ToolError::invalid_params("missing path"))?;
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| clarity_contract::ToolError::invalid_params("missing query"))?;
        let type_filter = args.get("type").and_then(|v| v.as_str());
        let limit = args
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|v| (v as usize).min(100))
            .unwrap_or(20);

        let resolved = resolve_bundle_path(path, &ctx.working_dir);
        let bundle = load_bundle(resolved).await?;

        let mut results: Vec<&crate::okf::OkfConcept> = if let Some(t) = type_filter {
            bundle
                .by_type(t)
                .into_iter()
                .filter(|c| {
                    c.id.to_lowercase().contains(&query.to_lowercase())
                        || c.frontmatter
                            .title
                            .as_deref()
                            .unwrap_or("")
                            .to_lowercase()
                            .contains(&query.to_lowercase())
                        || c.frontmatter
                            .description
                            .as_deref()
                            .unwrap_or("")
                            .to_lowercase()
                            .contains(&query.to_lowercase())
                        || c.frontmatter
                            .tags
                            .iter()
                            .any(|tag| tag.to_lowercase().contains(&query.to_lowercase()))
                        || c.body.to_lowercase().contains(&query.to_lowercase())
                })
                .collect()
        } else {
            bundle.search(query)
        };

        let total = results.len();
        results.truncate(limit);

        let matches: Vec<Value> = results
            .iter()
            .map(|c| {
                json!({
                    "id": c.id,
                    "type": c.frontmatter.r#type,
                    "title": c.frontmatter.title,
                    "description": c.frontmatter.description,
                    "tags": c.frontmatter.tags,
                    "summary": c.summary(),
                })
            })
            .collect();

        Ok(json!({
            "query": query,
            "type_filter": type_filter,
            "total": total,
            "returned": matches.len(),
            "matches": matches,
        }))
    }
}

/// Tool that reads a single OKF concept by id.
pub struct OkfReadTool;

impl OkfReadTool {
    /// Create a new OKF read tool.
    pub fn new() -> Self {
        Self
    }
}

impl Default for OkfReadTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for OkfReadTool {
    fn name(&self) -> &str {
        "okf_read"
    }

    fn description(&self) -> &str {
        "Read a specific concept from an OKF bundle by id. Returns the full \
         Markdown body, frontmatter fields, and outgoing/incoming links."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the OKF bundle directory. May be absolute or relative to the working directory."
                },
                "id": {
                    "type": "string",
                    "description": "Concept id, e.g. 'metrics/wau' or 'index'."
                }
            },
            "required": ["path", "id"]
        })
    }

    async fn execute(&self, args: Value, ctx: ToolContext) -> ToolResult<Value> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| clarity_contract::ToolError::invalid_params("missing path"))?;
        let id = args
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| clarity_contract::ToolError::invalid_params("missing id"))?;

        let resolved = resolve_bundle_path(path, &ctx.working_dir);
        let bundle = load_bundle(resolved).await?;
        let graph = bundle.into_graph();

        let concept = graph.get(id).ok_or_else(|| {
            clarity_contract::ToolError::execution_failed(format!("Concept not found: {id}"))
        })?;

        let outgoing: Vec<Value> = graph
            .outgoing(id)
            .iter()
            .map(|l| json!({"target": l.target, "url": l.url}))
            .collect();
        let incoming: Vec<Value> = graph
            .incoming(id)
            .iter()
            .map(|l| json!({"source": l.source, "url": l.url}))
            .collect();

        Ok(json!({
            "id": concept.id,
            "type": concept.frontmatter.r#type,
            "title": concept.frontmatter.title,
            "description": concept.frontmatter.description,
            "resource": concept.frontmatter.resource,
            "tags": concept.frontmatter.tags,
            "timestamp": concept.frontmatter.timestamp,
            "is_reserved": concept.is_reserved,
            "outgoing": outgoing,
            "incoming": incoming,
            "context": concept.build_context(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::ToolContext;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_bundle() -> (TempDir, PathBuf) {
        let dir = TempDir::new().unwrap();
        let root = dir.path().to_path_buf();

        let mut index = std::fs::File::create(root.join("index.md")).unwrap();
        index
            .write_all(b"# Index\n\n- [WAU](metrics/wau.md)\n")
            .unwrap();

        let metrics = root.join("metrics");
        std::fs::create_dir(&metrics).unwrap();
        let mut wau = std::fs::File::create(metrics.join("wau.md")).unwrap();
        wau.write_all(
            b"---\ntype: Metric\ntitle: WAU\ndescription: Weekly active users\ntags:\n  - engagement\n---\n\n# Weekly Active Users\n\nSee also [MAU](../metrics/mau.md).",
        )
        .unwrap();

        let mut mau = std::fs::File::create(metrics.join("mau.md")).unwrap();
        mau.write_all(b"---\ntype: Metric\ntitle: MAU\n---\n\n# Monthly Active Users\n")
            .unwrap();

        (dir, root)
    }

    #[tokio::test]
    async fn test_okf_load_tool() {
        let (_dir, root) = create_test_bundle();
        let tool = OkfLoadTool::new();
        let ctx = ToolContext {
            working_dir: root.clone(),
            ..Default::default()
        };
        let result = tool
            .execute(json!({"path": root.to_string_lossy()}), ctx)
            .await
            .unwrap();
        assert_eq!(result["concept_count"].as_u64().unwrap(), 3);
        assert!(
            result["types"]
                .as_array()
                .unwrap()
                .contains(&json!("Metric"))
        );
        assert!(
            result["reserved_files"]
                .as_array()
                .unwrap()
                .contains(&json!("index"))
        );
    }

    #[tokio::test]
    async fn test_okf_load_tool_reports_skipped_files() {
        let (dir, root) = create_test_bundle();
        let mut bad = std::fs::File::create(root.join("draft.md")).unwrap();
        bad.write_all(b"# Draft\nNo frontmatter yet.").unwrap();

        let tool = OkfLoadTool::new();
        let ctx = ToolContext {
            working_dir: root.clone(),
            ..Default::default()
        };
        let result = tool
            .execute(json!({"path": root.to_string_lossy()}), ctx)
            .await
            .unwrap();
        assert_eq!(result["concept_count"].as_u64().unwrap(), 3);
        assert_eq!(result["skipped_count"].as_u64().unwrap(), 1);

        let skipped = result["skipped_files"].as_array().unwrap();
        assert!(
            skipped
                .iter()
                .any(|s| s.as_str().unwrap().contains("draft.md"))
        );

        // Simulate agent auto-fix: prepend frontmatter to the skipped file.
        let draft_path = root.join("draft.md");
        let original = std::fs::read_to_string(&draft_path).unwrap();
        let fixed = format!("---\ntype: note\ntitle: Draft\n---\n\n{}", original);
        std::fs::write(&draft_path, fixed).unwrap();

        // Invalidate the cache so the next load sees the fixed file.
        crate::okf::OkfBundleCache::global()
            .invalidate(&root)
            .unwrap();

        let ctx = ToolContext {
            working_dir: root.clone(),
            ..Default::default()
        };
        let result = tool
            .execute(json!({"path": root.to_string_lossy()}), ctx)
            .await
            .unwrap();
        assert_eq!(result["concept_count"].as_u64().unwrap(), 4);
        assert_eq!(result["skipped_count"].as_u64().unwrap(), 0);
        assert!(result["types"].as_array().unwrap().contains(&json!("note")));

        // Keep the TempDir alive until the end of the test.
        let _ = dir;
    }

    #[tokio::test]
    async fn test_okf_search_tool() {
        let (_dir, root) = create_test_bundle();
        let tool = OkfSearchTool::new();
        let ctx = ToolContext {
            working_dir: root.clone(),
            ..Default::default()
        };
        let result = tool
            .execute(
                json!({"path": root.to_string_lossy(), "query": "active"}),
                ctx,
            )
            .await
            .unwrap();
        let matches = result["matches"].as_array().unwrap();
        assert!(!matches.is_empty());
    }

    #[tokio::test]
    async fn test_okf_read_tool() {
        let (_dir, root) = create_test_bundle();
        let tool = OkfReadTool::new();
        let ctx = ToolContext {
            working_dir: root.clone(),
            ..Default::default()
        };
        let result = tool
            .execute(
                json!({"path": root.to_string_lossy(), "id": "metrics/wau"}),
                ctx,
            )
            .await
            .unwrap();
        assert_eq!(result["id"].as_str().unwrap(), "metrics/wau");
        assert_eq!(result["type"].as_str().unwrap(), "Metric");
        assert!(
            result["context"]
                .as_str()
                .unwrap()
                .contains("Weekly Active Users")
        );

        let outgoing = result["outgoing"].as_array().unwrap();
        assert!(outgoing.iter().any(|l| l["target"] == "metrics/mau"));
    }
}
