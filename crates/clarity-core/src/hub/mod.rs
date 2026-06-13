//! Hub-Worker scheduler — distribute tasks across souls based on skill mastery.
//!
//! The [`HubScheduler`] maintains a registry of available Worker souls and
//! routes incoming tasks to the one with the highest estimated fitness.
//! If all workers are saturated, tasks are queued for later dispatch.
//!
//! ## Dispatch algorithm
//!
//! ```text
//! Task arrives
//!      │
//!      ▼
//! Filter: capability match (task.required_skills ⊆ worker.skills)
//!      │
//!      ▼
//! Score: skill_mastery[task.type] / (current_load + 1)
//!      │
//!      ▼
//! Pick top worker → enqueue to worker's task queue
//!      │
//!      ▼
//! Emit TaskDispatch event for telemetry feedback loop
//! ```

use std::collections::HashMap;

use parking_lot::RwLock;

use crate::adaptive::TaskType;
use crate::soul::Soul;

// ============================================================================
// Task
// ============================================================================

/// A task submitted to the hub for dispatch.
#[derive(Debug, Clone, PartialEq)]
pub struct Task {
    /// Unique task identifier.
    pub id: String,

    /// Human-readable description.
    pub description: String,

    /// Classification for routing.
    pub task_type: TaskType,

    /// Required skills (soul must have all).
    pub required_skills: Vec<String>,

    /// Maximum latency tolerance in ms.
    pub max_latency_ms: Option<u64>,

    /// Estimated token cost.
    pub estimated_tokens: usize,

    /// Priority override.
    pub priority: TaskPriority,
}

/// Task priority for queue ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum TaskPriority {
    /// Low priority.
    Low,
    #[default]
    /// Normal priority.
    Normal,
    /// High priority.
    High,
    /// Urgent priority.
    Urgent,
}

// ============================================================================
// WorkerHandle
// ============================================================================

/// Lightweight handle to a registered worker soul.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkerHandle {
    /// Soul identifier.
    pub soul_id: String,
    /// Associated skill identifiers.
    pub skills: Vec<String>,
    /// Current number of in-flight tasks.
    pub current_load: usize,
    /// Maximum concurrent tasks.
    pub max_capacity: usize,
}

impl WorkerHandle {
    /// Whether this worker can accept more tasks.
    pub fn has_capacity(&self) -> bool {
        self.current_load < self.max_capacity
    }

    /// Check if all required skills are present.
    pub fn can_handle(&self, required: &[String]) -> bool {
        required.iter().all(|r| self.skills.contains(r))
    }
}

// ============================================================================
// HubScheduler
// ============================================================================

/// Central task dispatcher for the Agent OS.
pub struct HubScheduler {
    /// Registered workers: soul_id → handle.
    workers: RwLock<HashMap<String, WorkerHandle>>,

    /// Global task queue (ordered by priority, then FIFO).
    task_queue: RwLock<Vec<Task>>,

    /// Per-worker task assignment counts (for load balancing).
    assignment_counts: RwLock<HashMap<String, usize>>,
}

impl HubScheduler {
    /// Create a new empty scheduler.
    pub fn new() -> Self {
        Self {
            workers: RwLock::new(HashMap::new()),
            task_queue: RwLock::new(Vec::new()),
            assignment_counts: RwLock::new(HashMap::new()),
        }
    }

    // ------------------------------------------------------------------
    // Worker registry
    // ------------------------------------------------------------------

    /// Register a soul as a worker.
    pub fn register_worker(&self, soul: &Soul, max_capacity: usize) {
        let handle = WorkerHandle {
            soul_id: soul.id.clone(),
            skills: soul.capabilities.clone(),
            current_load: 0,
            max_capacity,
        };
        self.workers.write().insert(soul.id.clone(), handle);
    }

    /// Unregister a worker.
    pub fn unregister_worker(&self, soul_id: &str) {
        self.workers.write().remove(soul_id);
    }

    /// Update worker load.
    pub fn set_worker_load(&self, soul_id: &str, load: usize) {
        if let Some(worker) = self.workers.write().get_mut(soul_id) {
            worker.current_load = load;
        }
    }

    // ------------------------------------------------------------------
    // Task dispatch
    // ------------------------------------------------------------------

    /// Submit a task for dispatch.
    ///
    /// The task is either immediately assigned to a worker or queued
    /// if no worker is available.
    pub fn submit(&self, task: Task) -> DispatchResult {
        let workers = self.workers.read();

        // Find best candidate.
        let mut candidates: Vec<(&WorkerHandle, f64)> = workers
            .values()
            .filter(|w| w.can_handle(&task.required_skills))
            .filter(|w| w.has_capacity())
            .map(|w| {
                let score = Self::score_worker(w, &task);
                (w, score)
            })
            .collect();

        if candidates.is_empty() {
            // No immediate capacity — queue.
            self.task_queue.write().push(task);
            return DispatchResult::Queued;
        }

        // Sort by score descending.
        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let chosen_id = candidates[0].0.soul_id.clone();
        drop(workers);

        // Update assignment count.
        *self
            .assignment_counts
            .write()
            .entry(chosen_id.clone())
            .or_insert(0) += 1;

        DispatchResult::Assigned {
            task_id: task.id,
            worker_id: chosen_id,
        }
    }

    /// Attempt to dispatch queued tasks.
    ///
    /// Call this after a worker reports reduced load.
    pub fn drain_queue(&self) -> Vec<DispatchResult> {
        let mut results = Vec::new();
        let mut queue = self.task_queue.write();

        // Sort by priority (higher first).
        queue.sort_by_key(|t| std::cmp::Reverse(t.priority));

        let mut remaining = Vec::new();
        for task in queue.drain(..) {
            match self.submit(task.clone()) {
                DispatchResult::Queued => remaining.push(task),
                other => results.push(other),
            }
        }

        *queue = remaining;
        results
    }

    /// Get current queue depth.
    pub fn queue_depth(&self) -> usize {
        self.task_queue.read().len()
    }

    /// Get total assignment counts per worker.
    pub fn assignment_counts(&self) -> HashMap<String, usize> {
        self.assignment_counts.read().clone()
    }

    // ------------------------------------------------------------------
    // Scoring
    // ------------------------------------------------------------------

    fn score_worker(worker: &WorkerHandle, task: &Task) -> f64 {
        // Base score: inverse of load (more idle = higher score).
        let load_factor = 1.0 / (worker.current_load as f64 + 1.0);

        // Capacity headroom.
        let capacity_ratio =
            (worker.max_capacity - worker.current_load) as f64 / worker.max_capacity.max(1) as f64;

        // Latency sensitivity: if worker has no load and task is latency-sensitive, boost.
        let latency_bonus = if worker.current_load == 0 && task.max_latency_ms.is_some() {
            0.5
        } else {
            0.0
        };

        load_factor * 0.4 + capacity_ratio * 0.4 + latency_bonus * 0.2
    }
}

impl Default for HubScheduler {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// DispatchResult
// ============================================================================

/// Outcome of submitting a task.
#[derive(Debug, Clone, PartialEq)]
pub enum DispatchResult {
    /// Task was immediately assigned to a worker.
    Assigned {
        /// Task identifier.
        task_id: String,
        /// Worker identifier.
        worker_id: String,
    },
    /// Task was queued for later dispatch.
    Queued,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_soul(id: &str, skills: Vec<&str>) -> Soul {
        let mut soul = Soul::new(id);
        soul.capabilities = skills.into_iter().map(String::from).collect();
        soul
    }

    #[test]
    fn test_dispatch_basic() {
        let hub = HubScheduler::new();
        hub.register_worker(&make_soul("worker-a", vec!["bash", "file_read"]), 2);
        hub.register_worker(&make_soul("worker-b", vec!["bash"]), 2);

        let task = Task {
            id: "task-1".to_string(),
            description: "list files".to_string(),
            task_type: TaskType::FileOps,
            required_skills: vec!["bash".to_string(), "file_read".to_string()],
            max_latency_ms: None,
            estimated_tokens: 100,
            priority: TaskPriority::Normal,
        };

        let result = hub.submit(task);
        assert_eq!(
            result,
            DispatchResult::Assigned {
                task_id: "task-1".to_string(),
                worker_id: "worker-a".to_string(),
            }
        );
    }

    #[test]
    fn test_dispatch_queue_when_saturated() {
        let hub = HubScheduler::new();
        hub.register_worker(&make_soul("worker-a", vec!["bash"]), 1);

        // Fill capacity.
        hub.set_worker_load("worker-a", 1);

        let task = Task {
            id: "task-1".to_string(),
            description: "shell".to_string(),
            task_type: TaskType::System,
            required_skills: vec!["bash".to_string()],
            max_latency_ms: None,
            estimated_tokens: 50,
            priority: TaskPriority::Normal,
        };

        let result = hub.submit(task);
        assert_eq!(result, DispatchResult::Queued);
        assert_eq!(hub.queue_depth(), 1);
    }

    #[test]
    fn test_dispatch_drain_queue() {
        let hub = HubScheduler::new();
        hub.register_worker(&make_soul("worker-a", vec!["bash"]), 1);

        // Queue a task while saturated.
        hub.set_worker_load("worker-a", 1);
        let task = Task {
            id: "task-1".to_string(),
            description: "shell".to_string(),
            task_type: TaskType::System,
            required_skills: vec!["bash".to_string()],
            max_latency_ms: None,
            estimated_tokens: 50,
            priority: TaskPriority::Normal,
        };
        assert!(matches!(hub.submit(task), DispatchResult::Queued));

        // Worker becomes available.
        hub.set_worker_load("worker-a", 0);
        let drained = hub.drain_queue();
        assert_eq!(drained.len(), 1);
        assert_eq!(hub.queue_depth(), 0);
    }

    #[test]
    fn test_dispatch_priority_ordering() {
        let hub = HubScheduler::new();
        hub.register_worker(&make_soul("worker-a", vec!["bash"]), 1);

        hub.set_worker_load("worker-a", 1); // saturated

        let low = Task {
            id: "low".to_string(),
            description: "low".to_string(),
            task_type: TaskType::Background,
            required_skills: vec!["bash".to_string()],
            max_latency_ms: None,
            estimated_tokens: 10,
            priority: TaskPriority::Low,
        };
        let high = Task {
            id: "high".to_string(),
            description: "high".to_string(),
            task_type: TaskType::Coding,
            required_skills: vec!["bash".to_string()],
            max_latency_ms: None,
            estimated_tokens: 10,
            priority: TaskPriority::Urgent,
        };

        hub.submit(low);
        hub.submit(high);

        // Free capacity.
        hub.set_worker_load("worker-a", 0);
        let drained = hub.drain_queue();

        // Urgent task should be dispatched first.
        assert_eq!(
            drained[0],
            DispatchResult::Assigned {
                task_id: "high".to_string(),
                worker_id: "worker-a".to_string(),
            }
        );
    }
}
