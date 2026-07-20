use crate::App;
use crate::services::gateway_task_client::GatewayTaskClient;
use crate::ui::types::UiEvent;

impl App {
    /// Refresh the task list from the Gateway if it is online,
    /// otherwise fall back to the local `TaskStore`.
    pub(crate) fn refresh_tasks(&self) {
        let gateway_client = GatewayTaskClient::new();
        let local_store = self.context.state.task_store.clone();
        let tx = self.context.ui_tx.clone();

        self.context.runtime.spawn(async move {
            // Try Gateway first
            match gateway_client.list_tasks().await {
                Ok(tasks) => {
                    if let Err(e) = tx.send(UiEvent::TaskList(tasks)) {
                        tracing::warn!("Failed to send TaskList from Gateway: {}", e);
                    }
                    return;
                }
                Err(e) => {
                    tracing::debug!(
                        "Gateway task list failed ({}), falling back to local store",
                        e
                    );
                }
            }

            // Fallback to local store
            match local_store.list_all().await {
                Ok(tasks) => {
                    if let Err(e) = tx.send(UiEvent::TaskList(tasks)) {
                        tracing::warn!("Failed to send TaskList from local store: {}", e);
                    }
                }
                Err(e) => tracing::warn!("Failed to list tasks from local store: {}", e),
            }
        });
    }
}
