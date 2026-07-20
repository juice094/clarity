//! Query and result types for knowledge retrieval.

use std::path::PathBuf;

/// A query against the knowledge index.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SearchQuery {
    /// Free-text query.
    pub text: String,
    /// Maximum number of results to return.
    pub limit: usize,
    /// Whether to include graph neighbors in the result set.
    pub include_graph_neighbors: bool,
    /// Optional tag filter.
    pub tag: Option<String>,
    /// Optional session id for recall-effectiveness tracking.
    pub session_id: Option<String>,
}

impl SearchQuery {
    /// Create a simple text query with default limit.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            limit: 10,
            include_graph_neighbors: false,
            tag: None,
            session_id: None,
        }
    }

    /// Set the result limit.
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }

    /// Enable inclusion of graph neighbors.
    pub fn with_neighbors(mut self) -> Self {
        self.include_graph_neighbors = true;
        self
    }

    /// Filter by tag.
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = Some(tag.into());
        self
    }

    /// Associate the query with a session for recall-effectiveness tracking.
    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }
}

/// A single search result.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchResult {
    /// Path to the matching file.
    pub path: PathBuf,
    /// Optional human-readable title.
    pub title: Option<String>,
    /// Matched snippet or summary.
    pub snippet: String,
    /// Relevance score (higher is better).
    pub score: f64,
    /// Tags matched by the query.
    pub matched_tags: Vec<String>,
    /// Graph distance from an explicit match (0 = direct match).
    pub graph_distance: usize,
}
