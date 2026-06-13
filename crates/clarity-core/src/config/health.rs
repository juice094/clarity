//! Configuration health checks and validation snapshots.
//!
//! `ConfigHealth` captures which sources contributed to the active configuration,
//! validates required fields, flags hardcoded secrets, and records rollback points.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use super::Config;
use super::audit::{ConfigAudit, ConfigChangeType, hash_content};

/// A source layer that contributed to the loaded configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ConfigSource {
    /// Built-in defaults.
    Default,
    /// User config directory file.
    UserFile(PathBuf),
    /// Project directory file.
    ProjectFile(PathBuf),
    /// Environment variable override.
    EnvVar(String),
}

impl std::fmt::Display for ConfigSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigSource::Default => write!(f, "default"),
            ConfigSource::UserFile(p) => write!(f, "user:{}", p.display()),
            ConfigSource::ProjectFile(p) => write!(f, "project:{}", p.display()),
            ConfigSource::EnvVar(k) => write!(f, "env:{}", k),
        }
    }
}

/// A single configuration health issue.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConfigHealthIssue {
    /// A required field is missing.
    MissingField {
        /// Dot-separated path to the field (e.g. `profiles.default.model`).
        path: String,
        /// Source layer where the field was expected.
        source: String,
    },
    /// Two sources provided conflicting values for the same key.
    SourceConflict {
        /// Dot-separated path to the conflicting key.
        path: String,
        /// Source names that provided different values.
        sources: Vec<String>,
    },
    /// A sensitive value appears to be hardcoded instead of injected via env.
    HardcodedSecret {
        /// Dot-separated path to the secret field.
        path: String,
    },
    /// A referenced profile does not exist.
    UnknownProfile {
        /// Profile name that was referenced but not found.
        name: String,
    },
}

/// A rollback point captured during config loading.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConfigRollbackPoint {
    /// Source string (file path or env var key).
    pub source: String,
    /// Command that would revert the change, if known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rollback_command: Option<String>,
}

/// Snapshot of configuration health at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ConfigHealth {
    healthy: bool,
    layers: Vec<(ConfigSource, bool)>,
    issues: Vec<ConfigHealthIssue>,
    rollback_points: Vec<ConfigRollbackPoint>,
}

impl ConfigHealth {
    /// Create a new, empty health snapshot.
    pub fn new() -> Self {
        Self {
            healthy: true,
            layers: Vec::new(),
            issues: Vec::new(),
            rollback_points: Vec::new(),
        }
    }

    /// Record a source layer and whether it contributed active values.
    pub fn record_layer(&mut self, source: ConfigSource, active: bool) {
        self.layers.push((source, active));
    }

    /// Record a rollback point for a source layer.
    pub fn record_rollback(&mut self, source: impl Into<String>, command: Option<String>) {
        self.rollback_points.push(ConfigRollbackPoint {
            source: source.into(),
            rollback_command: command,
        });
    }

    /// Add an issue and mark the snapshot as unhealthy.
    pub fn add_issue(&mut self, issue: ConfigHealthIssue) {
        self.healthy = false;
        self.issues.push(issue);
    }

    /// Check that a required string field is present and non-empty.
    pub fn check_required(&mut self, path: &str, value: Option<&str>, source: &ConfigSource) {
        if value.map(str::trim).map(str::is_empty).unwrap_or(true) {
            self.add_issue(ConfigHealthIssue::MissingField {
                path: path.to_string(),
                source: source.to_string(),
            });
        }
    }

    /// Check that a secret value uses `${env:...}` injection.
    ///
    /// If `value` is present and is not an env-reference, it is flagged as
    /// hardcoded. Empty secrets are ignored (handled by `check_required`).
    pub fn check_env_injected(&mut self, path: &str, value: Option<&str>) {
        if let Some(v) = value {
            if !v.trim().is_empty() && !v.starts_with("${env:") {
                self.add_issue(ConfigHealthIssue::HardcodedSecret {
                    path: path.to_string(),
                });
            }
        }
    }

    /// Record a source conflict when two layers define the same key differently.
    pub fn record_conflict(&mut self, path: &str, sources: &[ConfigSource]) {
        if sources.len() < 2 {
            return;
        }
        self.add_issue(ConfigHealthIssue::SourceConflict {
            path: path.to_string(),
            sources: sources.iter().map(ConfigSource::to_string).collect(),
        });
    }

    /// Returns `true` if no health issues were found.
    pub fn is_healthy(&self) -> bool {
        self.healthy && self.issues.is_empty()
    }

    /// Access the recorded source layers.
    pub fn layers(&self) -> &[(ConfigSource, bool)] {
        &self.layers
    }

    /// Access the discovered issues.
    pub fn issues(&self) -> &[ConfigHealthIssue] {
        &self.issues
    }

    /// Access the rollback points.
    pub fn rollback_points(&self) -> &[ConfigRollbackPoint] {
        &self.rollback_points
    }

    /// Serialize the health snapshot to a JSON string.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails.
    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(self)
    }
}

impl Config {
    /// Load configuration and produce a `ConfigHealth` snapshot.
    ///
    /// This mirrors [`Config::load`](super::Config::load) but records every
    /// source layer, validates required fields, and flags hardcoded secrets.
    ///
    /// # Errors
    ///
    /// Returns an error if a config file exists but cannot be parsed.
    pub fn load_with_health() -> anyhow::Result<(Self, ConfigHealth)> {
        let mut health = ConfigHealth::new();
        let mut config = Config::default();
        let mut default_profile_sources: Vec<ConfigSource> = Vec::new();

        // 1. Built-in defaults.
        health.record_layer(ConfigSource::Default, true);
        default_profile_sources.push(ConfigSource::Default);

        // 2. User config directory.
        let user_path = dirs::config_dir()
            .map(|p| p.join("clarity").join("config.toml"))
            .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?;
        let user_active = Self::load_file(
            &user_path,
            ConfigSource::UserFile(user_path.clone()),
            &mut config,
            &mut health,
        )?;
        if user_active {
            default_profile_sources.push(ConfigSource::UserFile(user_path.clone()));
            health.record_rollback(
                user_path.display().to_string(),
                Some(format!("rm {}", user_path.display())),
            );
        }

        // 3. Project directory.
        let project_path = PathBuf::from(".clarity.toml");
        let project_active = Self::load_file(
            &project_path,
            ConfigSource::ProjectFile(project_path.clone()),
            &mut config,
            &mut health,
        )?;
        if project_active {
            default_profile_sources.push(ConfigSource::ProjectFile(project_path.clone()));
            health.record_rollback(
                project_path.display().to_string(),
                Some(format!("rm {}", project_path.display())),
            );
        }

        // 4. Environment variables.
        config.apply_env_vars();
        for key in [
            "CLARITY_DEFAULT_PROFILE",
            "CLARITY_API_KEY",
            "CLARITY_MODEL",
            "CLARITY_PROVIDER",
            "CLARITY_BASE_URL",
        ] {
            if std::env::var(key).is_ok() {
                health.record_layer(ConfigSource::EnvVar(key.to_string()), true);
            }
        }
        if std::env::var("CLARITY_DEFAULT_PROFILE").is_ok() {
            default_profile_sources
                .push(ConfigSource::EnvVar("CLARITY_DEFAULT_PROFILE".to_string()));
        }

        // 5. Validate required fields.
        health.check_required(
            "default_profile",
            Some(&config.default_profile),
            default_profile_sources
                .last()
                .unwrap_or(&ConfigSource::Default),
        );

        if !config.profiles.contains_key(&config.default_profile) {
            health.add_issue(ConfigHealthIssue::UnknownProfile {
                name: config.default_profile.clone(),
            });
        }

        let default = config
            .profiles
            .entry(config.default_profile.clone())
            .or_default();
        health.check_required(
            "profiles.default.model",
            Some(&default.model),
            &ConfigSource::Default,
        );
        health.check_required(
            "profiles.default.provider",
            Some(&default.provider),
            &ConfigSource::Default,
        );
        health.check_env_injected("profiles.default.api_key", default.api_key.as_deref());

        // 6. Audit snapshot.
        if let Ok(serialized) = toml::to_string(&config) {
            let hash = hash_content(serialized.as_bytes());
            let mut audit = ConfigAudit::new();
            audit.record_change(
                "~/.clarity/config.toml",
                ConfigChangeType::Update,
                "config health snapshot",
                None,
                Some(hash),
            );
            let _ = audit.flush();
        }

        // 7. Conflict detection: default_profile defined by multiple layers.
        if default_profile_sources.len() > 1 {
            health.record_conflict("default_profile", &default_profile_sources);
        }

        Ok((config, health))
    }

    /// Helper to load a single config file and update the health snapshot.
    fn load_file(
        path: &Path,
        source: ConfigSource,
        config: &mut Config,
        health: &mut ConfigHealth,
    ) -> anyhow::Result<bool> {
        if !path.exists() {
            return Ok(false);
        }

        let contents = std::fs::read_to_string(path)?;
        let parsed: Config = toml::from_str(&contents)?;
        config.merge(parsed);
        health.record_layer(source, true);
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_default_config_is_healthy() {
        let config = Config::default();
        let mut health = ConfigHealth::new();
        health.record_layer(ConfigSource::Default, true);
        health.check_required(
            "default_profile",
            Some(&config.default_profile),
            &ConfigSource::Default,
        );

        assert!(health.is_healthy());
        assert!(health.issues().is_empty());
    }

    #[test]
    fn test_missing_field_is_unhealthy() {
        let mut health = ConfigHealth::new();
        health.check_required("profiles.default.model", None, &ConfigSource::Default);
        assert!(!health.is_healthy());
        assert_eq!(health.issues().len(), 1);
        assert!(matches!(
            health.issues()[0],
            ConfigHealthIssue::MissingField { .. }
        ));
    }

    #[test]
    fn test_hardcoded_secret_is_unhealthy() {
        let mut health = ConfigHealth::new();
        health.check_env_injected("profiles.default.api_key", Some("sk-hardcoded"));
        assert!(!health.is_healthy());
        assert!(matches!(
            health.issues()[0],
            ConfigHealthIssue::HardcodedSecret { .. }
        ));
    }

    #[test]
    fn test_env_injected_secret_is_healthy() {
        let mut health = ConfigHealth::new();
        health.check_env_injected("profiles.default.api_key", Some("${env:OPENAI_API_KEY}"));
        assert!(health.is_healthy());
    }

    #[test]
    fn test_empty_secret_is_ignored() {
        let mut health = ConfigHealth::new();
        health.check_env_injected("profiles.default.api_key", Some("   "));
        assert!(health.is_healthy());
    }

    #[test]
    fn test_source_conflict_recorded() {
        let mut health = ConfigHealth::new();
        health.record_conflict(
            "default_profile",
            &[
                ConfigSource::UserFile(PathBuf::from("user.toml")),
                ConfigSource::EnvVar("CLARITY_DEFAULT_PROFILE".to_string()),
            ],
        );
        assert!(!health.is_healthy());
        assert!(matches!(
            health.issues()[0],
            ConfigHealthIssue::SourceConflict { .. }
        ));
    }

    #[test]
    fn test_rollback_points_recorded() {
        let mut health = ConfigHealth::new();
        health.record_rollback("user.toml", Some("cp user.toml.bak user.toml".to_string()));
        assert_eq!(health.rollback_points().len(), 1);
        assert_eq!(health.rollback_points()[0].source, "user.toml");
    }

    #[test]
    fn test_config_health_json_roundtrip() {
        let mut health = ConfigHealth::new();
        health.record_layer(ConfigSource::Default, true);
        health.check_env_injected("profiles.default.api_key", Some("sk-123"));
        let json = health.to_json().unwrap();
        let parsed: ConfigHealth = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, health);
    }

    #[test]
    fn test_unknown_profile_is_unhealthy() {
        let config = Config {
            default_profile: "missing".to_string(),
            profiles: HashMap::new(),
        };
        let mut health = ConfigHealth::new();
        health.record_layer(ConfigSource::Default, true);
        if !config.profiles.contains_key(&config.default_profile) {
            health.add_issue(ConfigHealthIssue::UnknownProfile {
                name: config.default_profile.clone(),
            });
        }
        assert!(!health.is_healthy());
    }
}
