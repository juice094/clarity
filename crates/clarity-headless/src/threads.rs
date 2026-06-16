//! Thread management subcommands for `clarity-headless`.
//!
//! These commands operate on the V2 thread/rollout store under `.clarity`.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};
use clarity_contract::{Message, ThreadId};
use clarity_core::thread::ThreadManager;
use clarity_thread_store::{ForkSnapshot, LocalThreadStore, RolloutConfig};

use crate::{OutputFormat, ProviderType, build_provider};

// ------------------------------------------------------------------
// Arguments
// ------------------------------------------------------------------

/// Thread management commands.
#[derive(Parser, Debug, Clone)]
pub struct ThreadsArgs {
    /// Override the Clarity home directory (default: `.clarity`).
    #[arg(long, global = true)]
    pub clarity_home: Option<PathBuf>,

    /// Subcommand action.
    #[command(subcommand)]
    pub action: ThreadsAction,
}

/// Available thread actions.
#[derive(Subcommand, Debug, Clone)]
pub enum ThreadsAction {
    /// List recent threads.
    List(ListArgs),
    /// Create a new empty thread.
    Create(CreateArgs),
    /// Show a thread summary and optional history.
    Show(ShowArgs),
    /// Archive a thread.
    Archive(IdArgs),
    /// Delete a thread.
    Delete(IdArgs),
    /// Fork a thread.
    Fork(ForkArgs),
    /// Resume a thread with a new user prompt.
    Resume(ResumeArgs),
    /// Migrate legacy V1 sessions.db to V2 threads.
    #[cfg(feature = "session-migration")]
    Migrate(MigrateArgs),
}

/// List threads arguments.
#[derive(Parser, Debug, Clone)]
pub struct ListArgs {
    /// Maximum number of threads to return.
    #[arg(short, long, default_value = "20")]
    pub limit: usize,

    /// Include archived threads.
    #[arg(long)]
    pub archived: bool,

    /// Output format.
    #[arg(short, long, value_enum, default_value = "markdown")]
    pub output: OutputFormat,
}

/// Create thread arguments.
#[derive(Parser, Debug, Clone)]
pub struct CreateArgs {
    /// Optional human-readable title.
    #[arg(short, long)]
    pub title: Option<String>,

    /// Output format.
    #[arg(short, long, value_enum, default_value = "json")]
    pub output: OutputFormat,
}

/// Thread-id-only arguments.
#[derive(Parser, Debug, Clone)]
pub struct IdArgs {
    /// Thread identifier (UUID).
    pub thread_id: String,

    /// Output format.
    #[arg(short, long, value_enum, default_value = "json")]
    pub output: OutputFormat,
}

/// Show thread arguments.
#[derive(Parser, Debug, Clone)]
pub struct ShowArgs {
    /// Thread identifier (UUID).
    pub thread_id: String,

    /// Include the full rollout history.
    #[arg(long)]
    pub history: bool,

    /// Output format.
    #[arg(short, long, value_enum, default_value = "json")]
    pub output: OutputFormat,
}

/// Fork thread arguments.
#[derive(Parser, Debug, Clone)]
pub struct ForkArgs {
    /// Source thread identifier (UUID).
    pub thread_id: String,

    /// Fork before the nth user message (1-based). If omitted, forks at the
    /// current tip (`Interrupted` snapshot).
    #[arg(long)]
    pub before_user: Option<usize>,

    /// Output format.
    #[arg(short, long, value_enum, default_value = "json")]
    pub output: OutputFormat,
}

/// Resume thread arguments.
#[derive(Parser, Debug, Clone)]
pub struct ResumeArgs {
    /// Thread identifier (UUID).
    pub thread_id: String,

    /// User prompt to continue the conversation.
    #[arg(short, long)]
    pub prompt: String,

    /// LLM provider to use.
    #[arg(short = 'P', long, value_enum, default_value = "openai")]
    pub provider: ProviderType,

    /// Model name override.
    #[arg(short, long)]
    pub model: Option<String>,

    /// API key override.
    #[arg(long)]
    pub api_key: Option<String>,

    /// Approval mode.
    #[arg(short, long, value_enum, default_value = "interactive")]
    pub approval: crate::ApprovalModeArg,

    /// Max iterations.
    #[arg(long, default_value = "10")]
    pub max_iterations: usize,

    /// Output format.
    #[arg(short, long, value_enum, default_value = "markdown")]
    pub output: OutputFormat,
}

/// Migrate arguments.
#[cfg(feature = "session-migration")]
#[derive(Parser, Debug, Clone)]
pub struct MigrateArgs {
    /// Path to the legacy V1 sessions.db.
    pub v1_db: PathBuf,

    /// Output format.
    #[arg(short, long, value_enum, default_value = "json")]
    pub output: OutputFormat,
}

// ------------------------------------------------------------------
// Dispatch
// ------------------------------------------------------------------

/// Run a thread subcommand.
pub async fn run(args: ThreadsArgs) -> Result<()> {
    let clarity_home = args.clarity_home.clone();
    match args.action {
        ThreadsAction::List(a) => list(a, clarity_home).await,
        ThreadsAction::Create(a) => create(a, clarity_home).await,
        ThreadsAction::Show(a) => show(a, clarity_home).await,
        ThreadsAction::Archive(a) => archive(a, clarity_home).await,
        ThreadsAction::Delete(a) => delete(a, clarity_home).await,
        ThreadsAction::Fork(a) => fork(a, clarity_home).await,
        ThreadsAction::Resume(a) => resume(a, clarity_home).await,
        #[cfg(feature = "session-migration")]
        ThreadsAction::Migrate(a) => migrate(a, clarity_home).await,
    }
}

// ------------------------------------------------------------------
// Helpers
// ------------------------------------------------------------------

/// Build a thread manager backed by the local thread store.
async fn thread_manager(clarity_home: Option<PathBuf>) -> Result<ThreadManager> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let clarity_home = clarity_home.unwrap_or_else(|| cwd.join(".clarity"));
    let config = RolloutConfig {
        clarity_home: clarity_home.clone(),
        sqlite_home: clarity_home.clone(),
        cwd: cwd.clone(),
        model_provider_id: String::new(),
        generate_memories: false,
    };
    let state_db = LocalThreadStore::default_state_db_path(&config);

    let store = tokio::task::spawn_blocking(move || {
        LocalThreadStore::new(config, state_db)
            .map(Arc::new)
            .map_err(|e| anyhow::anyhow!("Failed to open thread store: {e}"))
    })
    .await
    .context("thread store initialization panicked")??;

    Ok(ThreadManager::new(store))
}

fn parse_thread_id(s: &str) -> Result<ThreadId> {
    ThreadId::from_string(s).with_context(|| format!("Invalid thread id: {s}"))
}

fn format_time(ts: DateTime<Utc>) -> String {
    ts.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

fn print_json<T: serde::Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

// ------------------------------------------------------------------
// Commands
// ------------------------------------------------------------------

async fn list(args: ListArgs, clarity_home: Option<PathBuf>) -> Result<()> {
    let manager = thread_manager(clarity_home).await?;
    let threads = manager
        .list_threads(args.limit, args.archived, None)
        .await
        .context("Failed to list threads")?;

    match args.output {
        OutputFormat::Json => {
            let items: Vec<_> = threads
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "thread_id": t.thread_id.to_string(),
                        "session_id": t.session_id.to_string(),
                        "title": t.title,
                        "created_at": format_time(t.created_at),
                        "updated_at": format_time(t.updated_at),
                        "archived": t.archived,
                    })
                })
                .collect();
            print_json(&items)?;
        }
        OutputFormat::Markdown => {
            println!("# Threads");
            if threads.is_empty() {
                println!("\nNo threads found.");
                return Ok(());
            }
            for t in &threads {
                let title = t.title.as_deref().unwrap_or("(untitled)");
                println!(
                    "- `{}` — {} (updated {})",
                    t.thread_id,
                    title,
                    format_time(t.updated_at)
                );
            }
        }
    }
    Ok(())
}

async fn create(args: CreateArgs, clarity_home: Option<PathBuf>) -> Result<()> {
    let manager = thread_manager(clarity_home).await?;
    let thread_id = manager
        .create_thread(
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            "clarity-headless",
            Some(clarity_contract::SessionSource::Cli),
        )
        .await
        .context("Failed to create thread")?;

    if let Some(title) = args.title {
        manager
            .update_metadata(
                thread_id,
                clarity_thread_store::ThreadMetadataPatch {
                    title: Some(title),
                    archived: None,
                    extra: std::collections::HashMap::new(),
                },
            )
            .await
            .context("Failed to set thread title")?;
    }

    manager
        .flush(thread_id)
        .await
        .context("Failed to flush thread")?;

    match args.output {
        OutputFormat::Json => {
            print_json(&serde_json::json!({ "thread_id": thread_id.to_string() }))?;
        }
        OutputFormat::Markdown => {
            println!("Created thread `{}`", thread_id);
        }
    }
    Ok(())
}

async fn show(args: ShowArgs, clarity_home: Option<PathBuf>) -> Result<()> {
    let manager = thread_manager(clarity_home).await?;
    let thread_id = parse_thread_id(&args.thread_id)?;
    let stored = manager
        .read_thread(thread_id, args.history)
        .await
        .context("Failed to read thread")?;

    match args.output {
        OutputFormat::Json => {
            let mut json = serde_json::json!({
                "thread_id": stored.thread_id.to_string(),
                "session_id": stored.session_id.to_string(),
                "title": stored.title,
                "created_at": format_time(stored.created_at),
                "updated_at": format_time(stored.updated_at),
                "archived": stored.archived,
                "rollout_path": stored.rollout_path,
            });
            if args.history
                && let Some(history) = stored.history
            {
                json["history"] = serde_json::to_value(&history.items)?;
            }
            print_json(&json)?;
        }
        OutputFormat::Markdown => {
            println!("# Thread `{}`", stored.thread_id);
            println!(
                "- Title: {}",
                stored.title.as_deref().unwrap_or("(untitled)")
            );
            println!("- Created: {}", format_time(stored.created_at));
            println!("- Updated: {}", format_time(stored.updated_at));
            println!("- Archived: {}", stored.archived);
            if args.history
                && let Some(history) = stored.history
            {
                println!("\n## History ({} items)", history.items.len());
                for item in &history.items {
                    println!("- {:?}", item);
                }
            }
        }
    }
    Ok(())
}

async fn archive(args: IdArgs, clarity_home: Option<PathBuf>) -> Result<()> {
    let manager = thread_manager(clarity_home).await?;
    let thread_id = parse_thread_id(&args.thread_id)?;
    manager
        .archive(thread_id)
        .await
        .context("Failed to archive thread")?;
    match args.output {
        OutputFormat::Json => print_json(&serde_json::json!({ "archived": true }))?,
        OutputFormat::Markdown => println!("Archived thread `{}`", thread_id),
    }
    Ok(())
}

async fn delete(args: IdArgs, clarity_home: Option<PathBuf>) -> Result<()> {
    let manager = thread_manager(clarity_home).await?;
    let thread_id = parse_thread_id(&args.thread_id)?;
    manager
        .delete(thread_id)
        .await
        .context("Failed to delete thread")?;
    match args.output {
        OutputFormat::Json => print_json(&serde_json::json!({ "deleted": true }))?,
        OutputFormat::Markdown => println!("Deleted thread `{}`", thread_id),
    }
    Ok(())
}

async fn fork(args: ForkArgs, clarity_home: Option<PathBuf>) -> Result<()> {
    let manager = thread_manager(clarity_home).await?;
    let source_id = parse_thread_id(&args.thread_id)?;

    let snapshot = match args.before_user {
        Some(n) => ForkSnapshot::TruncateBeforeNthUserMessage(n),
        None => ForkSnapshot::Interrupted,
    };

    let new_id = manager
        .fork(source_id, snapshot, None)
        .await
        .context("Failed to fork thread")?;

    match args.output {
        OutputFormat::Json => {
            print_json(&serde_json::json!({
                "source_thread_id": source_id.to_string(),
                "new_thread_id": new_id.to_string(),
            }))?;
        }
        OutputFormat::Markdown => {
            println!("Forked `{}` → `{}`", source_id, new_id);
        }
    }
    Ok(())
}

async fn resume(args: ResumeArgs, clarity_home: Option<PathBuf>) -> Result<()> {
    let manager = thread_manager(clarity_home).await?;
    let thread_id = parse_thread_id(&args.thread_id)?;

    let mut messages = manager
        .load_llm_history(thread_id)
        .await
        .context("Failed to load thread history")?;
    messages.push(Message::user(&args.prompt));

    let provider = build_provider(
        args.provider,
        args.model.as_deref(),
        args.api_key.as_deref(),
    )
    .await
    .context("Failed to build LLM provider")?;

    let approval_mode = match args.approval {
        crate::ApprovalModeArg::Interactive => clarity_core::approval::ApprovalMode::Interactive,
        crate::ApprovalModeArg::Yolo => clarity_core::approval::ApprovalMode::Yolo,
        crate::ApprovalModeArg::Plan => clarity_core::approval::ApprovalMode::Plan,
    };

    let working_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let config = clarity_core::agent::AgentConfig::new()
        .with_max_iterations(args.max_iterations)
        .with_working_dir(working_dir);

    let registry = clarity_core::ToolRegistry::with_builtin_tools();
    let agent = clarity_core::Agent::with_config(registry, config)
        .with_llm(provider)
        .with_approval_mode(approval_mode);

    let response = agent
        .run_with_messages_sync(messages)
        .await
        .context("Agent execution failed")?;

    manager
        .append_turn(thread_id, &args.prompt, &response)
        .await
        .context("Failed to persist turn to thread")?;

    match args.output {
        OutputFormat::Json => {
            print_json(&serde_json::json!({
                "thread_id": thread_id.to_string(),
                "response": response,
            }))?;
        }
        OutputFormat::Markdown => {
            println!("{}", response);
        }
    }
    Ok(())
}

#[cfg(feature = "session-migration")]
async fn migrate(args: MigrateArgs, clarity_home: Option<PathBuf>) -> Result<()> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let clarity_home = clarity_home.unwrap_or_else(|| cwd.join(".clarity"));
    let config = RolloutConfig {
        clarity_home: clarity_home.clone(),
        sqlite_home: clarity_home.clone(),
        cwd,
        model_provider_id: String::new(),
        generate_memories: false,
    };

    let migrator = tokio::task::spawn_blocking(move || {
        clarity_core::session::thread_migration::ThreadMigrator::new(&args.v1_db, config)
    })
    .await
    .context("migration initialization panicked")??;

    let report = migrator.migrate().await.context("Migration failed")?;

    match args.output {
        OutputFormat::Json => print_json(&serde_json::json!({
            "sessions_migrated": report.sessions_migrated,
            "messages_migrated": report.messages_migrated,
            "errors": report.errors,
        }))?,
        OutputFormat::Markdown => {
            println!("# Migration Report");
            println!("- Sessions migrated: {}", report.sessions_migrated);
            println!("- Messages migrated: {}", report.messages_migrated);
            if !report.errors.is_empty() {
                println!("\n## Errors");
                for e in &report.errors {
                    println!("- {}", e);
                }
            }
        }
    }
    Ok(())
}

// ------------------------------------------------------------------
// Tests
// ------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_thread_id_valid() {
        let id_str = "550e8400-e29b-41d4-a716-446655440000";
        assert!(parse_thread_id(id_str).is_ok());
    }

    #[test]
    fn test_parse_thread_id_invalid() {
        assert!(parse_thread_id("not-a-uuid").is_err());
    }

    #[test]
    fn test_format_time() {
        let ts = Utc::now();
        let s = format_time(ts);
        assert!(s.contains('T'));
    }

    #[tokio::test]
    async fn test_thread_lifecycle_via_manager() {
        let tmp = tempfile::tempdir().unwrap();
        let manager = thread_manager(Some(tmp.path().to_path_buf()))
            .await
            .expect("thread manager");

        let thread_id = manager
            .create_thread(
                tmp.path(),
                "test",
                Some(clarity_contract::SessionSource::Test),
            )
            .await
            .expect("create thread");

        let list = manager
            .list_threads(10, false, None)
            .await
            .expect("list threads");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].thread_id, thread_id);

        manager
            .update_metadata(
                thread_id,
                clarity_thread_store::ThreadMetadataPatch {
                    title: Some("test thread".to_string()),
                    archived: None,
                    extra: std::collections::HashMap::new(),
                },
            )
            .await
            .expect("update metadata");

        let stored = manager
            .read_thread(thread_id, false)
            .await
            .expect("read thread");
        assert_eq!(stored.title, Some("test thread".to_string()));

        manager.archive(thread_id).await.expect("archive");
        let archived = manager
            .list_threads(10, true, None)
            .await
            .expect("list archived");
        assert!(archived.iter().any(|t| t.thread_id == thread_id));

        manager.delete(thread_id).await.expect("delete");
        let list_after = manager
            .list_threads(10, true, None)
            .await
            .expect("list after delete");
        assert!(!list_after.iter().any(|t| t.thread_id == thread_id));
    }

    #[tokio::test]
    async fn test_fork_thread_via_manager() {
        let tmp = tempfile::tempdir().unwrap();
        let manager = thread_manager(Some(tmp.path().to_path_buf()))
            .await
            .expect("thread manager");

        let source_id = manager
            .create_thread(
                tmp.path(),
                "test",
                Some(clarity_contract::SessionSource::Test),
            )
            .await
            .expect("create thread");

        let new_id = manager
            .fork(source_id, ForkSnapshot::Interrupted, None)
            .await
            .expect("fork thread");

        assert_ne!(source_id, new_id);

        let list = manager
            .list_threads(10, false, None)
            .await
            .expect("list threads");
        assert_eq!(list.len(), 2);
    }
}
