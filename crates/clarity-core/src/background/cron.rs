//! Cron Task Scheduler
//!
//! Provides cron-expression-based scheduling for recurring agent tasks.

use chrono::{DateTime, Utc};
use cron::Schedule;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

use crate::background::store::TaskSpec;

/// Error type for cron operations
#[derive(Debug, thiserror::Error)]
pub enum CronError {
    /// Invalid cron expression
    #[error("Invalid cron expression: {0}")]
    InvalidExpression(String),
    /// Could not calculate next run time
    #[error("Could not calculate next run time")]
    NextRunCalculationFailed,
}

/// A parsed cron schedule with its next scheduled execution time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronSchedule {
    /// The raw cron expression string
    pub expr: String,
    /// The next scheduled execution time
    pub next_run: DateTime<Utc>,
}

impl CronSchedule {
    /// Parse a cron expression and compute the first upcoming run time.
    pub fn new(expr: &str) -> Result<Self, CronError> {
        let schedule =
            Schedule::from_str(expr).map_err(|e| CronError::InvalidExpression(e.to_string()))?;

        let next_run = schedule
            .upcoming(Utc)
            .next()
            .ok_or(CronError::NextRunCalculationFailed)?;

        Ok(Self {
            expr: expr.to_string(),
            next_run,
        })
    }

    /// Recalculate the next run time after the given timestamp.
    pub fn compute_next(&mut self, after: DateTime<Utc>) -> DateTime<Utc> {
        if let Ok(schedule) = Schedule::from_str(&self.expr) {
            if let Some(next) = schedule.after(&after).next() {
                self.next_run = next;
            }
        }
        self.next_run
    }
}

/// A single cron-scheduled task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronTask {
    /// The task specification to execute
    pub task_spec: TaskSpec,
    /// The schedule metadata
    pub schedule: CronSchedule,
    /// Unique task identifier
    pub task_id: String,
    /// Whether the task is enabled
    pub enabled: bool,
}

/// In-memory cron scheduler that tracks recurring tasks
#[derive(Debug, Clone, Default)]
pub struct CronScheduler {
    tasks: Vec<CronTask>,
}

impl CronScheduler {
    /// Create a new empty cron scheduler
    pub fn new() -> Self {
        Self { tasks: Vec::new() }
    }

    /// Add a new cron task.
    ///
    /// Parses the cron expression, calculates the next run time, and stores
    /// the task. Returns the generated `task_id`.
    pub fn add_task(&mut self, task_spec: TaskSpec, cron_expr: &str) -> Result<String, CronError> {
        let schedule = CronSchedule::new(cron_expr)?;

        let task_id = format!(
            "cron_{}",
            &uuid::Uuid::new_v4().to_string().replace('-', "")[..12]
        );

        let cron_task = CronTask {
            task_spec,
            schedule,
            task_id: task_id.clone(),
            enabled: true,
        };

        self.tasks.push(cron_task);
        Ok(task_id)
    }

    /// Evaluate all tasks against the current time.
    ///
    /// Returns the [`TaskSpec`]s of every enabled task whose `next_run` has passed,
    /// and advances each triggered task to its subsequent occurrence.
    pub fn tick(&mut self, now: DateTime<Utc>) -> Vec<TaskSpec> {
        let mut due = Vec::new();

        for task in &mut self.tasks {
            if task.enabled && now >= task.schedule.next_run {
                due.push(task.task_spec.clone());
                task.schedule.compute_next(now);
            }
        }

        due
    }

    /// Return a read-only view of all tracked tasks
    pub fn tasks(&self) -> &[CronTask] {
        &self.tasks
    }

    /// Remove a task by its id. Returns `true` if a task was removed.
    pub fn remove_task(&mut self, task_id: &str) -> bool {
        let len = self.tasks.len();
        self.tasks.retain(|t| t.task_id != task_id);
        self.tasks.len() < len
    }

    /// Enable or disable a task by its id.
    /// Returns `true` if the task was found and its state changed.
    pub fn set_enabled(&mut self, task_id: &str, enabled: bool) -> bool {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.task_id == task_id) {
            task.enabled = enabled;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::background::store::TaskSpec;

    #[test]
    fn test_add_task_valid_cron() {
        let mut scheduler = CronScheduler::new();
        let spec = TaskSpec::new("daily_backup", "Run backup");
        let result = scheduler.add_task(spec, "0 0 2 * * *");
        assert!(result.is_ok());
        let task_id = result.unwrap();
        assert!(task_id.starts_with("cron_"));
        assert_eq!(scheduler.tasks().len(), 1);
    }

    #[test]
    fn test_add_task_invalid_cron() {
        let mut scheduler = CronScheduler::new();
        let spec = TaskSpec::new("bad", "test");
        let result = scheduler.add_task(spec, "not a cron");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            CronError::InvalidExpression(_)
        ));
    }

    #[test]
    fn test_tick_no_tasks_due() {
        let mut scheduler = CronScheduler::new();
        let spec = TaskSpec::new("future", "test");
        let _ = scheduler.add_task(spec.clone(), "0 0 2 * * *").unwrap();

        // Use a time far in the past so nothing is due
        let now = Utc::now() - chrono::Duration::hours(1);
        let due = scheduler.tick(now);
        assert!(due.is_empty());
    }

    #[test]
    fn test_tick_task_due() {
        let mut scheduler = CronScheduler::new();
        let spec = TaskSpec::new("every_minute", "test");
        let _ = scheduler.add_task(spec.clone(), "* * * * * *").unwrap();

        // Use a time far in the future so the task is due
        let now = Utc::now() + chrono::Duration::hours(1);
        let due = scheduler.tick(now);
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].name, "every_minute");
    }

    #[test]
    fn test_tick_updates_next_run() {
        let mut scheduler = CronScheduler::new();
        let spec = TaskSpec::new("hourly", "test");
        let _ = scheduler.add_task(spec, "0 0 * * * *").unwrap();

        let original_next_run = scheduler.tasks()[0].schedule.next_run;

        // Use a time far enough in the future to guarantee the task is due.
        // Compute `now` immediately after `add_task` to avoid clock drift.
        let now = Utc::now() + chrono::Duration::days(1);
        let due = scheduler.tick(now);

        // Ensure the task actually triggered; if it didn't, next_run won't change.
        assert!(
            !due.is_empty(),
            "task should be due by now; original_next_run={}",
            original_next_run
        );

        let new_next_run = scheduler.tasks()[0].schedule.next_run;
        assert!(
            new_next_run > original_next_run,
            "next_run should have advanced: original={}, new={}",
            original_next_run,
            new_next_run
        );
    }

    #[test]
    fn test_remove_task() {
        let mut scheduler = CronScheduler::new();
        let spec = TaskSpec::new("removable", "test");
        let task_id = scheduler.add_task(spec, "0 0 * * * *").unwrap();

        assert!(scheduler.remove_task(&task_id));
        assert!(!scheduler.remove_task(&task_id));
        assert!(scheduler.tasks().is_empty());
    }

    #[test]
    fn test_set_enabled() {
        let mut scheduler = CronScheduler::new();
        let spec = TaskSpec::new("toggle", "test");
        let task_id = scheduler.add_task(spec, "0 0 * * * *").unwrap();

        assert!(scheduler.tasks()[0].enabled);
        assert!(scheduler.set_enabled(&task_id, false));
        assert!(!scheduler.tasks()[0].enabled);
        assert!(!scheduler.set_enabled("nonexistent", false));
    }

    #[test]
    fn test_tick_skips_disabled_task() {
        let mut scheduler = CronScheduler::new();
        let spec = TaskSpec::new("disabled", "test");
        let task_id = scheduler.add_task(spec.clone(), "* * * * * *").unwrap();
        scheduler.set_enabled(&task_id, false);

        let now = Utc::now() + chrono::Duration::hours(1);
        let due = scheduler.tick(now);
        assert!(due.is_empty());
    }

    #[test]
    fn test_cron_schedule_new() {
        let schedule = CronSchedule::new("0 0 2 * * *");
        assert!(schedule.is_ok());
        let s = schedule.unwrap();
        assert_eq!(s.expr, "0 0 2 * * *");
        assert!(s.next_run > Utc::now());
    }

    #[test]
    fn test_cron_schedule_compute_next() {
        let mut schedule = CronSchedule::new("0 0 * * * *").unwrap();
        let original = schedule.next_run;
        let next = schedule.compute_next(Utc::now());
        assert!(next >= original);
    }
}
