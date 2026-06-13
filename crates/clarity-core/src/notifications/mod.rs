//! Notification system for Clarity
//!
//! This module provides a broadcast-based notification system for task lifecycle events,
//! tool execution results, and approval requests.

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

/// Maximum number of notifications buffered in the broadcast channel
const CHANNEL_CAPACITY: usize = 100;

/// Represents different types of notifications in the Clarity system
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Notification {
    /// A task has started execution
    TaskStarted {
        /// Unique identifier for the task
        task_id: String,
    },

    /// Task progress update with percentage completion
    TaskProgress {
        /// Unique identifier for the task
        task_id: String,
        /// Percentage of completion (0-100)
        percent: u8,
    },

    /// Task completed successfully with result
    TaskCompleted {
        /// Unique identifier for the task
        task_id: String,
        /// Result of the task execution
        result: String,
    },

    /// Task failed with error message
    TaskFailed {
        /// Unique identifier for the task
        task_id: String,
        /// Error message describing the failure
        error: String,
    },

    /// A tool has been executed
    ToolExecuted {
        /// Name of the tool that was executed
        tool_name: String,
        /// Result of the tool execution
        result: String,
    },

    /// Approval is required before proceeding
    ApprovalRequired {
        /// Unique identifier for the approval request
        request_id: String,
        /// Name of the tool requiring approval
        tool_name: String,
    },
}

/// Manager for broadcasting notifications to multiple subscribers
#[derive(Debug, Clone)]
pub struct NotificationManager {
    sender: broadcast::Sender<Notification>,
}

impl Default for NotificationManager {
    fn default() -> Self {
        Self::new()
    }
}

impl NotificationManager {
    /// Creates a new NotificationManager with a broadcast channel
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(CHANNEL_CAPACITY);
        Self { sender }
    }

    /// Subscribes to receive notifications
    ///
    /// Returns a receiver that will receive all notifications published after subscription
    pub fn subscribe(&self) -> broadcast::Receiver<Notification> {
        self.sender.subscribe()
    }

    /// Publishes a notification to all subscribers
    pub fn publish(&self, notification: Notification) {
        let _ = self.sender.send(notification);
    }

    /// Broadcasts a notification to all subscribers
    ///
    /// This is an alias for `publish` for API consistency
    pub fn broadcast(&self, notification: Notification) {
        self.publish(notification);
    }
}

/// 创建任务状态变更通知的辅助函数
pub fn task_status_notification(
    task_id: impl Into<String>,
    _task_name: impl Into<String>,
    status: impl Into<String>,
) -> Notification {
    let task_id = task_id.into();
    let status = status.into();

    match status.as_str() {
        "pending" => Notification::TaskStarted { task_id },
        "running" => Notification::TaskStarted { task_id },
        "completed" => Notification::TaskCompleted {
            task_id,
            result: "Task completed successfully".to_string(),
        },
        "failed" => Notification::TaskFailed {
            task_id,
            error: "Task failed".to_string(),
        },
        "cancelled" => Notification::TaskFailed {
            task_id,
            error: "Task cancelled".to_string(),
        },
        _ => Notification::TaskStarted { task_id },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{Duration, timeout};

    #[test]
    fn test_notification_manager_new() {
        let manager = NotificationManager::new();
        assert!(manager.sender.receiver_count() == 0);
    }

    #[test]
    fn test_notification_manager_default() {
        let manager: NotificationManager = Default::default();
        assert!(manager.sender.receiver_count() == 0);
    }

    #[test]
    fn test_subscribe() {
        let manager = NotificationManager::new();
        let _receiver = manager.subscribe();
        assert_eq!(manager.sender.receiver_count(), 1);
    }

    #[test]
    fn test_multiple_subscribers() {
        let manager = NotificationManager::new();
        let _rx1 = manager.subscribe();
        let _rx2 = manager.subscribe();
        let _rx3 = manager.subscribe();
        assert_eq!(manager.sender.receiver_count(), 3);
    }

    #[tokio::test]
    async fn test_publish_task_started() {
        let manager = NotificationManager::new();
        let mut receiver = manager.subscribe();

        let notification = Notification::TaskStarted {
            task_id: "task-123".to_string(),
        };

        manager.publish(notification.clone());

        let received = timeout(Duration::from_millis(100), receiver.recv()).await;
        assert!(received.is_ok());
        assert_eq!(received.unwrap().unwrap(), notification);
    }

    #[tokio::test]
    async fn test_broadcast_task_progress() {
        let manager = NotificationManager::new();
        let mut receiver = manager.subscribe();

        let notification = Notification::TaskProgress {
            task_id: "task-456".to_string(),
            percent: 50,
        };

        manager.broadcast(notification.clone());

        let received = timeout(Duration::from_millis(100), receiver.recv()).await;
        assert!(received.is_ok());
        assert_eq!(received.unwrap().unwrap(), notification);
    }

    #[tokio::test]
    async fn test_publish_task_completed() {
        let manager = NotificationManager::new();
        let mut receiver = manager.subscribe();

        let notification = Notification::TaskCompleted {
            task_id: "task-789".to_string(),
            result: "Success".to_string(),
        };

        manager.publish(notification.clone());

        let received = timeout(Duration::from_millis(100), receiver.recv()).await;
        assert!(received.is_ok());
        assert_eq!(received.unwrap().unwrap(), notification);
    }

    #[tokio::test]
    async fn test_publish_task_failed() {
        let manager = NotificationManager::new();
        let mut receiver = manager.subscribe();

        let notification = Notification::TaskFailed {
            task_id: "task-000".to_string(),
            error: "Something went wrong".to_string(),
        };

        manager.publish(notification.clone());

        let received = timeout(Duration::from_millis(100), receiver.recv()).await;
        assert!(received.is_ok());
        assert_eq!(received.unwrap().unwrap(), notification);
    }

    #[tokio::test]
    async fn test_publish_tool_executed() {
        let manager = NotificationManager::new();
        let mut receiver = manager.subscribe();

        let notification = Notification::ToolExecuted {
            tool_name: "shell".to_string(),
            result: "Command output".to_string(),
        };

        manager.publish(notification.clone());

        let received = timeout(Duration::from_millis(100), receiver.recv()).await;
        assert!(received.is_ok());
        assert_eq!(received.unwrap().unwrap(), notification);
    }

    #[tokio::test]
    async fn test_publish_approval_required() {
        let manager = NotificationManager::new();
        let mut receiver = manager.subscribe();

        let notification = Notification::ApprovalRequired {
            request_id: "req-abc".to_string(),
            tool_name: "dangerous_tool".to_string(),
        };

        manager.publish(notification.clone());

        let received = timeout(Duration::from_millis(100), receiver.recv()).await;
        assert!(received.is_ok());
        assert_eq!(received.unwrap().unwrap(), notification);
    }

    #[tokio::test]
    async fn test_broadcast_to_multiple_receivers() {
        let manager = NotificationManager::new();
        let mut rx1 = manager.subscribe();
        let mut rx2 = manager.subscribe();
        let mut rx3 = manager.subscribe();

        let notification = Notification::TaskStarted {
            task_id: "multi-task".to_string(),
        };

        manager.broadcast(notification.clone());

        // All receivers should get the notification
        let r1 = timeout(Duration::from_millis(100), rx1.recv())
            .await
            .unwrap()
            .unwrap();
        let r2 = timeout(Duration::from_millis(100), rx2.recv())
            .await
            .unwrap()
            .unwrap();
        let r3 = timeout(Duration::from_millis(100), rx3.recv())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(r1, notification);
        assert_eq!(r2, notification);
        assert_eq!(r3, notification);
    }

    #[tokio::test]
    async fn test_late_subscriber_misses_previous_messages() {
        let manager = NotificationManager::new();

        // Publish before subscription
        manager.publish(Notification::TaskStarted {
            task_id: "early-task".to_string(),
        });

        // Subscribe after
        let mut receiver = manager.subscribe();

        // Publish after subscription
        manager.publish(Notification::TaskStarted {
            task_id: "late-task".to_string(),
        });

        // Should only receive the late message
        let received = timeout(Duration::from_millis(100), receiver.recv()).await;
        assert!(received.is_ok());
        let notification = received.unwrap().unwrap();

        match notification {
            Notification::TaskStarted { task_id } => {
                assert_eq!(task_id, "late-task");
            }
            _ => panic!("Expected TaskStarted notification"),
        }
    }

    #[tokio::test]
    async fn test_notification_serialization() {
        let notification = Notification::TaskCompleted {
            task_id: "serialize-test".to_string(),
            result: "Done".to_string(),
        };

        let json = serde_json::to_string(&notification).unwrap();
        let deserialized: Notification = serde_json::from_str(&json).unwrap();

        assert_eq!(notification, deserialized);
    }

    #[tokio::test]
    async fn test_notification_clone() {
        let notification = Notification::ToolExecuted {
            tool_name: "test_tool".to_string(),
            result: "output".to_string(),
        };

        let cloned = notification.clone();
        assert_eq!(notification, cloned);
    }

    #[test]
    fn test_notification_partial_eq() {
        let n1 = Notification::TaskStarted {
            task_id: "task-1".to_string(),
        };
        let n2 = Notification::TaskStarted {
            task_id: "task-1".to_string(),
        };
        let n3 = Notification::TaskStarted {
            task_id: "task-2".to_string(),
        };

        assert_eq!(n1, n2);
        assert_ne!(n1, n3);
    }
}
