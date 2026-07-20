//! Knowledge panel state for browsing OKF bundles and querying the knowledge field.

use clarity_core::okf::{OkfBundle, OkfConcept};
#[cfg(test)]
use clarity_knowledge::ExtractedDocument;
use clarity_knowledge::{FieldResult, FileWatcher, KnowledgeField, SearchQuery};
use std::path::PathBuf;
use std::sync::Arc;

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

    // ── Knowledge Field state ──
    /// Filesystem path to an external markdown vault (e.g. Obsidian).
    pub vault_path: String,
    /// Reference to the shared knowledge field. `None` only in tests that do
    /// not need field queries.
    pub field: Option<Arc<KnowledgeField>>,
    /// Current query against the knowledge field.
    pub field_query: String,
    /// Results from the last knowledge-field search.
    pub field_results: Vec<FieldResult>,
    /// Path of the currently selected field result.
    pub selected_field_path: Option<PathBuf>,
    /// Whether the vault watcher is currently running.
    pub vault_watching: bool,
    /// Abort handle for the background vault watcher task.
    pub vault_watcher_abort: Option<tokio::task::AbortHandle>,
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

    /// Attach the shared knowledge field so the panel can query it.
    pub fn set_field(&mut self, field: Arc<KnowledgeField>) {
        self.field = Some(field);
    }

    /// Search the knowledge field and store the results.
    pub fn search_field(&mut self) {
        self.field_results.clear();
        let Some(ref field) = self.field else {
            return;
        };
        let query = SearchQuery::new(&self.field_query).with_limit(10);
        match field.search(&query) {
            Ok(results) => self.field_results = results,
            Err(e) => {
                tracing::warn!("Knowledge field search failed: {}", e);
            }
        }
    }

    /// Update the knowledge-field query. The search itself is triggered
    /// explicitly (e.g. on Enter or button click) to avoid indexing cost
    /// on every keystroke.
    pub fn set_field_query(&mut self, query: String) {
        self.field_query = query;
    }

    /// Select a field result by path.
    pub fn select_field_path(&mut self, path: PathBuf) {
        self.selected_field_path = Some(path);
    }

    /// Index the external markdown vault at `vault_path` into the shared
    /// knowledge field. Returns the number of files indexed.
    pub fn index_vault(&mut self) -> usize {
        let Some(ref field) = self.field else {
            return 0;
        };
        let path = PathBuf::from(&self.vault_path);
        if path.as_os_str().is_empty() {
            return 0;
        }
        match field.index_directory(&path) {
            Ok(n) => n,
            Err(e) => {
                tracing::warn!("Failed to index vault {:?}: {}", path, e);
                0
            }
        }
    }

    /// Start watching the vault at `vault_path` for changes.
    ///
    /// Before the watcher begins, the entire vault is indexed once on a
    /// blocking task to establish a baseline. Subsequent file-system events are
    /// debounced for 100 ms, batched, deduplicated, and sent back through
    /// `ui_tx` as `UiEvent::KnowledgeVaultEvents` to be applied incrementally
    /// to the shared knowledge field.
    pub fn start_watching_vault(
        &mut self,
        runtime: &tokio::runtime::Runtime,
        ui_tx: std::sync::mpsc::Sender<crate::ui::types::UiEvent>,
    ) {
        self.stop_watching_vault();
        let path = PathBuf::from(&self.vault_path);
        if path.as_os_str().is_empty() {
            return;
        }
        let Some(field) = self.field.clone() else {
            return;
        };

        let handle = runtime.spawn(async move {
            // Baseline: index the whole vault once before listening for deltas.
            let baseline = tokio::task::spawn_blocking({
                let path = path.clone();
                let field = field.clone();
                move || field.index_directory(&path)
            })
            .await;
            match baseline {
                Ok(Ok(count)) => {
                    tracing::info!("Indexed {} vault files as watcher baseline", count)
                }
                Ok(Err(e)) => tracing::warn!("Failed to baseline-index vault: {}", e),
                Err(e) => tracing::warn!("Baseline indexing task panicked: {}", e),
            }

            let mut watcher = clarity_knowledge::NotifyWatcher::new();
            let config = clarity_knowledge::SourceConfig::new(path);
            if let Err(e) = watcher.watch(config).await {
                tracing::warn!("Failed to start vault watcher: {}", e);
                return;
            }
            let debounce = std::time::Duration::from_millis(100);
            loop {
                // Wait for the first event in a batch.
                let first = match watcher.next_event().await {
                    Ok(Some(event)) => event,
                    Ok(None) => break,
                    Err(e) => {
                        tracing::warn!("Vault watcher error: {}", e);
                        break;
                    }
                };

                let mut batch = vec![first];
                // Collect additional events that arrive within the debounce window.
                loop {
                    match tokio::time::timeout(debounce, watcher.next_event()).await {
                        Ok(Ok(Some(event))) => batch.push(event),
                        Ok(Ok(None)) => {
                            // Watcher closed while debouncing; flush batch and exit.
                            let _ =
                                ui_tx.send(crate::ui::types::UiEvent::KnowledgeVaultEvents(batch));
                            return;
                        }
                        Ok(Err(e)) => {
                            tracing::warn!("Vault watcher error: {}", e);
                            let _ =
                                ui_tx.send(crate::ui::types::UiEvent::KnowledgeVaultEvents(batch));
                            return;
                        }
                        Err(_) => {
                            // Debounce window expired.
                            break;
                        }
                    }
                }

                let _ = ui_tx.send(crate::ui::types::UiEvent::KnowledgeVaultEvents(batch));
            }
        });

        self.vault_watching = true;
        self.vault_watcher_abort = Some(handle.abort_handle());
    }

    /// Stop the background vault watcher, if any.
    pub fn stop_watching_vault(&mut self) {
        if let Some(abort) = self.vault_watcher_abort.take() {
            abort.abort();
        }
        self.vault_watching = false;
    }

    /// Refresh `field_results` with the top-`k` most activated nodes from the
    /// shared knowledge field. Used by the "Top active" button in the UI.
    pub fn refresh_top_activated(&mut self, k: usize) {
        self.field_results.clear();
        let Some(ref field) = self.field else {
            return;
        };
        self.field_results = field
            .top_activated(k)
            .into_iter()
            .map(|(path, activation)| {
                let title = path.file_stem().map(|s| s.to_string_lossy().to_string());
                FieldResult {
                    path,
                    title,
                    snippet: String::new(),
                    activation,
                    semantic_score: 0.0,
                    matched_tags: Vec::new(),
                }
            })
            .collect();
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

    #[test]
    fn set_field_attaches_shared_field() {
        let mut store = KnowledgeStore::new();
        assert!(store.field.is_none());
        let field = Arc::new(KnowledgeField::new(Default::default()));
        store.set_field(field.clone());
        assert!(store.field.is_some());
        assert!(Arc::ptr_eq(store.field.as_ref().unwrap(), &field));
    }

    #[test]
    fn search_field_without_field_is_noop() {
        let mut store = KnowledgeStore::new();
        store.field_query = "activation".to_string();
        store.search_field();
        assert!(store.field_results.is_empty());
    }

    #[test]
    fn search_field_returns_indexed_results() {
        let field = Arc::new(KnowledgeField::new(Default::default()));
        field
            .index_document(ExtractedDocument {
                path: std::path::PathBuf::from("/vault/activation.md"),
                title: Some("Activation dynamics".to_string()),
                content: "Spreading activation moves energy through the graph.".to_string(),
                frontmatter: serde_json::Value::Null,
                links: Vec::new(),
                tags: vec!["knowledge".to_string()],
                headings: Vec::new(),
            })
            .unwrap();

        let mut store = KnowledgeStore::new();
        store.set_field(field);
        store.field_query = "spreading energy".to_string();
        store.search_field();

        assert!(!store.field_results.is_empty());
        let top = &store.field_results[0];
        assert_eq!(top.path, std::path::PathBuf::from("/vault/activation.md"));
        assert_eq!(top.title, Some("Activation dynamics".to_string()));
        assert!(top.activation > 0.0);
    }

    #[test]
    fn refresh_top_activated_returns_most_active_node() {
        let field = Arc::new(KnowledgeField::new(Default::default()));
        field
            .index_document(ExtractedDocument {
                path: std::path::PathBuf::from("/vault/decay.md"),
                title: Some("Decay over time".to_string()),
                content: "Energy decays according to half-life.".to_string(),
                frontmatter: serde_json::Value::Null,
                links: Vec::new(),
                tags: Vec::new(),
                headings: Vec::new(),
            })
            .unwrap();
        field
            .index_document(ExtractedDocument {
                path: std::path::PathBuf::from("/vault/inhibition.md"),
                title: Some("Lateral inhibition".to_string()),
                content: "Active nodes suppress neighbors.".to_string(),
                frontmatter: serde_json::Value::Null,
                links: Vec::new(),
                tags: Vec::new(),
                headings: Vec::new(),
            })
            .unwrap();

        let mut store = KnowledgeStore::new();
        store.set_field(field.clone());
        store.field_query = "half-life".to_string();
        store.search_field();
        store.refresh_top_activated(2);

        assert!(!store.field_results.is_empty());
        assert!(store.field_results.len() <= 2);
        assert_eq!(
            store.field_results[0].path,
            std::path::PathBuf::from("/vault/decay.md")
        );
    }

    #[test]
    fn select_field_path_updates_selected_path() {
        let mut store = KnowledgeStore::new();
        assert!(store.selected_field_path.is_none());
        let path = std::path::PathBuf::from("/vault/edge_cases.md");
        store.select_field_path(path.clone());
        assert_eq!(store.selected_field_path, Some(path));
    }

    #[test]
    fn watcher_baseline_indexes_existing_vault() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("note.md");
        std::fs::write(&file, "# Note\nContent here.").unwrap();

        let runtime = tokio::runtime::Runtime::new().unwrap();
        let (tx, _rx) = std::sync::mpsc::channel();
        let mut store = KnowledgeStore::new();
        store.set_field(Arc::new(KnowledgeField::new(Default::default())));
        store.vault_path = dir.path().to_string_lossy().to_string();

        store.start_watching_vault(&runtime, tx);
        assert!(store.vault_watching);

        // Wait for the baseline indexing task to finish.
        std::thread::sleep(std::time::Duration::from_millis(300));

        let results = store
            .field
            .as_ref()
            .unwrap()
            .search(&SearchQuery::new("Content here").with_limit(5))
            .unwrap();
        assert!(results.iter().any(|r| r.path == file));

        store.stop_watching_vault();
        assert!(!store.vault_watching);
    }
}
