//! File-system watcher for live skill reloading.
//!
//! Watches well-known skill directories and triggers registry reload
//! when `.md` files are created, modified, or removed.

use super::SkillRegistry;
use notify::Watcher;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Debounced file-system watcher that reloads skills on change.
///
/// The watcher runs in a background thread and communicates with the
/// main registry via an `mpsc` channel. If the `notify` crate fails to
/// initialise (e.g. missing inotify on WSL), `SkillWatcher::start`
/// returns `None` and logs a warning.
pub struct SkillWatcher {
    _watcher: notify::RecommendedWatcher,
}

impl SkillWatcher {
    /// Start watching the given directories and reload the registry on changes.
    ///
    /// * `registry` – clone of the registry to reload.
    /// * `paths`    – directories to watch (typically user-level and project-level
    ///                `.clarity/skills/`).
    ///
    /// Returns `Some(SkillWatcher)` on success, or `None` if the watcher could
    /// not be initialised.
    pub fn start(registry: SkillRegistry, paths: Vec<PathBuf>) -> Option<Self> {
        let (tx, rx) = std::sync::mpsc::channel::<notify::Event>();

        let mut watcher = match notify::recommended_watcher(
            move |res: Result<notify::Event, notify::Error>| {
                if let Ok(event) = res {
                    let _ = tx.send(event);
                }
            },
        ) {
            Ok(w) => w,
            Err(e) => {
                tracing::warn!(
                    "Failed to initialise file-system watcher for skills: {}. \
                     Hot-reload is disabled.",
                    e
                );
                return None;
            }
        };

        for path in &paths {
            if path.exists() {
                if let Err(e) = watcher.watch(path, notify::RecursiveMode::NonRecursive) {
                    tracing::warn!(
                        "Failed to watch skill directory {}: {}",
                        path.display(),
                        e
                    );
                }
            }
        }

        std::thread::spawn(move || {
            let mut pending = false;
            let mut last_event = Instant::now();

            loop {
                match rx.recv_timeout(Duration::from_millis(100)) {
                    Ok(event) => {
                        if is_relevant(&event) {
                            pending = true;
                            last_event = Instant::now();
                        }
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        if pending && last_event.elapsed() >= Duration::from_millis(500) {
                            pending = false;
                            if let Err(e) = registry.reload_all(&paths) {
                                tracing::warn!("Skill hot-reload failed: {}", e);
                            } else {
                                let count = registry.len();
                                let active = registry.active_ids().len();
                                tracing::info!(
                                    "Skills hot-reloaded: {} registered, {} active",
                                    count,
                                    active
                                );
                            }
                        }
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        });

        Some(Self { _watcher: watcher })
    }
}

fn is_relevant(event: &notify::Event) -> bool {
    use notify::EventKind;
    matches!(
        event.kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) | EventKind::Any
    )
}
