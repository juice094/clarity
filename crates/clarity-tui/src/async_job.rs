use std::fmt::Debug;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};

#[derive(Clone, Debug, PartialEq)]
#[allow(dead_code)]
pub enum JobState<T: Clone + Debug + PartialEq> {
    Pending,
    Running,
    Done(Result<T, String>),
}

#[allow(dead_code)]
pub struct AsyncSingleJob<T: Clone + Debug + PartialEq + Send + 'static> {
    state: JobState<T>,
    rx: Option<UnboundedReceiver<JobState<T>>>,
}

#[allow(dead_code)]
impl<T: Clone + Debug + PartialEq + Send + 'static> AsyncSingleJob<T> {
    pub fn new() -> Self {
        Self {
            state: JobState::Pending,
            rx: None,
        }
    }

    pub fn state(&self) -> &JobState<T> {
        &self.state
    }

    pub fn is_running(&self) -> bool {
        matches!(self.state, JobState::Running)
    }

    pub fn spawn<F>(&mut self, work: F)
    where
        F: FnOnce() -> T + Send + 'static,
    {
        self.state = JobState::Running;
        let (tx, rx) = unbounded_channel();
        self.rx = Some(rx);
        tokio::spawn(async move {
            let result = work();
            let _ = tx.send(JobState::Done(Ok(result)));
        });
    }

    pub fn spawn_fallible<F>(&mut self, work: F)
    where
        F: FnOnce() -> Result<T, String> + Send + 'static,
    {
        self.state = JobState::Running;
        let (tx, rx) = unbounded_channel();
        self.rx = Some(rx);
        tokio::spawn(async move {
            let result = work();
            let _ = tx.send(JobState::Done(result));
        });
    }

    pub fn check(&mut self) {
        if let Some(ref mut rx) = self.rx {
            if let Ok(state) = rx.try_recv() {
                self.state = state;
                self.rx = None;
            }
        }
    }

    pub fn take_result(&mut self) -> Option<Result<T, String>> {
        if let JobState::Done(r) = std::mem::replace(&mut self.state, JobState::Pending) {
            Some(r)
        } else {
            None
        }
    }
}

impl<T: Clone + Debug + PartialEq + Send + 'static> Default for AsyncSingleJob<T> {
    fn default() -> Self {
        Self::new()
    }
}

pub type ToolCallJob = AsyncSingleJob<String>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_async_job_initial_state() {
        let job: AsyncSingleJob<i32> = AsyncSingleJob::new();
        assert!(matches!(job.state(), JobState::Pending));
        assert!(!job.is_running());
    }

    #[tokio::test]
    async fn test_async_job_spawn_and_check() {
        let mut job: AsyncSingleJob<i32> = AsyncSingleJob::new();
        job.spawn(|| 42);
        assert!(job.is_running());
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        job.check();
        assert_eq!(job.state(), &JobState::Done(Ok(42)));
    }

    #[tokio::test]
    async fn test_async_job_take_result() {
        let mut job: AsyncSingleJob<String> = AsyncSingleJob::new();
        job.spawn(|| "hello".to_string());
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        job.check();
        let result = job.take_result();
        assert_eq!(result, Some(Ok("hello".to_string())));
        assert!(matches!(job.state(), JobState::Pending));
    }

    #[tokio::test]
    async fn test_async_job_spawn_fallible_error() {
        let mut job: AsyncSingleJob<i32> = AsyncSingleJob::new();
        job.spawn_fallible(|| Err("oops".to_string()));
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        job.check();
        assert_eq!(job.state(), &JobState::Done(Err("oops".to_string())));
    }
}
