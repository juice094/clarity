use crate::ui::types::UiEvent;
use crate::App;

impl App {
    pub(crate) fn refresh_tasks(&self) {
        let store = self.state.task_store.clone();
        let tx = self.ui_tx.clone();
        self.runtime.spawn(async move {
            match store.list_all().await {
                Ok(tasks) => {
                    if let Err(e) = tx.send(UiEvent::TaskList(tasks)) {
                        tracing::warn!("Failed to send TaskList: {}", e);
                    }
                }
                Err(e) => tracing::warn!("Failed to list tasks: {}", e),
            }
        });
    }
}
