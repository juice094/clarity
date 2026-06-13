//! Personality configuration and template variable substitution.

use std::collections::HashMap;

/// Runtime-configurable personality parameters.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct PersonalityConfig {
    /// Key-value pairs injected into prompt templates at runtime.
    #[serde(default)]
    pub template_variables: HashMap<String, String>,
}

impl PersonalityConfig {
    /// Create a new `PersonalityConfig`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the template variables.
    pub fn with_template_variables(mut self, vars: HashMap<String, String>) -> Self {
        self.template_variables = vars;
        self
    }

    /// Fill `{{key}}` placeholders in `template` with values from `template_variables`.
    pub fn fill_variables(&self, template: &str) -> String {
        let mut result = template.to_string();
        for (key, value) in &self.template_variables {
            result = result.replace(&format!("{{{{{}}}}}", key), value);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fill_variables_empty() {
        let config = PersonalityConfig::default();
        assert_eq!(config.fill_variables("Hello, world!"), "Hello, world!");
    }

    #[test]
    fn test_fill_variables_basic() {
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), "Clarity".to_string());
        let config = PersonalityConfig::new().with_template_variables(vars);
        assert_eq!(config.fill_variables("Hello, {{name}}!"), "Hello, Clarity!");
    }

    #[test]
    fn test_fill_variables_override() {
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), "Custom".to_string());
        let config = PersonalityConfig::new().with_template_variables(vars);
        assert_eq!(
            config.fill_variables("{{name}} says {{name}}"),
            "Custom says Custom"
        );
    }
}
