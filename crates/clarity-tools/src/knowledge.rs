//! Knowledge field tools for dynamic graph retrieval.

use async_trait::async_trait;
use clarity_contract::{Tool, ToolContext, ToolError, ToolResult};
use clarity_knowledge::{KnowledgeField, SearchQuery};
use serde_json::{Value, json};
use std::sync::Arc;

/// Search the knowledge field for relevant files and concepts.
///
/// The tool activates anchor nodes matching the query, propagates activation
/// through the graph, and returns the most activated results. This lets the
/// agent discover context that is structurally related but not lexically
/// similar to the query.
pub struct KnowledgeSearchTool {
    field: Arc<KnowledgeField>,
}

impl KnowledgeSearchTool {
    /// Create a new knowledge search tool backed by the given field.
    pub fn new(field: Arc<KnowledgeField>) -> Self {
        Self { field }
    }
}

#[async_trait]
impl Tool for KnowledgeSearchTool {
    fn name(&self) -> &str {
        "knowledge_search"
    }

    fn description(&self) -> &str {
        "Search the local knowledge field for files and concepts relevant to a query. \
         Returns ranked results with activation scores and snippets."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Free-text query. Supports tag:path: and file: operators."
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of results to return",
                    "minimum": 1,
                    "maximum": 50,
                    "default": 10
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, args: Value, _ctx: ToolContext) -> ToolResult<Value> {
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::invalid_params("missing query"))?;
        let limit = args
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|v| (v as usize).min(50))
            .unwrap_or(10);

        let search_query = SearchQuery::new(query).with_limit(limit);
        let results = self
            .field
            .search(&search_query)
            .map_err(|e| ToolError::execution_failed(format!("knowledge search failed: {e}")))?;

        let items: Vec<Value> = results
            .into_iter()
            .map(|r| {
                json!({
                    "path": r.path.to_string_lossy(),
                    "title": r.title,
                    "snippet": r.snippet,
                    "activation": r.activation,
                    "matched_tags": r.matched_tags,
                })
            })
            .collect();

        Ok(json!({
            "query": query,
            "results": items,
            "returned": items.len()
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clarity_contract::Tool;
    use clarity_knowledge::FieldConfig;
    use serde_json::json;

    #[tokio::test]
    async fn knowledge_search_tool_returns_results() {
        let field = Arc::new(KnowledgeField::new(FieldConfig::default()));
        let tool = KnowledgeSearchTool::new(field);

        let args = json!({"query": "rust", "limit": 5});
        let result = tool.execute(args, ToolContext::new()).await.unwrap();

        assert!(result["results"].is_array());
        assert_eq!(result["query"], "rust");
    }
}
