//! Append-only event log for `SessionStoreV2`.

use chrono::Utc;

use crate::session_store_v2::payload_hash;
use crate::session_store_v2::session::SessionStoreV2;
use crate::types::Result as MemoryResult;

/// Event type for the append-only event log.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventType {
    /// A message sent by the user
    UserMessage,
    /// A message sent by the assistant
    AssistantMessage,
    /// A tool call
    ToolCall,
    /// A successful tool result
    ToolResult,
    /// A tool error
    ToolError,
    /// A compaction event
    Compaction,
    /// A configuration change
    ConfigChange,
    /// Session start marker
    SessionStart,
    /// Session end marker
    SessionEnd,
    /// Unknown or unrecognized event type
    Unknown,
}

impl EventType {
    /// Return the string representation stored in the database
    pub fn as_str(&self) -> &'static str {
        match self {
            EventType::UserMessage => "user_message",
            EventType::AssistantMessage => "assistant_message",
            EventType::ToolCall => "tool_call",
            EventType::ToolResult => "tool_result",
            EventType::ToolError => "tool_error",
            EventType::Compaction => "compaction",
            EventType::ConfigChange => "config_change",
            EventType::SessionStart => "session_start",
            EventType::SessionEnd => "session_end",
            EventType::Unknown => "unknown",
        }
    }
}

impl std::str::FromStr for EventType {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "user_message" => Ok(EventType::UserMessage),
            "assistant_message" => Ok(EventType::AssistantMessage),
            "tool_call" => Ok(EventType::ToolCall),
            "tool_result" => Ok(EventType::ToolResult),
            "tool_error" => Ok(EventType::ToolError),
            "compaction" => Ok(EventType::Compaction),
            "config_change" => Ok(EventType::ConfigChange),
            "session_start" => Ok(EventType::SessionStart),
            "session_end" => Ok(EventType::SessionEnd),
            _ => Ok(EventType::Unknown),
        }
    }
}

/// A single event record from the event log.
#[derive(Debug, Clone, PartialEq)]
pub struct EventRecord {
    /// Session identifier
    pub session_id: String,
    /// Turn identifier within the session
    pub turn_id: i64,
    /// Event identifier within the turn
    pub event_id: i64,
    /// Timestamp when the event was recorded (milliseconds since epoch)
    pub timestamp: i64,
    /// Type of event
    pub event_type: EventType,
    /// Event payload
    pub payload: serde_json::Value,
    /// Hash of the payload bytes
    pub payload_hash: String,
}

impl SessionStoreV2 {
    // ------------------------------------------------------------------
    // Event log (append-only)
    // ------------------------------------------------------------------

    /// Append an event to the event log.
    pub fn append_event(
        &self,
        session_id: &str,
        turn_id: i64,
        event_type: EventType,
        payload: &serde_json::Value,
    ) -> MemoryResult<()> {
        let payload_bytes = serde_json::to_vec(payload)?;
        let payload_hash = format!("{:016x}", payload_hash::hash(&payload_bytes));
        let now = Utc::now().timestamp_millis();

        // event_id = max existing + 1 within this session.
        let next_event_id: i64 = self
            .conn
            .query_row(
                "SELECT COALESCE(MAX(event_id), 0) + 1 FROM event_log WHERE session_id = ?1",
                [session_id],
                |row| row.get(0),
            )
            .unwrap_or(1);

        self.conn.execute(
        "INSERT INTO event_log (session_id, turn_id, event_id, timestamp, event_type, payload, payload_hash)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![
            session_id,
            turn_id,
            next_event_id,
            now,
            event_type.as_str(),
            payload_bytes,
            payload_hash,
        ],
    )?;

        // Update session updated_at.
        self.conn.execute(
            "UPDATE sessions_v2 SET updated_at = ?1 WHERE id = ?2",
            rusqlite::params![now, session_id],
        )?;

        Ok(())
    }

    /// Read all events for a session, ordered by (turn_id, event_id).
    pub fn read_events(&self, session_id: &str) -> MemoryResult<Vec<EventRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT session_id, turn_id, event_id, timestamp, event_type, payload, payload_hash
         FROM event_log WHERE session_id = ?1
         ORDER BY turn_id ASC, event_id ASC",
        )?;
        let events = stmt
            .query_map([session_id], |row| {
                let payload: Vec<u8> = row.get(5)?;
                let payload_json =
                    serde_json::from_slice(&payload).unwrap_or(serde_json::Value::Null);
                Ok(EventRecord {
                    session_id: row.get(0)?,
                    turn_id: row.get(1)?,
                    event_id: row.get(2)?,
                    timestamp: row.get(3)?,
                    event_type: row
                        .get::<_, String>(4)?
                        .parse()
                        .unwrap_or(EventType::Unknown),
                    payload: payload_json,
                    payload_hash: row.get(6)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(events)
    }

    /// Read events up to a specific (turn_id, event_id) inclusive.
    pub fn read_events_until(
        &self,
        session_id: &str,
        turn_id: i64,
        event_id: i64,
    ) -> MemoryResult<Vec<EventRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT session_id, turn_id, event_id, timestamp, event_type, payload, payload_hash
         FROM event_log WHERE session_id = ?1
           AND (turn_id < ?2 OR (turn_id = ?3 AND event_id <= ?4))
         ORDER BY turn_id ASC, event_id ASC",
        )?;
        let events = stmt
            .query_map(
                rusqlite::params![session_id, turn_id, turn_id, event_id],
                |row| {
                    let payload: Vec<u8> = row.get(5)?;
                    let payload_json =
                        serde_json::from_slice(&payload).unwrap_or(serde_json::Value::Null);
                    Ok(EventRecord {
                        session_id: row.get(0)?,
                        turn_id: row.get(1)?,
                        event_id: row.get(2)?,
                        timestamp: row.get(3)?,
                        event_type: row
                            .get::<_, String>(4)?
                            .parse()
                            .unwrap_or(EventType::Unknown),
                        payload: payload_json,
                        payload_hash: row.get(6)?,
                    })
                },
            )?
            .collect::<Result<_, _>>()?;
        Ok(events)
    }
}
