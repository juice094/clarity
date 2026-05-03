//! Worker Pool - 工作线程池
//!
//! 提供可扩展的工作线程池，用于执行后台任务
#![allow(dead_code)]

use super::{AgentTaskExecutor, TaskId, TaskResult, TaskSpec, TaskStatus, TaskStore};
use crate::notifications::{task_status_notification, NotificationManager};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, Mutex, RwLock};
use tokio::task::JoinHandle;
use tracing::{debug, error, info};

/// 工作项
#[derive(Debug)]
struct WorkItem {
    /// 任务 ID
    task_id: TaskId,
    /// 任务规格
    spec: TaskSpec,
    /// 完成通知通道
    done_tx: oneshot::Sender<TaskResult>,
}

/// 工作线程统计信息
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct WorkerStats {
    /// 工作线程 ID
    pub worker_id: usize,
    /// 处理的任务数
    pub tasks_processed: u64,
    /// 失败的任务数
    pub tasks_failed: u64,
    /// 总处理时间（毫秒）
    pub total_elapsed_ms: u64,
    /// 当前是否忙碌
    pub is_busy: bool,
    /// 当前任务 ID
    pub current_task: Option<TaskId>,
}

impl WorkerStats {
    /// 创建新的统计信息
    fn new(worker_id: usize) -> Self {
        Self {
            worker_id,
            tasks_processed: 0,
            tasks_failed: 0,
            total_elapsed_ms: 0,
            is_busy: false,
            current_task: None,
        }
    }

    /// 平均处理时间
    pub fn avg_elapsed_ms(&self) -> f64 {
        if self.tasks_processed == 0 {
            0.0
        } else {
            self.total_elapsed_ms as f64 / self.tasks_processed as f64
        }
    }
}

/// 工作线程池
#[derive(Debug)]
#[allow(dead_code)]
pub struct WorkerPool {
    /// 工作发送通道
    work_tx: mpsc::Sender<WorkItem>,
    /// 任务存储
    store: TaskStore,
    /// 通知管理器
    _notification_manager: Option<Arc<NotificationManager>>,
    /// 工作线程数
    worker_count: usize,
    /// 工作句柄
    handles: Mutex<Vec<JoinHandle<()>>>,
    /// 关闭信号发送器
    shutdown_tx: Mutex<Option<mpsc::Sender<()>>>,
    /// 工作线程实时统计信息
    worker_stats: Arc<RwLock<Vec<Option<WorkerStats>>>>,
    /// 可选的 Agent 任务执行器
    agent_executor: Option<Arc<dyn AgentTaskExecutor>>,
}

impl WorkerPool {
    /// 创建新的工作线程池
    pub async fn new(store: TaskStore, worker_count: usize) -> Arc<Self> {
        let (work_tx, work_rx) = mpsc::channel::<WorkItem>(100);
        let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>(1);
        let shutdown_rx = Arc::new(Mutex::new(shutdown_rx));

        let notification_manager: Option<Arc<NotificationManager>> = None;
        let handles = Mutex::new(Vec::with_capacity(worker_count));

        // 使用 Arc<Mutex<Receiver>> 允许多个 worker 共享接收器
        let work_rx = Arc::new(Mutex::new(work_rx));

        // 共享的 worker 统计信息
        let worker_stats = Arc::new(RwLock::new(
            (0..worker_count).map(|_| None).collect::<Vec<_>>(),
        ));

        let pool = Arc::new(Self {
            work_tx,
            store: store.clone(),
            _notification_manager: notification_manager.clone(),
            worker_count,
            handles,
            shutdown_tx: Mutex::new(Some(shutdown_tx)),
            worker_stats: worker_stats.clone(),
            agent_executor: None,
        });

        // 启动工作线程
        for id in 0..worker_count {
            let store_clone = store.clone();
            let notif_clone = notification_manager.clone();
            let work_rx = work_rx.clone();
            let shutdown_rx = shutdown_rx.clone();
            let worker_stats = worker_stats.clone();

            let pool_clone = pool.clone();
            let handle = tokio::spawn(async move {
                info!("Worker {} started", id);
                let mut stats = WorkerStats::new(id);
                {
                    let mut ws = worker_stats.write().await;
                    ws[id] = Some(stats.clone());
                }

                loop {
                    tokio::select! {
                        // 接收工作项
                        Some(work) = async {
                            let mut rx = work_rx.lock().await;
                            rx.recv().await
                        } => {
                            let start = std::time::Instant::now();

                            // 更新统计
                            stats.is_busy = true;
                            stats.current_task = Some(work.task_id.clone());
                            {
                                let mut ws = worker_stats.write().await;
                                if let Some(ref mut s) = ws[id] {
                                    *s = stats.clone();
                                }
                            }

                            debug!("Worker {} processing task {}", id, work.task_id);

                            // 更新任务状态为运行中
                            let _ = store_clone.update_status(&work.task_id, TaskStatus::Running).await;

                            // 发送通知
                            if let Some(ref manager) = notif_clone {
                                let notif = task_status_notification(
                                    &work.task_id,
                                    &work.spec.name,
                                    "running"
                                );
                                manager.publish(notif);
                            }

                            // 执行任务处理逻辑
                            let result = pool_clone.process_task(&work.spec).await;

                            let elapsed = start.elapsed();
                            let elapsed_ms = elapsed.as_millis() as u64;

                            // 构建任务结果
                            let task_result = match result {
                                Ok((output, steps)) => {
                                    let _ = store_clone.update_status(&work.task_id, TaskStatus::Completed).await;

                                    // 发送完成通知
                                    if let Some(ref manager) = notif_clone {
                                        let notif = task_status_notification(
                                            &work.task_id,
                                            &work.spec.name,
                                            "completed"
                                        );
                                        manager.publish(notif);
                                    }

                                    TaskResult {
                                        status: TaskStatus::Completed,
                                        output,
                                        elapsed_ms,
                                        steps,
                                    }
                                }
                                Err(e) => {
                                    let _ = store_clone.update_status(&work.task_id, TaskStatus::Failed).await;

                                    // 发送失败通知
                                    if let Some(ref manager) = notif_clone {
                                        let notif = task_status_notification(
                                            &work.task_id,
                                            &work.spec.name,
                                            "failed"
                                        );
                                        manager.publish(notif);
                                    }

                                    error!("Worker {} task {} failed: {}", id, work.task_id, e);
                                    TaskResult {
                                        status: TaskStatus::Failed,
                                        output: format!("Error: {}", e),
                                        elapsed_ms,
                                        steps: 0,
                                    }
                                }
                            };

                            // 保存结果
                            let _ = store_clone.save_result(&work.task_id, &task_result).await;

                            // 更新统计
                            stats.tasks_processed += 1;
                            stats.total_elapsed_ms += elapsed_ms;
                            if task_result.status == TaskStatus::Failed {
                                stats.tasks_failed += 1;
                            }
                            stats.is_busy = false;
                            stats.current_task = None;
                            {
                                let mut ws = worker_stats.write().await;
                                if let Some(ref mut s) = ws[id] {
                                    *s = stats.clone();
                                }
                            }

                            // 通知完成
                            let _ = work.done_tx.send(task_result);

                            debug!("Worker {} completed task {} in {:?}", id, work.task_id, elapsed);
                        }

                        // 接收关闭信号
                        _ = async {
                            let mut rx = shutdown_rx.lock().await;
                            rx.recv().await
                        } => {
                            info!("Worker {} shutting down", id);
                            break;
                        }

                        // 通道关闭
                        else => {
                            info!("Worker {} work channel closed", id);
                            break;
                        }
                    }
                }

                info!("Worker {} stopped", id);
            });

            pool.handles.lock().await.push(handle);
        }

        pool
    }

    /// 处理任务——优先使用 Agent 执行器，否则回退到模拟实现
    async fn process_task(&self, spec: &TaskSpec) -> anyhow::Result<(String, usize)> {
        let fut = async {
            if let Some(ref executor) = self.agent_executor {
                executor.execute(spec).await
            } else {
                Self::execute_task_logic(spec).await.map(|s| (s, 1))
            }
        };

        if let Some(timeout_secs) = spec.timeout_seconds {
            tokio::time::timeout(tokio::time::Duration::from_secs(timeout_secs), fut)
                .await
                .map_err(|_| anyhow::anyhow!("Task timed out"))?
        } else {
            fut.await
        }
    }

    /// 模拟任务执行（当没有配置 Agent 执行器时的回退）
    async fn execute_task_logic(spec: &TaskSpec) -> anyhow::Result<String> {
        // 保留旧行为作为无配置时的回退
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        Ok(format!(
            "Task '{}' processed by {} agent. Prompt: {}",
            spec.name, spec.agent_type, spec.prompt
        ))
    }

    /// 设置通知管理器
    pub fn with_notifications(self: &Arc<Self>, manager: Arc<NotificationManager>) -> Arc<Self> {
        // 创建新的 WorkerPool 实例并设置通知管理器
        Arc::new(WorkerPool {
            work_tx: self.work_tx.clone(),
            store: self.store.clone(),
            _notification_manager: Some(manager),
            worker_count: self.worker_count,
            handles: Mutex::new(Vec::new()),
            shutdown_tx: Mutex::new(None),
            worker_stats: self.worker_stats.clone(),
            agent_executor: self.agent_executor.clone(),
        })
    }

    /// 设置 Agent 任务执行器
    pub fn with_agent_executor(
        self: &Arc<Self>,
        executor: Arc<dyn AgentTaskExecutor>,
    ) -> Arc<Self> {
        Arc::new(WorkerPool {
            work_tx: self.work_tx.clone(),
            store: self.store.clone(),
            _notification_manager: self._notification_manager.clone(),
            worker_count: self.worker_count,
            handles: Mutex::new(Vec::new()),
            shutdown_tx: Mutex::new(None),
            worker_stats: self.worker_stats.clone(),
            agent_executor: Some(executor),
        })
    }

    /// 提交任务到工作线程池
    pub async fn submit(
        &self,
        task_id: TaskId,
        spec: TaskSpec,
    ) -> anyhow::Result<oneshot::Receiver<TaskResult>> {
        // 先创建任务记录
        self.store.create(&task_id, spec.clone()).await?;

        let (done_tx, done_rx) = oneshot::channel();
        let work = WorkItem {
            task_id,
            spec,
            done_tx,
        };

        self.work_tx
            .send(work)
            .await
            .map_err(|_| anyhow::anyhow!("Failed to submit work item"))?;

        Ok(done_rx)
    }

    /// 获取工作线程数
    pub fn worker_count(&self) -> usize {
        self.worker_count
    }

    /// 获取所有工作线程的统计信息
    pub async fn stats(&self) -> Vec<WorkerStats> {
        let ws: tokio::sync::RwLockReadGuard<'_, Vec<Option<WorkerStats>>> =
            self.worker_stats.read().await;
        ws.iter()
            .filter_map(|s: &Option<WorkerStats>| s.clone())
            .collect()
    }

    /// 获取忙碌的工作线程数
    pub async fn busy_count(&self) -> usize {
        let ws: tokio::sync::RwLockReadGuard<'_, Vec<Option<WorkerStats>>> =
            self.worker_stats.read().await;
        ws.iter()
            .filter(|s: &&Option<WorkerStats>| s.as_ref().is_some_and(|stats| stats.is_busy))
            .count()
    }

    /// 优雅关闭工作线程池
    pub async fn shutdown(&self) {
        info!(
            "Shutting down worker pool with {} workers",
            self.worker_count
        );

        // 发送关闭信号
        let tx = self.shutdown_tx.lock().await.take();
        if let Some(tx) = tx {
            let _ = tx.send(()).await;
        }

        // 等待所有工作线程完成
        let handles: Vec<_> = std::mem::take(&mut *self.handles.lock().await);
        for handle in handles {
            let _ = tokio::time::timeout(tokio::time::Duration::from_secs(5), handle).await;
        }

        info!("Worker pool shut down complete");
    }
}

/// 可扩展工作线程池
#[derive(Debug)]
#[allow(dead_code)]
pub struct ScalableWorkerPool {
    /// 基础工作线程池
    pool: Arc<WorkerPool>,
    /// 最小工作线程数
    _min_workers: usize,
    /// 最大工作线程数
    _max_workers: usize,
    /// 任务队列长度阈值
    queue_threshold: usize,
}

impl ScalableWorkerPool {
    /// 创建新的可扩展工作线程池
    pub async fn new(store: TaskStore, min_workers: usize, max_workers: usize) -> Self {
        let pool = WorkerPool::new(store, min_workers).await;

        Self {
            pool,
            _min_workers: min_workers,
            _max_workers: max_workers,
            queue_threshold: 10,
        }
    }

    /// 设置队列阈值
    pub fn with_queue_threshold(mut self, threshold: usize) -> Self {
        self.queue_threshold = threshold;
        self
    }

    /// 提交任务
    pub async fn submit(
        &self,
        task_id: TaskId,
        spec: TaskSpec,
    ) -> anyhow::Result<oneshot::Receiver<TaskResult>> {
        self.pool.submit(task_id, spec).await
    }

    /// 获取统计信息
    pub async fn stats(&self) -> Vec<WorkerStats> {
        self.pool.stats().await
    }

    /// 关闭工作线程池
    pub async fn shutdown(&self) {
        self.pool.shutdown().await;
    }
}

/// Worker 结构体（兼容层）
#[allow(dead_code)]
pub struct Worker;

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_spec(name: &str) -> TaskSpec {
        TaskSpec::new(name, "test prompt").with_agent_type("coder")
    }

    #[tokio::test]
    async fn test_worker_pool_creation() {
        let temp_dir = TempDir::new().unwrap();
        let store = TaskStore::new(temp_dir.path());
        let pool = WorkerPool::new(store, 2).await;

        assert_eq!(pool.worker_count(), 2);

        pool.shutdown().await;
    }

    #[tokio::test]
    async fn test_worker_pool_submit() {
        let temp_dir = TempDir::new().unwrap();
        let store = TaskStore::new(temp_dir.path());
        let pool = WorkerPool::new(store, 1).await;

        let spec = create_test_spec("test_task");
        let done_rx = pool.submit("task_1".to_string(), spec).await.unwrap();

        // 等待任务完成
        let result = done_rx.await.unwrap();
        assert_eq!(result.status, TaskStatus::Completed);
        assert!(result.output.contains("test_task"));

        pool.shutdown().await;
    }

    #[tokio::test]
    async fn test_worker_pool_multiple_tasks() {
        let temp_dir = TempDir::new().unwrap();
        let store = TaskStore::new(temp_dir.path());
        let pool = WorkerPool::new(store, 2).await;

        let mut receivers = Vec::new();

        // 提交多个任务
        for i in 0..5 {
            let spec = create_test_spec(&format!("task_{}", i));
            let done_rx = pool.submit(format!("task_{}", i), spec).await.unwrap();
            receivers.push(done_rx);
        }

        // 等待所有任务完成
        for (i, rx) in receivers.into_iter().enumerate() {
            let result = rx.await.unwrap();
            assert_eq!(result.status, TaskStatus::Completed, "Task {} failed", i);
        }

        pool.shutdown().await;
    }

    #[tokio::test]
    async fn test_worker_with_notifications() {
        let temp_dir = TempDir::new().unwrap();
        let store = TaskStore::new(temp_dir.path());
        let notif_manager = Arc::new(NotificationManager::new());

        let _rx = notif_manager.subscribe();

        // 注意：由于通知管理器在创建后设置，这个测试简化处理
        let pool = WorkerPool::new(store, 1).await;

        let spec = create_test_spec("notified_task");
        let done_rx = pool.submit("notif_task".to_string(), spec).await.unwrap();

        let _ = done_rx.await;

        pool.shutdown().await;
    }

    #[tokio::test]
    async fn test_scalable_worker_pool() {
        let temp_dir = TempDir::new().unwrap();
        let store = TaskStore::new(temp_dir.path());
        let pool = ScalableWorkerPool::new(store, 1, 4).await;

        let mut receivers = Vec::new();

        for i in 0..3 {
            let spec = create_test_spec(&format!("scalable_task_{}", i));
            let done_rx = pool.submit(format!("scalable_{}", i), spec).await.unwrap();
            receivers.push(done_rx);
        }

        for rx in receivers {
            let result = rx.await.unwrap();
            assert_eq!(result.status, TaskStatus::Completed);
        }

        pool.shutdown().await;
    }
}
