//! Plugin toolbar store.
//!
//! S6 Phase C: the left-rail Plugins panel exposes user-customisable shortcuts
//! that can be reordered in layout-edit mode.  Plugins may come from built-in
//! actions, active skills, connected MCP tools, or persisted web tabs.
//!
//! Note: currently staged; not yet wired into App. Kept for upcoming
//! left-rail plugin integration.
//!
//! # Layering
//!
//! This store depends only on `crate::settings::GuiSettings` (data type) and
//! `clarity_core::agent::Agent` (core crate). MCP-aware functions accept their
//! configuration as a parameter so the store never imports sibling stores
//! (avoiding the only cross-store dependency in the entire stores/ layer).
#![allow(dead_code)]

use crate::settings::GuiSettings;
use serde::{Deserialize, Serialize};

/// Source of a plugin entry.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case", tag = "source", content = "data")]
pub enum PluginSource {
    /// Built-in action shipped with the GUI.
    Builtin { id: String },
    /// Skill exposed as a shortcut.
    Skill { skill_id: String },
    /// MCP tool exposed as a shortcut.
    Mcp { tool_name: String },
    /// Web tab shortcut.
    WebTab { url: String },
}

/// A single entry in the plugin toolbar.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct PluginItem {
    /// Stable identifier used for ordering and persistence.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Semantic icon key.  The UI maps this key to an actual egui icon/font
    /// glyph (rendered via Lucide icon font; see ADR-010).
    pub icon: String,
    /// Where this plugin came from.
    pub source: PluginSource,
}

impl PluginItem {
    /// Create a built-in plugin item.
    pub fn builtin(id: &str, name: &str, icon: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            icon: icon.to_string(),
            source: PluginSource::Builtin { id: id.to_string() },
        }
    }
}

/// UI state for the plugin toolbar.
#[derive(Default)]
pub struct PluginStore {
    /// Index of the plugin currently being dragged, if any.
    pub drag_index: Option<usize>,
}

impl PluginStore {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Built-in plugins that are always available.
pub fn builtin_plugins() -> Vec<PluginItem> {
    vec![
        PluginItem::builtin("chat", "通用对话", "message_square"),
        PluginItem::builtin("code", "代码助手", "file_code"),
        PluginItem::builtin("work", "工作/项目", "briefcase"),
        PluginItem::builtin("claw", "远程设备", "bot"),
        PluginItem::builtin("doc", "文档", "file_text"),
        PluginItem::builtin("web", "网站", "globe"),
        PluginItem::builtin("sheet", "表格", "table"),
        PluginItem::builtin("ppt", "PPT", "presentation"),
    ]
}

/// Derive plugin items from currently loaded skills.
pub fn skill_plugins(agent: &clarity_core::agent::Agent) -> Vec<PluginItem> {
    agent
        .list_skills()
        .into_iter()
        .map(|skill| {
            let id = skill.meta.id.clone();
            let icon = skill_icon_for(&skill.meta.tags, &skill.meta.skill_type);
            PluginItem {
                id: format!("skill:{}", id),
                name: if skill.meta.name.is_empty() {
                    id.clone()
                } else {
                    skill.meta.name.clone()
                },
                icon,
                source: PluginSource::Skill { skill_id: id },
            }
        })
        .collect()
}

/// Derive plugin items from connected MCP servers.
///
/// Accepts the MCP configuration directly rather than the entire `McpStore`
/// so this module has zero cross-store dependencies.
pub fn mcp_plugins(mcp_config: Option<&clarity_core::mcp::config::McpConfig>) -> Vec<PluginItem> {
    let Some(config) = mcp_config else {
        return Vec::new();
    };
    config
        .servers
        .iter()
        .filter(|(_, entry)| !entry.disabled)
        .map(|(name, _)| PluginItem {
            id: format!("mcp:{}", name),
            name: name.clone(),
            icon: "wrench".to_string(),
            source: PluginSource::Mcp {
                tool_name: name.clone(),
            },
        })
        .collect()
}

/// Derive plugin items from persisted web tabs.
pub fn webtab_plugins(settings: &GuiSettings) -> Vec<PluginItem> {
    settings
        .web_tabs
        .iter()
        .map(|tab| {
            let host = tab
                .url
                .trim_start_matches("https://")
                .trim_start_matches("http://")
                .split('/')
                .next()
                .unwrap_or("web")
                .to_string();
            PluginItem {
                id: format!("webtab:{}", tab.url),
                name: if tab.title.is_empty() {
                    host.clone()
                } else {
                    tab.title.clone()
                },
                icon: "globe".to_string(),
                source: PluginSource::WebTab {
                    url: tab.url.clone(),
                },
            }
        })
        .collect()
}

/// Collect all available plugin items.
///
/// Accepts the MCP configuration directly rather than the entire `McpStore`
/// so this module has zero cross-store dependencies.
pub fn all_plugins(
    agent: &clarity_core::agent::Agent,
    mcp_config: Option<&clarity_core::mcp::config::McpConfig>,
    settings: &GuiSettings,
) -> Vec<PluginItem> {
    let mut items = builtin_plugins();
    items.extend(skill_plugins(agent));
    items.extend(mcp_plugins(mcp_config));
    items.extend(webtab_plugins(settings));
    items
}

/// Reorder `all` according to `order`.  Items not present in `order` are
/// appended at the end in their original order.
pub fn ordered_plugins(all: &[PluginItem], order: &[String]) -> Vec<PluginItem> {
    let mut ordered: Vec<PluginItem> = Vec::with_capacity(all.len());
    let mut used = std::collections::HashSet::with_capacity(all.len());

    for id in order {
        if let Some(item) = all.iter().find(|p| &p.id == id) {
            if used.insert(id.clone()) {
                ordered.push(item.clone());
            }
        }
    }

    for item in all {
        if used.insert(item.id.clone()) {
            ordered.push(item.clone());
        }
    }

    ordered
}

fn skill_icon_for(tags: &[String], skill_type: &str) -> String {
    let lowered: Vec<_> = tags.iter().map(|t| t.to_lowercase()).collect();
    if lowered
        .iter()
        .any(|t| t.contains("web") || t.contains("browser"))
    {
        return "globe".to_string();
    }
    if lowered
        .iter()
        .any(|t| t.contains("doc") || t.contains("file"))
    {
        return "file".to_string();
    }
    if lowered
        .iter()
        .any(|t| t.contains("chart") || t.contains("table"))
    {
        return "table".to_string();
    }
    if skill_type == "flow" {
        return "flow".to_string();
    }
    "book".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordered_plugins_sorts_by_persisted_order() {
        let all = vec![
            PluginItem::builtin("a", "A", "x"),
            PluginItem::builtin("b", "B", "x"),
            PluginItem::builtin("c", "C", "x"),
        ];
        let ordered = ordered_plugins(&all, &["b".into(), "a".into()]);
        assert_eq!(ordered[0].id, "b");
        assert_eq!(ordered[1].id, "a");
        assert_eq!(ordered[2].id, "c");
    }

    #[test]
    fn ordered_plugins_appends_unknown_order_items() {
        let all = vec![
            PluginItem::builtin("x", "X", "x"),
            PluginItem::builtin("y", "Y", "x"),
        ];
        let ordered = ordered_plugins(&all, &[]);
        assert_eq!(ordered.len(), 2);
        assert_eq!(ordered[0].id, "x");
        assert_eq!(ordered[1].id, "y");
    }

    #[test]
    fn ordered_plugins_ignores_missing_ids() {
        let all = vec![PluginItem::builtin("only", "Only", "x")];
        let ordered = ordered_plugins(&all, &["missing".into(), "only".into()]);
        assert_eq!(ordered.len(), 1);
        assert_eq!(ordered[0].id, "only");
    }

    #[test]
    fn skill_icon_mapping_uses_tags() {
        assert_eq!(skill_icon_for(&["web-search".into()], "standard"), "globe");
        assert_eq!(skill_icon_for(&["doc".into()], "standard"), "file");
        assert_eq!(skill_icon_for(&["chart".into()], "standard"), "table");
        assert_eq!(skill_icon_for(&[], "flow"), "flow");
        assert_eq!(skill_icon_for(&[], "standard"), "book");
    }
}
