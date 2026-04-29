//! Model download manager for first-run onboarding.
//!
//! Provides a minimal, progress-aware downloader for pre-configured
//! HuggingFace GGUF models. Uses `reqwest` (already a core dependency)
//! for HTTP streaming and `tokio::sync::mpsc` for progress callbacks.

use futures::StreamExt;
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;

/// Progress event emitted during model download.
#[derive(Debug, Clone)]
pub enum ModelDownloadProgress {
    /// Download has started.
    Started,
    /// Progress update with bytes downloaded and total (if known).
    Progress {
        bytes_downloaded: u64,
        total_bytes: Option<u64>,
    },
    /// Download completed successfully.
    Complete,
    /// Download failed with an error message.
    Failed(String),
}

/// A pre-configured model available for one-click download.
#[derive(Debug, Clone, Copy)]
pub struct PreconfiguredModel {
    pub repo_id: &'static str,
    pub filename: &'static str,
    pub display_name: &'static str,
    pub size_mb: u32,
}

/// The built-in model catalogue (Qwen2 only — matches LocalGgufProvider support).
pub static PRECONFIGURED_MODELS: &[PreconfiguredModel] = &[PreconfiguredModel {
    repo_id: "Qwen/Qwen2.5-1.5B-Instruct-GGUF",
    filename: "qwen2.5-1.5b-instruct-q4_k_m.gguf",
    display_name: "Qwen2.5 1.5B Instruct (Q4_K_M, ~1.0 GB)",
    size_mb: 1024,
}];

/// Download a pre-configured model to `dest_dir`.
///
/// Respects the `HF_ENDPOINT` environment variable for mirror sites.
/// Progress is reported via `progress_tx` after every chunk.
pub async fn download_model(
    model: &PreconfiguredModel,
    dest_dir: PathBuf,
    progress_tx: mpsc::Sender<ModelDownloadProgress>,
) -> Result<PathBuf, String> {
    let _ = progress_tx.send(ModelDownloadProgress::Started).await;

    let result = async {
        let endpoint = std::env::var("HF_ENDPOINT")
            .unwrap_or_else(|_| "https://huggingface.co".to_string());

        let url = format!(
            "{}/{}/resolve/main/{}",
            endpoint.trim_end_matches('/'),
            model.repo_id,
            model.filename
        );

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(3600))
            .build()
            .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

        let response = client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Download request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!(
                "Download failed with status {}: {}",
                response.status(),
                url
            ));
        }

        let total_bytes = response.content_length();

        std::fs::create_dir_all(&dest_dir)
            .map_err(|e| format!("Failed to create model directory: {}", e))?;

        let dest_path = dest_dir.join(model.filename);
        let mut file = tokio::fs::File::create(&dest_path)
            .await
            .map_err(|e| format!("Failed to create model file: {}", e))?;

        let mut bytes_downloaded: u64 = 0;
        let mut stream = response.bytes_stream();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| format!("Download stream error: {}", e))?;
            file.write_all(&chunk)
                .await
                .map_err(|e| format!("Failed to write model chunk: {}", e))?;
            bytes_downloaded += chunk.len() as u64;

            // Throttle progress updates: only send every 1 MB to avoid flooding the UI.
            if chunk.len() >= 1_048_576 || total_bytes.is_none() {
                let _ = progress_tx
                    .send(ModelDownloadProgress::Progress {
                        bytes_downloaded,
                        total_bytes,
                    })
                    .await;
            }
        }

        file.flush()
            .await
            .map_err(|e| format!("Failed to flush model file: {}", e))?;

        let _ = progress_tx
            .send(ModelDownloadProgress::Complete)
            .await;

        Ok(dest_path)
    }
    .await;

    if let Err(ref e) = result {
        let _ = progress_tx.send(ModelDownloadProgress::Failed(e.clone())).await;
    }

    result
}

/// Return the default local model directory (`~/models` on Windows `%USERPROFILE%/models`).
pub fn default_model_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
        .join("models")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preconfigured_models_not_empty() {
        assert!(!PRECONFIGURED_MODELS.is_empty());
    }

    #[test]
    fn test_default_model_dir() {
        let dir = default_model_dir();
        assert!(dir.ends_with("models"));
    }

    #[tokio::test]
    async fn test_download_progress_channel() {
        let (tx, mut rx) = mpsc::channel(4);
        let progress = ModelDownloadProgress::Progress {
            bytes_downloaded: 100,
            total_bytes: Some(1000),
        };
        tx.send(progress.clone()).await.unwrap();
        let received = rx.recv().await.unwrap();
        match received {
            ModelDownloadProgress::Progress { bytes_downloaded, total_bytes } => {
                assert_eq!(bytes_downloaded, 100);
                assert_eq!(total_bytes, Some(1000));
            }
            _ => panic!("Expected Progress variant"),
        }
    }
}
