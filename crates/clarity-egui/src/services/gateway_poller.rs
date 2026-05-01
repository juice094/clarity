use crate::ui::types::UiEvent;
use crate::App;

impl App {
    /// Poll all tracked parallel batch statuses from the Gateway.
    pub(crate) fn poll_parallel_batches(&mut self) {
        let gateway = std::env::var("CLARITY_GATEWAY_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:18790".to_string());
        let batch_ids: Vec<String> = self
            .subagent_store
            .parallel_batches
            .iter()
            .map(|b| b.batch_id.clone())
            .collect();
        let tx = self.ui_tx.clone();

        self.subagent_store.last_parallel_poll = std::time::Instant::now();

        self.runtime.spawn(async move {
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new());

            for batch_id in &batch_ids {
                let url = format!("{}/v1/parallel/{}/status", gateway, batch_id);
                match client.get(&url).send().await {
                    Ok(resp) if resp.status().is_success() => {
                        if let Ok(status) = resp.json::<serde_json::Value>().await {
                            let _ = tx.send(UiEvent::SubAgentBatch(batch_id.clone(), status));
                        }
                    }
                    _ => {}
                }
            }
        });
    }
}
