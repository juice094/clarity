use std::path::PathBuf;
use std::process::Command;

/// Locate the computer_bridge.py script.
fn find_bridge_script() -> Option<PathBuf> {
    // 1. Try compile-time path (dev builds)
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let dev_path = manifest_dir
        .join("..")
        .join("clarity-core")
        .join("scripts")
        .join("computer_bridge.py");
    if dev_path.exists() {
        return Some(dev_path.canonicalize().unwrap_or(dev_path));
    }

    // 2. Try relative to current exe (release builds)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            let candidates = [
                exe_dir.join("computer_bridge.py"),
                exe_dir.join("scripts").join("computer_bridge.py"),
                exe_dir.join("..").join("scripts").join("computer_bridge.py"),
                exe_dir
                    .join("..")
                    .join("clarity-core")
                    .join("scripts")
                    .join("computer_bridge.py"),
                exe_dir
                    .join("..")
                    .join("..")
                    .join("clarity-core")
                    .join("scripts")
                    .join("computer_bridge.py"),
            ];
            for p in &candidates {
                if p.exists() {
                    return Some(p.canonicalize().unwrap_or(p.clone()));
                }
            }
        }
    }

    None
}

/// Build the JSON payload for the bridge script.
fn build_payload(action: &str, args: serde_json::Value) -> String {
    serde_json::json!({
        "action": action,
        "args": args,
    })
    .to_string()
}

/// Run the bridge script with the given action and args.
fn run_bridge(action: &str, args: serde_json::Value) -> Result<serde_json::Value, String> {
    let script = find_bridge_script().ok_or("computer_bridge.py not found")?;

    let payload = build_payload(action, args);

    // Try python3 first, then python
    let python_cmd = if Command::new("python3")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        "python3"
    } else {
        "python"
    };

    let output = Command::new(python_cmd)
        .arg(&script)
        .arg(&payload)
        .output()
        .map_err(|e| format!("Failed to run bridge: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Bridge process failed: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let result: serde_json::Value = serde_json::from_str(&stdout)
        .map_err(|e| format!("Invalid JSON from bridge: {} (raw: {})", e, stdout))?;

    if result.get("success").and_then(|v| v.as_bool()) != Some(true) {
        let error = result
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown bridge error");
        return Err(error.to_string());
    }

    Ok(result)
}

/// Capture a screenshot and return base64 PNG.
#[tauri::command]
pub async fn computer_screenshot() -> Result<String, String> {
    let result = run_bridge("screenshot", serde_json::json!({}))?;
    let data = result
        .get("data")
        .and_then(|v| v.as_str())
        .ok_or("Missing data field in screenshot response")?;
    Ok(data.to_string())
}

/// Click at screen coordinates.
#[tauri::command]
pub async fn computer_click(x: i32, y: i32) -> Result<(), String> {
    run_bridge("click", serde_json::json!({ "x": x, "y": y }))?;
    Ok(())
}

/// Type text at current cursor position.
#[tauri::command]
pub async fn computer_type(text: String) -> Result<(), String> {
    run_bridge("type", serde_json::json!({ "text": text }))?;
    Ok(())
}

/// Scroll at coordinates.
#[tauri::command]
pub async fn computer_scroll(x: i32, y: i32, amount: i32) -> Result<(), String> {
    run_bridge(
        "scroll",
        serde_json::json!({ "x": x, "y": y, "amount": amount }),
    )?;
    Ok(())
}

/// Check if python3/python and computer_bridge.py are available.
#[tauri::command]
pub async fn computer_check_bridge() -> Result<bool, String> {
    // Check python3 first
    let python_ok = Command::new("python3")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    let python_ok = if python_ok {
        true
    } else {
        // Fallback to python
        Command::new("python")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    };

    if !python_ok {
        return Ok(false);
    }

    // Check script exists
    match find_bridge_script() {
        Some(path) => Ok(path.exists()),
        None => Ok(false),
    }
}
