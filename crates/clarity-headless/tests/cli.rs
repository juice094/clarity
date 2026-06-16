#![cfg_attr(
    test,
    allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)
)]
//! Process-level integration tests for the `clarity-headless` CLI.

use std::ffi::OsStr;
use std::process::Command;

/// Path to the compiled `clarity-headless` binary, injected by Cargo at compile time.
const BIN: &str = env!("CARGO_BIN_EXE_clarity-headless");

/// Spawn the binary with the provided arguments and return captured stdout/stderr.
///
/// Panics if the process fails to spawn or exits with a non-zero status code.
fn run<I, S>(args: I) -> (String, String)
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = Command::new(BIN)
        .args(args)
        .output()
        .expect("failed to spawn clarity-headless");

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

    if !output.status.success() {
        panic!(
            "clarity-headless exited with {}\nstdout:\n{}\nstderr:\n{}",
            output.status, stdout, stderr
        );
    }

    (stdout, stderr)
}

#[test]
fn test_threads_create_and_list() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let home = tmp.path().to_string_lossy().to_string();
    let home_ref: &str = home.as_str();

    let create_args: Vec<&str> = vec![
        "threads",
        "--clarity-home",
        home_ref,
        "create",
        "--title",
        "cli test thread",
        "--output",
        "json",
    ];
    let (stdout, _stderr) = run(create_args);
    let created: serde_json::Value =
        serde_json::from_str(&stdout).expect("create output is not valid JSON");
    let thread_id = created["thread_id"]
        .as_str()
        .expect("thread_id missing or not a string")
        .to_string();

    let list_args: Vec<&str> = vec![
        "threads",
        "--clarity-home",
        home_ref,
        "list",
        "--output",
        "json",
    ];
    let (list_stdout, _list_stderr) = run(list_args);
    let list: Vec<serde_json::Value> =
        serde_json::from_str(&list_stdout).expect("list output is not valid JSON");
    assert!(
        list.iter()
            .any(|t| t["thread_id"].as_str() == Some(thread_id.as_str())),
        "created thread {} not found in list: {}",
        thread_id,
        list_stdout
    );
}

#[test]
fn test_threads_show_after_create() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let home = tmp.path().to_string_lossy().to_string();
    let home_ref: &str = home.as_str();

    let create_args: Vec<&str> = vec![
        "threads",
        "--clarity-home",
        home_ref,
        "create",
        "--title",
        "show me",
        "--output",
        "json",
    ];
    let (stdout, _stderr) = run(create_args);
    let created: serde_json::Value =
        serde_json::from_str(&stdout).expect("create output is not valid JSON");
    let thread_id = created["thread_id"]
        .as_str()
        .expect("thread_id missing or not a string")
        .to_string();

    let show_args: Vec<&str> = vec![
        "threads",
        "--clarity-home",
        home_ref,
        "show",
        thread_id.as_str(),
        "--output",
        "json",
    ];
    let (show_stdout, _show_stderr) = run(show_args);
    let shown: serde_json::Value =
        serde_json::from_str(&show_stdout).expect("show output is not valid JSON");
    assert_eq!(shown["thread_id"].as_str(), Some(thread_id.as_str()));
    assert_eq!(shown["title"].as_str(), Some("show me"));
}

#[test]
fn test_threads_archive_and_list_archived() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let home = tmp.path().to_string_lossy().to_string();
    let home_ref: &str = home.as_str();

    let create_args: Vec<&str> = vec![
        "threads",
        "--clarity-home",
        home_ref,
        "create",
        "--title",
        "to archive",
        "--output",
        "json",
    ];
    let (stdout, _stderr) = run(create_args);
    let created: serde_json::Value =
        serde_json::from_str(&stdout).expect("create output is not valid JSON");
    let thread_id = created["thread_id"]
        .as_str()
        .expect("thread_id missing or not a string")
        .to_string();

    let archive_args: Vec<&str> = vec![
        "threads",
        "--clarity-home",
        home_ref,
        "archive",
        thread_id.as_str(),
        "--output",
        "json",
    ];
    let (archive_stdout, _archive_stderr) = run(archive_args);
    let archived: serde_json::Value =
        serde_json::from_str(&archive_stdout).expect("archive output is not valid JSON");
    assert_eq!(archived["archived"].as_bool(), Some(true));

    let list_args: Vec<&str> = vec![
        "threads",
        "--clarity-home",
        home_ref,
        "list",
        "--archived",
        "--output",
        "json",
    ];
    let (list_stdout, _list_stderr) = run(list_args);
    let list: Vec<serde_json::Value> =
        serde_json::from_str(&list_stdout).expect("list output is not valid JSON");
    assert!(
        list.iter()
            .any(|t| t["thread_id"].as_str() == Some(thread_id.as_str())),
        "archived thread {} not found in archived list: {}",
        thread_id,
        list_stdout
    );
}

#[test]
fn test_threads_delete() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let home = tmp.path().to_string_lossy().to_string();
    let home_ref: &str = home.as_str();

    let create_args: Vec<&str> = vec![
        "threads",
        "--clarity-home",
        home_ref,
        "create",
        "--title",
        "to delete",
        "--output",
        "json",
    ];
    let (stdout, _stderr) = run(create_args);
    let created: serde_json::Value =
        serde_json::from_str(&stdout).expect("create output is not valid JSON");
    let thread_id = created["thread_id"]
        .as_str()
        .expect("thread_id missing or not a string")
        .to_string();

    let delete_args: Vec<&str> = vec![
        "threads",
        "--clarity-home",
        home_ref,
        "delete",
        thread_id.as_str(),
        "--output",
        "json",
    ];
    let (delete_stdout, _delete_stderr) = run(delete_args);
    let deleted: serde_json::Value =
        serde_json::from_str(&delete_stdout).expect("delete output is not valid JSON");
    assert_eq!(deleted["deleted"].as_bool(), Some(true));

    let list_args: Vec<&str> = vec![
        "threads",
        "--clarity-home",
        home_ref,
        "list",
        "--output",
        "json",
    ];
    let (list_stdout, _list_stderr) = run(list_args);
    let list: Vec<serde_json::Value> =
        serde_json::from_str(&list_stdout).expect("list output is not valid JSON");
    assert!(
        !list
            .iter()
            .any(|t| t["thread_id"].as_str() == Some(thread_id.as_str())),
        "deleted thread {} still present in list: {}",
        thread_id,
        list_stdout
    );
}

#[test]
fn test_threads_fork() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let home = tmp.path().to_string_lossy().to_string();
    let home_ref: &str = home.as_str();

    let create_args: Vec<&str> = vec![
        "threads",
        "--clarity-home",
        home_ref,
        "create",
        "--title",
        "fork source",
        "--output",
        "json",
    ];
    let (stdout, _stderr) = run(create_args);
    let created: serde_json::Value =
        serde_json::from_str(&stdout).expect("create output is not valid JSON");
    let thread_id = created["thread_id"]
        .as_str()
        .expect("thread_id missing or not a string")
        .to_string();

    let fork_args: Vec<&str> = vec![
        "threads",
        "--clarity-home",
        home_ref,
        "fork",
        thread_id.as_str(),
        "--output",
        "json",
    ];
    let (fork_stdout, _fork_stderr) = run(fork_args);
    let forked: serde_json::Value =
        serde_json::from_str(&fork_stdout).expect("fork output is not valid JSON");
    let new_thread_id = forked["new_thread_id"]
        .as_str()
        .expect("new_thread_id missing or not a string")
        .to_string();
    assert_ne!(new_thread_id, thread_id);

    let show_args: Vec<&str> = vec![
        "threads",
        "--clarity-home",
        home_ref,
        "show",
        new_thread_id.as_str(),
        "--output",
        "json",
    ];
    let (show_stdout, _show_stderr) = run(show_args);
    let shown: serde_json::Value =
        serde_json::from_str(&show_stdout).expect("show output is not valid JSON");
    assert_eq!(shown["thread_id"].as_str(), Some(new_thread_id.as_str()));
}

#[test]
fn test_threads_fork_before_user() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let home = tmp.path().to_string_lossy().to_string();
    let home_ref: &str = home.as_str();

    let create_args: Vec<&str> = vec![
        "threads",
        "--clarity-home",
        home_ref,
        "create",
        "--title",
        "fork before user source",
        "--output",
        "json",
    ];
    let (stdout, _stderr) = run(create_args);
    let created: serde_json::Value =
        serde_json::from_str(&stdout).expect("create output is not valid JSON");
    let thread_id = created["thread_id"]
        .as_str()
        .expect("thread_id missing or not a string")
        .to_string();

    let fork_args: Vec<&str> = vec![
        "threads",
        "--clarity-home",
        home_ref,
        "fork",
        thread_id.as_str(),
        "--before-user",
        "1",
        "--output",
        "json",
    ];
    let (fork_stdout, _fork_stderr) = run(fork_args);
    let forked: serde_json::Value =
        serde_json::from_str(&fork_stdout).expect("fork output is not valid JSON");
    let new_thread_id = forked["new_thread_id"]
        .as_str()
        .expect("new_thread_id missing or not a string")
        .to_string();

    let show_args: Vec<&str> = vec![
        "threads",
        "--clarity-home",
        home_ref,
        "show",
        new_thread_id.as_str(),
        "--history",
        "--output",
        "json",
    ];
    let (show_stdout, _show_stderr) = run(show_args);
    let shown: serde_json::Value =
        serde_json::from_str(&show_stdout).expect("show output is not valid JSON");
    let history = shown["history"]
        .as_array()
        .expect("history missing or not an array");
    assert_eq!(history.len(), 1, "expected exactly one history item");
    assert_eq!(
        history[0]["type"].as_str(),
        Some("session_meta"),
        "expected only SessionMeta item, got {:?}",
        history[0]
    );
}

#[test]
fn test_threads_show_history() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let home = tmp.path().to_string_lossy().to_string();
    let home_ref: &str = home.as_str();

    let create_args: Vec<&str> = vec![
        "threads",
        "--clarity-home",
        home_ref,
        "create",
        "--title",
        "history check",
        "--output",
        "json",
    ];
    let (stdout, _stderr) = run(create_args);
    let created: serde_json::Value =
        serde_json::from_str(&stdout).expect("create output is not valid JSON");
    let thread_id = created["thread_id"]
        .as_str()
        .expect("thread_id missing or not a string")
        .to_string();

    let show_args: Vec<&str> = vec![
        "threads",
        "--clarity-home",
        home_ref,
        "show",
        thread_id.as_str(),
        "--history",
        "--output",
        "json",
    ];
    let (show_stdout, _show_stderr) = run(show_args);
    let shown: serde_json::Value =
        serde_json::from_str(&show_stdout).expect("show output is not valid JSON");
    let history = shown["history"]
        .as_array()
        .expect("history missing or not an array");
    assert!(
        !history.is_empty(),
        "expected at least one history item, got {}",
        history.len()
    );
}
