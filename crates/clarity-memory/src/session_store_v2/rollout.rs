//! Rollout index tracking for `SessionStoreV2`.

use chrono::Utc;
use rusqlite::OptionalExtension;

use crate::session_store_v2::session::SessionStoreV2;
use crate::types::Result as MemoryResult;

impl SessionStoreV2 {
    /// Register or update the on-disk rollout path for a thread.
    ///
    /// ponytail: assumes sessions_v2 row exists; caller should create the session
    /// first. Upgrade to lazy session creation if this becomes a nuisance.
    pub fn register_rollout(
        &self,
        thread_id: &str,
        rollout_path: &std::path::Path,
        last_seq: i64,
    ) -> MemoryResult<()> {
        let now = Utc::now().timestamp_millis();
        self.conn.execute(
            "INSERT INTO rollout_index (thread_id, rollout_path, last_seq, updated_at)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(thread_id) DO UPDATE SET
             rollout_path = excluded.rollout_path,
             last_seq = excluded.last_seq,
             updated_at = excluded.updated_at",
            rusqlite::params![
                thread_id,
                rollout_path.as_os_str().to_string_lossy().as_ref(),
                last_seq,
                now,
            ],
        )?;
        Ok(())
    }

    /// Retrieve the registered rollout path and last known sequence for a thread.
    pub fn get_rollout(&self, thread_id: &str) -> MemoryResult<Option<(std::path::PathBuf, i64)>> {
        let row = self
            .conn
            .query_row(
                "SELECT rollout_path, last_seq FROM rollout_index WHERE thread_id = ?1",
                [thread_id],
                |row| {
                    let path: String = row.get(0)?;
                    let last_seq: i64 = row.get(1)?;
                    Ok((std::path::PathBuf::from(path), last_seq))
                },
            )
            .optional()?;
        Ok(row)
    }

    /// Update only the last seen sequence number for a thread's rollout.
    pub fn update_rollout_seq(&self, thread_id: &str, last_seq: i64) -> MemoryResult<()> {
        let now = Utc::now().timestamp_millis();
        self.conn.execute(
            "UPDATE rollout_index SET last_seq = ?2, updated_at = ?3 WHERE thread_id = ?1",
            rusqlite::params![thread_id, last_seq, now],
        )?;
        Ok(())
    }

    /// Delete a session and all associated events / compacted contexts (cascade).
    pub fn delete_session(&self, session_id: &str) -> MemoryResult<()> {
        self.conn
            .execute("DELETE FROM sessions_v2 WHERE id = ?1", [session_id])?;
        Ok(())
    }
}
