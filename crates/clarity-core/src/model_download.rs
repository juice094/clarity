//! Model download manager for first-run onboarding.
//!
//! Provides a minimal, progress-aware downloader for pre-configured
//! HuggingFace GGUF models. Uses `reqwest` (already a core dependency)
//! for HTTP streaming and `tokio::sync::mpsc` for progress callbacks.
//!
//! NOTE: Evaluate migration to clarity-infrastructure crate if the module
//! grows beyond 500 lines or gains additional infrastructure-only deps.

use futures::StreamExt;
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

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
    /// Download was cancelled by user.
    Cancelled,
    /// Download failed with an error message.
    Failed(String),
}

/// A pre-configured model available for one-click download.
#[derive(Debug, Clone, Copy)]
pub struct PreconfiguredModel {
    pub repo_id: &'static str,
    pub filename: &'static str,
    pub tokenizer_repo_id: Option<&'static str>,
    pub tokenizer_filename: Option<&'static str>,
    pub display_name: &'static str,
    pub size_mb: u32,
}

/// The built-in model catalogue (Qwen2 only — matches LocalGgufProvider support).
pub static PRECONFIGURED_MODELS: &[PreconfiguredModel] = &[PreconfiguredModel {
    repo_id: "Qwen/Qwen2.5-1.5B-Instruct-GGUF",
    filename: "qwen2.5-1.5b-instruct-q4_k_m.gguf",
    tokenizer_repo_id: Some("Qwen/Qwen2.5-1.5B-Instruct"),
    tokenizer_filename: Some("tokenizer.json"),
    display_name: "Qwen2.5 1.5B Instruct (Q4_K_M, ~1.0 GB)",
    size_mb: 1024,
}];

/// Download a pre-configured model (GGUF + optional tokenizer.json) to `dest_dir`.
///
/// Respects the `HF_ENDPOINT` environment variable for mirror sites.
/// Progress is reported via `progress_tx` after every chunk.
/// Cancellation is cooperative: the caller drops the token or calls `.cancel()`.
pub async fn download_model_files(
    model: &PreconfiguredModel,
    dest_dir: PathBuf,
    progress_tx: mpsc::Sender<ModelDownloadProgress>,
    cancel_token: CancellationToken,
) -> Result<PathBuf, String> {
    let _ = progress_tx.send(ModelDownloadProgress::Started).await;

    let result: Result<PathBuf, String> = async {
        std::fs::create_dir_all(&dest_dir)
            .map_err(|e| format!("Failed to create model directory: {}", e))?;

        // 1. Download the main .gguf file
        let dest_path = download_single_file(
            model.repo_id,
            model.filename,
            &dest_dir,
            &progress_tx,
            &cancel_token,
        )
        .await?;

        // 2. Download tokenizer companion if configured
        if let (Some(t_repo), Some(t_file)) = (model.tokenizer_repo_id, model.tokenizer_filename) {
            let _ = progress_tx
                .send(ModelDownloadProgress::Progress {
                    bytes_downloaded: 0,
                    total_bytes: Some(1),
                })
                .await;
            if let Err(e) =
                download_single_file(t_repo, t_file, &dest_dir, &progress_tx, &cancel_token).await
            {
                tracing::warn!("Tokenizer download failed (non-fatal): {}", e);
            }
            let _ = progress_tx
                .send(ModelDownloadProgress::Progress {
                    bytes_downloaded: 1,
                    total_bytes: Some(1),
                })
                .await;
        }

        let _ = progress_tx.send(ModelDownloadProgress::Complete).await;
        Ok(dest_path)
    }
    .await;

    match &result {
        Ok(_) => {}
        Err(e) if e == "Cancelled" => {
            let _ = progress_tx.send(ModelDownloadProgress::Cancelled).await;
        }
        Err(e) => {
            let _ = progress_tx
                .send(ModelDownloadProgress::Failed(e.clone()))
                .await;
        }
    }

    result
}

async fn download_single_file(
    repo_id: &str,
    filename: &str,
    dest_dir: &std::path::Path,
    progress_tx: &mpsc::Sender<ModelDownloadProgress>,
    cancel_token: &CancellationToken,
) -> Result<PathBuf, String> {
    if cancel_token.is_cancelled() {
        return Err("Cancelled".to_string());
    }

    let endpoint =
        std::env::var("HF_ENDPOINT").unwrap_or_else(|_| "https://huggingface.co".to_string());

    let url = format!(
        "{}/{}/resolve/main/{}",
        endpoint.trim_end_matches('/'),
        repo_id,
        filename
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

    let dest_path = dest_dir.join(filename);
    let mut file = tokio::fs::File::create(&dest_path)
        .await
        .map_err(|e| format!("Failed to create model file: {}", e))?;

    let mut bytes_downloaded: u64 = 0;
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        if cancel_token.is_cancelled() {
            drop(file);
            let _ = tokio::fs::remove_file(&dest_path).await;
            return Err("Cancelled".to_string());
        }

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

    Ok(dest_path)
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
            ModelDownloadProgress::Progress {
                bytes_downloaded,
                total_bytes,
            } => {
                assert_eq!(bytes_downloaded, 100);
                assert_eq!(total_bytes, Some(1000));
            }
            _ => panic!("Expected Progress variant"),
        }
    }

    #[tokio::test]
    async fn test_cancellation_token_early_exit() {
        // Verify that a cancelled token immediately aborts download_single_file
        let token = CancellationToken::new();
        token.cancel();

        let (tx, _rx) = mpsc::channel(4);
        let dir = std::env::temp_dir().join("clarity_test_cancel");
        let result = download_single_file("dummy/repo", "dummy.gguf", &dir, &tx, &token).await;

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Cancelled");
    }
}
