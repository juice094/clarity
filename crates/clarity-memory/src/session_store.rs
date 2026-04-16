//! JSONL-based session storage

use crate::types::{Message, Result, SessionRecord};
use chrono::Utc;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use tracing::{debug, info, instrument, warn};

/// Stores conversation sessions as JSONL files
#[derive(Debug, Clone)]
pub struct SessionStore {
    sessions_dir: PathBuf,
}

impl SessionStore {
    /// Create a new SessionStore
    ///
    /// The sessions directory will be created if it doesn't exist.
    pub fn new(sessions_dir: impl AsRef<Path>) -> Result<Self> {
        let sessions_dir = sessions_dir.as_ref().to_path_buf();

        if !sessions_dir.exists() {
            fs::create_dir_all(&sessions_dir)?;
            info!("Created sessions directory at {:?}", sessions_dir);
        }

        Ok(Self { sessions_dir })
    }

    /// Get the path to a session file
    fn session_path(&self, session_id: &str) -> PathBuf {
        // Sanitize session_id for filesystem safety
        let sanitized = session_id.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
        self.sessions_dir.join(format!("{}.jsonl", sanitized))
    }

    /// Append a message to a session
    #[instrument(skip(self, content))]
    pub fn append_message(&self, session_id: &str, role: &str, content: &str) -> Result<()> {
        let path = self.session_path(session_id);
        let record = SessionRecord::Message {
            message: crate::types::MessageRecord {
                role: role.to_string(),
                content: content.to_string(),
            },
            timestamp: Utc::now(),
        };

        let mut file = OpenOptions::new().create(true).append(true).open(&path)?;

        let line = serde_json::to_string(&record)?;
        writeln!(file, "{}", line)?;

        debug!("Appended message to session {}", session_id);
        Ok(())
    }

    /// Append a summary to a session
    #[instrument(skip(self, content))]
    pub fn append_summary(&self, session_id: &str, content: &str) -> Result<()> {
        let path = self.session_path(session_id);
        let record = SessionRecord::Summary {
            content: content.to_string(),
            timestamp: Utc::now(),
        };

        let mut file = OpenOptions::new().create(true).append(true).open(&path)?;

        let line = serde_json::to_string(&record)?;
        writeln!(file, "{}", line)?;

        debug!("Appended summary to session {}", session_id);
        Ok(())
    }

    /// Get all messages from a session
    #[instrument(skip(self))]
    pub fn get_messages(&self, session_id: &str) -> Result<Vec<Message>> {
        let path = self.session_path(session_id);

        if !path.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(&path)?;
        let reader = BufReader::new(file);
        let mut messages = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }

            match serde_json::from_str::<SessionRecord>(&line) {
                Ok(record) => {
                    if let Some(msg) = Option::<Message>::from(record) {
                        messages.push(msg);
                    }
                }
                Err(e) => {
                    warn!("Failed to parse session record: {}", e);
                }
            }
        }

        Ok(messages)
    }

    /// Get all session records (including summaries)
    pub fn get_all_records(&self, session_id: &str) -> Result<Vec<SessionRecord>> {
        let path = self.session_path(session_id);

        if !path.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(&path)?;
        let reader = BufReader::new(file);
        let mut records = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }

            match serde_json::from_str::<SessionRecord>(&line) {
                Ok(record) => records.push(record),
                Err(e) => {
                    warn!("Failed to parse session record: {}", e);
                }
            }
        }

        Ok(records)
    }

    /// Get all session file paths
    pub fn get_session_paths(&self) -> Vec<PathBuf> {
        let mut paths = Vec::new();

        if let Ok(entries) = fs::read_dir(&self.sessions_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "jsonl").unwrap_or(false) {
                    paths.push(path);
                }
            }
        }

        paths.sort();
        paths
    }

    /// Get all session IDs
    pub fn get_session_ids(&self) -> Vec<String> {
        self.get_session_paths()
            .iter()
            .filter_map(|p| {
                p.file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
            })
            .collect()
    }

    /// Check if a session exists
    pub fn session_exists(&self, session_id: &str) -> bool {
        self.session_path(session_id).exists()
    }

    /// Get message count for a session
    pub fn get_message_count(&self, session_id: &str) -> Result<usize> {
        Ok(self.get_messages(session_id)?.len())
    }

    /// Delete a session
    pub fn delete_session(&self, session_id: &str) -> Result<bool> {
        let path = self.session_path(session_id);

        if path.exists() {
            fs::remove_file(&path)?;
            info!("Deleted session {}", session_id);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Get raw content of a session file
    pub fn get_session_content(&self, session_id: &str) -> Result<String> {
        let path = self.session_path(session_id);

        if !path.exists() {
            return Ok(String::new());
        }

        Ok(fs::read_to_string(&path)?)
    }

    /// Read recent messages from all sessions (for compilation)
    pub fn read_all_sessions(
        &self,
        since: Option<chrono::DateTime<Utc>>,
    ) -> Result<Vec<(String, Vec<Message>)>> {
        let mut result = Vec::new();

        for session_id in self.get_session_ids() {
            let messages = self.get_messages(&session_id)?;

            let filtered: Vec<Message> = if let Some(since_time) = since {
                messages
                    .into_iter()
                    .filter(|m| m.timestamp > since_time)
                    .collect()
            } else {
                messages
            };

            if !filtered.is_empty() {
                result.push((session_id, filtered));
            }
        }

        Ok(result)
    }

    /// Calculate a fingerprint of session content for change detection
    pub fn calculate_fingerprint(&self, session_id: &str) -> Result<Option<String>> {
        use sha2::{Digest, Sha256};

        let content = self.get_session_content(session_id)?;

        if content.is_empty() {
            return Ok(None);
        }

        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let hash = hex::encode(hasher.finalize());

        Ok(Some(hash))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_store() -> (TempDir, SessionStore) {
        let temp_dir = TempDir::new().unwrap();
        let store = SessionStore::new(temp_dir.path()).unwrap();
        (temp_dir, store)
    }

    #[test]
    fn test_append_and_get_messages() {
        let (_temp, store) = create_test_store();

        store.append_message("session-1", "user", "Hello").unwrap();
        store
            .append_message("session-1", "assistant", "Hi there!")
            .unwrap();
        store
            .append_message("session-1", "user", "How are you?")
            .unwrap();

        let messages = store.get_messages("session-1").unwrap();
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].content, "Hello");
        assert_eq!(messages[1].role, "assistant");
        assert_eq!(messages[2].content, "How are you?");
    }

    #[test]
    fn test_multiple_sessions() {
        let (_temp, store) = create_test_store();

        store
            .append_message("session-a", "user", "Message A")
            .unwrap();
        store
            .append_message("session-b", "user", "Message B")
            .unwrap();

        let messages_a = store.get_messages("session-a").unwrap();
        let messages_b = store.get_messages("session-b").unwrap();

        assert_eq!(messages_a.len(), 1);
        assert_eq!(messages_a[0].content, "Message A");

        assert_eq!(messages_b.len(), 1);
        assert_eq!(messages_b[0].content, "Message B");
    }

    #[test]
    fn test_get_session_ids() {
        let (_temp, store) = create_test_store();

        store.append_message("alpha", "user", "test").unwrap();
        store.append_message("beta", "user", "test").unwrap();
        store.append_message("gamma", "user", "test").unwrap();

        let ids = store.get_session_ids();
        assert_eq!(ids.len(), 3);
        assert!(ids.contains(&"alpha".to_string()));
        assert!(ids.contains(&"beta".to_string()));
        assert!(ids.contains(&"gamma".to_string()));
    }

    #[test]
    fn test_delete_session() {
        let (_temp, store) = create_test_store();

        store.append_message("to-delete", "user", "test").unwrap();
        assert!(store.session_exists("to-delete"));

        assert!(store.delete_session("to-delete").unwrap());
        assert!(!store.session_exists("to-delete"));

        // Deleting non-existent should return false
        assert!(!store.delete_session("non-existent").unwrap());
    }

    #[test]
    fn test_empty_session() {
        let (_temp, store) = create_test_store();

        let messages = store.get_messages("non-existent").unwrap();
        assert!(messages.is_empty());
    }

    #[test]
    fn test_append_summary() {
        let (_temp, store) = create_test_store();

        store.append_message("session", "user", "Hello").unwrap();
        store
            .append_summary("session", "Summary of conversation")
            .unwrap();
        store.append_message("session", "user", "Goodbye").unwrap();

        // get_messages should filter out summaries
        let messages = store.get_messages("session").unwrap();
        assert_eq!(messages.len(), 2);

        // get_all_records should include summaries
        let records = store.get_all_records("session").unwrap();
        assert_eq!(records.len(), 3);
    }

    #[test]
    fn test_fingerprint() {
        let (_temp, store) = create_test_store();

        // Empty session has no fingerprint
        let fp1 = store.calculate_fingerprint("session").unwrap();
        assert!(fp1.is_none());

        // After adding content, we get a fingerprint
        store.append_message("session", "user", "Hello").unwrap();
        let fp2 = store.calculate_fingerprint("session").unwrap();
        assert!(fp2.is_some());

        // Same content should produce same fingerprint
        let fp3 = store.calculate_fingerprint("session").unwrap();
        assert_eq!(fp2, fp3);

        // Different content should produce different fingerprint
        store.append_message("session", "user", "World").unwrap();
        let fp4 = store.calculate_fingerprint("session").unwrap();
        assert_ne!(fp2, fp4);
    }

    #[test]
    fn test_jsonl_format() {
        let (_temp, store) = create_test_store();

        store.append_message("test", "user", "Hello").unwrap();
        store.append_message("test", "assistant", "Hi!").unwrap();

        // Read raw content
        let content = store.get_session_content("test").unwrap();
        let lines: Vec<&str> = content.lines().collect();

        assert_eq!(lines.len(), 2);

        // Verify each line is valid JSON
        for line in &lines {
            let record: SessionRecord = serde_json::from_str(line).unwrap();
            match record {
                SessionRecord::Message { message, .. } => {
                    assert!(!message.role.is_empty());
                    assert!(!message.content.is_empty());
                }
                _ => panic!("Expected message record"),
            }
        }
    }
}
