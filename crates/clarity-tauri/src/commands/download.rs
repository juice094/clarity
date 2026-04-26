use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;

/// Download a .gguf model from HuggingFace and save it to ~/models/.
/// Emits `download:progress` events with { repo_id, filename, bytes: u64, total: Option<u64> }.
#[tauri::command]
pub async fn download_model(
    repo_id: String,
    filename: String,
    app: AppHandle,
) -> Result<String, String> {
    let url = format!(
        "https://huggingface.co/{}/resolve/main/{}",
        repo_id, filename
    );

    let models_dir = dirs::home_dir()
        .ok_or("Unable to determine home directory")?
        .join("models");
    std::fs::create_dir_all(&models_dir).map_err(|e| e.to_string())?;

    let dest_path = models_dir.join(&filename);

    let mut response = reqwest::get(&url)
        .await
        .map_err(|e| format!("Failed to start download: {}", e))?;

    let total = response.content_length();
    let status = response.status();
    if !status.is_success() {
        return Err(format!("HTTP {}: {}", status.as_u16(), status));
    }

    let mut downloaded: u64 = 0;
    let mut file = tokio::fs::File::create(&dest_path)
        .await
        .map_err(|e| format!("Failed to create file: {}", e))?;

    while let Some(chunk) = response.chunk().await.map_err(|e| format!("Download error: {}", e))? {
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("Write error: {}", e))?;
        downloaded += chunk.len() as u64;

        let _ = app.emit(
            "download:progress",
            serde_json::json!({
                "repo_id": &repo_id,
                "filename": &filename,
                "bytes": downloaded,
                "total": total,
            }),
        );
    }

    file.flush().await.map_err(|e| e.to_string())?;

    let _ = app.emit(
        "download:complete",
        serde_json::json!({
            "repo_id": &repo_id,
            "filename": &filename,
            "path": dest_path.to_string_lossy(),
        }),
    );

    Ok(dest_path.to_string_lossy().to_string())
}
