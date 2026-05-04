use async_trait::async_trait;
use chrono::{DateTime, Datelike, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;

/// A single record of tool execution, capturing environment context and outcome.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolExecutionMemory {
    pub timestamp: DateTime<Utc>,
    pub tool_name: String,
    pub tool_args: serde_json::Value,
    /// Environment context — working directory at the time of execution.
    pub working_dir: PathBuf,
    /// Active shell identifier.
    pub shell: String,
    /// Host OS information.
    pub os_info: String,
    /// Execution outcome.
    pub outcome: Outcome,
    /// Captured standard output.
    pub stdout: Option<String>,
    /// Captured standard error.
    pub stderr: Option<String>,
    /// Process exit code, if applicable.
    pub exit_code: Option<i32>,
    /// Classified error category.
    pub error_category: ErrorCategory,
}

/// The result of a tool execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Outcome {
    Success,
    Failure { retry_count: u8 },
    Partial { warning: String },
}

/// Taxonomy of tool execution failures for pattern learning.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ErrorCategory {
    NotFound,
    PermissionDenied,
    EncodingError,
    Timeout,
    EnvironmentMismatch,
    Unknown,
}

/// A recurring fragility pattern observed for a specific tool / category.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FragilityPattern {
    /// Tool name or generic pattern identifier.
    pub pattern: String,
    pub category: ErrorCategory,
    /// How many times this pattern has been observed.
    pub frequency: u32,
}

/// Environment-wide cognition generated from historical error memory.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EnvironmentCognition {
    pub known_fragilities: Vec<FragilityPattern>,
    /// Tool name → reliability score in [0.0, 1.0].
    pub tool_reliability: HashMap<String, f32>,
}

/// Abstract storage for `ToolExecutionMemory` records.
#[async_trait]
pub trait ErrorMemoryStore: Send + Sync {
    /// Persist a single execution memory.
    async fn save(&self, memory: &ToolExecutionMemory) -> anyhow::Result<()>;
    /// Retrieve the most recent `limit` memories, ordered from oldest to newest.
    async fn query_recent(&self, limit: usize) -> anyhow::Result<Vec<ToolExecutionMemory>>;
    /// Derive an `EnvironmentCognition` from all stored records.
    async fn load_cognition(&self) -> anyhow::Result<EnvironmentCognition>;
}

/// Filesystem-backed `ErrorMemoryStore` using JSON Lines under
/// `~/.clarity/error-memory/YYYY-MM/memory.jsonl`.
pub struct FileSystemErrorMemoryStore {
    base_dir: PathBuf,
}

impl FileSystemErrorMemoryStore {
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    /// Create a store using the default user-local path.
    pub fn with_default_dir() -> anyhow::Result<Self> {
        let home = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Failed to get home directory"))?;
        Ok(Self::new(home.join(".clarity").join("error-memory")))
    }

    fn month_file_path(&self, timestamp: DateTime<Utc>) -> PathBuf {
        let month = format!("{:04}-{:02}", timestamp.year(), timestamp.month());
        self.base_dir.join(&month).join("memory.jsonl")
    }

    fn all_month_files(&self) -> anyhow::Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        if !self.base_dir.exists() {
            return Ok(files);
        }
        for month_entry in std::fs::read_dir(&self.base_dir)? {
            let month_entry = month_entry?;
            let month_path = month_entry.path();
            if month_path.is_dir() {
                for entry in std::fs::read_dir(&month_path)? {
                    let entry = entry?;
                    let path = entry.path();
                    if path.extension().map(|e| e == "jsonl").unwrap_or(false) {
                        files.push(path);
                    }
                }
            }
        }
        files.sort();
        Ok(files)
    }
}

#[async_trait]
impl ErrorMemoryStore for FileSystemErrorMemoryStore {
    async fn save(&self, memory: &ToolExecutionMemory) -> anyhow::Result<()> {
        let path = self.month_file_path(memory.timestamp);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let line = serde_json::to_string(memory)?;
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await?;
        file.write_all(line.as_bytes()).await?;
        file.write_all(b"\n").await?;
        Ok(())
    }

    async fn query_recent(&self, limit: usize) -> anyhow::Result<Vec<ToolExecutionMemory>> {
        let files = self.all_month_files()?;
        let mut results = Vec::new();
        // Iterate newest month first.
        for path in files.iter().rev() {
            let content = tokio::fs::read_to_string(path).await?;
            // Iterate newest line first.
            for line in content.lines().rev() {
                if line.trim().is_empty() {
                    continue;
                }
                let mem: ToolExecutionMemory = serde_json::from_str(line)?;
                results.push(mem);
                if results.len() >= limit {
                    break;
                }
            }
            if results.len() >= limit {
                break;
            }
        }
        // Restore chronological order.
        results.reverse();
        Ok(results)
    }

    async fn load_cognition(&self) -> anyhow::Result<EnvironmentCognition> {
        let files = self.all_month_files()?;
        // (success_count, total_count)
        let mut stats_by_tool: HashMap<String, (u32, u32)> = HashMap::new();
        let mut fragility_counts: HashMap<(ErrorCategory, String), u32> = HashMap::new();

        for path in files {
            let content = tokio::fs::read_to_string(path).await?;
            for line in content.lines() {
                if line.trim().is_empty() {
                    continue;
                }
                let mem: ToolExecutionMemory = serde_json::from_str(line)?;
                let entry = stats_by_tool
                    .entry(mem.tool_name.clone())
                    .or_insert((0, 0));
                entry.1 += 1;
                match &mem.outcome {
                    Outcome::Success => entry.0 += 1,
                    Outcome::Failure { .. } | Outcome::Partial { .. } => {
                        let key = (mem.error_category.clone(), mem.tool_name.clone());
                        *fragility_counts.entry(key).or_insert(0) += 1;
                    }
                }
            }
        }

        let tool_reliability: HashMap<String, f32> = stats_by_tool
            .iter()
            .map(|(tool, (success, total))| {
                let reliability = if *total > 0 {
                    *success as f32 / *total as f32
                } else {
                    1.0
                };
                (tool.clone(), reliability)
            })
            .collect();

        let mut known_fragilities: Vec<FragilityPattern> = fragility_counts
            .into_iter()
            .map(|((category, tool), count)| FragilityPattern {
                pattern: tool,
                category,
                frequency: count,
            })
            .collect();
        known_fragilities.sort_by(|a, b| b.frequency.cmp(&a.frequency));

        Ok(EnvironmentCognition {
            known_fragilities,
            tool_reliability,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use tempfile::tempdir;

    fn sample_memory(
        tool_name: &str,
        outcome: Outcome,
        category: ErrorCategory,
    ) -> ToolExecutionMemory {
        ToolExecutionMemory {
            timestamp: Utc.with_ymd_and_hms(2026, 5, 4, 1, 0, 0).unwrap(),
            tool_name: tool_name.to_string(),
            tool_args: serde_json::json!({}),
            working_dir: PathBuf::from("/tmp"),
            shell: "powershell".to_string(),
            os_info: "windows".to_string(),
            outcome,
            stdout: None,
            stderr: None,
            exit_code: None,
            error_category: category,
        }
    }

    #[tokio::test]
    async fn test_save_and_load_memory() {
        let dir = tempdir().unwrap();
        let store = FileSystemErrorMemoryStore::new(dir.path().to_path_buf());
        let mem = sample_memory("test_tool", Outcome::Success, ErrorCategory::Unknown);
        store.save(&mem).await.unwrap();

        let recent = store.query_recent(10).await.unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0], mem);
    }

    #[tokio::test]
    async fn test_query_recent() {
        let dir = tempdir().unwrap();
        let store = FileSystemErrorMemoryStore::new(dir.path().to_path_buf());
        for i in 0..5 {
            let mut mem = sample_memory("tool", Outcome::Success, ErrorCategory::Unknown);
            mem.timestamp = Utc.with_ymd_and_hms(2026, 5, 4, 1, 0, i).unwrap();
            store.save(&mem).await.unwrap();
        }

        let recent = store.query_recent(3).await.unwrap();
        assert_eq!(recent.len(), 3);
        // Should be the 3 latest entries, returned in chronological order.
        assert_eq!(
            recent[0].timestamp,
            Utc.with_ymd_and_hms(2026, 5, 4, 1, 0, 2).unwrap()
        );
        assert_eq!(
            recent[2].timestamp,
            Utc.with_ymd_and_hms(2026, 5, 4, 1, 0, 4).unwrap()
        );
    }

    #[tokio::test]
    async fn test_error_category_classification() {
        let dir = tempdir().unwrap();
        let store = FileSystemErrorMemoryStore::new(dir.path().to_path_buf());
        let mem1 = sample_memory(
            "cat_tool",
            Outcome::Failure { retry_count: 1 },
            ErrorCategory::NotFound,
        );
        let mem2 = sample_memory(
            "cat_tool",
            Outcome::Failure { retry_count: 2 },
            ErrorCategory::PermissionDenied,
        );
        store.save(&mem1).await.unwrap();
        store.save(&mem2).await.unwrap();

        let cognition = store.load_cognition().await.unwrap();
        assert_eq!(cognition.known_fragilities.len(), 2);

        let not_found = cognition
            .known_fragilities
            .iter()
            .find(|f| f.category == ErrorCategory::NotFound)
            .expect("NotFound fragility should exist");
        assert_eq!(not_found.frequency, 1);

        let perm = cognition
            .known_fragilities
            .iter()
            .find(|f| f.category == ErrorCategory::PermissionDenied)
            .expect("PermissionDenied fragility should exist");
        assert_eq!(perm.frequency, 1);

        let reliability = cognition
            .tool_reliability
            .get("cat_tool")
            .expect("cat_tool reliability should be present");
        assert_eq!(*reliability, 0.0);
    }
}
