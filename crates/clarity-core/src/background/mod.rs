//! Background Task Management System
//!
//! 提供后台任务管理能力，支持：
//! - 任务持久化和恢复
//! - 任务状态机管理
//! - 并发控制
//! - 优先级调度
//! - 工作线程池
//! - 通知集成

pub mod agent_executor;
pub(crate) mod cron;
pub mod store;
pub(crate) mod worker;

pub(crate) use cron::CronScheduler;
pub use store::{TaskId, TaskInfo, TaskPriority, TaskResult, TaskSpec, TaskStatus, TaskStore};


use async_trait::async_trait;

/// Executor trait for running real Agent tasks in the background.
///
/// Implementations receive a [`TaskSpec`] and must build/run an [`crate::agent::Agent`],
/// returning the textual output and the number of steps taken.
#[async_trait]
pub trait AgentTaskExecutor: Send + Sync + std::fmt::Debug {
    async fn execute(&self, spec: &TaskSpec) -> anyhow::Result<(String, usize)>;
}

use crate::notifications::{task_status_notification, NotificationManager};
use std::collections::BinaryHeap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tracing::{error, info};

// Re-export chrono DateTime/Utc for cron consumers
pub use chrono::{DateTime, Utc};

/// 调度任务项（用于优先级队列）
#[derive(Debug, Clone)]
struct ScheduledTask {
    /// 优先级数值（越高越优先）
    priority_value: u8,
    /// 创建时间戳（用于相同优先级时的 FIFO）
    created_at: u64,
    /// 任务 ID
    task_id: TaskId,
    /// 任务规格
    spec: TaskSpec,
}

impl PartialEq for ScheduledTask {
    fn eq(&self, other: &Self) -> bool {
        self.priority_value == other.priority_value && self.created_at == other.created_at
    }
}

impl Eq for ScheduledTask {}

impl PartialOrd for ScheduledTask {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ScheduledTask {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // 首先按优先级降序（高优先级在前）
        self.priority_value
            .cmp(&other.priority_value)
            .then_with(|| {
                // 相同优先级时按创建时间升序（先创建的先执行）
                self.created_at.cmp(&other.created_at)
            })
    }
}

/// 任务调度器
///
/// 使用优先级队列管理待执行任务
#[derive(Debug)]
pub(crate) struct TaskScheduler {
    /// 优先级队列
    queue: Mutex<BinaryHeap<ScheduledTask>>,
    /// 任务存储
    store: TaskStore,
    /// 序列号生成器（用于 FIFO 排序）
    sequence: RwLock<u64>,
}

#[allow(dead_code)]
impl TaskScheduler {
    /// 创建新的任务调度器
    pub fn new(store: TaskStore) -> Self {
        Self {
            queue: Mutex::new(BinaryHeap::new()),
            store,
            sequence: RwLock::new(0),
        }
    }

    /// 调度任务
    pub async fn schedule(&self, task_id: TaskId, spec: TaskSpec) -> anyhow::Result<()> {
        // 保存任务到存储
        self.store.create(&task_id, spec.clone()).await?;

        // 生成序列号
        let seq = {
            let mut seq = self.sequence.write().await;
            *seq += 1;
            *seq
        };

        // 添加到优先级队列
        let scheduled = ScheduledTask {
            priority_value: spec.priority.value(),
            created_at: seq,
            task_id: task_id.clone(),
            spec: spec.clone(),
        };

        let mut queue = self.queue.lock().await;
        queue.push(scheduled);

        info!(
            "Scheduled task {} with priority {:?}",
            task_id, spec.priority
        );
        Ok(())
    }

    /// 获取下一个任务
    pub async fn next_task(&self) -> Option<(TaskId, TaskSpec)> {
        let mut queue = self.queue.lock().await;
        queue.pop().map(|t| (t.task_id, t.spec))
    }

    /// 查看下一个任务（不弹出）
    pub async fn peek_task(&self) -> Option<(TaskId, TaskSpec)> {
        let queue = self.queue.lock().await;
        queue.peek().map(|t| (t.task_id.clone(), t.spec.clone()))
    }

    /// 获取队列长度
    pub async fn queue_len(&self) -> usize {
        let queue = self.queue.lock().await;
        queue.len()
    }

    /// 检查队列是否为空
    pub async fn is_empty(&self) -> bool {
        let queue = self.queue.lock().await;
        queue.is_empty()
    }

    /// 获取存储引用
    pub fn store(&self) -> &TaskStore {
        &self.store
    }

    /// 从存储加载所有待处理任务
    pub async fn load_pending(&self) -> anyhow::Result<usize> {
        let pending = self.store.list_by_status(TaskStatus::Pending).await?;
        let count = pending.len();

        // 生成序列号
        let seq = {
            let mut seq = self.sequence.write().await;
            *seq += 1;
            *seq
        };

        let mut queue = self.queue.lock().await;
        for info in pending {
            let scheduled = ScheduledTask {
                priority_value: info.spec.priority.value(),
                created_at: seq,
                task_id: info.id,
                spec: info.spec,
            };
            queue.push(scheduled);
        }

        info!("Loaded {} pending tasks into scheduler", count);
        Ok(count)
    }
}

/// 后台任务管理器
///
/// 负责任务的创建、调度、监控和生命周期管理
#[derive(Debug, Clone)]
pub struct BackgroundTaskManager {
    /// 任务存储
    store: TaskStore,
    /// 工作目录
    #[allow(dead_code)]
    work_dir: PathBuf,
    /// 上下文目录
    #[allow(dead_code)]
    context_dir: PathBuf,
    /// 运行中的任务
    running_tasks: Arc<RwLock<std::collections::HashMap<TaskId, TaskHandle>>>,
    /// 最大并发数
    max_concurrency: usize,
    /// 信号量
    semaphore: Arc<tokio::sync::Semaphore>,
    /// 通知管理器
    notification_manager: Option<Arc<NotificationManager>>,
    /// 任务调度器
    scheduler: Option<Arc<TaskScheduler>>,
    /// Agent 任务执行器
    agent_executor: Option<Arc<dyn AgentTaskExecutor>>,
    /// Cron 任务调度器
    cron_scheduler: Option<Arc<tokio::sync::Mutex<CronScheduler>>>,
}

#[allow(dead_code)]
impl BackgroundTaskManager {
    /// 创建新的后台任务管理器
    pub fn new(
        store_dir: impl AsRef<Path>,
        work_dir: impl AsRef<Path>,
        context_dir: impl AsRef<Path>,
    ) -> Self {
        let max_concurrency = 4;
        Self {
            store: TaskStore::new(store_dir),
            work_dir: work_dir.as_ref().to_path_buf(),
            context_dir: context_dir.as_ref().to_path_buf(),
            running_tasks: Arc::new(RwLock::new(std::collections::HashMap::new())),
            max_concurrency,
            semaphore: Arc::new(tokio::sync::Semaphore::new(max_concurrency)),
            notification_manager: None,
            scheduler: None,
            agent_executor: None,
            cron_scheduler: None,
        }
    }

    /// 设置最大并发数
    pub fn with_max_concurrency(mut self, max: usize) -> Self {
        self.max_concurrency = max;
        self.semaphore = Arc::new(tokio::sync::Semaphore::new(max));
        self
    }

    /// 设置通知管理器
    pub fn with_notifications(mut self, manager: Arc<NotificationManager>) -> Self {
        self.notification_manager = Some(manager);
        self
    }

    /// 设置调度器
    pub(crate) fn with_scheduler(mut self, scheduler: Arc<TaskScheduler>) -> Self {
        self.scheduler = Some(scheduler);
        self
    }

    /// 设置 Agent 任务执行器
    pub fn with_agent_executor(mut self, executor: Arc<dyn AgentTaskExecutor>) -> Self {
        self.agent_executor = Some(executor);
        self
    }

    /// 设置 Cron 任务调度器
    pub fn with_cron_scheduler(
        mut self,
        scheduler: Arc<tokio::sync::Mutex<CronScheduler>>,
    ) -> Self {
        self.cron_scheduler = Some(scheduler);
        self
    }

    /// 获取通知管理器
    pub fn notification_manager(&self) -> Option<Arc<NotificationManager>> {
        self.notification_manager.clone()
    }

    /// 获取调度器
    pub(crate) fn scheduler(&self) -> Option<Arc<TaskScheduler>> {
        self.scheduler.clone()
    }

    /// 获取 Agent 任务执行器
    pub fn agent_executor(&self) -> Option<Arc<dyn AgentTaskExecutor>> {
        self.agent_executor.clone()
    }

    /// 获取 Cron 任务调度器
    pub fn cron_scheduler(&self) -> Option<Arc<tokio::sync::Mutex<CronScheduler>>> {
        self.cron_scheduler.clone()
    }

    /// 发送任务状态变更通知
    async fn notify_status_change(&self, task_id: &str, task_name: &str, status: TaskStatus) {
        if let Some(ref manager) = self.notification_manager {
            let notif = task_status_notification(task_id, task_name, status.as_str());
            manager.publish(notif);
        }
    }

    /// 创建并启动后台任务
    pub async fn spawn<F, Fut>(&self, spec: TaskSpec, task_fn: F) -> anyhow::Result<TaskId>
    where
        F: FnOnce(TaskSpec) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = anyhow::Result<TaskResult>> + Send + 'static,
    {
        self.spawn_with_id(generate_task_id(), spec, task_fn).await
    }

    /// 使用指定的 task_id 启动后台任务。
    async fn spawn_with_id<F, Fut>(
        &self,
        task_id: TaskId,
        spec: TaskSpec,
        task_fn: F,
    ) -> anyhow::Result<TaskId>
    where
        F: FnOnce(TaskSpec) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = anyhow::Result<TaskResult>> + Send + 'static,
    {
        let task_id_clone = task_id.clone();
        let task_name = spec.name.clone();

        // 1. 保存任务到存储
        self.store.create(&task_id, spec.clone()).await?;
        info!("Created background task: {}", task_id);

        // 2. 发送创建通知
        self.notify_status_change(&task_id, &task_name, TaskStatus::Pending)
            .await;

        // 3. 获取信号量许可（控制并发）
        let permit = self.semaphore.clone().acquire_owned().await?;

        // 4. 更新状态为运行中
        self.store
            .update_status(&task_id, TaskStatus::Running)
            .await?;
        self.notify_status_change(&task_id, &task_name, TaskStatus::Running)
            .await;

        // 5. 启动任务执行
        let store = self.store.clone();
        let running_tasks = self.running_tasks.clone();
        let notification_manager = self.notification_manager.clone();

        let handle = tokio::spawn(async move {
            // 执行任务
            let start = std::time::Instant::now();
            let result = task_fn(spec).await;
            let elapsed = start.elapsed();

            // 保存结果
            let task_result = match result {
                Ok(mut r) => {
                    r.elapsed_ms = elapsed.as_millis() as u64;
                    let _ = store.save_result(&task_id_clone, &r).await;
                    let _ = store
                        .update_status(&task_id_clone, TaskStatus::Completed)
                        .await;

                    // 发送完成通知
                    if let Some(ref manager) = notification_manager {
                        let notif =
                            task_status_notification(&task_id_clone, &task_name, "completed");
                        manager.publish(notif);
                    }

                    info!("Task {} completed in {:?}", task_id_clone, elapsed);
                    r
                }
                Err(e) => {
                    let error_result = TaskResult {
                        status: TaskStatus::Failed,
                        output: format!("Error: {}", e),
                        elapsed_ms: elapsed.as_millis() as u64,
                        steps: 0,
                    };
                    let _ = store.save_result(&task_id_clone, &error_result).await;
                    let _ = store
                        .update_status(&task_id_clone, TaskStatus::Failed)
                        .await;

                    // 发送失败通知
                    if let Some(ref manager) = notification_manager {
                        let notif = task_status_notification(&task_id_clone, &task_name, "failed");
                        manager.publish(notif);
                    }

                    error!("Task {} failed: {}", task_id_clone, e);
                    error_result
                }
            };

            // 从运行中移除
            let mut running = running_tasks.write().await;
            running.remove(&task_id_clone);

            // 释放许可
            drop(permit);

            task_result
        });

        // 保存任务句柄
        let task_handle = TaskHandle {
            task_id: task_id.clone(),
            abort_handle: handle.abort_handle(),
        };

        let mut running = self.running_tasks.write().await;
        running.insert(task_id.clone(), task_handle);

        info!("Started background task: {}", task_id);
        Ok(task_id)
    }

    /// 启动一个真实的 Agent 后台任务，使用指定的 task_id。
    ///
    /// 需要事先通过 `with_agent_executor` 配置执行器，否则会返回错误。
    pub async fn spawn_agent_with_id(
        &self,
        task_id: TaskId,
        spec: TaskSpec,
    ) -> anyhow::Result<TaskId> {
        let executor = self
            .agent_executor
            .as_ref()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "AgentTaskExecutor not configured. Use with_agent_executor() first."
                )
            })?
            .clone();

        self.spawn_with_id(task_id, spec, move |spec| async move {
            match executor.execute(&spec).await {
                Ok((output, steps)) => Ok(TaskResult {
                    status: TaskStatus::Completed,
                    output,
                    elapsed_ms: 0,
                    steps,
                }),
                Err(e) => Err(e),
            }
        })
        .await
    }

    /// 启动一个真实的 Agent 后台任务。
    ///
    /// 需要事先通过 `with_agent_executor` 配置执行器，否则会返回错误。
    pub async fn spawn_agent(&self, spec: TaskSpec) -> anyhow::Result<TaskId> {
        self.spawn_agent_with_id(generate_task_id(), spec).await
    }

    /// 使用调度器安排任务
    pub async fn schedule(&self, spec: TaskSpec) -> anyhow::Result<TaskId> {
        let scheduler = self
            .scheduler
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Scheduler not configured"))?;

        let task_id = generate_task_id();
        scheduler.schedule(task_id.clone(), spec.clone()).await?;

        // 发送通知
        self.notify_status_change(&task_id, &spec.name, TaskStatus::Pending)
            .await;

        Ok(task_id)
    }

    /// 处理调度器中的下一个任务
    pub async fn process_next_scheduled<F, Fut>(&self, task_fn: F) -> anyhow::Result<Option<TaskId>>
    where
        F: FnOnce(TaskSpec) -> Fut + Send + Clone + 'static,
        Fut: std::future::Future<Output = anyhow::Result<TaskResult>> + Send + 'static,
    {
        let scheduler = match self.scheduler.as_ref() {
            Some(s) => s,
            None => return Ok(None),
        };

        if let Some((task_id, spec)) = scheduler.next_task().await {
            // 更新状态为运行中
            self.store
                .update_status(&task_id, TaskStatus::Running)
                .await?;

            // 启动任务
            let actual_task_id = self.spawn(spec, task_fn).await?;
            Ok(Some(actual_task_id))
        } else {
            Ok(None)
        }
    }

    /// 获取任务状态
    pub async fn status(&self, task_id: &TaskId) -> anyhow::Result<TaskStatus> {
        let info = self.store.get(task_id).await?;
        Ok(info.status)
    }

    /// 等待任务完成
    pub async fn wait(&self, task_id: &TaskId) -> anyhow::Result<TaskResult> {
        // 轮询等待
        let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(100));

        loop {
            interval.tick().await;

            let status = self.status(task_id).await?;
            match status {
                TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled => {
                    break;
                }
                _ => continue,
            }
        }

        // 获取结果
        self.store.get_result(task_id).await
    }

    /// 取消任务
    pub async fn cancel(&self, task_id: &TaskId) -> anyhow::Result<()> {
        info!("Cancelling task: {}", task_id);

        // 如果任务正在运行，发送取消信号
        let mut running = self.running_tasks.write().await;
        if let Some(handle) = running.get(task_id) {
            handle.abort();
            running.remove(task_id);
        }
        drop(running);

        // 获取任务信息以发送通知
        let task_info = self.store.get(task_id).await.ok();

        // 更新存储状态
        self.store
            .update_status(task_id, TaskStatus::Cancelled)
            .await?;

        // 发送通知
        if let Some(info) = task_info {
            self.notify_status_change(task_id, &info.spec.name, TaskStatus::Cancelled)
                .await;
        }

        Ok(())
    }

    /// 列出所有任务
    pub async fn list(&self) -> anyhow::Result<Vec<TaskInfo>> {
        self.store.list_all().await
    }

    /// 获取存储
    pub fn store(&self) -> &TaskStore {
        &self.store
    }

    /// 获取运行中的任务数
    pub async fn running_count(&self) -> usize {
        let running = self.running_tasks.read().await;
        running.len()
    }

    /// 清理已完成的任务
    pub async fn cleanup_completed(&self) {
        let mut running = self.running_tasks.write().await;
        let completed: Vec<TaskId> = running
            .iter()
            .filter(|(_, handle)| handle.is_finished())
            .map(|(id, _)| id.clone())
            .collect();

        for id in completed {
            running.remove(&id);
        }
    }

    /// Process the next scheduled agent task using the configured executor.
    /// Returns the task id if one was started, None if queue is empty.
    pub async fn process_next_agent_task(&self) -> anyhow::Result<Option<TaskId>> {
        let scheduler = match self.scheduler.as_ref() {
            Some(s) => s,
            None => return Ok(None),
        };

        if let Some((task_id, spec)) = scheduler.next_task().await {
            self.store
                .update_status(&task_id, TaskStatus::Running)
                .await?;
            self.spawn_agent_with_id(task_id.clone(), spec).await?;
            Ok(Some(task_id))
        } else {
            Ok(None)
        }
    }

    /// Start a background loop that continuously processes scheduled agent tasks.
    /// Returns a JoinHandle that can be used to abort the loop.
    pub fn start_agent_scheduler_loop(
        &self,
        interval: std::time::Duration,
    ) -> tokio::task::JoinHandle<()> {
        let manager = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(interval);
            loop {
                interval.tick().await;
                match manager.process_next_agent_task().await {
                    Ok(Some(id)) => info!("Scheduler loop started agent task: {}", id),
                    Ok(None) => {}
                    Err(e) => error!("Scheduler loop error: {}", e),
                }
            }
        })
    }

    /// Schedule a recurring agent task using a cron expression.
    ///
    /// Returns the generated cron task id, or an error if the expression is invalid
    /// or the cron scheduler is not configured.
    pub async fn schedule_cron(&self, spec: TaskSpec, cron_expr: &str) -> anyhow::Result<String> {
        let cron = self
            .cron_scheduler
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Cron scheduler not configured"))?;

        let mut guard = cron.lock().await;
        let task_id = guard
            .add_task(spec.clone(), cron_expr)
            .map_err(|e| anyhow::anyhow!("Failed to schedule cron task: {}", e))?;
        drop(guard);

        // Persist to TaskStore
        let schedule = crate::background::cron::CronSchedule::new(cron_expr)
            .map_err(|e| anyhow::anyhow!("Invalid cron expression: {}", e))?;
        let cron_task = crate::background::cron::CronTask {
            task_id: task_id.clone(),
            task_spec: spec,
            schedule,
            enabled: true,
        };
        self.store.save_cron(&cron_task).await?;

        Ok(task_id)
    }

    /// List all cron tasks from the scheduler.
    pub async fn list_cron_tasks(&self) -> anyhow::Result<Vec<crate::background::cron::CronTask>> {
        let cron = self
            .cron_scheduler
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Cron scheduler not configured"))?;

        let guard = cron.lock().await;
        Ok(guard.tasks().to_vec())
    }

    /// Cancel (remove) a cron task by its id.
    pub async fn cancel_cron(&self, task_id: &str) -> anyhow::Result<()> {
        let cron = self
            .cron_scheduler
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Cron scheduler not configured"))?;

        let mut guard = cron.lock().await;
        if !guard.remove_task(task_id) {
            return Err(anyhow::anyhow!("Cron task not found: {}", task_id));
        }
        drop(guard);

        // Also remove from persistent store (best-effort)
        let _ = self.store.remove_cron(task_id).await;

        info!("Cancelled cron task: {}", task_id);
        Ok(())
    }

    /// Check for due cron tasks and spawn them.
    ///
    /// Returns the number of tasks that were spawned.
    async fn check_cron_tasks(&self) -> anyhow::Result<usize> {
        let cron = match self.cron_scheduler.as_ref() {
            Some(c) => c,
            None => return Ok(0),
        };

        let now = Utc::now();
        let mut guard = cron.lock().await;
        let specs = guard.tick(now);
        drop(guard);

        let mut count = 0;
        for spec in specs {
            self.spawn_agent(spec).await?;
            count += 1;
        }
        Ok(count)
    }

    /// Start a background loop that periodically checks for due cron tasks
    /// and spawns agent executions for them.
    ///
    /// The loop checks the provided `cancel_token` every tick and exits gracefully
    /// when the token is cancelled.
    ///
    /// Returns a [`tokio::task::JoinHandle`] that can be awaited or aborted.
    pub fn start_cron_loop(
        &self,
        interval: std::time::Duration,
        cancel_token: tokio_util::sync::CancellationToken,
    ) -> tokio::task::JoinHandle<()> {
        let manager = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(interval);
            loop {
                tokio::select! {
                    _ = interval.tick() => {}
                    _ = cancel_token.cancelled() => {
                        info!("Cron loop shutting down gracefully");
                        break;
                    }
                }

                match manager.check_cron_tasks().await {
                    Ok(count) => {
                        if count > 0 {
                            info!("Cron loop spawned {} agent task(s)", count);
                        }
                    }
                    Err(e) => error!("Cron loop error: {}", e),
                }
            }
        })
    }
}

/// 任务句柄
#[derive(Debug)]
pub(crate) struct TaskHandle {
    #[allow(dead_code)]
    task_id: TaskId,
    abort_handle: tokio::task::AbortHandle,
}

impl TaskHandle {
    /// 中止任务
    pub fn abort(&self) {
        self.abort_handle.abort();
    }

    /// 检查是否完成
    pub fn is_finished(&self) -> bool {
        self.abort_handle.is_finished()
    }
}

/// 生成任务 ID
fn generate_task_id() -> TaskId {
    use rand::RngExt;
    let mut rng = rand::rng();
    let id: String = (0..12)
        .map(|_| rng.sample(rand::distr::Alphanumeric) as char)
        .collect();
    format!("task_{}", id.to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notifications::NotificationManager;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_task_id_generation() {
        let id1 = generate_task_id();
        let id2 = generate_task_id();

        assert!(id1.starts_with("task_"));
        assert!(id2.starts_with("task_"));
        assert_ne!(id1, id2);
        assert_eq!(id1.len(), 17); // "task_" + 12 chars
    }

    #[tokio::test]
    async fn test_background_task_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let manager = BackgroundTaskManager::new(
            temp_dir.path().join("store"),
            temp_dir.path().join("work"),
            temp_dir.path().join("context"),
        );

        let list = manager.list().await.unwrap();
        assert!(list.is_empty());
    }

    #[tokio::test]
    async fn test_spawn_and_wait() {
        let temp_dir = TempDir::new().unwrap();
        let manager = BackgroundTaskManager::new(
            temp_dir.path().join("store"),
            temp_dir.path().join("work"),
            temp_dir.path().join("context"),
        );

        let spec = TaskSpec::new("test", "test prompt")
            .with_agent_type("coder")
            .with_max_iterations(10)
            .with_timeout_seconds(30);

        let task_id = manager
            .spawn(spec, |_spec| async {
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                Ok(TaskResult::success("done")
                    .with_elapsed_ms(100)
                    .with_steps(1))
            })
            .await
            .unwrap();

        // 等待完成
        let result = manager.wait(&task_id).await.unwrap();
        assert_eq!(result.status, TaskStatus::Completed);
        assert_eq!(result.output, "done");
    }

    #[tokio::test]
    async fn test_concurrency_limit() {
        let temp_dir = TempDir::new().unwrap();
        let manager = BackgroundTaskManager::new(
            temp_dir.path().join("store"),
            temp_dir.path().join("work"),
            temp_dir.path().join("context"),
        )
        .with_max_concurrency(2);

        // 启动多个任务
        let mut task_ids = Vec::new();
        for i in 0..5 {
            let spec = TaskSpec::new(format!("task_{}", i), "test");

            let id = manager
                .spawn(spec, move |_spec| async move {
                    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                    Ok(TaskResult::success(format!("task_{}", i)))
                })
                .await
                .unwrap();

            task_ids.push(id);
        }

        // 等待所有完成
        for id in task_ids {
            let result = manager.wait(&id).await.unwrap();
            assert_eq!(result.status, TaskStatus::Completed);
        }
    }

    #[tokio::test]
    async fn test_cancel_task() {
        let temp_dir = TempDir::new().unwrap();
        let manager = BackgroundTaskManager::new(
            temp_dir.path().join("store"),
            temp_dir.path().join("work"),
            temp_dir.path().join("context"),
        );

        let spec = TaskSpec::new("long_running", "test");

        let task_id = manager
            .spawn(spec, |_spec| async {
                // 长时间运行的任务
                tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
                Ok(TaskResult::success("completed"))
            })
            .await
            .unwrap();

        // 短暂等待确保任务开始
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // 取消任务
        manager.cancel(&task_id).await.unwrap();

        // 等待状态更新
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let status = manager.status(&task_id).await.unwrap();
        assert_eq!(status, TaskStatus::Cancelled);
    }

    #[tokio::test]
    async fn test_task_scheduler() {
        let temp_dir = TempDir::new().unwrap();
        let store = TaskStore::new(temp_dir.path().join("store"));
        let scheduler = Arc::new(TaskScheduler::new(store.clone()));

        // 调度任务
        let spec1 = TaskSpec::new("low_priority", "test").with_priority(TaskPriority::Low);
        let spec2 = TaskSpec::new("high_priority", "test").with_priority(TaskPriority::High);

        scheduler
            .schedule("task_1".to_string(), spec1)
            .await
            .unwrap();
        scheduler
            .schedule("task_2".to_string(), spec2)
            .await
            .unwrap();

        assert_eq!(scheduler.queue_len().await, 2);

        // 验证高优先级任务先出队
        let (next_id, next_spec) = scheduler.peek_task().await.unwrap();
        assert_eq!(next_id, "task_2");
        assert_eq!(next_spec.priority, TaskPriority::High);

        // 弹出并验证
        let (popped_id, _) = scheduler.next_task().await.unwrap();
        assert_eq!(popped_id, "task_2");

        let (popped_id, _) = scheduler.next_task().await.unwrap();
        assert_eq!(popped_id, "task_1");

        assert!(scheduler.is_empty().await);
    }

    #[tokio::test]
    async fn test_task_scheduler_with_manager() {
        let temp_dir = TempDir::new().unwrap();
        let store = TaskStore::new(temp_dir.path().join("store"));
        let scheduler = Arc::new(TaskScheduler::new(store));

        let manager = BackgroundTaskManager::new(
            temp_dir.path().join("store"),
            temp_dir.path().join("work"),
            temp_dir.path().join("context"),
        )
        .with_scheduler(scheduler.clone());

        // 调度任务
        let spec = TaskSpec::new("scheduled_task", "test").with_priority(TaskPriority::Normal);
        let task_id = manager.schedule(spec).await.unwrap();

        assert!(task_id.starts_with("task_"));
        assert_eq!(scheduler.queue_len().await, 1);
    }

    #[tokio::test]
    async fn test_notification_integration() {
        let temp_dir = TempDir::new().unwrap();
        let notif_manager = Arc::new(NotificationManager::new());

        let manager = BackgroundTaskManager::new(
            temp_dir.path().join("store"),
            temp_dir.path().join("work"),
            temp_dir.path().join("context"),
        )
        .with_notifications(notif_manager.clone());

        // 订阅通知
        let mut receiver = notif_manager.subscribe();

        // 启动任务
        let spec = TaskSpec::new("notified_task", "test");
        let task_id = manager
            .spawn(spec, |_spec| async {
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                Ok(TaskResult::success("done"))
            })
            .await
            .unwrap();

        // 等待任务完成
        let _ = manager.wait(&task_id).await;

        // 应该收到多个通知（pending, running, completed）
        let mut notification_count = 0;
        while let Ok(_notif) = receiver.try_recv() {
            notification_count += 1;
        }

        assert!(notification_count >= 2); // 至少 running 和 completed
    }

    #[tokio::test]
    async fn test_task_with_priority_spawn() {
        let temp_dir = TempDir::new().unwrap();
        let manager = BackgroundTaskManager::new(
            temp_dir.path().join("store"),
            temp_dir.path().join("work"),
            temp_dir.path().join("context"),
        );

        let spec = TaskSpec::new("critical_task", "test").with_priority(TaskPriority::Critical);

        let task_id = manager
            .spawn(spec.clone(), |_spec| async {
                Ok(TaskResult::success("done"))
            })
            .await
            .unwrap();

        let info = manager.store().get(&task_id).await.unwrap();
        assert_eq!(info.spec.priority, TaskPriority::Critical);
    }

    #[tokio::test]
    async fn test_scheduled_task_ordering() {
        let temp_dir = TempDir::new().unwrap();
        let store = TaskStore::new(temp_dir.path().join("store"));
        let scheduler = Arc::new(TaskScheduler::new(store));

        // 按不同优先级调度任务
        let priorities = vec![
            ("task_low", TaskPriority::Low),
            ("task_critical", TaskPriority::Critical),
            ("task_normal", TaskPriority::Normal),
            ("task_high", TaskPriority::High),
            ("task_background", TaskPriority::Background),
        ];

        for (id, priority) in &priorities {
            let spec = TaskSpec::new(*id, "test").with_priority(*priority);
            scheduler.schedule(id.to_string(), spec).await.unwrap();
        }

        // 验证出队顺序
        let mut order = Vec::new();
        for _ in 0..5 {
            if let Some((id, _)) = scheduler.next_task().await {
                order.push(id);
            }
        }

        assert_eq!(
            order,
            vec![
                "task_critical",
                "task_high",
                "task_normal",
                "task_low",
                "task_background",
            ]
        );
    }

    #[tokio::test]
    async fn test_spawn_agent_with_mock_llm() {
        use crate::agent::MockLlm;
        use crate::background::agent_executor::DefaultAgentTaskExecutor;
        use crate::registry::ToolRegistry;
        use std::sync::Arc;

        let temp_dir = TempDir::new().unwrap();
        let manager = BackgroundTaskManager::new(
            temp_dir.path().join("store"),
            temp_dir.path().join("work"),
            temp_dir.path().join("context"),
        );

        // 未配置 executor 时 spawn_agent 应报错
        let spec = TaskSpec::new("agent_task", "Say hello").with_agent_type("coder");
        let err = manager.spawn_agent(spec.clone()).await;
        assert!(err.is_err());
        assert!(err
            .unwrap_err()
            .to_string()
            .contains("AgentTaskExecutor not configured"));

        // 配置 executor 后应能成功启动真实 Agent 任务
        let registry = ToolRegistry::with_builtin_tools();
        let llm = Arc::new(MockLlm);
        let executor = Arc::new(DefaultAgentTaskExecutor::new(
            llm,
            registry,
            temp_dir.path(),
        ));

        let manager = BackgroundTaskManager::new(
            temp_dir.path().join("store2"),
            temp_dir.path().join("work"),
            temp_dir.path().join("context"),
        )
        .with_agent_executor(executor);

        let task_id = manager.spawn_agent(spec).await.unwrap();
        let result = manager.wait(&task_id).await.unwrap();

        assert_eq!(result.status, TaskStatus::Completed);
        assert_eq!(result.output, "This is a mock response");
    }

    #[tokio::test]
    async fn test_process_next_agent_task_empty_queue() {
        let temp_dir = TempDir::new().unwrap();
        let manager = BackgroundTaskManager::new(
            temp_dir.path().join("store"),
            temp_dir.path().join("work"),
            temp_dir.path().join("context"),
        );

        // No scheduler configured → returns None
        let result = manager.process_next_agent_task().await.unwrap();
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_process_next_agent_task_scheduled() {
        use crate::agent::MockLlm;
        use crate::background::agent_executor::DefaultAgentTaskExecutor;
        use crate::registry::ToolRegistry;
        use std::sync::Arc;

        let temp_dir = TempDir::new().unwrap();
        let store = TaskStore::new(temp_dir.path().join("store"));
        let scheduler = Arc::new(TaskScheduler::new(store));

        let registry = ToolRegistry::with_builtin_tools();
        let llm = Arc::new(MockLlm);
        let executor = Arc::new(DefaultAgentTaskExecutor::new(
            llm,
            registry,
            temp_dir.path(),
        ));

        let manager = BackgroundTaskManager::new(
            temp_dir.path().join("store"),
            temp_dir.path().join("work"),
            temp_dir.path().join("context"),
        )
        .with_scheduler(scheduler.clone())
        .with_agent_executor(executor);

        // Schedule an agent task
        let spec = TaskSpec::new("loop_task", "Say hello").with_agent_type("coder");
        let _scheduled_id = manager.schedule(spec).await.unwrap();

        // Queue should have 1 item
        assert_eq!(scheduler.queue_len().await, 1);

        // Process it
        let started_id = manager.process_next_agent_task().await.unwrap();
        assert!(started_id.is_some());

        // Wait for completion
        let result = manager.wait(&started_id.unwrap()).await.unwrap();
        assert_eq!(result.status, TaskStatus::Completed);
    }

    #[tokio::test]
    async fn test_start_agent_scheduler_loop() {
        use crate::agent::MockLlm;
        use crate::background::agent_executor::DefaultAgentTaskExecutor;
        use crate::registry::ToolRegistry;
        use std::sync::Arc;

        let temp_dir = TempDir::new().unwrap();
        let store = TaskStore::new(temp_dir.path().join("store"));
        let scheduler = Arc::new(TaskScheduler::new(store));

        let registry = ToolRegistry::with_builtin_tools();
        let llm = Arc::new(MockLlm);
        let executor = Arc::new(DefaultAgentTaskExecutor::new(
            llm,
            registry,
            temp_dir.path(),
        ));

        let manager = BackgroundTaskManager::new(
            temp_dir.path().join("store"),
            temp_dir.path().join("work"),
            temp_dir.path().join("context"),
        )
        .with_scheduler(scheduler.clone())
        .with_agent_executor(executor);

        // Start the scheduler loop (tick every 100ms for fast test)
        let handle = manager.start_agent_scheduler_loop(std::time::Duration::from_millis(100));

        // Schedule a task after loop starts
        let spec = TaskSpec::new("auto_task", "Say hello").with_agent_type("coder");
        let task_id = manager.schedule(spec).await.unwrap();

        // Wait for the loop to pick it up and complete
        let result =
            tokio::time::timeout(tokio::time::Duration::from_secs(5), manager.wait(&task_id))
                .await
                .expect("timeout waiting for scheduler loop")
                .expect("wait error");

        assert_eq!(result.status, TaskStatus::Completed);

        // Abort the loop
        handle.abort();
    }
}
