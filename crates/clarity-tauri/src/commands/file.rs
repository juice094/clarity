use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const MAX_DEPTH: usize = 10;

fn validate_path(input: &str, cwd: &Path) -> Result<PathBuf, String> {
    if input.contains("..") {
        return Err("Path contains invalid component '..'".to_string());
    }

    let path = if Path::new(input).is_absolute() {
        PathBuf::from(input)
    } else {
        cwd.join(input)
    };

    let canonical = path.canonicalize().map_err(|e| format!("Failed to resolve path: {e}"))?;

    let canonical_cwd = cwd
        .canonicalize()
        .map_err(|e| format!("Failed to resolve cwd: {e}"))?;

    if !canonical.starts_with(&canonical_cwd) {
        return Err("Path is outside the working directory".to_string());
    }

    Ok(canonical)
}

fn build_tree(path: &Path, relative: &Path, depth: usize) -> Result<Value, String> {
    if depth > MAX_DEPTH {
        return Err(format!("Max recursion depth ({MAX_DEPTH}) exceeded"));
    }

    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();

    let rel_path = relative.to_string_lossy().to_string();

    let metadata = std::fs::metadata(path).map_err(|e| format!("Failed to read metadata: {e}"))?;

    if metadata.is_dir() {
        let mut children = Vec::new();
        let mut entries: Vec<_> = std::fs::read_dir(path)
            .map_err(|e| format!("Failed to read directory: {e}"))?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .map(|n| !n.starts_with('.'))
                    .unwrap_or(true)
            })
            .collect();

        entries.sort_by(|a, b| {
            let a_name = a.file_name();
            let b_name = b.file_name();
            let a_is_dir = a.file_type().map(|t| t.is_dir()).unwrap_or(false);
            let b_is_dir = b.file_type().map(|t| t.is_dir()).unwrap_or(false);
            match b_is_dir.cmp(&a_is_dir) {
                std::cmp::Ordering::Equal => a_name.cmp(&b_name),
                other => other,
            }
        });

        for entry in entries {
            let child_path = entry.path();
            let child_rel = relative.join(entry.file_name());
            match build_tree(&child_path, &child_rel, depth + 1) {
                Ok(node) => children.push(node),
                Err(_) => continue,
            }
        }

        Ok(json!({
            "name": name,
            "type": "directory",
            "path": rel_path,
            "children": children,
        }))
    } else {
        Ok(json!({
            "name": name,
            "type": "file",
            "path": rel_path,
            "size": metadata.len(),
        }))
    }
}

#[tauri::command]
pub async fn get_file_tree(path: Option<String>) -> Result<Value, String> {
    let cwd = std::env::current_dir().map_err(|e| format!("Failed to get current dir: {e}"))?;

    let target = match path {
        Some(p) => validate_path(&p, &cwd)?,
        None => cwd.clone(),
    };

    let relative = if target == cwd {
        PathBuf::from("")
    } else {
        target.strip_prefix(&cwd).unwrap_or(&target).to_path_buf()
    };

    build_tree(&target, &relative, 0)
}

#[tauri::command]
pub async fn read_file(
    path: String,
    offset: Option<u64>,
    limit: Option<u64>,
) -> Result<String, String> {
    let cwd = std::env::current_dir().map_err(|e| format!("Failed to get current dir: {e}"))?;
    let target = validate_path(&path, &cwd)?;

    let metadata = std::fs::metadata(&target).map_err(|e| format!("Failed to read metadata: {e}"))?;
    if metadata.is_dir() {
        return Err("Path is a directory, not a file".to_string());
    }

    let mut file = std::fs::File::open(&target).map_err(|e| format!("Failed to open file: {e}"))?;

    use std::io::{Read, Seek};

    if let Some(off) = offset {
        file.seek(std::io::SeekFrom::Start(off))
            .map_err(|e| format!("Failed to seek: {e}"))?;
    }

    let buffer = if let Some(lim) = limit {
        let mut buf = vec![0u8; lim as usize];
        let n = file.read(&mut buf).map_err(|e| format!("Failed to read file: {e}"))?;
        buf.truncate(n);
        buf
    } else {
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)
            .map_err(|e| format!("Failed to read file: {e}"))?;
        buf
    };

    String::from_utf8(buffer).map_err(|e| format!("File is not valid UTF-8: {e}"))
}
