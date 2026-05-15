//! Endpoint abstraction layer — multi-frontend / multi-persona / multi-site contract.
//!
//! This module defines the **canonical descriptor** for any "endpoint" in the
//! Clarity ecosystem. An endpoint is anything that owns identity + capabilities
//! + a dispatch surface:
//!
//! - **Personas** (Clarity): "Gray" reasoner, "Analyst" data persona, "Programmer"
//!   coder persona. Each binds to a specific LLM provider + system prompt.
//! - **AI Sites** (OpenTeam-Core, future): ChatGPT / Claude / Gemini / DeepSeek.
//!   Each binds to a CDP-injection script + DOM selector set.
//! - **Frontend Adapters**: TUI / GUI / Headless. Each binds to a render path.
//!
//! By sharing a single `EndpointDescriptor` contract, downstream tooling (UI
//! switchers, routers, debug overlays) becomes endpoint-agnostic.
//!
//! ## Non-goals
//!
//! - This module does **not** dispatch traffic — it only describes endpoints.
//! - Concrete dispatch (chat send, browser eval, render frame) lives in the
//!   crate that owns the endpoint kind.
//!
//! ## Design constraints
//!
//! - Zero external deps beyond `serde` + `smol_str`.
//! - All types `Clone + Send + Sync` for cross-thread safety.
//! - Frozen versioned schema: changing `EndpointDescriptor` requires an ADR.

use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

/// What an endpoint can do. Frontends use this to render compatible icons
/// and to enable/disable feature panels per active endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EndpointCapability {
    /// Natural-language chat (the default).
    Chat,
    /// Code generation and editing.
    Coding,
    /// Structured data analysis (tables, charts, SQL).
    Analysis,
    /// Web browsing / DOM-level page interaction.
    Browse,
    /// Image understanding or generation.
    Vision,
    /// Tool / MCP invocation.
    ToolUse,
    /// Long-running planning workflows.
    Planning,
}

/// Where this endpoint physically dispatches its work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EndpointKind {
    /// Local GGUF inference via Candle.
    LocalLlm,
    /// Remote LLM API provider (OpenAI / Anthropic / etc.).
    RemoteLlm,
    /// Browser-injected AI site (OpenTeam Core).
    BrowserSite,
    /// In-process frontend adapter (GUI / TUI render layer).
    Frontend,
    /// External tool reachable via MCP.
    McpTool,
}

/// The canonical descriptor for any endpoint.
///
/// Versioned: do not add fields without bumping the schema version in
/// `ENDPOINT_SCHEMA_VERSION` and writing an ADR.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EndpointDescriptor {
    /// Schema version for migration safety.
    #[serde(default = "default_schema_version")]
    pub schema_version: u8,
    /// Stable kebab-case identifier (e.g. `"gray"`, `"chatgpt"`).
    pub id: SmolStr,
    /// Human-friendly display name (e.g. `"Gray"`, `"ChatGPT"`).
    pub display_name: SmolStr,
    /// One-line description for tooltips / switcher menus.
    pub description: SmolStr,
    /// Physical dispatch kind.
    pub kind: EndpointKind,
    /// What this endpoint can do — drives UI affordance gating.
    pub capabilities: Vec<EndpointCapability>,
    /// Optional Lucide icon name (e.g. `"brain"`, `"code"`, `"globe"`).
    /// Frontends may fall back to the first letter of `display_name` if unset.
    #[serde(default)]
    pub icon: Option<SmolStr>,
    /// Optional theme accent color override (hex `"#RRGGBB"`).
    #[serde(default)]
    pub accent: Option<SmolStr>,
}

/// Current schema version. Bump on any breaking change to `EndpointDescriptor`.
pub const ENDPOINT_SCHEMA_VERSION: u8 = 1;

fn default_schema_version() -> u8 {
    ENDPOINT_SCHEMA_VERSION
}

impl EndpointDescriptor {
    /// Convenience constructor for a Clarity persona backed by local inference.
    pub fn local_persona(
        id: impl Into<SmolStr>,
        display_name: impl Into<SmolStr>,
        description: impl Into<SmolStr>,
        capabilities: Vec<EndpointCapability>,
    ) -> Self {
        Self {
            schema_version: ENDPOINT_SCHEMA_VERSION,
            id: id.into(),
            display_name: display_name.into(),
            description: description.into(),
            kind: EndpointKind::LocalLlm,
            capabilities,
            icon: None,
            accent: None,
        }
    }

    /// Convenience constructor for a browser-injected AI site (OpenTeam-Core).
    pub fn browser_site(
        id: impl Into<SmolStr>,
        display_name: impl Into<SmolStr>,
        description: impl Into<SmolStr>,
    ) -> Self {
        Self {
            schema_version: ENDPOINT_SCHEMA_VERSION,
            id: id.into(),
            display_name: display_name.into(),
            description: description.into(),
            kind: EndpointKind::BrowserSite,
            capabilities: vec![EndpointCapability::Chat, EndpointCapability::Browse],
            icon: Some("globe".into()),
            accent: None,
        }
    }

    /// Returns true if this endpoint supports the given capability.
    pub fn supports(&self, cap: EndpointCapability) -> bool {
        self.capabilities.contains(&cap)
    }
}

/// In-memory registry of endpoints addressable by id.
///
/// Both the GUI persona switcher and the OpenTeam-Core site selector should
/// pull from a `EndpointRegistry` instance to avoid hard-coded lists.
#[derive(Debug, Clone, Default)]
pub struct EndpointRegistry {
    entries: Vec<EndpointDescriptor>,
}

impl EndpointRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new endpoint. Re-registering the same id overwrites (LIFO).
    pub fn register(&mut self, descriptor: EndpointDescriptor) {
        self.entries.retain(|e| e.id != descriptor.id);
        self.entries.push(descriptor);
    }

    /// Look up an endpoint by id.
    pub fn get(&self, id: &str) -> Option<&EndpointDescriptor> {
        self.entries.iter().find(|e| e.id.as_str() == id)
    }

    /// Iterate all registered endpoints.
    pub fn iter(&self) -> impl Iterator<Item = &EndpointDescriptor> {
        self.entries.iter()
    }

    /// Filter endpoints by capability — useful for "show me everything that
    /// can do Browse" in the OpenTeam switcher.
    pub fn with_capability(
        &self,
        cap: EndpointCapability,
    ) -> impl Iterator<Item = &EndpointDescriptor> {
        self.entries.iter().filter(move |e| e.supports(cap))
    }

    /// Number of registered endpoints.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Build the default Clarity persona set (Gray / Analyst / Programmer).
///
/// This is the minimum viable registry for S8 P3B.1 Top Bar Persona switcher.
/// Future personas may be added by external configuration without touching
/// this code path (see `personality/domain.rs` for the TOML loader).
pub fn default_clarity_personas() -> EndpointRegistry {
    let mut reg = EndpointRegistry::new();

    reg.register(EndpointDescriptor {
        schema_version: ENDPOINT_SCHEMA_VERSION,
        id: "gray".into(),
        display_name: "Gray".into(),
        description: "Default reasoner — balanced general-purpose persona.".into(),
        kind: EndpointKind::LocalLlm,
        capabilities: vec![
            EndpointCapability::Chat,
            EndpointCapability::Planning,
            EndpointCapability::ToolUse,
        ],
        icon: Some("brain".into()),
        accent: None,
    });

    reg.register(EndpointDescriptor {
        schema_version: ENDPOINT_SCHEMA_VERSION,
        id: "analyst".into(),
        display_name: "Analyst".into(),
        description: "Data persona — tables, SQL, structured analysis.".into(),
        kind: EndpointKind::LocalLlm,
        capabilities: vec![
            EndpointCapability::Chat,
            EndpointCapability::Analysis,
            EndpointCapability::ToolUse,
        ],
        icon: Some("bar-chart".into()),
        accent: Some("#5B8DEF".into()),
    });

    reg.register(EndpointDescriptor {
        schema_version: ENDPOINT_SCHEMA_VERSION,
        id: "programmer".into(),
        display_name: "Programmer".into(),
        description: "Code persona — generation, refactoring, debugging.".into(),
        kind: EndpointKind::LocalLlm,
        capabilities: vec![
            EndpointCapability::Chat,
            EndpointCapability::Coding,
            EndpointCapability::ToolUse,
        ],
        icon: Some("code".into()),
        accent: Some("#6BCB8A".into()),
    });

    reg
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_personas_has_three_entries() {
        let reg = default_clarity_personas();
        assert_eq!(reg.len(), 3);
        assert!(reg.get("gray").is_some());
        assert!(reg.get("analyst").is_some());
        assert!(reg.get("programmer").is_some());
    }

    #[test]
    fn register_overwrites_same_id() {
        let mut reg = EndpointRegistry::new();
        reg.register(EndpointDescriptor::local_persona(
            "test",
            "First",
            "desc1",
            vec![EndpointCapability::Chat],
        ));
        reg.register(EndpointDescriptor::local_persona(
            "test",
            "Second",
            "desc2",
            vec![EndpointCapability::Chat],
        ));
        assert_eq!(reg.len(), 1);
        assert_eq!(reg.get("test").unwrap().display_name, "Second");
    }

    #[test]
    fn capability_filter_works() {
        let reg = default_clarity_personas();
        let coders: Vec<_> = reg.with_capability(EndpointCapability::Coding).collect();
        assert_eq!(coders.len(), 1);
        assert_eq!(coders[0].id, "programmer");

        let chatters: Vec<_> = reg.with_capability(EndpointCapability::Chat).collect();
        assert_eq!(chatters.len(), 3);
    }

    #[test]
    fn browser_site_factory_sets_browse_capability() {
        let site = EndpointDescriptor::browser_site(
            "chatgpt",
            "ChatGPT",
            "OpenAI's flagship assistant via DOM injection.",
        );
        assert_eq!(site.kind, EndpointKind::BrowserSite);
        assert!(site.supports(EndpointCapability::Browse));
        assert!(site.supports(EndpointCapability::Chat));
        assert_eq!(site.icon.as_deref(), Some("globe"));
    }

    #[test]
    fn serde_roundtrip_preserves_all_fields() {
        let original = EndpointDescriptor {
            schema_version: ENDPOINT_SCHEMA_VERSION,
            id: "test".into(),
            display_name: "Test".into(),
            description: "A test endpoint.".into(),
            kind: EndpointKind::LocalLlm,
            capabilities: vec![EndpointCapability::Chat, EndpointCapability::Vision],
            icon: Some("eye".into()),
            accent: Some("#FFAA00".into()),
        };
        let json = serde_json::to_string(&original).unwrap();
        let restored: EndpointDescriptor = serde_json::from_str(&json).unwrap();
        assert_eq!(original, restored);
    }

    #[test]
    fn empty_registry_returns_none_on_get() {
        let reg = EndpointRegistry::new();
        assert!(reg.is_empty());
        assert!(reg.get("anything").is_none());
    }
}
