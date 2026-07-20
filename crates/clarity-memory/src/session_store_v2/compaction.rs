//! Compacted context snapshots for `SessionStoreV2`.

use chrono::Utc;
use rusqlite::OptionalExtension;

use crate::session_store_v2::session::SessionStoreV2;
use crate::types::Result as MemoryResult;

/// A compacted context snapshot.
#[derive(Debug, Clone, PartialEq)]
pub struct CompactedContext {
    /// Session identifier
    pub session_id: String,
    /// Turn identifier up to which the context was compacted
    pub turn_id: i64,
    /// Last event identifier included in the compaction
    pub event_id: i64,
    /// Compacted context as JSON
    pub context_json: serde_json::Value,
    /// Method used to compress the context
    pub compression_method: String,
    /// Hash of the source events used to build the snapshot
    pub source_hash: String,
    /// Timestamp when the snapshot was created (milliseconds since epoch)
    pub created_at: i64,
}

impl SessionStoreV2 {
    // ------------------------------------------------------------------
    // Compacted context
    // ------------------------------------------------------------------

    /// Store a compacted context snapshot.
    pub fn store_compacted_context(
        &self,
        session_id: &str,
        turn_id: i64,
        event_id: i64,
        context_json: &serde_json::Value,
        compression_method: &str,
        source_hash: &str,
    ) -> MemoryResult<()> {
        let context_bytes = serde_json::to_vec(context_json)?;
        let now = Utc::now().timestamp_millis();

        self.conn.execute(
            "INSERT OR REPLACE INTO compacted_context
         (session_id, turn_id, event_id, context_json, compression_method, source_hash, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                session_id,
                turn_id,
                event_id,
                context_bytes,
                compression_method,
                source_hash,
                now,
            ],
        )?;
        Ok(())
    }

    /// Load the latest compacted context for a session.
    pub fn load_compacted_context(
        &self,
        session_id: &str,
    ) -> MemoryResult<Option<CompactedContext>> {
        let mut stmt = self.conn.prepare(
        "SELECT session_id, turn_id, event_id, context_json, compression_method, source_hash, created_at
         FROM compacted_context WHERE session_id = ?1"
    )?;
        let row = stmt
            .query_row([session_id], |row| {
                let context_bytes: Vec<u8> = row.get(3)?;
                let context_json =
                    serde_json::from_slice(&context_bytes).unwrap_or(serde_json::Value::Null);
                Ok(CompactedContext {
                    session_id: row.get(0)?,
                    turn_id: row.get(1)?,
                    event_id: row.get(2)?,
                    context_json,
                    compression_method: row.get(4)?,
                    source_hash: row.get(5)?,
                    created_at: row.get(6)?,
                })
            })
            .optional()?;
        Ok(row)
    }
}
