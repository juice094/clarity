//! System prompt construction and tool description filtering.

use super::config::load_prompt_from_file;
use super::Agent;
use crate::approval::ApprovalMode;
use serde_json::Value;
use std::collections::HashMap;
/// Component that can be conditionally injected into the system prompt.
#[derive(Debug, Clone)]
pub enum PromptComponent {
    /// Static text block.
    Text(String),
    /// Tool descriptions section.
    Tools(Vec<String>),
    /// Entry-specific context (methodology, persona, etc.).
    EntryContext(String),
    /// Skill context blocks.
    Skills(Vec<String>),
    /// Approval-mode behavioural notice.
    ApprovalNotice(ApprovalMode),
    /// Offline-mode notice.
    #[allow(dead_code)]
    OfflineNotice,
}

/// Builder for assembling the system prompt from conditional components.
///
/// Replaces the previous monolithic `build_system_prompt()` implementation
/// with a declarative, testable pipeline.
#[derive(Debug, Clone, Default)]
pub struct SystemPromptBuilder {
    components: Vec<PromptComponent>,
    template_variables: HashMap<String, String>,
}

impl SystemPromptBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_base(mut self, text: impl Into<String>) -> Self {
        self.components.push(PromptComponent::Text(text.into()));
        self
    }

    pub fn with_tools(mut self, tools: Vec<String>) -> Self {
        if !tools.is_empty() {
            self.components.push(PromptComponent::Tools(tools));
        }
        self
    }

    pub fn with_entry_context(mut self, ctx: impl Into<String>) -> Self {
        let ctx = ctx.into();
        if !ctx.is_empty() {
            self.components.push(PromptComponent::EntryContext(ctx));
        }
        self
    }

    pub fn with_skills(mut self, skills: Vec<String>) -> Self {
        if !skills.is_empty() {
            self.components.push(PromptComponent::Skills(skills));
        }
        self
    }

    pub fn with_approval_mode(mut self, mode: ApprovalMode) -> Self {
        self.components.push(PromptComponent::ApprovalNotice(mode));
        self
    }

    #[allow(dead_code)]
    pub fn with_offline_notice(mut self) -> Self {
        self.components.push(PromptComponent::OfflineNotice);
        self
    }

    pub fn with_template_vars(mut self, vars: HashMap<String, String>) -> Self {
        self.template_variables = vars;
        self
    }

    /// Assemble the final prompt string.
    pub fn build(&self) -> String {
        let mut sections: Vec<String> = Vec::new();

        for comp in &self.components {
            match comp {
                PromptComponent::Text(text) => sections.push(text.clone()),
                PromptComponent::Tools(tools) => {
                    sections.push(format!("## Available Tools\n{}", tools.join("\n")));
                }
                PromptComponent::EntryContext(ctx) => sections.push(ctx.clone()),
                PromptComponent::Skills(skills) => {
                    sections.push(skills.join("\n\n"));
                }
                PromptComponent::ApprovalNotice(mode) => {
                    let notice = match mode {
                        ApprovalMode::Yolo => {
                            "You are running in YOLO mode. You may execute tools automatically without asking for confirmation, but you must still log sensitive operations."
                        }
                        ApprovalMode::Interactive => {
                            "You are running in Interactive mode. Before executing any tool that modifies files, accesses sensitive data, or controls the desktop, you must wait for explicit user approval."
                        }
                        ApprovalMode::Plan => {
                            "You are running in Plan mode. Follow the pre-generated plan step-by-step and do not deviate unless the user explicitly requests a change."
                        }
                    };
                    sections.push(format!("## Approval Mode\n{}", notice));
                }
                PromptComponent::OfflineNotice => {
                    sections.push(
                        "## Network Status\nYou are currently offline. Only local tools are available."
                            .to_string(),
                    );
                }
            }
        }

        let mut result = sections.join("\n\n");

        // Apply runtime template variable substitution.
        for (key, value) in &self.template_variables {
            result = result.replace(&format!("{{{}}}", key), value);
        }

        result
    }
}

impl Agent {
    /// Build the system prompt using the declarative builder.
    pub fn build_system_prompt(&self) -> String {
        let tool_descs = self.get_tool_descriptions();

        // Determine entry type from entry_context
        let entry = if self.config.entry_context.contains("方法论")
            || self.config.entry_context.contains("科学")
        {
            "window"
        } else if self.config.entry_context.contains("工程") {
            "cli"
        } else {
            "claw"
        };

        // Try to load prompt from file if prompts_dir is set.
        let file_prompt = if self.config.prompts_dir.is_some() {
            let cached = self.file_prompt_cache();
            cached.or_else(|| {
                let prompt_path = self
                    .config
                    .prompts_dir
                    .as_ref()
                    // SAFE: guarded by is_some() check on line 149.
                    .unwrap()
                    .join(format!("{}.md", entry));
                let loaded = load_prompt_from_file(&prompt_path);
                self.set_file_prompt_cache(loaded.clone());
                loaded
            })
        } else {
            None
        };

        let base = file_prompt.unwrap_or_else(|| self.config.system_prompt.clone());

        // Collect skill contexts
        let mut skill_contexts = Vec::new();
        if let Some(ref skill_id) = self.snapshotted_active_skill() {
            if let Some(ref registry) = self.skill_registry() {
                if let Some(skill) = registry.get(skill_id) {
                    skill_contexts.push(skill.build_context());
                }
            }
        }
        if let Some(ref registry) = self.skill_registry() {
            for id in registry.active_ids() {
                if Some(&id) != self.snapshotted_active_skill().as_ref() {
                    if let Some(skill) = registry.get(&id) {
                        skill_contexts.push(skill.build_context());
                    }
                }
            }
        }

        SystemPromptBuilder::new()
            .with_base(base)
            .with_tools(tool_descs)
            .with_entry_context(self.config.entry_context.clone())
            .with_skills(skill_contexts)
            .with_approval_mode(self.approval_mode())
            .with_template_vars(self.config.template_variables.clone())
            .build()
    }

    /// Get tool descriptions from the registry for the system prompt.
    fn get_tool_descriptions(&self) -> Vec<String> {
        match self.registry.get_tool_schemas() {
            Ok(schemas) => {
                let allowed = self.active_skill_tool_whitelist();
                schemas
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|f| {
                                let func = f.get("function")?;
                                let name = func.get("name")?.as_str()?;
                                if let Some(ref whitelist) = allowed {
                                    if !whitelist.iter().any(|w| w == name) {
                                        return None;
                                    }
                                }
                                let description = func.get("description")?.as_str()?;
                                Some(format!("- {}: {}", name, description))
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            }
            Err(_) => vec![],
        }
    }

    /// Return the tool whitelist for the active skill, if any.
    fn active_skill_tool_whitelist(&self) -> Option<Vec<String>> {
        let active = self.snapshotted_active_skill()?;
        let registry = self.skill_registry()?;
        let skill = registry.get(&active)?;
        if skill.meta.tools.is_empty() {
            None
        } else {
            Some(skill.meta.tools.clone())
        }
    }

    /// Filter a tools JSON value to only include tools in the active skill whitelist.
    pub(crate) fn filter_tools_value(&self, tools: &Value) -> Value {
        let allowed = match self.active_skill_tool_whitelist() {
            Some(w) => w,
            None => return tools.clone(),
        };
        let allowed_set: std::collections::HashSet<String> = allowed.into_iter().collect();
        match tools.as_array() {
            Some(arr) => {
                let filtered: Vec<Value> = arr
                    .iter()
                    .filter(|v| {
                        v.get("function")
                            .and_then(|f| f.get("name"))
                            .and_then(|n| n.as_str())
                            .map(|name| allowed_set.contains(name))
                            .unwrap_or(false)
                    })
                    .cloned()
                    .collect();
                Value::Array(filtered)
            }
            None => tools.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_basic() {
        let prompt = SystemPromptBuilder::new()
            .with_base("You are helpful.")
            .with_tools(vec!["- read: Read a file".to_string()])
            .build();
        assert!(prompt.contains("You are helpful."));
        assert!(prompt.contains("Available Tools"));
        assert!(prompt.contains("- read"));
    }

    #[test]
    fn test_builder_approval_mode_yolo() {
        let prompt = SystemPromptBuilder::new()
            .with_base("Base.")
            .with_approval_mode(ApprovalMode::Yolo)
            .build();
        assert!(prompt.contains("YOLO mode"));
    }

    #[test]
    fn test_builder_approval_mode_interactive() {
        let prompt = SystemPromptBuilder::new()
            .with_base("Base.")
            .with_approval_mode(ApprovalMode::Interactive)
            .build();
        assert!(prompt.contains("Interactive mode"));
    }

    #[test]
    fn test_builder_offline_notice() {
        let prompt = SystemPromptBuilder::new()
            .with_base("Base.")
            .with_offline_notice()
            .build();
        assert!(prompt.contains("offline"));
    }

    #[test]
    fn test_builder_template_vars() {
        let mut vars = HashMap::new();
        vars.insert("agent_name".to_string(), "Clarity".to_string());
        let prompt = SystemPromptBuilder::new()
            .with_base("Hello, {agent_name}!")
            .with_template_vars(vars)
            .build();
        assert_eq!(prompt, "Hello, Clarity!");
    }

    #[test]
    fn test_builder_skips_empty_tools() {
        let prompt = SystemPromptBuilder::new()
            .with_base("Base.")
            .with_tools(vec![])
            .build();
        assert!(!prompt.contains("Available Tools"));
    }

    #[test]
    fn test_builder_skips_empty_entry_context() {
        let prompt = SystemPromptBuilder::new()
            .with_base("Base.")
            .with_entry_context("")
            .build();
        assert_eq!(prompt, "Base.");
    }
}
