//! Configuration for the LSP stdio client.

/// Configuration used to spawn and manage a language server.
#[derive(Debug, Clone)]
pub struct LspClientConfig {
    /// Command to execute (e.g. "rust-analyzer")
    pub command: String,
    /// Arguments passed to the command.
    pub args: Vec<String>,
    /// Workspace root URI (e.g. "file:///C:/project").
    /// If `None`, computed from `AgentConfig::working_dir` at hook creation time.
    pub root_uri: Option<String>,
    /// File extensions this server handles (e.g. `["rs"]`).
    pub file_extensions: Vec<String>,
    /// Whether the LSP client is enabled.
    pub enabled: bool,
}

impl LspClientConfig {
    /// Create a new config for a given command.
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            args: Vec::new(),
            root_uri: None,
            file_extensions: Vec::new(),
            enabled: true,
        }
    }

    /// Set command arguments.
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    /// Set workspace root URI.
    pub fn with_root_uri(mut self, uri: impl Into<String>) -> Self {
        self.root_uri = Some(uri.into());
        self
    }

    /// Set handled file extensions.
    pub fn with_extensions(mut self, exts: Vec<String>) -> Self {
        self.file_extensions = exts;
        self
    }

    /// Enable or disable the client.
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lsp_config_builder() {
        let config = LspClientConfig::new("rust-analyzer")
            .with_args(vec!["--test".to_string()])
            .with_root_uri("file:///project")
            .with_extensions(vec!["rs".to_string()])
            .with_enabled(false);
        assert_eq!(config.command, "rust-analyzer");
        assert_eq!(config.args, vec!["--test"]);
        assert_eq!(config.root_uri, Some("file:///project".to_string()));
        assert_eq!(config.file_extensions, vec!["rs"]);
        assert!(!config.enabled);
    }

    #[test]
    fn test_lsp_config_default_enabled() {
        let config = LspClientConfig::new("tsc");
        assert!(config.enabled);
        assert!(config.args.is_empty());
        assert!(config.file_extensions.is_empty());
    }
}
