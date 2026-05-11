//! Task Store - 任务存储管理
//!
//! 负责任务定义、状态和结果的持久化存储

use crate::background::cron::CronTask;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{debug, info};

/// 任务 ID
pub type TaskId = String;

/// 任务优先级
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord, Hash, Default,
)]
pub enum TaskPriority {
    /// 后台任务，最低优先级
    Background = 0,
    /// 低优先级
    Low = 1,
    /// 正常优先级（默认）
    #[default]
    Normal = 2,
    /// 高优先级
    High = 3,
    /// 关键任务，最高优先级
    Critical = 4,
}

impl TaskPriority {
    /// 获取优先级数值
    pub fn value(&self) -> u8 {
        *self as u8
    }

    /// 从数值创建优先级
    pub fn from_value(value: u8) -> Self {
        match value {
            0 => TaskPriority::Background,
            1 => TaskPriority::Low,
            2 => TaskPriority::Normal,
            3 => TaskPriority::High,
            4 => TaskPriority::Critical,
            _ => TaskPriority::Normal,
        }
    }
}

/// 任务状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl TaskStatus {
    /// 是否终止状态
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
        )
    }

    /// 转换为字符串
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskStatus::Pending => "pending",
            TaskStatus::Running => "running",
            TaskStatus::Completed => "completed",
            TaskStatus::Failed => "failed",
            TaskStatus::Cancelled => "cancelled",
        }
    }
}

/// 任务规格
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSpec {
    pub name: String,
    pub description: String,
    pub agent_type: String,
    pub prompt: String,
    pub max_iterations: Option<usize>,
    #[serde(alias = "timeout_secs")]
    pub timeout_seconds: Option<u64>,
    #[serde(default)]
    pub priority: TaskPriority,
    /// 模型别名覆盖（从 ModelRegistry 查找）
    #[serde(default)]
    pub model_alias: Option<String>,
}

impl TaskSpec {
    /// 创建新的任务规格
    pub fn new(name: impl Into<String>, prompt: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            agent_type: "default".to_string(),
            prompt: prompt.into(),
            max_iterations: None,
            timeout_seconds: None,
            priority: TaskPriority::Normal,
            model_alias: None,
        }
    }

    /// 设置描述
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// 设置 agent 类型
    pub fn with_agent_type(mut self, agent_type: impl Into<String>) -> Self {
        self.agent_type = agent_type.into();
        self
    }

    /// 设置最大迭代次数
    pub fn with_max_iterations(mut self, max: usize) -> Self {
        self.max_iterations = Some(max);
        self
    }

    /// 设置超时时间（秒）
    pub fn with_timeout_seconds(mut self, seconds: u64) -> Self {
        self.timeout_seconds = Some(seconds);
        self
    }

    /// 设置模型别名（从 ModelRegistry 动态选择 LLM）
    pub fn with_model_alias(mut self, alias: impl Into<String>) -> Self {
        self.model_alias = Some(alias.into());
        self
    }

    /// 设置优先级
    pub fn with_priority(mut self, priority: TaskPriority) -> Self {
        self.priority = priority;
        self
    }

    /// 获取超时时间（秒）
    pub fn timeout_secs(&self) -> Option<u64> {
        self.timeout_seconds
    }
}

/// 任务结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub status: TaskStatus,
    pub output: String,
    pub elapsed_ms: u64,
    pub steps: usize,
}

impl TaskResult {
    /// 创建成功结果
    pub fn success(output: impl Into<String>) -> Self {
        Self {
            status: TaskStatus::Completed,
            output: output.into(),
            elapsed_ms: 0,
            steps: 0,
        }
    }

    /// 创建失败结果
    pub fn failure(error: impl Into<String>) -> Self {
        Self {
            status: TaskStatus::Failed,
            output: error.into(),
            elapsed_ms: 0,
            steps: 0,
        }
    }

    /// 设置耗时
    pub fn with_elapsed_ms(mut self, ms: u64) -> Self {
        self.elapsed_ms = ms;
        self
    }

    /// 设置步数
    pub fn with_steps(mut self, steps: usize) -> Self {
        self.steps = steps;
        self
    }
}

/// 任务信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskInfo {
    pub id: TaskId,
    pub spec: TaskSpec,
    pub status: TaskStatus,
    pub created_at: u64,
    pub updated_at: u64,
}

/// 任务存储
#[derive(Debug, Clone)]
pub struct TaskStore {
    root_dir: PathBuf,
    cache: std::sync::Arc<tokio::sync::RwLock<HashMap<TaskId, TaskInfo>>>,
}

impl TaskStore {
    /// 创建新的任务存储
    pub fn new(root_dir: impl AsRef<Path>) -> Self {
        Self {
            root_dir: root_dir.as_ref().to_path_buf(),
            cache: std::sync::Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
    }

    /// 创建新任务
    pub async fn create(&self, task_id: impl AsRef<str>, spec: TaskSpec) -> anyhow::Result<()> {
        let task_id = task_id.as_ref();
        let now = now_timestamp();
        let info = TaskInfo {
            id: task_id.to_string(),
            spec,
            status: TaskStatus::Pending,
            created_at: now,
            updated_at: now,
        };

        // 写入文件
        let task_dir = self.task_dir(task_id);
        fs::create_dir_all(&task_dir).await?;

        let info_path = task_dir.join("info.json");
        let info_json = serde_json::to_string_pretty(&info)?;
        fs::write(&info_path, info_json).await?;

        // 更新缓存
        let mut cache = self.cache.write().await;
        cache.insert(task_id.to_string(), info);

        info!("Created task: {}", task_id);
        Ok(())
    }

    /// 获取任务信息
    pub async fn get(&self, task_id: impl AsRef<str>) -> anyhow::Result<TaskInfo> {
        let task_id = task_id.as_ref();
        // 先检查缓存
        {
            let cache = self.cache.read().await;
            if let Some(info) = cache.get(task_id) {
                return Ok(info.clone());
            }
        }

        // 从文件加载
        let info_path = self.task_dir(task_id).join("info.json");
        let info_json = fs::read_to_string(&info_path).await?;
        let info: TaskInfo = serde_json::from_str(&info_json)?;

        // 更新缓存
        let mut cache = self.cache.write().await;
        cache.insert(task_id.to_string(), info.clone());

        Ok(info)
    }

    /// 更新任务状态
    pub async fn update_status(
        &self,
        task_id: impl AsRef<str>,
        status: TaskStatus,
    ) -> anyhow::Result<()> {
        let task_id = task_id.as_ref();
        let mut info = self.get(task_id).await?;
        info.status = status;
        info.updated_at = now_timestamp();

        // 写入文件
        let info_path = self.task_dir(task_id).join("info.json");
        let info_json = serde_json::to_string_pretty(&info)?;
        fs::write(&info_path, info_json).await?;

        // 更新缓存
        let mut cache = self.cache.write().await;
        cache.insert(task_id.to_string(), info);

        debug!("Updated task {} status to {:?}", task_id, status);
        Ok(())
    }

    /// 保存任务结果
    pub async fn save_result(
        &self,
        task_id: impl AsRef<str>,
        result: &TaskResult,
    ) -> anyhow::Result<()> {
        let task_id = task_id.as_ref();
        let task_dir = self.task_dir(task_id);
        fs::create_dir_all(&task_dir).await?;

        let result_path = task_dir.join("result.json");
        let result_json = serde_json::to_string_pretty(result)?;
        fs::write(&result_path, result_json).await?;

        debug!("Saved result for task: {}", task_id);
        Ok(())
    }

    /// 获取任务结果
    pub async fn get_result(&self, task_id: impl AsRef<str>) -> anyhow::Result<TaskResult> {
        let task_id = task_id.as_ref();
        let result_path = self.task_dir(task_id).join("result.json");
        let result_json = fs::read_to_string(&result_path).await?;
        let result: TaskResult = serde_json::from_str(&result_json)?;
        Ok(result)
    }

    /// 获取任务结果（graceful：文件不存在时返回 None 而非 Error）
    pub async fn get_result_opt(
        &self,
        task_id: impl AsRef<str>,
    ) -> anyhow::Result<Option<TaskResult>> {
        let task_id = task_id.as_ref();
        let result_path = self.task_dir(task_id).join("result.json");
        if !result_path.exists() {
            return Ok(None);
        }
        let result_json = fs::read_to_string(&result_path).await?;
        let result: TaskResult = serde_json::from_str(&result_json)?;
        Ok(Some(result))
    }

    /// 列出所有任务
    pub async fn list_all(&self) -> anyhow::Result<Vec<TaskInfo>> {
        let mut tasks = Vec::new();

        // 如果目录不存在，返回空列表
        if !self.root_dir.exists() {
            return Ok(tasks);
        }

        // 读取目录
        let mut entries = fs::read_dir(&self.root_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_dir() {
                let task_id = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| s.to_string())
                    .unwrap_or_default();

                if let Ok(info) = self.get(&task_id).await {
                    tasks.push(info);
                }
            }
        }

        // 按创建时间排序
        tasks.sort_by_key(|b| std::cmp::Reverse(b.created_at));
        Ok(tasks)
    }

    /// 按状态列出任务
    pub async fn list_by_status(&self, status: TaskStatus) -> anyhow::Result<Vec<TaskInfo>> {
        let all = self.list_all().await?;
        Ok(all.into_iter().filter(|t| t.status == status).collect())
    }

    /// 按优先级列出待处理任务（高优先级在前）
    pub async fn list_pending_by_priority(&self) -> anyhow::Result<Vec<TaskInfo>> {
        let pending = self.list_by_status(TaskStatus::Pending).await?;
        let mut tasks: Vec<_> = pending.into_iter().collect();
        // 按优先级降序排列
        tasks.sort_by_key(|b| std::cmp::Reverse(b.spec.priority));
        Ok(tasks)
    }

    /// 删除任务
    pub async fn delete(&self, task_id: impl AsRef<str>) -> anyhow::Result<()> {
        let task_id = task_id.as_ref();
        let task_dir = self.task_dir(task_id);
        if task_dir.exists() {
            fs::remove_dir_all(&task_dir).await?;
        }

        // 更新缓存
        let mut cache = self.cache.write().await;
        cache.remove(task_id);

        info!("Deleted task: {}", task_id);
        Ok(())
    }

    /// 任务目录
    fn task_dir(&self, task_id: impl AsRef<str>) -> PathBuf {
        self.root_dir.join(task_id.as_ref())
    }

    /// Cron 任务存储文件路径
    fn cron_file(&self) -> PathBuf {
        self.root_dir.join("cron_tasks.json")
    }

    /// 保存 cron 任务
    pub async fn save_cron(&self, task: &CronTask) -> anyhow::Result<()> {
        let mut tasks = self.list_cron().await?;
        // 去重：如果存在相同 task_id，先移除旧条目
        tasks.retain(|t: &CronTask| t.task_id != task.task_id);
        tasks.push(task.clone());

        let json = serde_json::to_string_pretty(&tasks)?;
        fs::create_dir_all(&self.root_dir).await?;
        fs::write(self.cron_file(), json).await?;

        info!("Saved cron task: {}", task.task_id);
        Ok(())
    }

    /// 列出所有 cron 任务
    pub async fn list_cron(&self) -> anyhow::Result<Vec<CronTask>> {
        let path = self.cron_file();
        if !path.exists() {
            return Ok(Vec::new());
        }

        let json = fs::read_to_string(&path).await?;
        let tasks: Vec<CronTask> = serde_json::from_str(&json)?;
        Ok(tasks)
    }

    /// 删除 cron 任务
    pub async fn remove_cron(&self, task_id: impl AsRef<str>) -> anyhow::Result<()> {
        let task_id = task_id.as_ref();
        let mut tasks = self.list_cron().await?;
        let original_len = tasks.len();
        tasks.retain(|t| t.task_id != task_id);

        if tasks.len() == original_len {
            return Err(anyhow::anyhow!("Cron task not found: {}", task_id));
        }

        let json = serde_json::to_string_pretty(&tasks)?;
        fs::write(self.cron_file(), json).await?;

        info!("Removed cron task: {}", task_id);
        Ok(())
    }
}

/// 获取当前时间戳
fn now_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_spec(name: &str) -> TaskSpec {
        TaskSpec {
            name: name.to_string(),
            description: "test".to_string(),
            agent_type: "coder".to_string(),
            prompt: "test".to_string(),
            max_iterations: Some(10),
            timeout_seconds: Some(30),
            priority: TaskPriority::Normal,
            model_alias: None,
        }
    }

    #[tokio::test]
    async fn test_create_and_get() {
        let temp_dir = TempDir::new().unwrap();
        let store = TaskStore::new(temp_dir.path());

        let spec = create_test_spec("test_task");
        store.create("task_1", spec.clone()).await.unwrap();

        let info = store.get("task_1").await.unwrap();
        assert_eq!(info.id, "task_1");
        assert_eq!(info.spec.name, "test_task");
        assert_eq!(info.status, TaskStatus::Pending);
    }

    #[tokio::test]
    async fn test_update_status() {
        let temp_dir = TempDir::new().unwrap();
        let store = TaskStore::new(temp_dir.path());

        let spec = create_test_spec("test_task");
        store.create("task_1", spec).await.unwrap();

        store
            .update_status("task_1", TaskStatus::Running)
            .await
            .unwrap();

        let info = store.get("task_1").await.unwrap();
        assert_eq!(info.status, TaskStatus::Running);
    }

    #[tokio::test]
    async fn test_save_and_get_result() {
        let temp_dir = TempDir::new().unwrap();
        let store = TaskStore::new(temp_dir.path());

        let spec = create_test_spec("test_task");
        store.create("task_1", spec).await.unwrap();

        let result = TaskResult {
            status: TaskStatus::Completed,
            output: "success".to_string(),
            elapsed_ms: 1000,
            steps: 5,
        };
        store.save_result("task_1", &result).await.unwrap();

        let loaded = store.get_result("task_1").await.unwrap();
        assert_eq!(loaded.status, TaskStatus::Completed);
        assert_eq!(loaded.output, "success");
    }

    #[tokio::test]
    async fn test_list_all() {
        let temp_dir = TempDir::new().unwrap();
        let store = TaskStore::new(temp_dir.path());

        store
            .create("task_1", create_test_spec("task_1"))
            .await
            .unwrap();
        store
            .create("task_2", create_test_spec("task_2"))
            .await
            .unwrap();
        store
            .create("task_3", create_test_spec("task_3"))
            .await
            .unwrap();

        let list = store.list_all().await.unwrap();
        assert_eq!(list.len(), 3);
    }

    #[tokio::test]
    async fn test_list_by_status() {
        let temp_dir = TempDir::new().unwrap();
        let store = TaskStore::new(temp_dir.path());

        store
            .create("task_1", create_test_spec("task_1"))
            .await
            .unwrap();
        store
            .create("task_2", create_test_spec("task_2"))
            .await
            .unwrap();
        store
            .update_status("task_2", TaskStatus::Completed)
            .await
            .unwrap();

        let completed = store.list_by_status(TaskStatus::Completed).await.unwrap();
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].id, "task_2");
    }

    #[tokio::test]
    async fn test_delete() {
        let temp_dir = TempDir::new().unwrap();
        let store = TaskStore::new(temp_dir.path());

        store
            .create("task_1", create_test_spec("task_1"))
            .await
            .unwrap();
        assert!(store.get("task_1").await.is_ok());

        store.delete("task_1").await.unwrap();
        assert!(store.get("task_1").await.is_err());
    }

    #[test]
    fn test_task_priority_ordering() {
        assert!(TaskPriority::Critical > TaskPriority::High);
        assert!(TaskPriority::High > TaskPriority::Normal);
        assert!(TaskPriority::Normal > TaskPriority::Low);
        assert!(TaskPriority::Low > TaskPriority::Background);
    }

    #[test]
    fn test_task_priority_from_value() {
        assert_eq!(TaskPriority::from_value(0), TaskPriority::Background);
        assert_eq!(TaskPriority::from_value(1), TaskPriority::Low);
        assert_eq!(TaskPriority::from_value(2), TaskPriority::Normal);
        assert_eq!(TaskPriority::from_value(3), TaskPriority::High);
        assert_eq!(TaskPriority::from_value(4), TaskPriority::Critical);
        assert_eq!(TaskPriority::from_value(99), TaskPriority::Normal);
    }

    #[test]
    fn test_task_spec_builder() {
        let spec = TaskSpec::new("test", "prompt")
            .with_description("desc")
            .with_agent_type("coder")
            .with_max_iterations(10)
            .with_timeout_seconds(60)
            .with_priority(TaskPriority::High);

        assert_eq!(spec.name, "test");
        assert_eq!(spec.description, "desc");
        assert_eq!(spec.agent_type, "coder");
        assert_eq!(spec.max_iterations, Some(10));
        assert_eq!(spec.timeout_seconds, Some(60));
        assert_eq!(spec.priority, TaskPriority::High);
    }

    #[test]
    fn test_task_result_builder() {
        let result = TaskResult::success("output")
            .with_elapsed_ms(1000)
            .with_steps(5);

        assert_eq!(result.status, TaskStatus::Completed);
        assert_eq!(result.output, "output");
        assert_eq!(result.elapsed_ms, 1000);
        assert_eq!(result.steps, 5);
    }

    #[tokio::test]
    async fn test_list_pending_by_priority() {
        let temp_dir = TempDir::new().unwrap();
        let store = TaskStore::new(temp_dir.path());

        let low_priority = TaskSpec::new("low", "test").with_priority(TaskPriority::Low);
        let high_priority = TaskSpec::new("high", "test").with_priority(TaskPriority::High);
        let critical_priority =
            TaskSpec::new("critical", "test").with_priority(TaskPriority::Critical);

        store.create("task_low", low_priority).await.unwrap();
        store.create("task_high", high_priority).await.unwrap();
        store
            .create("task_critical", critical_priority)
            .await
            .unwrap();

        let pending = store.list_pending_by_priority().await.unwrap();
        assert_eq!(pending.len(), 3);
        assert_eq!(pending[0].spec.name, "critical");
        assert_eq!(pending[1].spec.name, "high");
        assert_eq!(pending[2].spec.name, "low");
    }

    #[tokio::test]
    async fn test_save_and_list_cron() {
        let temp_dir = TempDir::new().unwrap();
        let store = TaskStore::new(temp_dir.path());

        let spec = TaskSpec::new("daily", "backup");
        let task = CronTask {
            task_id: "cron_001".to_string(),
            task_spec: spec,
            schedule: crate::background::cron::CronSchedule::new("0 0 2 * * *").unwrap(),
            enabled: true,
        };

        store.save_cron(&task).await.unwrap();

        let list = store.list_cron().await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].task_id, "cron_001");
        assert!(list[0].enabled);
    }

    #[tokio::test]
    async fn test_remove_cron() {
        let temp_dir = TempDir::new().unwrap();
        let store = TaskStore::new(temp_dir.path());

        let spec = TaskSpec::new("daily", "backup");
        let task = CronTask {
            task_id: "cron_002".to_string(),
            task_spec: spec,
            schedule: crate::background::cron::CronSchedule::new("0 0 2 * * *").unwrap(),
            enabled: true,
        };

        store.save_cron(&task).await.unwrap();
        assert_eq!(store.list_cron().await.unwrap().len(), 1);

        store.remove_cron("cron_002").await.unwrap();
        assert!(store.list_cron().await.unwrap().is_empty());

        // Removing non-existent should fail
        assert!(store.remove_cron("nonexistent").await.is_err());
    }

    #[tokio::test]
    async fn test_save_cron_updates_existing() {
        let temp_dir = TempDir::new().unwrap();
        let store = TaskStore::new(temp_dir.path());

        let spec1 = TaskSpec::new("daily", "backup");
        let mut task1 = CronTask {
            task_id: "cron_003".to_string(),
            task_spec: spec1,
            schedule: crate::background::cron::CronSchedule::new("0 0 2 * * *").unwrap(),
            enabled: true,
        };

        store.save_cron(&task1).await.unwrap();

        // Update the same task_id
        task1.enabled = false;
        store.save_cron(&task1).await.unwrap();

        let list = store.list_cron().await.unwrap();
        assert_eq!(list.len(), 1);
        assert!(!list[0].enabled);
    }
}
