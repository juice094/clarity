use crate::App;
use crate::ui::types::{GatewayStatus, UiEvent};

impl App {
    /// Poll all tracked parallel batch statuses from the Gateway.
    pub(crate) fn poll_parallel_batches(&mut self) {
        let gateway = std::env::var("CLARITY_GATEWAY_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:18790".to_string());
        let batch_ids: Vec<String> = self
            .subagent_store()
            .parallel_batches
            .iter()
            .map(|b| b.batch_id.clone())
            .collect();
        let tx = self.context.ui_tx.clone();

        self.subagent_store_mut().last_parallel_poll = std::time::Instant::now();

        self.context.runtime.spawn(async move {
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .unwrap_or_else(|e| {
                    tracing::warn!(
                        "Failed to build reqwest Client for Gateway poll, retrying with timeout: {}",
                        e
                    );
                    reqwest::Client::builder()
                        .timeout(std::time::Duration::from_secs(5))
                        .build()
                        .unwrap_or_else(|_| reqwest::Client::new())
                });

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

    /// Poll Gateway /health endpoint and update the UI status indicator.
    pub(crate) fn poll_gateway_health(&mut self) {
        let gateway = std::env::var("CLARITY_GATEWAY_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:18790".to_string());
        let tx = self.context.ui_tx.clone();

        // Set Checking while the request is in flight.
        self.chat_store_mut().gateway_status = GatewayStatus::Checking;
        let _ = tx.send(UiEvent::GatewayHealth(GatewayStatus::Checking));

        self.context.runtime.spawn(async move {
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .unwrap_or_else(|e| {
                    tracing::warn!(
                        "Failed to build reqwest Client for Gateway poll, retrying with timeout: {}",
                        e
                    );
                    reqwest::Client::builder()
                        .timeout(std::time::Duration::from_secs(5))
                        .build()
                        .unwrap_or_else(|_| reqwest::Client::new())
                });

            let url = format!("{}/health", gateway);
            let status = match client.get(&url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    match resp.json::<serde_json::Value>().await {
                        Ok(body) => {
                            if body.get("status").and_then(|v| v.as_str()) == Some("healthy") {
                                GatewayStatus::Online
                            } else {
                                GatewayStatus::Offline
                            }
                        }
                        Err(_) => GatewayStatus::Offline,
                    }
                }
                _ => GatewayStatus::Offline,
            };

            let _ = tx.send(UiEvent::GatewayHealth(status));
        });
    }
}
