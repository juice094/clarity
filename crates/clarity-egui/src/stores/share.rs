//! Share panel state — export format and sharing options.

/// Export format options for the right-rail Share panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ExportFormat {
    /// Markdown export.
    #[default]
    Markdown,
    /// JSON export.
    Json,
    /// HTML export.
    Html,
}

impl ExportFormat {
    /// Human-readable label key (translated via `app.t()` at render time).
    pub fn label_key(&self) -> &'static str {
        match self {
            Self::Markdown => "Markdown",
            Self::Json => "JSON",
            Self::Html => "HTML",
        }
    }

    /// File extension for the export format.
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Markdown => "md",
            Self::Json => "json",
            Self::Html => "html",
        }
    }
}

/// UI state for the Share panel.
#[derive(Clone, Debug, Default)]
pub struct ShareStore {
    /// Currently selected export format.
    pub export_format: ExportFormat,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_format_is_markdown() {
        let store = ShareStore::default();
        assert_eq!(store.export_format, ExportFormat::Markdown);
    }

    #[test]
    fn extensions_match_format() {
        assert_eq!(ExportFormat::Markdown.extension(), "md");
        assert_eq!(ExportFormat::Json.extension(), "json");
        assert_eq!(ExportFormat::Html.extension(), "html");
    }
}
