//! Knowledge panel state for browsing and searching OKF bundles.

use clarity_core::okf::{OkfBundle, OkfConcept};

/// UI state for the OKF Knowledge panel.
#[derive(Clone, Debug, Default)]
pub struct KnowledgeStore {
    /// Filesystem path to the OKF bundle directory.
    pub bundle_path: String,
    /// Last successfully loaded bundle.
    pub bundle: Option<OkfBundle>,
    /// Current search query.
    pub query: String,
    /// Concepts matching the current query (or all concepts when empty).
    pub results: Vec<OkfConcept>,
    /// Currently selected concept id.
    pub selected_id: Option<String>,
    /// Loading indicator.
    pub loading: bool,
    /// Last error message, if any.
    pub error: Option<String>,
}

impl KnowledgeStore {
    /// Create a new knowledge store with an empty bundle path.
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace the current bundle and refresh the result list.
    pub fn set_bundle(&mut self, bundle: OkfBundle) {
        self.error = None;
        self.results = self.filter(&bundle);
        self.bundle = Some(bundle);
    }

    /// Update the search query and refresh results from the loaded bundle.
    pub fn set_query(&mut self, query: String) {
        self.query = query;
        if let Some(bundle) = &self.bundle {
            self.results = self.filter(bundle);
        }
    }

    /// Select a concept by id.
    pub fn select(&mut self, id: String) {
        self.selected_id = if id.is_empty() { None } else { Some(id) };
    }

    fn filter(&self, bundle: &OkfBundle) -> Vec<OkfConcept> {
        let trimmed = self.query.trim();
        if trimmed.is_empty() {
            bundle.iter().cloned().collect()
        } else {
            bundle.search(trimmed).into_iter().cloned().collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clarity_core::okf::{OkfConcept, OkfFrontmatter};
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn sample_bundle() -> OkfBundle {
        let mut concepts = HashMap::new();
        concepts.insert(
            "metrics/wau".to_string(),
            OkfConcept {
                id: "metrics/wau".to_string(),
                path: PathBuf::from("/tmp/metrics/wau.md"),
                frontmatter: OkfFrontmatter {
                    r#type: "Metric".to_string(),
                    title: Some("WAU".to_string()),
                    description: Some("Weekly active users".to_string()),
                    tags: vec!["engagement".to_string()],
                    ..Default::default()
                },
                body: "# Details".to_string(),
                is_reserved: false,
            },
        );
        concepts.insert(
            "datasets/users".to_string(),
            OkfConcept {
                id: "datasets/users".to_string(),
                path: PathBuf::from("/tmp/datasets/users.md"),
                frontmatter: OkfFrontmatter {
                    r#type: "Dataset".to_string(),
                    title: Some("User Table".to_string()),
                    description: Some("User records".to_string()),
                    tags: vec!["analytics".to_string()],
                    ..Default::default()
                },
                body: "# Schema".to_string(),
                is_reserved: false,
            },
        );
        OkfBundle {
            root: PathBuf::from("/tmp"),
            concepts,
            warnings: vec![],
        }
    }

    #[test]
    fn set_bundle_populates_results() {
        let mut store = KnowledgeStore::new();
        let bundle = sample_bundle();
        store.set_bundle(bundle);
        assert_eq!(store.results.len(), 2);
        assert!(store.error.is_none());
    }

    #[test]
    fn search_filters_results() {
        let mut store = KnowledgeStore::new();
        store.set_bundle(sample_bundle());
        store.set_query("weekly".to_string());
        assert_eq!(store.results.len(), 1);
        assert_eq!(store.results[0].id, "metrics/wau");
    }

    #[test]
    fn select_updates_selected_id() {
        let mut store = KnowledgeStore::new();
        store.select("metrics/wau".to_string());
        assert_eq!(store.selected_id.as_deref(), Some("metrics/wau"));
        store.select(String::new());
        assert!(store.selected_id.is_none());
    }
}
