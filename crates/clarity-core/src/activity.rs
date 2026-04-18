//! Activity Logger —— 向 devbase 写入原始活动日志
//!
//! window/cli 的关键操作写入 devbase，供外部子代理/本地模型整理为知识库。
//! 只负责追加写入，不负责整理、摘要、向量化。

use std::fs::{create_dir_all, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use chrono::Utc;
use serde::{Deserialize, Serialize};

/// 入口类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActivityEntry {
    Window(WindowActivity),
    Cli(CliActivity),
}

/// Window 端活动
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowActivity {
    pub timestamp: String,
    pub activity_type: String,
    pub topic: String,
    pub tools_used: Vec<String>,
    pub files_involved: Vec<String>,
    pub conclusion: String,
}

/// Cli 端活动
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliActivity {
    pub timestamp: String,
    pub activity_type: String,
    pub task: String,
    pub tools_used: Vec<String>,
    pub files_changed: Vec<String>,
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
        Self { devbase_path: devbase }
    }

    /// 指定 devbase 路径
    pub fn with_path(path: impl Into<PathBuf>) -> Self {
        Self { devbase_path: path.into() }
    }

    /// 记录 Window 活动（追加到 window-YYYY-MM-DD.jsonl）
    pub fn log_window(&self, activity: WindowActivity) {
        let _ = self.append_jsonl("window", &activity);
    }

    /// 记录 Cli 活动（追加到 cli-YYYY-MM-DD.jsonl）
    pub fn log_cli(&self, activity: CliActivity) {
        let _ = self.append_jsonl("cli", &activity);
    }

    fn append_jsonl<T: Serialize>(&self, prefix: &str, entry: &T) -> Result<(), String> {
        create_dir_all(&self.devbase_path).map_err(|e| e.to_string())?;

        let date = Utc::now().format("%Y-%m-%d");
        let path = self.devbase_path.join(format!("{}-{}.jsonl", prefix, date));

        let line = serde_json::to_string(entry).map_err(|e| e.to_string())?;

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| e.to_string())?;

        writeln!(file, "{}", line).map_err(|e| e.to_string())?;
        Ok(())
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

    #[test]
    fn test_log_window_activity() {
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

        logger.log_window(activity);

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
