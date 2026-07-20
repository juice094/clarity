//! Obsidian vault configuration parsing and wikilink resolution.
//!
//! This module reads publicly documented `.obsidian/*.json` files and uses them
//! to resolve wikilinks the same way Obsidian does, without depending on
//! Obsidian's proprietary code.

use crate::error::{KnowledgeError, Result};
use crate::extract::WikiLink;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Aggregated configuration for an Obsidian-compatible vault.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VaultConfig {
    /// Global application settings from `.obsidian/app.json`.
    pub app: AppConfig,
    /// Window layout state from `.obsidian/workspace.json`.
    pub workspace: Option<serde_json::Value>,
    /// Daily note settings from `.obsidian/daily-notes.json`.
    pub daily_notes: Option<DailyNotesConfig>,
    /// Template settings from `.obsidian/templates.json`.
    pub templates: Option<TemplatesConfig>,
    /// Property type mappings from `.obsidian/types.json`.
    pub types: Option<HashMap<String, String>>,
    /// Graph view settings from `.obsidian/graph.json`.
    pub graph: Option<serde_json::Value>,
    /// Enabled core plugins from `.obsidian/core-plugins.json`.
    pub core_plugins: Vec<String>,
    /// Enabled community plugins from `.obsidian/community-plugins.json`.
    pub community_plugins: Vec<String>,
}

impl VaultConfig {
    /// Load all known configuration files from a vault's `.obsidian` directory.
    ///
    /// Missing files are silently ignored; malformed files return an error.
    pub fn load(vault_root: &Path) -> Result<Self> {
        let obsidian_dir = vault_root.join(".obsidian");

        Ok(Self {
            app: Self::load_json(&obsidian_dir.join("app.json"))?.unwrap_or_default(),
            workspace: Self::load_json(&obsidian_dir.join("workspace.json"))?,
            daily_notes: Self::load_json(&obsidian_dir.join("daily-notes.json"))?,
            templates: Self::load_json(&obsidian_dir.join("templates.json"))?,
            types: Self::load_json(&obsidian_dir.join("types.json"))?,
            graph: Self::load_json(&obsidian_dir.join("graph.json"))?,
            core_plugins: Self::load_json(&obsidian_dir.join("core-plugins.json"))?
                .unwrap_or_default(),
            community_plugins: Self::load_json(&obsidian_dir.join("community-plugins.json"))?
                .unwrap_or_default(),
        })
    }

    fn load_json<T>(path: &Path) -> Result<Option<T>>
    where
        T: for<'de> Deserialize<'de> + Default,
    {
        if !path.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(path)?;
        if content.trim().is_empty() {
            return Ok(None);
        }
        serde_json::from_str(&content)
            .map(Some)
            .map_err(|e| KnowledgeError::Io(std::io::Error::other(format!("{path:?}: {e}"))))
    }
}

/// Global application settings.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    /// Default location for new notes.
    pub new_file_location: NewFileLocation,
    /// Default folder for attachments, relative to vault root.
    pub attachment_folder_path: Option<String>,
    /// Whether to use standard Markdown links instead of wikilinks.
    pub use_markdown_links: bool,
    /// Whether to show the note title inline in the editor.
    pub show_inline_title: bool,
    /// Whether Live Preview mode is enabled.
    pub live_preview: bool,
    /// Default editor view mode.
    pub default_view_mode: ViewMode,
    /// Whether to limit reading line length.
    pub readable_line_length: bool,
    /// Tab size in the editor.
    pub tab_size: u32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            new_file_location: NewFileLocation::Root,
            attachment_folder_path: None,
            use_markdown_links: false,
            show_inline_title: false,
            live_preview: true,
            default_view_mode: ViewMode::Source,
            readable_line_length: true,
            tab_size: 4,
        }
    }
}

/// Default location for newly created notes.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NewFileLocation {
    /// Vault root.
    #[default]
    Root,
    /// Same folder as the active note.
    Current,
    /// A specific folder path.
    #[serde(untagged)]
    Path(String),
}

/// Editor view mode.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ViewMode {
    /// Source Markdown editing.
    #[default]
    Source,
    /// Rendered preview.
    Preview,
}

/// Daily note plugin configuration.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct DailyNotesConfig {
    /// Folder where daily notes are stored.
    pub folder: Option<String>,
    /// Date format used for file names (moment.js style).
    pub format: Option<String>,
    /// Template file path.
    pub template: Option<String>,
}

/// Template plugin configuration.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct TemplatesConfig {
    /// Folder containing template files.
    pub folder: Option<String>,
    /// Date format used in template variables.
    pub date_format: Option<String>,
    /// Time format used in template variables.
    pub time_format: Option<String>,
}

/// Resolves wikilinks and Markdown links within a vault.
#[derive(Debug, Clone, Default)]
pub struct LinkResolver {
    config: AppConfig,
    vault_root: Option<PathBuf>,
    /// Maps file stem to all paths with that stem.
    index: HashMap<String, Vec<PathBuf>>,
}

impl LinkResolver {
    /// Create a new resolver for the given vault configuration.
    pub fn new(config: AppConfig) -> Self {
        Self {
            config,
            vault_root: None,
            index: HashMap::new(),
        }
    }

    /// Index all files under the vault root.
    ///
    /// Both Markdown notes and attachments (images, PDFs, etc.) are indexed by
    /// their file stem so that wikilinks and embeds can be resolved.
    pub fn index_vault(&mut self, vault_root: &Path) -> Result<()> {
        self.vault_root = Some(vault_root.to_path_buf());
        self.index.clear();
        for entry in walkdir::WalkDir::new(vault_root)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            self.add_path_to_index(path.to_path_buf());
        }
        Ok(())
    }

    /// Index an explicit set of paths.
    ///
    /// This is useful when the file system has already changed and the resolver
    /// must reason about the state captured in an in-memory index.
    pub fn index_paths(&mut self, vault_root: &Path, paths: impl IntoIterator<Item = PathBuf>) {
        self.vault_root = Some(vault_root.to_path_buf());
        self.index.clear();
        for path in paths {
            self.add_path_to_index(path);
        }
    }

    fn add_path_to_index(&mut self, path: PathBuf) {
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        self.index.entry(stem).or_default().push(path);
    }

    /// Resolve a wikilink to an absolute vault path.
    ///
    /// # Errors
    ///
    /// Returns an error if the target cannot be found or is ambiguous.
    pub fn resolve_wikilink(&self, link: &WikiLink, source_path: &Path) -> Result<PathBuf> {
        let target = &link.target;

        // Same-file heading reference: [[#heading]]
        if target.is_empty() {
            return Ok(source_path.to_path_buf());
        }

        // Absolute vault path already provided.
        if target.contains('/') {
            // Try as absolute vault path first.
            if let Some(root_path) = self.resolve_absolute_path(target) {
                return Ok(root_path);
            }
        }

        // Shortest path mode: match by stem.
        let stem = Path::new(target)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| target.clone());

        match self.index.get(&stem) {
            Some(paths) if paths.len() == 1 => Ok(paths[0].clone()),
            Some(paths) if paths.len() > 1 => {
                // Prefer a path in the same directory as the source.
                if let Some(same_dir) = paths.iter().find(|p| p.parent() == source_path.parent()) {
                    return Ok(same_dir.clone());
                }
                Err(KnowledgeError::Io(std::io::Error::other(format!(
                    "ambiguous wikilink '{}': matches {:?}",
                    target, paths
                ))))
            }
            _ => Err(KnowledgeError::Io(std::io::Error::other(format!(
                "wikilink target not found: {}",
                target
            )))),
        }
    }

    fn resolve_absolute_path(&self, target: &str) -> Option<PathBuf> {
        let target_path = Path::new(target);
        let vault_root = self.vault_root.as_ref()?;
        let stem = target_path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())?;
        self.index.get(&stem).and_then(|paths| {
            paths
                .iter()
                .find(|p| {
                    let Ok(rel) = p.strip_prefix(vault_root) else {
                        return false;
                    };
                    rel.file_stem() == target_path.file_stem()
                        && rel.parent() == target_path.parent()
                })
                .cloned()
        })
    }

    /// Return whether the vault uses standard Markdown links instead of wikilinks.
    pub fn uses_markdown_links(&self) -> bool {
        self.config.use_markdown_links
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_default_app_config() {
        let dir = tempfile::tempdir().unwrap();
        let config = VaultConfig::load(dir.path()).unwrap();
        assert!(!config.app.use_markdown_links);
        assert!(config.app.readable_line_length);
    }

    #[test]
    fn resolve_wikilink() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path();
        let notes = vault.join("notes");
        std::fs::create_dir(&notes).unwrap();

        let target = notes.join("target.md");
        std::fs::File::create(&target).unwrap();

        let mut resolver = LinkResolver::new(AppConfig::default());
        resolver.index_vault(vault).unwrap();

        let link = WikiLink {
            target: "target".to_string(),
            alias: None,
            heading: None,
            block_id: None,
            is_embed: false,
            raw: "[[target]]".to_string(),
        };

        let source = vault.join("source.md");
        let resolved = resolver.resolve_wikilink(&link, &source).unwrap();
        assert_eq!(resolved, target);
    }

    #[test]
    fn resolve_ambiguous_prefers_same_dir() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path();
        let notes = vault.join("notes");
        std::fs::create_dir(&notes).unwrap();

        let same_dir_target = vault.join("target.md");
        let other_target = notes.join("target.md");
        std::fs::File::create(&same_dir_target).unwrap();
        std::fs::File::create(&other_target).unwrap();

        let mut resolver = LinkResolver::new(AppConfig::default());
        resolver.index_vault(vault).unwrap();

        let link = WikiLink {
            target: "target".to_string(),
            alias: None,
            heading: None,
            block_id: None,
            is_embed: false,
            raw: "[[target]]".to_string(),
        };

        let source = vault.join("source.md");
        let resolved = resolver.resolve_wikilink(&link, &source).unwrap();
        assert_eq!(resolved, same_dir_target);
    }
}
