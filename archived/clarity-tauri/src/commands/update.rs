//! Update checker — queries GitHub Releases for newer versions.

use serde::Deserialize;

#[derive(Deserialize)]
struct GitHubRelease {
    tag_name: String,
}

/// Compare two SemVer strings (e.g. "0.2.1" vs "0.3.0").
/// Returns `true` if `remote` is strictly newer than `local`.
fn is_newer(local: &str, remote: &str) -> bool {
    let parse = |s: &str| {
        s.trim_start_matches('v')
            .split('.')
            .filter_map(|p| p.parse::<u32>().ok())
            .collect::<Vec<_>>()
    };
    let l = parse(local);
    let r = parse(remote);
    for i in 0..l.len().max(r.len()) {
        let lv = l.get(i).copied().unwrap_or(0);
        let rv = r.get(i).copied().unwrap_or(0);
        if rv > lv {
            return true;
        }
        if rv < lv {
            return false;
        }
    }
    false
}

/// Check whether a newer release exists on GitHub.
///
/// Returns `Some(latest_version)` if a newer tag is found,
/// or `None` when the current version is up-to-date or the check fails.
#[tauri::command]
pub async fn check_update() -> Option<String> {
    let current = env!("CARGO_PKG_VERSION");
    let url = "https://api.github.com/repos/juice094/clarity/releases/latest";

    let client = reqwest::Client::builder()
        .user_agent("Clarity-Updater")
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .ok()?;

    let release: GitHubRelease = client.get(url).send().await.ok()?.json().await.ok()?;
    let latest = release.tag_name;

    if is_newer(current, &latest) {
        Some(latest)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_newer() {
        assert!(is_newer("0.2.1", "0.3.0"));
        assert!(is_newer("0.2.1", "v0.3.0"));
        assert!(!is_newer("0.3.0", "0.2.1"));
        assert!(!is_newer("0.2.1", "0.2.1"));
        assert!(is_newer("0.2.1", "0.2.2"));
        assert!(is_newer("0.2", "0.2.1"));
    }
}
