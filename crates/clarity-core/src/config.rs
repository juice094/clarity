//! Configuration management for Clarity
//!
//! Supports loading from:
//! 1. Environment variables (highest priority)
//! 2. Project directory `.clarity.toml`
//! 3. User config directory `~/.config/clarity/config.toml` (lowest priority)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::Path;
use tracing::{debug, info};

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Default profile name to use when none is specified
    #[serde(default = "default_profile_name")]
    pub default_profile: String,
    /// Map of profile names to their configurations
    #[serde(default)]
    pub profiles: HashMap<String, Profile>,
}

fn default_profile_name() -> String {
    "default".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_profile: default_profile_name(),
            profiles: HashMap::new(),
        }
    }
}

/// Profile configuration for a specific LLM provider
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Profile {
    /// Model name (e.g., "gpt-4", "kimi-latest")
    pub model: String,
    /// Provider name (e.g., "openai", "kimi", "ollama")
    pub provider: String,
    /// API key for authentication
    pub api_key: Option<String>,
    /// Base URL for API requests (optional, for custom endpoints)
    pub base_url: Option<String>,
}

impl Config {
    /// Load configuration from all sources
    ///
    /// Loading order (later sources override earlier ones):
    /// 1. User config directory (`~/.config/clarity/config.toml`)
    /// 2. Project directory (`.clarity.toml`)
    /// 3. Environment variables (`CLARITY_*`)
    ///
    /// # Errors
    ///
    /// Returns an error if a config file exists but cannot be parsed.
    pub fn load() -> anyhow::Result<Self> {
        let mut config = Config::default();

        // 1. Load from user config directory (lowest priority)
        if let Some(user_config) = Self::load_user_config()? {
            debug!("Loaded user config");
            config.merge(user_config);
        }

        // 2. Load from project directory
        if let Some(project_config) = Self::load_project_config()? {
            debug!("Loaded project config");
            config.merge(project_config);
        }

        // 3. Apply environment variables (highest priority)
        debug!("Applying environment variables");
        config.apply_env_vars();

        info!("Config loaded with {} profiles", config.profiles.len());

        Ok(config)
    }

    /// Load configuration from user config directory
    ///
    /// Looks for `config.toml` in the user's config directory
    /// (e.g., `~/.config/clarity/config.toml` on Linux/macOS)
    fn load_user_config() -> anyhow::Result<Option<Config>> {
        let config_dir = dirs::config_dir()
            .map(|p| p.join("clarity"))
            .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?;

        let config_path = config_dir.join("config.toml");

        if !config_path.exists() {
            debug!("User config file not found: {:?}", config_path);
            return Ok(None);
        }

        info!("Loading user config from: {:?}", config_path);
        let contents = fs::read_to_string(&config_path)?;
        let config: Config = toml::from_str(&contents)?;
        Ok(Some(config))
    }

    /// Load configuration from project directory
    ///
    /// Looks for `.clarity.toml` in the current working directory
    fn load_project_config() -> anyhow::Result<Option<Config>> {
        let config_path = Path::new(".clarity.toml");

        if !config_path.exists() {
            debug!("Project config file not found: {:?}", config_path);
            return Ok(None);
        }

        info!("Loading project config from: {:?}", config_path);
        let contents = fs::read_to_string(config_path)?;
        let config: Config = toml::from_str(&contents)?;
        Ok(Some(config))
    }

    /// Apply environment variable overrides
    ///
    /// Supported environment variables:
    /// - `CLARITY_DEFAULT_PROFILE` - Set default profile name
    /// - `CLARITY_API_KEY` - Set API key for default profile
    /// - `CLARITY_MODEL` - Set model for default profile
    /// - `CLARITY_PROVIDER` - Set provider for default profile
    /// - `CLARITY_BASE_URL` - Set base URL for default profile
    fn apply_env_vars(&mut self) {
        // Override default profile
        if let Ok(profile) = env::var("CLARITY_DEFAULT_PROFILE") {
            info!("Overriding default profile from env: {}", profile);
            self.default_profile = profile;
        }

        // Ensure default profile exists
        let default_profile = self.default_profile.clone();
        let profile = self.profiles.entry(default_profile).or_default();

        // Apply environment variable overrides to default profile
        if let Ok(api_key) = env::var("CLARITY_API_KEY") {
            debug!("Setting API key from environment variable");
            profile.api_key = Some(api_key);
        }

        if let Ok(model) = env::var("CLARITY_MODEL") {
            debug!("Setting model from environment variable: {}", model);
            profile.model = model;
        }

        if let Ok(provider) = env::var("CLARITY_PROVIDER") {
            debug!("Setting provider from environment variable: {}", provider);
            profile.provider = provider;
        }

        if let Ok(base_url) = env::var("CLARITY_BASE_URL") {
            debug!("Setting base URL from environment variable: {}", base_url);
            profile.base_url = Some(base_url);
        }
    }

    /// Merge another config into this one
    ///
    /// Later values override earlier ones for the same keys
    fn merge(&mut self, other: Config) {
        if !other.default_profile.is_empty() {
            self.default_profile = other.default_profile;
        }

        for (name, profile) in other.profiles {
            self.profiles.insert(name, profile);
        }
    }

    /// Get a profile by name, or the default profile if name is None
    ///
    /// # Errors
    ///
    /// Returns an error if the profile does not exist
    pub fn get_profile(&self, name: Option<&str>) -> anyhow::Result<&Profile> {
        let name = name.unwrap_or(&self.default_profile);
        self.profiles
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("Profile '{}' not found", name))
    }

    /// Get the default profile
    ///
    /// # Errors
    ///
    /// Returns an error if the default profile does not exist
    pub fn default_profile(&self) -> anyhow::Result<&Profile> {
        self.get_profile(None)
    }

    /// Export profile settings to provider-specific environment variables.
    ///
    /// This allows `LlmFactory::auto()` to pick up TOML-configured credentials
    /// without modifying the factory itself. Only sets variables that are **not**
    /// already present in the environment, preserving the "env var wins" priority.
    pub fn export_to_env(&self) {
        let profile = match self.default_profile() {
            Ok(p) => p,
            Err(_) => return,
        };

        if let Some(ref api_key) = profile.api_key {
            let provider = profile.provider.to_lowercase();
            match provider.as_str() {
                "anthropic" | "claude" => {
                    if env::var("ANTHROPIC_AUTH_TOKEN").is_err() {
                        env::set_var("ANTHROPIC_AUTH_TOKEN", api_key);
                        info!("Exported ANTHROPIC_AUTH_TOKEN from config profile");
                    }
                }
                "kimi" | "kimi-code" | "moonshot" => {
                    if env::var("KIMI_API_KEY").is_err() && env::var("KIMI_CODE_API_KEY").is_err() {
                        env::set_var("KIMI_API_KEY", api_key);
                        info!("Exported KIMI_API_KEY from config profile");
                    }
                }
                "deepseek" => {
                    if env::var("DEEPSEEK_API_KEY").is_err() {
                        env::set_var("DEEPSEEK_API_KEY", api_key);
                        info!("Exported DEEPSEEK_API_KEY from config profile");
                    }
                }
                "openai" => {
                    if env::var("OPENAI_API_KEY").is_err() {
                        env::set_var("OPENAI_API_KEY", api_key);
                        info!("Exported OPENAI_API_KEY from config profile");
                    }
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.default_profile, "default");
        assert!(config.profiles.is_empty());
    }

    #[test]
    fn test_profile_default() {
        let profile = Profile::default();
        assert!(profile.model.is_empty());
        assert!(profile.provider.is_empty());
        assert!(profile.api_key.is_none());
        assert!(profile.base_url.is_none());
    }

    #[test]
    fn test_config_merge() {
        let mut config1 = Config {
            default_profile: "profile1".to_string(),
            profiles: {
                let mut map = HashMap::new();
                map.insert(
                    "profile1".to_string(),
                    Profile {
                        model: "model1".to_string(),
                        provider: "provider1".to_string(),
                        api_key: Some("key1".to_string()),
                        base_url: None,
                    },
                );
                map
            },
        };

        let config2 = Config {
            default_profile: "profile2".to_string(),
            profiles: {
                let mut map = HashMap::new();
                map.insert(
                    "profile2".to_string(),
                    Profile {
                        model: "model2".to_string(),
                        provider: "provider2".to_string(),
                        api_key: Some("key2".to_string()),
                        base_url: Some("http://example.com".to_string()),
                    },
                );
                map
            },
        };

        config1.merge(config2);

        assert_eq!(config1.default_profile, "profile2");
        assert_eq!(config1.profiles.len(), 2);
        assert!(config1.profiles.contains_key("profile1"));
        assert!(config1.profiles.contains_key("profile2"));
    }

    #[test]
    fn test_config_from_toml() {
        let toml_str = r#"
default_profile = "production"

[profiles.default]
model = "gpt-4"
provider = "openai"
api_key = "sk-test"

[profiles.production]
model = "gpt-4-turbo"
provider = "openai"
api_key = "sk-prod"
base_url = "https://api.openai.com/v1"
"#;

        let config: Config = toml::from_str(toml_str).unwrap();

        assert_eq!(config.default_profile, "production");
        assert_eq!(config.profiles.len(), 2);

        let default = config.profiles.get("default").unwrap();
        assert_eq!(default.model, "gpt-4");
        assert_eq!(default.provider, "openai");
        assert_eq!(default.api_key, Some("sk-test".to_string()));
        assert_eq!(default.base_url, None);

        let production = config.profiles.get("production").unwrap();
        assert_eq!(production.model, "gpt-4-turbo");
        assert_eq!(
            production.base_url,
            Some("https://api.openai.com/v1".to_string())
        );
    }

    #[test]
    fn test_get_profile() {
        let mut config = Config::default();
        config.profiles.insert(
            "test".to_string(),
            Profile {
                model: "gpt-4".to_string(),
                provider: "openai".to_string(),
                api_key: None,
                base_url: None,
            },
        );

        let profile = config.get_profile(Some("test")).unwrap();
        assert_eq!(profile.model, "gpt-4");

        // Test non-existent profile
        assert!(config.get_profile(Some("nonexistent")).is_err());
    }

    #[test]
    fn test_apply_env_vars() {
        // Save original values
        let original_default = env::var("CLARITY_DEFAULT_PROFILE").ok();
        let original_api_key = env::var("CLARITY_API_KEY").ok();
        let original_model = env::var("CLARITY_MODEL").ok();
        let original_provider = env::var("CLARITY_PROVIDER").ok();
        let original_base_url = env::var("CLARITY_BASE_URL").ok();

        // Set test values
        env::set_var("CLARITY_DEFAULT_PROFILE", "env_profile");
        env::set_var("CLARITY_API_KEY", "env_api_key");
        env::set_var("CLARITY_MODEL", "env_model");
        env::set_var("CLARITY_PROVIDER", "env_provider");
        env::set_var("CLARITY_BASE_URL", "http://env.example.com");

        let mut config = Config::default();
        config.apply_env_vars();

        assert_eq!(config.default_profile, "env_profile");

        let profile = config.default_profile().unwrap();
        assert_eq!(profile.api_key, Some("env_api_key".to_string()));
        assert_eq!(profile.model, "env_model");
        assert_eq!(profile.provider, "env_provider");
        assert_eq!(profile.base_url, Some("http://env.example.com".to_string()));

        // Restore original values
        match original_default {
            Some(v) => env::set_var("CLARITY_DEFAULT_PROFILE", v),
            None => env::remove_var("CLARITY_DEFAULT_PROFILE"),
        }
        match original_api_key {
            Some(v) => env::set_var("CLARITY_API_KEY", v),
            None => env::remove_var("CLARITY_API_KEY"),
        }
        match original_model {
            Some(v) => env::set_var("CLARITY_MODEL", v),
            None => env::remove_var("CLARITY_MODEL"),
        }
        match original_provider {
            Some(v) => env::set_var("CLARITY_PROVIDER", v),
            None => env::remove_var("CLARITY_PROVIDER"),
        }
        match original_base_url {
            Some(v) => env::set_var("CLARITY_BASE_URL", v),
            None => env::remove_var("CLARITY_BASE_URL"),
        }
    }
}
