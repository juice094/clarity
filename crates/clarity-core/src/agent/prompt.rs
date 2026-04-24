//! System prompt construction and tool description filtering.

use super::config::load_prompt_from_file;
use super::Agent;
use serde_json::Value;

impl Agent {
    /// Build the system prompt
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
        // Cache the result to avoid repeated disk I/O on every agent turn.
        let file_prompt = if self.config.prompts_dir.is_some() {
            let cached = self.file_prompt_cache();
            cached.or_else(|| {
                let prompt_path = self
                    .config
                    .prompts_dir
                    .as_ref()
                    .unwrap()
                    .join(format!("{}.md", entry));
                let loaded = load_prompt_from_file(&prompt_path);
                self.set_file_prompt_cache(loaded.clone());
                loaded
            })
        } else {
            None
        };

        let base = if let Some(prompt) = file_prompt {
            if tool_descs.is_empty() {
                prompt
            } else {
                format!(
                    "{}\n\n## Available Tools\n{}",
                    prompt,
                    tool_descs.join("\n")
                )
            }
        } else {
            if tool_descs.is_empty() {
                self.config.system_prompt.clone()
            } else {
                format!(
                    "{}\n\n## Available Tools\n{}",
                    self.config.system_prompt,
                    tool_descs.join("\n")
                )
            }
        };

        let with_entry = if self.config.entry_context.is_empty() {
            base
        } else {
            format!("{}\n\n{}", base, self.config.entry_context)
        };

        let mut skill_contexts = Vec::new();

        // Inject snapshotted active skill context if set
        if let Some(ref skill_id) = self.snapshotted_active_skill() {
            if let Some(ref registry) = self.skill_registry {
                if let Some(skill) = registry.get(skill_id) {
                    skill_contexts.push(skill.build_context());
                }
            }
        }

        // Inject dynamically activated skill contexts
        if let Some(ref registry) = self.skill_registry {
            for id in registry.active_ids() {
                if Some(&id) != self.snapshotted_active_skill().as_ref() {
                    if let Some(skill) = registry.get(&id) {
                        skill_contexts.push(skill.build_context());
                    }
                }
            }
        }

        if !skill_contexts.is_empty() {
            format!("{}\n\n{}", with_entry, skill_contexts.join("\n\n"))
        } else {
            with_entry
        }
    }

    /// Get tool descriptions from the registry for the system prompt.
    fn get_tool_descriptions(&self) -> Vec<String> {
        // Convert tool schemas to descriptions
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
        let registry = self.skill_registry.as_ref()?;
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
