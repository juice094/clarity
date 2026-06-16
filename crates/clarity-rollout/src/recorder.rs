//! JSONL rollout recorder.
//!
//! Writes canonical session rollout items to `.jsonl` files so sessions can be
//! replayed or inspected later. Modeled after `codex_rollout::recorder` from
//! the OpenAI Codex project, licensed under Apache-2.0. See `NOTICES.md` for
//! attribution.

use std::path::{Path, PathBuf};

use chrono::Utc;
use clarity_contract::{
    CreateRolloutParams, ResumeRolloutParams, RolloutItem, RolloutLine, SessionMeta,
    SessionMetaLine,
};
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tracing::{error, info};

use crate::config::RolloutConfigView;
use crate::policy::persisted_rollout_items;

/// Command sent to the background rollout writer task.
enum RolloutCmd {
    AddItems(Vec<RolloutItem>),
    Persist {
        ack: oneshot::Sender<std::io::Result<()>>,
    },
    Flush {
        ack: oneshot::Sender<std::io::Result<()>>,
    },
    Shutdown {
        ack: oneshot::Sender<std::io::Result<()>>,
    },
}

/// Writes canonical session rollout items to JSONL.
#[derive(Clone, Debug)]
pub struct RolloutRecorder {
    tx: mpsc::Sender<RolloutCmd>,
    /// Path to the rollout file being written.
    pub rollout_path: PathBuf,
    /// Thread identifier for this rollout.
    pub thread_id: clarity_contract::ThreadId,
}

impl RolloutRecorder {
    /// Create a new recorder for a thread.
    pub async fn create(
        config: &impl RolloutConfigView,
        params: CreateRolloutParams,
    ) -> std::io::Result<Self> {
        let sessions_dir = config.clarity_home().join("sessions");
        tokio::fs::create_dir_all(&sessions_dir).await?;

        let filename = format!("rollout-{}.jsonl", params.thread_id);
        let rollout_path = sessions_dir.join(filename);

        let file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&rollout_path)
            .await?;
        let writer = BufWriter::new(file);

        let initial_item = if params.skip_initial_meta {
            None
        } else {
            let meta = SessionMetaLine {
                meta: SessionMeta {
                    id: params.thread_id,
                    forked_from_id: params.forked_from_id,
                    parent_thread_id: params.parent_thread_id,
                    timestamp: Utc::now().to_rfc3339(),
                    cwd: params.cwd,
                    originator: params.originator,
                    cli_version: params.cli_version,
                    source: params.source,
                    thread_source: params.thread_source,
                    agent_nickname: None,
                    agent_role: None,
                    agent_path: None,
                    model_provider: params.model_provider,
                    base_instructions: params.base_instructions,
                    dynamic_tools: if params.dynamic_tools.is_empty() {
                        None
                    } else {
                        Some(params.dynamic_tools)
                    },
                    memory_mode: None,
                    multi_agent_version: params.multi_agent_version,
                },
                git: None,
            };
            Some(RolloutItem::SessionMeta(meta))
        };

        let (tx, rx) = mpsc::channel::<RolloutCmd>(1024);
        let thread_id = params.thread_id;
        let path = rollout_path.clone();
        spawn_writer_task(rx, writer, initial_item, path.clone());

        info!(
            thread_id = %thread_id,
            path = %path.display(),
            "created rollout recorder"
        );

        Ok(Self {
            tx,
            rollout_path: path,
            thread_id,
        })
    }

    /// Resume an existing rollout file for appending.
    pub async fn resume(
        _config: &impl RolloutConfigView,
        params: ResumeRolloutParams,
    ) -> std::io::Result<Self> {
        let file = OpenOptions::new().append(true).open(&params.path).await?;
        let writer = BufWriter::new(file);

        // Extract thread id from the first line if possible; otherwise generate a placeholder.
        let thread_id = read_thread_id_from_rollout(&params.path)
            .await
            .unwrap_or_default();

        let (tx, rx) = mpsc::channel::<RolloutCmd>(1024);
        let path = params.path;
        spawn_writer_task(rx, writer, None, path.clone());

        Ok(Self {
            tx,
            rollout_path: path,
            thread_id,
        })
    }

    /// Append rollout items to the durable log.
    pub async fn add_items(&self, items: Vec<RolloutItem>) -> std::io::Result<()> {
        self.tx
            .send(RolloutCmd::AddItems(items))
            .await
            .map_err(|_| std::io::Error::other("rollout writer gone"))?;
        Ok(())
    }

    /// Ensure all prior writes are flushed to disk.
    pub async fn persist(&self) -> std::io::Result<()> {
        let (ack, rx) = oneshot::channel();
        self.tx
            .send(RolloutCmd::Persist { ack })
            .await
            .map_err(|_| std::io::Error::other("rollout writer gone"))?;
        rx.await
            .map_err(|_| std::io::Error::other("persist ack dropped"))?
    }

    /// Flush all queued items and return once they are durable.
    pub async fn flush(&self) -> std::io::Result<()> {
        let (ack, rx) = oneshot::channel();
        self.tx
            .send(RolloutCmd::Flush { ack })
            .await
            .map_err(|_| std::io::Error::other("rollout writer gone"))?;
        rx.await
            .map_err(|_| std::io::Error::other("flush ack dropped"))?
    }

    /// Flush pending items and close the live writer.
    pub async fn shutdown(&self) -> std::io::Result<()> {
        let (ack, rx) = oneshot::channel();
        self.tx
            .send(RolloutCmd::Shutdown { ack })
            .await
            .map_err(|_| std::io::Error::other("rollout writer gone"))?;
        rx.await
            .map_err(|_| std::io::Error::other("shutdown ack dropped"))?
    }
}

fn spawn_writer_task(
    mut rx: mpsc::Receiver<RolloutCmd>,
    mut writer: BufWriter<File>,
    initial_item: Option<RolloutItem>,
    path: PathBuf,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        if let Some(item) = initial_item {
            if let Err(e) = write_item(&mut writer, &item).await {
                error!(path = %path.display(), error = %e, "failed to write session meta");
            }
        }

        while let Some(cmd) = rx.recv().await {
            match cmd {
                RolloutCmd::AddItems(items) => {
                    let items = persisted_rollout_items(&items);
                    for item in items {
                        if let Err(e) = write_item(&mut writer, &item).await {
                            error!(path = %path.display(), error = %e, "failed to write rollout item");
                            break;
                        }
                    }
                }
                RolloutCmd::Persist { ack } => {
                    let res = writer.flush().await;
                    let _ = ack.send(res);
                }
                RolloutCmd::Flush { ack } => {
                    let res = writer.flush().await;
                    let _ = ack.send(res);
                }
                RolloutCmd::Shutdown { ack } => {
                    let res = writer.flush().await;
                    let _ = ack.send(res);
                    break;
                }
            }
        }
    })
}

async fn write_item(writer: &mut BufWriter<File>, item: &RolloutItem) -> std::io::Result<()> {
    let line = RolloutLine {
        timestamp: Utc::now().to_rfc3339(),
        item: item.clone(),
    };
    let json = serde_json::to_vec(&line).map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("serialization failed: {e}"),
        )
    })?;
    writer.write_all(&json).await?;
    writer.write_all(b"\n").await?;
    Ok(())
}

async fn read_thread_id_from_rollout(path: &Path) -> Option<clarity_contract::ThreadId> {
    let file = File::open(path).await.ok()?;
    let reader = BufReader::new(file);
    let mut lines = reader.lines();
    let first = lines.next_line().await.ok()??;
    let line: RolloutLine = serde_json::from_str(&first).ok()?;
    if let RolloutItem::SessionMeta(meta) = line.item {
        Some(meta.meta.id)
    } else {
        None
    }
}

/// Load all rollout items from a file.
pub async fn load_rollout_items(path: &Path) -> std::io::Result<Vec<RolloutItem>> {
    let file = File::open(path).await?;
    let reader = BufReader::new(file);
    let mut items = Vec::new();
    let mut lines = reader.lines();
    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        let rollout_line: RolloutLine = serde_json::from_str(&line).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("invalid rollout line in {}: {e}", path.display()),
            )
        })?;
        items.push(rollout_line.item);
    }
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clarity_contract::{
        CompactedItem, CreateRolloutParams, ResumeRolloutParams, RolloutEventMsg, RolloutItem,
        RolloutResponseItem, SessionSource, ThreadSource,
    };
    use std::path::PathBuf;
    use tempfile::TempDir;

    struct TestConfig {
        home: PathBuf,
    }

    impl RolloutConfigView for TestConfig {
        fn clarity_home(&self) -> &Path {
            &self.home
        }
        fn sqlite_home(&self) -> &Path {
            &self.home
        }
        fn cwd(&self) -> &Path {
            &self.home
        }
        fn model_provider_id(&self) -> &str {
            "test"
        }
        fn generate_memories(&self) -> bool {
            false
        }
    }

    fn create_params(thread_id: clarity_contract::ThreadId, dir: &TempDir) -> CreateRolloutParams {
        CreateRolloutParams {
            thread_id,
            session_id: clarity_contract::SessionId::from(thread_id),
            forked_from_id: None,
            parent_thread_id: None,
            source: SessionSource::Cli,
            thread_source: Some(ThreadSource::New),
            cwd: dir.path().to_path_buf(),
            originator: "test".into(),
            cli_version: "0.0.0".into(),
            base_instructions: None,
            dynamic_tools: Vec::new(),
            model_provider: None,
            multi_agent_version: None,
            skip_initial_meta: false,
        }
    }

    #[tokio::test]
    async fn roundtrip_rollout_items() {
        let dir = TempDir::new().unwrap();
        let config = TestConfig {
            home: dir.path().to_path_buf(),
        };
        let thread_id = clarity_contract::ThreadId::new();
        let recorder = RolloutRecorder::create(&config, create_params(thread_id, &dir))
            .await
            .unwrap();

        recorder
            .add_items(vec![RolloutItem::ResponseItem(
                RolloutResponseItem::Message {
                    role: "user".into(),
                    content: "hello".into(),
                },
            )])
            .await
            .unwrap();
        recorder.flush().await.unwrap();
        recorder.shutdown().await.unwrap();

        let items = load_rollout_items(&recorder.rollout_path).await.unwrap();
        assert!(items.len() >= 2);
    }

    #[tokio::test]
    async fn resume_appends_to_existing_rollout() {
        let dir = TempDir::new().unwrap();
        let config = TestConfig {
            home: dir.path().to_path_buf(),
        };
        let thread_id = clarity_contract::ThreadId::new();
        let recorder = RolloutRecorder::create(&config, create_params(thread_id, &dir))
            .await
            .unwrap();

        recorder
            .add_items(vec![RolloutItem::EventMsg(RolloutEventMsg::UserMessage(
                "hello".into(),
            ))])
            .await
            .unwrap();
        recorder.flush().await.unwrap();
        recorder.shutdown().await.unwrap();

        let resumed = RolloutRecorder::resume(
            &config,
            ResumeRolloutParams {
                path: recorder.rollout_path.clone(),
            },
        )
        .await
        .unwrap();

        resumed
            .add_items(vec![RolloutItem::ResponseItem(
                RolloutResponseItem::Message {
                    role: "assistant".into(),
                    content: "hi".into(),
                },
            )])
            .await
            .unwrap();
        resumed.flush().await.unwrap();
        resumed.shutdown().await.unwrap();

        let items = load_rollout_items(&recorder.rollout_path).await.unwrap();
        assert_eq!(items.len(), 3);
        assert!(matches!(items[0], RolloutItem::SessionMeta(_)));
        assert!(matches!(
            items[1],
            RolloutItem::EventMsg(RolloutEventMsg::UserMessage(_))
        ));
        assert!(
            matches!(items[2], RolloutItem::ResponseItem(RolloutResponseItem::Message { ref role, .. }) if role == "assistant")
        );
        assert_eq!(resumed.thread_id, thread_id);
    }

    #[tokio::test]
    async fn compaction_and_replacement_history_roundtrip() {
        let dir = TempDir::new().unwrap();
        let config = TestConfig {
            home: dir.path().to_path_buf(),
        };
        let thread_id = clarity_contract::ThreadId::new();
        let recorder = RolloutRecorder::create(&config, create_params(thread_id, &dir))
            .await
            .unwrap();

        recorder
            .add_items(vec![
                RolloutItem::ResponseItem(RolloutResponseItem::Message {
                    role: "user".into(),
                    content: "a".into(),
                }),
                RolloutItem::Compacted(CompactedItem {
                    message: "summary".into(),
                    replacement_history: Some(vec![RolloutResponseItem::Message {
                        role: "user".into(),
                        content: "replaced".into(),
                    }]),
                    window_id: Some(1),
                }),
            ])
            .await
            .unwrap();
        recorder.flush().await.unwrap();
        recorder.shutdown().await.unwrap();

        let items = load_rollout_items(&recorder.rollout_path).await.unwrap();
        let compacted: Vec<_> = items
            .into_iter()
            .filter(|item| matches!(item, RolloutItem::Compacted(_)))
            .collect();
        assert_eq!(compacted.len(), 1);
        let RolloutItem::Compacted(compacted_item) = &compacted[0] else {
            panic!("expected compacted item");
        };
        let history = compacted_item
            .replacement_history
            .as_ref()
            .expect("replacement history missing");
        assert_eq!(history.len(), 1);
        assert!(
            matches!(&history[0], RolloutResponseItem::Message { content, .. } if content == "replaced")
        );
    }

    #[tokio::test]
    async fn load_rollout_items_skips_blank_lines_and_reports_invalid_json() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("rollout.jsonl");
        let thread_id = clarity_contract::ThreadId::new();
        let meta = SessionMetaLine {
            meta: SessionMeta {
                id: thread_id,
                forked_from_id: None,
                parent_thread_id: None,
                timestamp: "2024-01-01T00:00:00Z".into(),
                cwd: dir.path().to_path_buf(),
                originator: "test".into(),
                cli_version: "0.0.0".into(),
                source: SessionSource::Cli,
                thread_source: None,
                agent_nickname: None,
                agent_role: None,
                agent_path: None,
                model_provider: None,
                base_instructions: None,
                dynamic_tools: None,
                memory_mode: None,
                multi_agent_version: None,
            },
            git: None,
        };
        let line = RolloutLine {
            timestamp: "2024-01-01T00:00:00Z".into(),
            item: RolloutItem::SessionMeta(meta),
        };
        let json = serde_json::to_string(&line).unwrap();
        tokio::fs::write(&path, format!("{}\n\nnot json\n", json))
            .await
            .unwrap();

        let err = load_rollout_items(&path).await.unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    }

    #[tokio::test]
    async fn large_rollout_streaming_load() {
        let dir = TempDir::new().unwrap();
        let config = TestConfig {
            home: dir.path().to_path_buf(),
        };
        let thread_id = clarity_contract::ThreadId::new();
        let recorder = RolloutRecorder::create(&config, create_params(thread_id, &dir))
            .await
            .unwrap();

        for batch in 0..10 {
            let items: Vec<_> = (0..50)
                .map(|offset| {
                    let i = batch * 50 + offset;
                    RolloutItem::EventMsg(RolloutEventMsg::UserMessage(format!("msg-{i}")))
                })
                .collect();
            recorder.add_items(items).await.unwrap();
        }

        recorder.flush().await.unwrap();
        recorder.shutdown().await.unwrap();

        let items = load_rollout_items(&recorder.rollout_path).await.unwrap();
        assert_eq!(items.len(), 501);
        assert!(
            matches!(&items[500], RolloutItem::EventMsg(RolloutEventMsg::UserMessage(content)) if content == "msg-499")
        );
    }

    #[tokio::test]
    async fn resume_extracts_thread_id_from_meta() {
        let dir = TempDir::new().unwrap();
        let config = TestConfig {
            home: dir.path().to_path_buf(),
        };
        let thread_id = clarity_contract::ThreadId::new();
        let recorder = RolloutRecorder::create(&config, create_params(thread_id, &dir))
            .await
            .unwrap();
        recorder.flush().await.unwrap();
        recorder.shutdown().await.unwrap();

        let recorder2 = RolloutRecorder::resume(
            &config,
            ResumeRolloutParams {
                path: recorder.rollout_path.clone(),
            },
        )
        .await
        .unwrap();

        assert_eq!(recorder2.thread_id, thread_id);
    }
}
