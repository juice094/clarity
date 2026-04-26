import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { X } from "lucide-react";

interface TaskView {
  id: string;
  name: string;
  status: "running" | "pending" | "completed" | "failed";
  priority: string;
  created_at: string;
}

interface TaskPanelProps {
  isOpen: boolean;
  onClose: () => void;
}

function TaskPanel({ isOpen, onClose }: TaskPanelProps) {
  const [tasks, setTasks] = useState<TaskView[]>([]);
  const [error, setError] = useState("");

  const fetchTasks = useCallback(async () => {
    try {
      const data = await invoke<TaskView[]>("list_tasks");
      setTasks(data);
    } catch (e) {
      console.error("Failed to list tasks:", e);
      setError("Failed to load tasks");
    }
  }, []);

  useEffect(() => {
    if (!isOpen) return;
    fetchTasks();
    const interval = setInterval(fetchTasks, 5000);
    return () => clearInterval(interval);
  }, [isOpen, fetchTasks]);

  async function handleCancel(taskId: string) {
    try {
      await invoke("cancel_task", { taskId });
      fetchTasks();
    } catch (e) {
      console.error("Failed to cancel task:", e);
      setError("Failed to cancel task");
    }
  }

  function statusColor(status: string): string {
    switch (status) {
      case "running":
        return "var(--accent)";
      case "pending":
        return "var(--text-secondary)";
      case "completed":
        return "#238636";
      case "failed":
        return "var(--danger)";
      default:
        return "var(--text-secondary)";
    }
  }

  if (!isOpen) return null;

  return (
    <div className="task-panel">
      <div className="task-panel-header">
        <h2>Tasks</h2>
        <button className="task-panel-close" onClick={onClose} aria-label="Close">
          <X size={16} />
        </button>
      </div>
      {error && <div className="task-error-banner">{error}</div>}
      <div className="task-list">
        {tasks.length === 0 && !error && (
          <div className="task-empty">No tasks</div>
        )}
        {tasks.map((task) => (
          <div key={task.id} className="task-card">
            <div className="task-row">
              <span className="task-name">{task.name}</span>
              <span
                className="task-status-badge"
                style={{ background: statusColor(task.status) }}
              >
                {task.status}
              </span>
            </div>
            <div className="task-row task-meta">
              <span className="task-id">{task.id}</span>
              <span className="task-priority">{task.priority}</span>
              <span className="task-time">{task.created_at}</span>
            </div>
            {(task.status === "running" || task.status === "pending") && (
              <button
                className="task-cancel-btn"
                onClick={() => handleCancel(task.id)}
              >
                Cancel
              </button>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}

export default TaskPanel;
