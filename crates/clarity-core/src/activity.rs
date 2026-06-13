//! Activity Logger —— 向 devbase 写入原始活动日志
//!
//! window/cli 的关键操作写入 devbase，供外部子代理/本地模型整理为知识库。
//! 只负责追加写入，不负责整理、摘要、向量化。

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs::{OpenOptions, create_dir_all};
use std::io::Write;
use std::path::PathBuf;

/// 入口类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActivityEntry {
    /// Window-based activity.
    Window(WindowActivity),
    /// Command-line activity.
    Cli(CliActivity),
}

/// Window 端活动
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowActivity {
    /// Timestamp of the record.
    pub timestamp: String,
    /// Type of activity.
    pub activity_type: String,
    /// Announcement topic.
    pub topic: String,
    /// Tools used during the activity.
    pub tools_used: Vec<String>,
    /// Files involved in the activity.
    pub files_involved: Vec<String>,
    /// Conclusion of the activity.
    pub conclusion: String,
}

/// Cli 端活动
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliActivity {
    /// Timestamp of the record.
    pub timestamp: String,
    /// Type of activity.
    pub activity_type: String,
    /// Task performed.
    pub task: String,
    /// Tools used during the activity.
    pub tools_used: Vec<String>,
    /// Files changed by the task.
    pub files_changed: Vec<String>,
    /// Outcome of the task.
    pub outcome: String,
}

/// 活动日志记录器
#[derive(Debug, Clone)]
pub struct ActivityLogger {
    devbase_path: PathBuf,
}

impl ActivityLogger {
    /// 创建日志记录器，devbase 默认在当前工作目录的 `.clarity/devbase/`
    pub fn new() -> Self {
        let devbase = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(".clarity")
            .join("devbase");
        Self {
            devbase_path: devbase,
        }
    }

    /// 指定 devbase 路径
    pub fn with_path(path: impl Into<PathBuf>) -> Self {
        Self {
            devbase_path: path.into(),
        }
    }

    /// 记录 Window 活动（追加到 window-YYYY-MM-DD.jsonl）
    pub fn log_window(&self, activity: WindowActivity) -> tokio::task::JoinHandle<()> {
        self.append_jsonl("window", &activity)
    }

    /// 记录 Cli 活动（追加到 cli-YYYY-MM-DD.jsonl）
    pub fn log_cli(&self, activity: CliActivity) -> tokio::task::JoinHandle<()> {
        self.append_jsonl("cli", &activity)
    }

    fn append_jsonl<T: Serialize>(&self, prefix: &str, entry: &T) -> tokio::task::JoinHandle<()> {
        let devbase = self.devbase_path.clone();
        let prefix = prefix.to_string();
        let line = match serde_json::to_string(entry) {
            Ok(l) => l,
            Err(_) => return tokio::spawn(async {}),
        };

        // Offload blocking file I/O to Tokio's blocking thread pool
        // so that async callers (Agent loop, Gateway handlers) are not stalled.
        tokio::task::spawn_blocking(move || {
            let _ = create_dir_all(&devbase);
            let date = Utc::now().format("%Y-%m-%d");
            let path = devbase.join(format!("{}-{}.jsonl", prefix, date));
            let mut file = match OpenOptions::new().create(true).append(true).open(&path) {
                Ok(f) => f,
                Err(_) => return,
            };
            let _ = writeln!(file, "{}", line);
        })
    }

    /// 获取 devbase 路径
    pub fn path(&self) -> &PathBuf {
        &self.devbase_path
    }
}

impl Default for ActivityLogger {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufRead;

    #[tokio::test]
    async fn test_log_window_activity() {
        let temp = tempfile::tempdir().unwrap();
        let logger = ActivityLogger::with_path(temp.path());

        let activity = WindowActivity {
            timestamp: Utc::now().to_rfc3339(),
            activity_type: "query".to_string(),
            topic: "Axum state extractor".to_string(),
            tools_used: vec!["search_docs".to_string()],
            files_involved: vec!["src/server.rs".to_string()],
            conclusion: "Use State<Arc<AppState>>".to_string(),
        };

        let handle = logger.log_window(activity);
        handle.await.unwrap();

        let date = Utc::now().format("%Y-%m-%d");
        let path = temp.path().join(format!("window-{}.jsonl", date));
        assert!(path.exists());

        let file = std::fs::File::open(&path).unwrap();
        let reader = std::io::BufReader::new(file);
        let lines: Vec<String> = reader.lines().map(|l| l.unwrap()).collect();
        assert_eq!(lines.len(), 1);

        let parsed: WindowActivity = serde_json::from_str(&lines[0]).unwrap();
        assert_eq!(parsed.topic, "Axum state extractor");
    }
}
