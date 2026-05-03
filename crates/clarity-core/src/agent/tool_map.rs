//! KimiCLI-style tool name → Clarity built-in tool name mapping.

use tracing::warn;

/// Map a KimiCLI-style tool reference to a Clarity built-in tool name.
///
/// Returns `None` when the tool has no Clarity equivalent (e.g. mode-switch
/// pseudo-tools or not-yet-supported tools).
pub fn map_tool_name(kimi_tool: &str) -> Option<&'static str> {
    match kimi_tool {
        "kimi_cli.tools.shell:Shell" => {
            if cfg!(target_os = "windows") {
                Some("powershell")
            } else {
                Some("bash")
            }
        }
        "kimi_cli.tools.file:ReadFile" => Some("file_read"),
        "kimi_cli.tools.file:Glob" => Some("glob"),
        "kimi_cli.tools.file:Grep" => Some("grep"),
        "kimi_cli.tools.file:WriteFile" => Some("file_write"),
        "kimi_cli.tools.file:StrReplaceFile" => Some("file_edit"),
        "kimi_cli.tools.web:SearchWeb" => Some("web_search"),
        "kimi_cli.tools.web:FetchURL" => Some("web_fetch"),
        "kimi_cli.tools.ask_user:AskUserQuestion" => Some("ask_user"),
        "kimi_cli.tools.todo:SetTodoList" => Some("todo"),
        "kimi_cli.tools.background:TaskList" => Some("task_list"),
        "kimi_cli.tools.background:TaskOutput" => Some("task_output"),
        "kimi_cli.tools.background:TaskStop" => Some("task_stop"),
        // Mode-switch pseudo-tools — not real tools
        "kimi_cli.tools.plan:ExitPlanMode" => None,
        "kimi_cli.tools.plan.enter:EnterPlanMode" => None,
        // Agent self-reference — not a tool
        "kimi_cli.tools.agent:Agent" => None,
        // Not yet supported
        "kimi_cli.tools.file:ReadMediaFile" => None,
        _ => None,
    }
}

/// Build a new `ToolRegistry` containing only the tools that appear in
/// `whitelist` and have a valid Clarity mapping.
///
/// Unmapped or missing tools are logged at `warn` level and skipped
/// (fail-open).
pub fn filter_registry(
    base: &crate::registry::ToolRegistry,
    whitelist: &[String],
) -> Result<crate::registry::ToolRegistry, crate::error::AgentError> {
    let filtered = crate::registry::ToolRegistry::new();

    for kimi_name in whitelist {
        let clarity_name = match map_tool_name(kimi_name) {
            Some(name) => name,
            None => {
                warn!(
                    "KimiCLI tool '{}' has no Clarity equivalent; skipping.",
                    kimi_name
                );
                continue;
            }
        };

        let tool = match base.get(clarity_name)? {
            Some(t) => t,
            None => {
                warn!(
                    "Mapped tool '{}' (from '{}') not found in base registry; skipping.",
                    clarity_name, kimi_name
                );
                continue;
            }
        };

        filtered.register_shared(tool)?;
    }

    Ok(filtered)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_tool_name_known_tools() {
        assert_eq!(
            map_tool_name("kimi_cli.tools.file:ReadFile"),
            Some("file_read")
        );
        assert_eq!(
            map_tool_name("kimi_cli.tools.file:WriteFile"),
            Some("file_write")
        );
        assert_eq!(
            map_tool_name("kimi_cli.tools.file:StrReplaceFile"),
            Some("file_edit")
        );
        assert_eq!(map_tool_name("kimi_cli.tools.file:Glob"), Some("glob"));
        assert_eq!(map_tool_name("kimi_cli.tools.file:Grep"), Some("grep"));
        assert_eq!(
            map_tool_name("kimi_cli.tools.web:SearchWeb"),
            Some("web_search")
        );
        assert_eq!(
            map_tool_name("kimi_cli.tools.web:FetchURL"),
            Some("web_fetch")
        );
        assert_eq!(
            map_tool_name("kimi_cli.tools.ask_user:AskUserQuestion"),
            Some("ask_user")
        );
        assert_eq!(
            map_tool_name("kimi_cli.tools.todo:SetTodoList"),
            Some("todo")
        );
        assert_eq!(
            map_tool_name("kimi_cli.tools.background:TaskList"),
            Some("task_list")
        );
        assert_eq!(
            map_tool_name("kimi_cli.tools.background:TaskOutput"),
            Some("task_output")
        );
        assert_eq!(
            map_tool_name("kimi_cli.tools.background:TaskStop"),
            Some("task_stop")
        );
    }

    #[test]
    fn test_map_tool_name_shell() {
        let result = map_tool_name("kimi_cli.tools.shell:Shell");
        if cfg!(target_os = "windows") {
            assert_eq!(result, Some("powershell"));
        } else {
            assert_eq!(result, Some("bash"));
        }
    }

    #[test]
    fn test_map_tool_name_unknown_and_pseudo() {
        assert_eq!(map_tool_name("kimi_cli.tools.plan:ExitPlanMode"), None);
        assert_eq!(
            map_tool_name("kimi_cli.tools.plan.enter:EnterPlanMode"),
            None
        );
        assert_eq!(map_tool_name("kimi_cli.tools.agent:Agent"), None);
        assert_eq!(
            map_tool_name("kimi_cli.tools.file:ReadMediaFile"),
            None
        );
        assert_eq!(map_tool_name("unknown_tool"), None);
        assert_eq!(map_tool_name(""), None);
    }

    #[test]
    fn test_filter_registry() {
        let base = crate::registry::ToolRegistry::with_builtin_tools();
        let whitelist = vec![
            "kimi_cli.tools.file:ReadFile".to_string(),
            "kimi_cli.tools.file:WriteFile".to_string(),
            "kimi_cli.tools.web:SearchWeb".to_string(),
            // Pseudo-tool that should be skipped
            "kimi_cli.tools.plan:ExitPlanMode".to_string(),
            // Unknown tool that should be skipped
            "kimi_cli.tools.unknown:UnknownTool".to_string(),
        ];

        let filtered = filter_registry(&base, &whitelist).unwrap();

        let tools = filtered.list_tools().unwrap();
        assert!(tools.contains(&"file_read".to_string()));
        assert!(tools.contains(&"file_write".to_string()));
        assert!(tools.contains(&"web_search".to_string()));
        assert!(!tools.contains(&"glob".to_string()));
        assert!(!tools.contains(&"grep".to_string()));
        assert!(!tools.contains(&"powershell".to_string()));
        assert!(!tools.contains(&"bash".to_string()));
        assert_eq!(tools.len(), 3);
    }
}
