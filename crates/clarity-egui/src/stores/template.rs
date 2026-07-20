//! Templates panel state — built-in and remote prompt template library.

/// A built-in prompt template that can be injected into the chat input.
#[derive(Clone, Debug)]
pub struct BuiltInTemplate {
    pub name: &'static str,
    pub description: &'static str,
    pub icon: &'static str,
    pub prompt: &'static str,
}

/// Default built-in template library. Remote templates are merged here when
/// the marketplace backend ships.
pub const BUILT_IN_TEMPLATES: &[BuiltInTemplate] = &[
    BuiltInTemplate {
        name: "Code Review",
        description: "Review code for bugs, security issues, and style violations",
        icon: crate::theme::ICON_CHECK,
        prompt: "Please review the following code for bugs, security issues, and style violations. Provide specific, actionable feedback:\n\n```\n\n```",
    },
    BuiltInTemplate {
        name: "Bug Fix",
        description: "Investigate and fix a bug described below",
        icon: crate::theme::ICON_WARNING,
        prompt: "I need to fix a bug:\n\n**Steps to reproduce:**\n\n**Expected behavior:**\n\n**Actual behavior:**\n\n**Environment:**\n\nPlease investigate the root cause and propose a fix.",
    },
    BuiltInTemplate {
        name: "New Feature",
        description: "Implement a new feature from specification",
        icon: crate::theme::ICON_PLUS,
        prompt: "Please implement the following feature:\n\n**Goal:**\n\n**Requirements:**\n\n**Acceptance criteria:**\n\n",
    },
    BuiltInTemplate {
        name: "Refactor",
        description: "Restructure existing code without changing behavior",
        icon: crate::theme::ICON_WRENCH,
        prompt: "Please refactor the following code to improve clarity, performance, and maintainability without changing its external behavior:\n\n**Current issues:**\n\n**Target patterns:**\n\n",
    },
    BuiltInTemplate {
        name: "Write Tests",
        description: "Generate unit and integration tests for existing code",
        icon: crate::theme::ICON_FILE_CODE,
        prompt: "Please write comprehensive tests for the following code. Include:\n- Unit tests for edge cases\n- Integration tests where appropriate\n- Any necessary test fixtures\n\n",
    },
];

/// Remote template from the marketplace. Reserved for future backend integration.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct RemoteTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub author: String,
    pub downloads: u64,
    pub tags: Vec<String>,
}

/// Template library state.
#[derive(Clone, Debug)]
#[allow(dead_code)] // remote_templates/search_query reserved for marketplace backend
pub struct TemplateStore {
    pub built_in: Vec<BuiltInTemplate>,
    pub remote_templates: Option<Vec<RemoteTemplate>>,
    pub search_query: String,
}

impl Default for TemplateStore {
    fn default() -> Self {
        Self {
            built_in: BUILT_IN_TEMPLATES.to_vec(),
            remote_templates: None,
            search_query: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_store_loads_built_in_templates() {
        let store = TemplateStore::default();
        assert!(!store.built_in.is_empty());
    }

    #[test]
    fn built_in_templates_have_icon_and_prompt() {
        for tmpl in BUILT_IN_TEMPLATES {
            assert!(!tmpl.name.is_empty());
            assert!(!tmpl.icon.is_empty());
            assert!(!tmpl.prompt.is_empty());
        }
    }
}
