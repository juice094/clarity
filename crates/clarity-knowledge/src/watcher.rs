//! File-system change detection for knowledge sources.

use crate::error::{KnowledgeError, Result};
use crate::index::SourceConfig;
use notify::Watcher;
use std::path::PathBuf;

/// Events emitted when the file system changes.
#[derive(Debug, Clone, PartialEq)]
pub enum WatcherEvent {
    /// A file or directory was created.
    Created(PathBuf),
    /// A file or directory was modified.
    Modified(PathBuf),
    /// A file or directory was removed.
    Removed(PathBuf),
    /// A file or directory was renamed.
    Renamed {
        /// Previous path.
        from: PathBuf,
        /// New path.
        to: PathBuf,
    },
}

/// Watches a knowledge source for changes.
///
/// This trait is implemented by the built-in `NotifyWatcher`. It is kept as a
/// trait so that tests can supply a deterministic fake watcher.
#[async_trait::async_trait]
pub trait FileWatcher: Send + Sync {
    /// Start watching a source directory.
    async fn watch(&mut self, config: SourceConfig) -> Result<()>;

    /// Wait for the next file-system event.
    ///
    /// Returns `None` when the watcher has been stopped.
    async fn next_event(&mut self) -> Result<Option<WatcherEvent>>;
}

/// Built-in file watcher based on `notify`.
#[derive(Debug)]
pub struct NotifyWatcher {
    watcher: Option<notify::RecommendedWatcher>,
    rx: Option<tokio::sync::mpsc::Receiver<WatcherEvent>>,
}

impl NotifyWatcher {
    /// Create a new watcher instance.
    pub fn new() -> Self {
        Self {
            watcher: None,
            rx: None,
        }
    }
}

impl Default for NotifyWatcher {
    fn default() -> Self {
        Self::new()
    }
}

fn notify_err_to_io(err: notify::Error) -> std::io::Error {
    std::io::Error::other(err)
}

#[async_trait::async_trait]
impl FileWatcher for NotifyWatcher {
    async fn watch(&mut self, config: SourceConfig) -> Result<()> {
        let (tx, rx) = tokio::sync::mpsc::channel(256);
        let root = config.root.clone();

        let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            if let Ok(event) = res {
                let converted = convert_event(event);
                for evt in converted {
                    let _ = tx.try_send(evt);
                }
            }
        })
        .map_err(notify_err_to_io)
        .map_err(KnowledgeError::Io)?;

        watcher
            .watch(&root, notify::RecursiveMode::Recursive)
            .map_err(notify_err_to_io)
            .map_err(KnowledgeError::Io)?;

        self.watcher = Some(watcher);
        self.rx = Some(rx);
        Ok(())
    }

    async fn next_event(&mut self) -> Result<Option<WatcherEvent>> {
        match self.rx.as_mut() {
            Some(rx) => Ok(rx.recv().await),
            None => Err(KnowledgeError::NotInitialized),
        }
    }
}

fn convert_event(event: notify::Event) -> Vec<WatcherEvent> {
    use notify::EventKind;

    match event.kind {
        EventKind::Create(_) => event.paths.into_iter().map(WatcherEvent::Created).collect(),
        EventKind::Modify(_) if event.paths.len() >= 2 => {
            vec![WatcherEvent::Renamed {
                from: event.paths[0].clone(),
                to: event.paths[1].clone(),
            }]
        }
        EventKind::Modify(_) => event
            .paths
            .into_iter()
            .map(WatcherEvent::Modified)
            .collect(),
        EventKind::Remove(_) => event.paths.into_iter().map(WatcherEvent::Removed).collect(),
        _ => Vec::new(),
    }
}
