//! Subagent Handle - 非阻塞子代理 Spawn + Poll/Join
//!
//! 提供 [`SubagentHandle`]：`spawn` 立即返回，子代理在后台 tokio task 中执行；
//! 父代理可以在自己的 turn 中通过 [`SubagentHandle::poll`] 非阻塞查询状态/部分进度，
//! 或在方便时通过 [`SubagentHandle::join`] 等待最终结果。
//!
//! 完成通知挂点：spawn 时可传入 [`CompletionCallback`]，后台任务结束时回调收到
//! [`SubagentCompletion`]，调用方（如 core 侧 Agent 循环）可用
//! `SubagentCompletion::to_system_message()` 把结果注入父对话上下文。
//! 回调在 tokio task 内同步调用，应保持轻量（如写入 channel）。

use crate::runner::SubagentRunner;
use crate::store::SubagentStore;
use clarity_contract::subagent::{
    RunSpec, SubagentCompletion, SubagentError, SubagentProgressEvent, SubagentResult,
};
use parking_lot::Mutex;
use std::sync::Arc;
use tokio::sync::Notify;

/// 子代理完成回调：后台任务结束时恰好调用一次。
///
/// 用于完成通知挂点；core 侧可传入闭包，把
/// `SubagentCompletion::to_system_message()` 作为 system message 注入父上下文。
pub type CompletionCallback = Box<dyn FnOnce(SubagentCompletion) + Send + 'static>;

/// [`SubagentHandle::poll`] 的非阻塞状态快照。
#[derive(Debug, Clone)]
pub enum SubagentPoll {
    /// 仍在运行；附带目前已观察到的部分进度。
    Running {
        /// 代理 ID（runner 发出首个状态事件后可用）。
        agent_id: Option<String>,
        /// 最近观察到的进度事件（如有）。
        latest_event: Option<SubagentProgressEvent>,
    },
    /// 已成功完成。
    Completed(SubagentResult),
    /// 已失败结束。
    Failed(SubagentError),
}

#[derive(Debug, Default)]
struct HandleShared {
    result: Option<Result<SubagentResult, SubagentError>>,
    agent_id: Option<String>,
    latest_event: Option<SubagentProgressEvent>,
}

/// 后台运行子代理的句柄。
///
/// 由 [`SubagentHandle::spawn`] 创建；子代理在独立 tokio task 中执行，
/// 不阻塞调用方。句柄可安全地在 await 点之间持有（内部锁不跨 await）。
pub struct SubagentHandle {
    description: String,
    shared: Arc<Mutex<HandleShared>>,
    notify: Arc<Notify>,
}

impl SubagentHandle {
    /// 在后台 spawn 子代理并立即返回句柄。
    pub fn spawn(runner: SubagentRunner, spec: RunSpec) -> Self {
        Self::spawn_inner(runner, spec, None)
    }

    /// 在后台 spawn 子代理，完成时调用 `on_complete` 回调（完成通知挂点）。
    pub fn spawn_with_callback(
        runner: SubagentRunner,
        spec: RunSpec,
        on_complete: CompletionCallback,
    ) -> Self {
        Self::spawn_inner(runner, spec, Some(on_complete))
    }

    fn spawn_inner(
        runner: SubagentRunner,
        spec: RunSpec,
        on_complete: Option<CompletionCallback>,
    ) -> Self {
        let description = spec.description.clone();
        let shared = Arc::new(Mutex::new(HandleShared::default()));
        let notify = Arc::new(Notify::new());

        // 内部 progress channel：drainer task 把 runner 的进度事件镜像到共享状态，
        // 供 poll() 报告部分进度。
        let (progress_tx, mut progress_rx) =
            tokio::sync::mpsc::channel::<SubagentProgressEvent>(64);
        let runner = runner.with_progress_tx(progress_tx);
        let mut store = SubagentStore::new(runner.working_dir().join("spawned_store"));

        {
            let shared = Arc::clone(&shared);
            tokio::spawn(async move {
                while let Some(event) = progress_rx.recv().await {
                    let mut s = shared.lock();
                    if let SubagentProgressEvent::StatusChange { agent_id, .. } = &event {
                        s.agent_id = Some(agent_id.clone());
                    }
                    s.latest_event = Some(event);
                }
            });
        }

        {
            let shared = Arc::clone(&shared);
            let notify = Arc::clone(&notify);
            let description = description.clone();
            tokio::spawn(async move {
                let result = runner.run(spec, &mut store, None).await;
                {
                    let mut s = shared.lock();
                    s.result = Some(result.clone());
                }
                // Notify::notify_one 在无等待者时保留一个 permit，join 不会错过唤醒。
                notify.notify_one();
                if let Some(callback) = on_complete {
                    callback(SubagentCompletion {
                        description,
                        result,
                    });
                }
            });
        }

        Self {
            description,
            shared,
            notify,
        }
    }

    /// 来自原始 [`RunSpec`] 的任务描述。
    pub fn description(&self) -> &str {
        &self.description
    }

    /// 代理 ID（run 启动后可用）。
    pub fn agent_id(&self) -> Option<String> {
        self.shared.lock().agent_id.clone()
    }

    /// 后台任务是否已结束。
    pub fn is_finished(&self) -> bool {
        self.shared.lock().result.is_some()
    }

    /// 非阻塞查询当前状态/部分进度。
    pub fn poll(&self) -> SubagentPoll {
        let s = self.shared.lock();
        match &s.result {
            Some(Ok(r)) => SubagentPoll::Completed(r.clone()),
            Some(Err(e)) => SubagentPoll::Failed(e.clone()),
            None => SubagentPoll::Running {
                agent_id: s.agent_id.clone(),
                latest_event: s.latest_event.clone(),
            },
        }
    }

    /// 等待后台任务结束并返回最终结果。
    ///
    /// 可多次调用（结果按值克隆返回），也可与 `poll()` 混用。
    pub async fn join(&self) -> Result<SubagentResult, SubagentError> {
        loop {
            if let Some(result) = self.shared.lock().result.clone() {
                return result;
            }
            self.notify.notified().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clarity_contract::error::AgentError;
    use clarity_contract::llm::{LlmProvider, LlmResponse};
    use clarity_contract::subagent::ExecutionStatus;
    use clarity_contract::{Message, StreamDelta};
    use clarity_core::registry::ToolRegistry;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;
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

    fn test_spec(description: &str) -> RunSpec {
        RunSpec::new(description, "Do something")
            .with_type("coder")
            .without_git_context()
    }

    /// 长响应内容（>= 100 字符，避免触发 runner 的 continuation 重试）。
    const LONG_RESPONSE: &str =
        "This is a sufficiently long mock response from the slow LLM provider used in tests.";

    /// 慢速 mock LLM：complete 前 sleep，并统计并发调用峰值。
    struct SlowLlm {
        delay: Duration,
        active: Arc<AtomicUsize>,
        max_active: Arc<AtomicUsize>,
    }

    #[async_trait::async_trait]
    impl LlmProvider for SlowLlm {
        async fn complete(
            &self,
            _messages: &[Message],
            _tools: &serde_json::Value,
        ) -> Result<LlmResponse, AgentError> {
            let current = self.active.fetch_add(1, Ordering::SeqCst) + 1;
            self.max_active.fetch_max(current, Ordering::SeqCst);
            tokio::time::sleep(self.delay).await;
            self.active.fetch_sub(1, Ordering::SeqCst);
            Ok(LlmResponse {
                content: LONG_RESPONSE.to_string(),
                tool_calls: vec![],
                is_complete: true,
            })
        }

        fn stream(
            &self,
            _messages: &[Message],
            _tools: &serde_json::Value,
        ) -> Result<tokio::sync::mpsc::Receiver<Result<StreamDelta, AgentError>>, AgentError>
        {
            let (_tx, rx) = tokio::sync::mpsc::channel(1);
            Ok(rx)
        }

        fn set_prompt_cache_key(&self, _key: &str) {}
    }

    #[tokio::test]
    async fn test_spawn_does_not_block() {
        let (runner, _dir) = create_test_runner();
        let runner = runner.with_llm(Arc::new(SlowLlm {
            delay: Duration::from_millis(300),
            active: Arc::new(AtomicUsize::new(0)),
            max_active: Arc::new(AtomicUsize::new(0)),
        }));

        // spawn 是同步函数：返回时后台任务尚未跑完 300ms 的 LLM 调用。
        let handle = SubagentHandle::spawn(runner, test_spec("bg task"));
        assert_eq!(handle.description(), "bg task");
        assert!(!handle.is_finished());

        let result = tokio::time::timeout(Duration::from_secs(10), handle.join())
            .await
            .expect("join timed out")
            .expect("run failed");
        assert_eq!(result.status, ExecutionStatus::Success);
        assert!(handle.is_finished());
    }

    #[tokio::test]
    async fn test_poll_running_then_completed() {
        let (runner, _dir) = create_test_runner();
        let runner = runner.with_llm(Arc::new(SlowLlm {
            delay: Duration::from_millis(300),
            active: Arc::new(AtomicUsize::new(0)),
            max_active: Arc::new(AtomicUsize::new(0)),
        }));

        let handle = SubagentHandle::spawn(runner, test_spec("poll task"));

        // 完成前：poll 返回 Running，且最近进度事件可用。
        match handle.poll() {
            SubagentPoll::Running { .. } => {}
            other => panic!("expected Running, got {:?}", other),
        }
        // 轮询等待进度事件到达（而非固定 sleep，避免时序脆弱）。
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let mut saw_partial_progress = false;
        while std::time::Instant::now() < deadline {
            match handle.poll() {
                SubagentPoll::Running {
                    agent_id: Some(_),
                    latest_event: Some(_),
                } => {
                    saw_partial_progress = true;
                    break;
                }
                SubagentPoll::Running { .. } => {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                other => panic!("expected Running, got {:?}", other),
            }
        }
        assert!(
            saw_partial_progress,
            "no progress event observed while running"
        );

        let result = handle.join().await.expect("run failed");
        match handle.poll() {
            SubagentPoll::Completed(r) => assert_eq!(r.agent_id, result.agent_id),
            other => panic!("expected Completed, got {:?}", other),
        }
        // agent_id() 在完成后仍可查询。
        assert_eq!(handle.agent_id().as_deref(), Some(result.agent_id.as_str()));
    }

    #[tokio::test]
    async fn test_join_returns_result_and_is_repeatable() {
        let (runner, _dir) = create_test_runner();
        let runner = runner.with_llm(Arc::new(clarity_core::agent::MockLlm));

        let handle = SubagentHandle::spawn(runner, test_spec("join task"));
        let first = handle.join().await.expect("run failed");
        assert_eq!(first.status, ExecutionStatus::Success);
        assert!(!first.summary.is_empty());

        // join 可重复调用，返回同一结果。
        let second = handle.join().await.expect("run failed");
        assert_eq!(first.agent_id, second.agent_id);
        assert_eq!(first.summary, second.summary);
    }

    #[tokio::test]
    async fn test_join_failure_unknown_agent_type() {
        let (runner, _dir) = create_test_runner();
        let runner = runner.with_llm(Arc::new(clarity_core::agent::MockLlm));

        let spec = test_spec("bad task").with_type("nonexistent-type");
        let handle = SubagentHandle::spawn(runner, spec);

        let err = handle.join().await.expect_err("should fail");
        assert!(matches!(err, SubagentError::UnknownAgentType(_)));
        match handle.poll() {
            SubagentPoll::Failed(SubagentError::UnknownAgentType(_)) => {}
            other => panic!("expected Failed(UnknownAgentType), got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_multiple_handles_run_concurrently() {
        let active = Arc::new(AtomicUsize::new(0));
        let max_active = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();
        let mut _dirs = Vec::new();
        for i in 0..3 {
            let (runner, dir) = create_test_runner();
            _dirs.push(dir);
            let runner = runner.with_llm(Arc::new(SlowLlm {
                delay: Duration::from_millis(300),
                active: Arc::clone(&active),
                max_active: Arc::clone(&max_active),
            }));
            handles.push(SubagentHandle::spawn(
                runner,
                test_spec(&format!("task {}", i)),
            ));
        }

        let (r0, r1, r2) = tokio::time::timeout(Duration::from_secs(10), async {
            tokio::join!(handles[0].join(), handles[1].join(), handles[2].join())
        })
        .await
        .expect("joins timed out");

        for r in [r0, r1, r2] {
            assert_eq!(r.expect("run failed").status, ExecutionStatus::Success);
        }
        // 三个 300ms 的 LLM 调用若串行峰值并发为 1；真正并发时峰值 >= 2。
        assert!(
            max_active.load(Ordering::SeqCst) >= 2,
            "expected concurrent execution, max_active = {}",
            max_active.load(Ordering::SeqCst)
        );
    }

    #[tokio::test]
    async fn test_completion_callback_fires_once() {
        let (runner, _dir) = create_test_runner();
        let runner = runner.with_llm(Arc::new(clarity_core::agent::MockLlm));

        let (tx, mut rx) = tokio::sync::mpsc::channel::<SubagentCompletion>(1);
        let handle = SubagentHandle::spawn_with_callback(
            runner,
            test_spec("callback task"),
            Box::new(move |completion| {
                let _ = tx.try_send(completion);
            }),
        );

        let completion = tokio::time::timeout(Duration::from_secs(10), rx.recv())
            .await
            .expect("callback timed out")
            .expect("channel closed");
        assert_eq!(completion.description, "callback task");
        assert!(completion.result.is_ok());

        // system message 文本可用于注入父上下文。
        let msg = completion.to_system_message();
        assert!(msg.contains("[subagent completed]"));
        assert!(msg.contains("callback task"));

        // 回调触发时 handle 已处于完成状态。
        assert!(handle.is_finished());
    }

    #[tokio::test]
    async fn test_completion_to_system_message_on_failure() {
        let completion = SubagentCompletion {
            description: "failing task".to_string(),
            result: Err(SubagentError::Cancelled),
        };
        let msg = completion.to_system_message();
        assert!(msg.contains("[subagent failed]"));
        assert!(msg.contains("failing task"));
    }
}
