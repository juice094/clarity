//! Project Store
//!
//! Project list, archived projects, and session grouping.
//!
//! S6 Phase D: this store starts as a UI-layer mock / local experiment. Once
//! the project/session model stabilises it will be synced with `clarity-core`.

use crate::ui::types::Project;

/// Holds project UI state.
#[derive(Clone, Debug, Default)]
pub struct ProjectStore {
    pub projects: Vec<Project>,
    pub archived_projects: Vec<Project>,
    /// ID of the project currently selected in the navigation tree.
    pub selected_project_id: Option<String>,
}

impl ProjectStore {
    /// Create a new empty project store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Restore an archived project.
    pub fn unarchive(&mut self, id: &str) {
        if let Some(pos) = self.archived_projects.iter().position(|p| p.id == id) {
            let mut p = self.archived_projects.remove(pos);
            p.archived = false;
            self.projects.push(p);
        }
    }
}

// ============================================================================
// Tests
// ============================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::types::Project;

    fn sample_project(id: &str, name: &str) -> Project {
        Project {
            id: id.into(),
            name: name.into(),
            archived: false,
            has_workspace: true,
        }
    }

    #[test]
    fn project_store_starts_empty() {
        let store = ProjectStore::new();
        assert!(store.projects.is_empty());
        assert!(store.archived_projects.is_empty());
        assert!(store.selected_project_id.is_none());
    }

    #[test]
    fn unarchive_restores_project() {
        let mut store = ProjectStore::new();
        store.archived_projects.push(sample_project("p-2", "beta"));
        store.archived_projects[0].archived = true;
        store.unarchive("p-2");
        assert!(store.archived_projects.is_empty());
        assert_eq!(store.projects.len(), 1);
        assert!(!store.projects[0].archived);
    }
}
