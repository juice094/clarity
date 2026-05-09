//! Subagent Parallel Execution - 子代理并行执行
//!
//! 提供多个子代理并行执行能力，支持：
//! - 并发控制（信号量）
//! - 结果聚合
//! - 错误处理
//! - 超时控制

use clarity_core::background::{BackgroundTaskManager, TaskResult, TaskSpec, TaskStatus};
use crate::runner::{SubagentRunner};
use crate::store::SubagentStore;
use clarity_contract::subagent::{
    BatchProgress, BatchStatus, ParallelConfig, ParallelResult, RunSpec, SubagentError,
    SubagentResult,
};
use parking_lot::Mutex;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

/// 子代理批次构建器
///
/// 提供流畅的 API 构建并行执行批次
pub struct SubagentBatch {
    specs: Vec<RunSpec>,
    config: ParallelConfig,
}

impl Default for SubagentBatch {
    fn default() -> Self {
        Self::new()
    }
}

impl SubagentBatch {
    /// 创建新的批次
    pub fn new() -> Self {
        Self {
            specs: Vec::new(),
            config: ParallelConfig::default(),
        }
    }

    /// 添加任务规格
    #[allow(clippy::should_implement_trait)]
    pub fn add(mut self, spec: RunSpec) -> Self {
        self.specs.push(spec);
        self
    }

    /// 批量添加任务规格
    pub fn add_many(mut self, specs: Vec<RunSpec>) -> Self {
        self.specs.extend(specs);
        self
    }

    /// 设置配置
    pub fn with_config(mut self, config: ParallelConfig) -> Self {
        self.config = config;
        self
    }

    /// 获取任务数量
    pub fn len(&self) -> usize {
        self.specs.len()
    }

    /// 检查是否为空
    pub fn is_empty(&self) -> bool {
        self.specs.is_empty()
    }
}

/// 并行执行器
///
/// 基于 BackgroundTaskManager 实现子代理并行执行
pub struct ParallelExecutor {
    task_manager: BackgroundTaskManager,
    runner: SubagentRunner,
}

impl ParallelExecutor {
    /// 创建新的并行执行器
    pub fn new(task_manager: BackgroundTaskManager, runner: SubagentRunner) -> Self {
        Self {
            task_manager,
            runner,
        }
    }

    /// 并行执行多个子代理，可选的 progress 回调用于外部进度跟踪。
    pub async fn execute(
        &mut self,
        batch: SubagentBatch,
        progress: Option<std::sync::Arc<Mutex<BatchProgress>>>,
        cancel: Option<CancellationToken>,
    ) -> anyhow::Result<ParallelResult> {
        if batch.is_empty() {
            return Ok(ParallelResult {
                results: Vec::new(),
                failures: Vec::new(),
                total_elapsed_ms: 0,
                actual_concurrency: 0,
                aggregated_summary: None,
            });
        }

        let start_time = std::time::Instant::now();
        let config = batch.config.clone();
        let len = batch.len();

        info!(
            "Starting parallel execution of {} subagents with max concurrency {}",
            len, config.max_concurrency
        );

        // 创建信号量控制并发
        let semaphore = Arc::new(Semaphore::new(config.max_concurrency));

        // 任务列表
        let mut task_ids: Vec<String> = Vec::new();

        for spec in batch.specs.clone() {
            let sem = semaphore.clone();
            let runner = self.runner.clone();
            let mut store = SubagentStore::new(self.runner.working_dir().join("parallel_store"));

            // 创建后台任务
            let task_spec = TaskSpec {
                name: spec.description.clone(),
                description: format!("Parallel subagent: {}", spec.description),
                agent_type: spec.requested_type.clone(),
                prompt: spec.prompt.clone(),
                max_iterations: spec.max_iterations,
                timeout_seconds: config.timeout_secs,
                priority: clarity_core::background::TaskPriority::Normal,
                model_alias: spec.model_override.clone(),
            };

            let task_id = self
                .task_manager
                .spawn(task_spec, move |_ts| async move {
                    // 获取信号量许可
                    let _permit =
                        sem.acquire()
                            .await
                            .map_err(|e| SubagentError::ExecutionFailed {
                                message: e.to_string(),
                                brief: "semaphore error".to_string(),
                            })?;

                    // 执行子代理
                    let result = runner.run(spec, &mut store, None).await;

                    // 转换结果为 TaskResult
                    match result {
                        Ok(r) => Ok(TaskResult {
                            status: TaskStatus::Completed,
                            output: serde_json::to_string(&r).unwrap_or_default(),
                            elapsed_ms: r.elapsed_ms,
                            steps: r.steps_taken,
                        }),
                        Err(e) => Ok(TaskResult {
                            status: TaskStatus::Failed,
                            output: e.to_string(),
                            elapsed_ms: 0,
                            steps: 0,
                        }),
                    }
                })
                .await
                .map_err(|e| SubagentError::ExecutionFailed {
                    message: e.to_string(),
                    brief: "spawn failed".to_string(),
                })?;

            task_ids.push(task_id);
        }

        // 收集结果（带进度回调）
        let mut results = Vec::new();
        let mut failures = Vec::new();
        let mut should_cancel_others = false;

        for task_id in &task_ids {
                if let Some(ref c) = cancel {
                    if c.is_cancelled() {
                        warn!("Parallel execution cancelled by external signal");
                        for remaining in &task_ids {
                            if remaining != task_id {
                                if let Err(e) = self.task_manager.cancel(remaining).await {
                                    warn!("Failed to cancel task {}: {}", remaining, e);
                                }
                            }
                        }
                        break;
                    }
                }
            match self.task_manager.wait(task_id).await {
                Ok(task_result) => {
                    if task_result.status == TaskStatus::Completed {
                        if let Ok(subagent_result) =
                            serde_json::from_str::<SubagentResult>(&task_result.output)
                        {
                            results.push(subagent_result.clone());
                            // 更新进度：一个子代理完成
                            if let Some(ref p) = progress {
                                let mut p = p.lock();
                                p.completed += 1;
                                p.running.retain(|id| id != &subagent_result.agent_id);
                                p.results.push(subagent_result);
                                p.elapsed_ms = start_time.elapsed().as_millis() as u64;
                            }
                        } else {
                            failures.push((
                                task_id.clone(),
                                "Failed to parse subagent result".to_string(),
                            ));
                            if let Some(ref p) = progress {
                                let mut p = p.lock();
                                p.failed += 1;
                                p.elapsed_ms = start_time.elapsed().as_millis() as u64;
                            }
                        }
                    } else if task_result.status == TaskStatus::Cancelled {
                        failures.push((task_id.clone(), "Task was cancelled".to_string()));
                        if let Some(ref p) = progress {
                            let mut p = p.lock();
                            p.failed += 1;
                            p.elapsed_ms = start_time.elapsed().as_millis() as u64;
                        }
                    } else {
                        failures.push((
                            task_id.clone(),
                            format!("task failed: {}", task_result.output),
                        ));
                        if let Some(ref p) = progress {
                            let mut p = p.lock();
                            p.failed += 1;
                            p.elapsed_ms = start_time.elapsed().as_millis() as u64;
                        }

                        // 如果需要，取消其他正在运行的任务
                        if config.cancel_on_error && !should_cancel_others {
                            should_cancel_others = true;
                            warn!("Canceling remaining tasks due to failure");
                            for remaining in &task_ids {
                                if remaining != task_id {
                                    if let Err(e) = self.task_manager.cancel(remaining).await {
                                        warn!("Failed to cancel task {}: {}", remaining, e);
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    failures.push((task_id.clone(), format!("wait failed: {}", e)));
                    if let Some(ref p) = progress {
                        let mut p = p.lock();
                        p.failed += 1;
                        p.elapsed_ms = start_time.elapsed().as_millis() as u64;
                    }
                }
            }
        }

        // 标记进度为完成
        if let Some(ref p) = progress {
            let mut p = p.lock();
            p.running.clear();
            if failures.is_empty() {
                p.status = BatchStatus::Completed;
            } else if results.is_empty() {
                p.status = BatchStatus::Failed("All subagents failed".to_string());
            } else {
                p.status = BatchStatus::Completed; // partial success
            }
            p.elapsed_ms = start_time.elapsed().as_millis() as u64;
        }

        let elapsed = start_time.elapsed().as_millis() as u64;

        // 结果聚合
        let aggregated_summary = if config.enable_aggregation && !results.is_empty() {
            Some(Self::aggregate_results(&results))
        } else {
            None
        };

        info!(
            "Parallel execution completed: {} succeeded, {} failed, elapsed: {}ms",
            results.len(),
            failures.len(),
            elapsed
        );

        Ok(ParallelResult {
            results,
            failures,
            total_elapsed_ms: elapsed,
            actual_concurrency: config.max_concurrency.min(batch.len()),
            aggregated_summary,
        })
    }

    /// 聚合结果
    fn aggregate_results(results: &[SubagentResult]) -> String {
        let mut summary = String::from("# Parallel Execution Summary\n\n");

        summary.push_str(&format!("Total tasks: {}\n", results.len()));
        summary.push_str(&format!("Successful: {}\n", results.len()));

        // 按代理类型分组
        let mut by_type: std::collections::HashMap<String, Vec<&SubagentResult>> =
            std::collections::HashMap::new();
        for r in results {
            by_type.entry(r.agent_type.clone()).or_default().push(r);
        }

        summary.push_str("\n## By Agent Type\n\n");
        for (agent_type, items) in by_type {
            summary.push_str(&format!("- {}: {} tasks\n", agent_type, items.len()));
        }

        // 添加每个结果的摘要
        summary.push_str("\n## Individual Results\n\n");
        for (i, result) in results.iter().enumerate() {
            summary.push_str(&format!(
                "{}. {} ({}): {}\n",
                i + 1,
                result.agent_id,
                result.agent_type,
                result.summary.lines().next().unwrap_or("No summary")
            ));
        }

        summary
    }
}

/// 高级并行执行 API
///
/// 简化并行子代理执行的入口函数
pub async fn run_parallel(
    specs: Vec<RunSpec>,
    runner: SubagentRunner,
    task_manager: BackgroundTaskManager,
    config: ParallelConfig,
    progress: Option<Arc<Mutex<BatchProgress>>>,
    cancel: Option<CancellationToken>,
) -> anyhow::Result<ParallelResult> {
    let batch = SubagentBatch::new().add_many(specs).with_config(config);

    let mut executor = ParallelExecutor::new(task_manager, runner);
    executor.execute(batch, progress, cancel).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use clarity_contract::subagent::{ExecutionStatus, ParallelConfig, ParallelResult, RunSpec, SubagentResult};
    use clarity_core::registry::ToolRegistry;
    use tempfile::TempDir;

    fn create_test_runner() -> (SubagentRunner, TempDir) {
        let registry = ToolRegistry::with_builtin_tools();
        let temp_dir = TempDir::new().unwrap();

        let runner = SubagentRunner::new(
            registry,
            temp_dir.path().join("work"),
            temp_dir.path().join("context"),
        );

        (runner, temp_dir)
    }

    #[test]
    fn test_parallel_config_builder() {
        let config = ParallelConfig::new()
            .with_max_concurrency(5)
            .with_timeout(600)
            .cancel_on_error();

        assert_eq!(config.max_concurrency, 5);
        assert_eq!(config.timeout_secs, Some(600));
        assert!(config.cancel_on_error);
    }

    #[test]
    fn test_subagent_batch_builder() {
        let batch = SubagentBatch::new()
            .add(RunSpec::new("Task 1", "Do something").with_type("coder"))
            .add(RunSpec::new("Task 2", "Do another").with_type("explore"))
            .with_config(ParallelConfig::new().with_max_concurrency(2));

        assert_eq!(batch.len(), 2);
        assert!(!batch.is_empty());
    }

    #[test]
    fn test_parallel_result_helpers() {
        let result = ParallelResult {
            results: vec![
                SubagentResult {
                    agent_id: "a1".to_string(),
                    agent_type: "coder".to_string(),
                    status: ExecutionStatus::Success,
                    summary: "Success 1".to_string(),
                    full_output: "".to_string(),
                    resumed: false,
                    steps_taken: 5,
                    elapsed_ms: 1000,
                    started_at: 0,
                    completed_at: 0,
                    monitoring_enabled: false,
                },
                SubagentResult {
                    agent_id: "a2".to_string(),
                    agent_type: "coder".to_string(),
                    status: ExecutionStatus::Success,
                    summary: "Success 2".to_string(),
                    full_output: "".to_string(),
                    resumed: false,
                    steps_taken: 3,
                    elapsed_ms: 800,
                    started_at: 0,
                    completed_at: 0,
                    monitoring_enabled: false,
                },
            ],
            failures: vec![],
            total_elapsed_ms: 1000,
            actual_concurrency: 2,
            aggregated_summary: None,
        };

        assert!(result.all_succeeded());
        assert_eq!(result.success_rate(), 1.0);
        assert!(result.merged_output().contains("Success 1"));
        assert!(result.merged_output().contains("Success 2"));
    }

    #[tokio::test]
    async fn test_parallel_execution_empty_batch() {
        let (runner, temp_dir) = create_test_runner();
        let task_manager = BackgroundTaskManager::new(
            temp_dir.path().join("tasks"),
            temp_dir.path().join("work"),
            temp_dir.path().join("context"),
        );

        let mut executor = ParallelExecutor::new(task_manager, runner);
        let batch = SubagentBatch::new();

        let result = executor.execute(batch, None, None).await.unwrap();
        assert!(result.results.is_empty());
        assert!(result.failures.is_empty());
    }
}
