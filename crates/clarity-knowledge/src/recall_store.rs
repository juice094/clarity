//! Persistence for the recall-to-action feedback loop.
//!
//! Records which memories were recalled for which query, then correlates them
//! with outcome signals from the same session. The resulting effectiveness
//! score can be used to boost or dampen node importance in the knowledge graph.
//!
//! This is an intentionally minimal SQLite-backed implementation. It can be
//! replaced by a richer store once the feedback semantics are validated.

use crate::error::{KnowledgeError, Result};
use chrono::Utc;
use rusqlite::{Connection, OptionalExtension};
use serde_json;
use std::path::Path;
use std::time::Duration;

/// Types of outcome signals that can be correlated with a recall event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutcomeSignalType {
    /// N subsequent tool calls completed without errors.
    ErrorFreeRun,
    /// Tests passed after code changes informed by the recall.
    TestPass,
    /// No self-corrections were observed in the window.
    NoCorrections,
    /// The session ended with zero errors.
    SessionSuccess,
    /// The session ended with one or more errors.
    SessionFailure,
    /// A previous decision was reversed (negative signal).
    Correction,
    /// The agent explicitly marked the memory as helpful.
    ExplicitBoost,
}

impl OutcomeSignalType {
    fn as_str(&self) -> &'static str {
        match self {
            OutcomeSignalType::ErrorFreeRun => "error_free_run",
            OutcomeSignalType::TestPass => "test_pass",
            OutcomeSignalType::NoCorrections => "no_corrections",
            OutcomeSignalType::SessionSuccess => "session_success",
            OutcomeSignalType::SessionFailure => "session_failure",
            OutcomeSignalType::Correction => "correction",
            OutcomeSignalType::ExplicitBoost => "explicit_boost",
        }
    }
}

/// A single recall event: what was queried and which memories were returned.
#[derive(Debug, Clone)]
pub struct RecallEvent {
    /// Unique event id. If `None`, the store assigns a ULID-style id.
    pub id: Option<String>,
    /// Session that issued the recall.
    pub session_id: Option<String>,
    /// The raw query text.
    pub query: String,
    /// Stable identifiers of the returned memories (e.g. file paths).
    pub memory_ids: Vec<String>,
    /// Optional project / vault context.
    pub project: Option<String>,
}

/// An outcome signal tied to a session and time window.
#[derive(Debug, Clone)]
pub struct OutcomeSignal {
    /// Session that produced the signal.
    pub session_id: Option<String>,
    /// Signal kind.
    pub signal_type: OutcomeSignalType,
    /// Positive = success, negative = failure.
    pub value: i32,
    /// Optional source description (tool name, hook name, etc.).
    pub source: Option<String>,
    /// Optional project / vault context.
    pub project: Option<String>,
}

/// SQLite-backed store for recall events and outcome signals.
#[derive(Debug)]
pub struct RecallStore {
    conn: Connection,
}

impl RecallStore {
    /// Open or create a recall store at `path`.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn =
            Connection::open(path).map_err(|e| KnowledgeError::Io(std::io::Error::other(e)))?;
        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    /// Open an in-memory store for tests.
    #[cfg(test)]
    fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()
            .map_err(|e| KnowledgeError::Io(std::io::Error::other(e)))?;
        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    fn init_schema(&self) -> Result<()> {
        self.conn
            .execute_batch(
                "
                CREATE TABLE IF NOT EXISTS recall_events (
                    id TEXT PRIMARY KEY,
                    session_id TEXT,
                    query TEXT NOT NULL,
                    recalled_at TEXT NOT NULL,
                    memory_ids TEXT NOT NULL,
                    memory_count INTEGER NOT NULL,
                    project TEXT
                );
                CREATE INDEX IF NOT EXISTS idx_recall_events_session
                    ON recall_events(session_id);
                CREATE INDEX IF NOT EXISTS idx_recall_events_recalled_at
                    ON recall_events(recalled_at);

                CREATE TABLE IF NOT EXISTS outcome_signals (
                    id TEXT PRIMARY KEY,
                    session_id TEXT,
                    signal_type TEXT NOT NULL,
                    signal_value INTEGER NOT NULL,
                    occurred_at TEXT NOT NULL,
                    source TEXT,
                    project TEXT
                );
                CREATE INDEX IF NOT EXISTS idx_outcome_signals_session
                    ON outcome_signals(session_id);
                CREATE INDEX IF NOT EXISTS idx_outcome_signals_occurred_at
                    ON outcome_signals(occurred_at);
                
                CREATE TABLE IF NOT EXISTS recall_effectiveness (
                    memory_id TEXT NOT NULL,
                    recall_event_id TEXT NOT NULL,
                    effectiveness REAL NOT NULL,
                    signal_count INTEGER NOT NULL,
                    computed_at TEXT NOT NULL,
                    PRIMARY KEY (memory_id, recall_event_id)
                );
                CREATE INDEX IF NOT EXISTS idx_recall_effectiveness_memory
                    ON recall_effectiveness(memory_id);
                
                CREATE TABLE IF NOT EXISTS memory_boost (
                    memory_id TEXT PRIMARY KEY,
                    score REAL NOT NULL,
                    updated_at TEXT NOT NULL
                );
                
                PRAGMA user_version = 1;
                
                DELETE FROM recall_effectiveness WHERE 1=0;
                DELETE FROM memory_boost WHERE 1=0;
                
                PRAGMA user_version = 2;
                
                ",
            )
            .map_err(|e| KnowledgeError::Io(std::io::Error::other(e)))?;
        Ok(())
    }

    /// Log a recall event and return the assigned event id.
    pub fn log_recall_event(&self, event: RecallEvent) -> Result<String> {
        let id = event.id.unwrap_or_else(|| ulid::Ulid::new().to_string());
        let recalled_at = Utc::now().to_rfc3339();
        let memory_ids = serde_json::to_string(&event.memory_ids)
            .map_err(|e| KnowledgeError::Io(std::io::Error::other(e)))?;

        self.conn
            .execute(
                "INSERT INTO recall_events
                 (id, session_id, query, recalled_at, memory_ids, memory_count, project)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                (
                    &id,
                    event.session_id.as_deref(),
                    &event.query,
                    &recalled_at,
                    &memory_ids,
                    event.memory_ids.len() as i64,
                    event.project.as_deref(),
                ),
            )
            .map_err(|e| KnowledgeError::Io(std::io::Error::other(e)))?;
        Ok(id)
    }

    /// Record an outcome signal for a session.
    pub fn record_outcome_signal(&self, signal: OutcomeSignal) -> Result<()> {
        let id = ulid::Ulid::new().to_string();
        let occurred_at = Utc::now().to_rfc3339();
        self.conn
            .execute(
                "INSERT INTO outcome_signals
                 (id, session_id, signal_type, signal_value, occurred_at, source, project)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                (
                    &id,
                    signal.session_id.as_deref(),
                    signal.signal_type.as_str(),
                    signal.value,
                    &occurred_at,
                    signal.source.as_deref(),
                    signal.project.as_deref(),
                ),
            )
            .map_err(|e| KnowledgeError::Io(std::io::Error::other(e)))?;
        Ok(())
    }

    /// Compute effectiveness scores for every (memory_id, recall_event) pair.
    ///
    /// Signals are correlated when they share the same `session_id` and occur
    /// within `window` after the recall event.
    pub fn compute_effectiveness(
        &self,
        window: Duration,
    ) -> Result<Vec<(String, String, f32, usize)>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT
                    e.id,
                    e.memory_ids,
                    e.recalled_at,
                    COALESCE(SUM(s.signal_value), 0) as total,
                    COUNT(s.id) as signal_count
                 FROM recall_events e
                 LEFT JOIN outcome_signals s
                     ON e.session_id = s.session_id
                     AND s.occurred_at >= e.recalled_at
                     AND datetime(s.occurred_at) <= datetime(e.recalled_at, ?1)
                 GROUP BY e.id, e.memory_ids, e.recalled_at",
            )
            .map_err(|e| KnowledgeError::Io(std::io::Error::other(e)))?;

        let window_spec = format!("+{} seconds", window.as_secs());
        let rows = stmt
            .query_map([&window_spec], |row| {
                let event_id: String = row.get(0)?;
                let memory_ids_json: String = row.get(1)?;
                let total: i64 = row.get(3)?;
                let signal_count: i64 = row.get(4)?;
                Ok((event_id, memory_ids_json, total, signal_count))
            })
            .map_err(|e| KnowledgeError::Io(std::io::Error::other(e)))?;

        let mut results = Vec::new();
        for row in rows {
            let (event_id, memory_ids_json, total, signal_count) =
                row.map_err(|e| KnowledgeError::Io(std::io::Error::other(e)))?;
            let memory_ids: Vec<String> = serde_json::from_str(&memory_ids_json)
                .map_err(|e| KnowledgeError::Io(std::io::Error::other(e)))?;
            let score = (total as f32 / 10.0).clamp(-1.0, 1.0);
            for memory_id in memory_ids {
                results.push((memory_id, event_id.clone(), score, signal_count as usize));
            }
        }
        Ok(results)
    }

    /// Aggregate per-memory effectiveness and return a boost score for each.
    pub fn memory_boosts(&self, window: Duration) -> Result<Vec<(String, f32)>> {
        let mut agg: std::collections::HashMap<String, Vec<f32>> = std::collections::HashMap::new();
        for (memory_id, _event_id, score, _count) in self.compute_effectiveness(window)? {
            agg.entry(memory_id).or_default().push(score);
        }

        let mut boosts: Vec<(String, f32)> = agg
            .into_iter()
            .map(|(id, scores)| {
                let avg = scores.iter().sum::<f32>() / scores.len() as f32;
                (id, avg)
            })
            .collect();
        boosts.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        Ok(boosts)
    }

    /// Persist aggregated boost scores and return the number of rows written.
    pub fn persist_boosts(&self, window: Duration) -> Result<usize> {
        let boosts = self.memory_boosts(window)?;
        let tx = self
            .conn
            .unchecked_transaction()
            .map_err(|e| KnowledgeError::Io(std::io::Error::other(e)))?;

        self.conn
            .execute("DELETE FROM memory_boost", [])
            .map_err(|e| KnowledgeError::Io(std::io::Error::other(e)))?;

        let updated_at = Utc::now().to_rfc3339();
        let mut inserted = 0;
        for (memory_id, score) in boosts {
            self.conn
                .execute(
                    "INSERT INTO memory_boost (memory_id, score, updated_at)
                     VALUES (?1, ?2, ?3)",
                    (&memory_id, score, &updated_at),
                )
                .map_err(|e| KnowledgeError::Io(std::io::Error::other(e)))?;
            inserted += 1;
        }

        tx.commit()
            .map_err(|e| KnowledgeError::Io(std::io::Error::other(e)))?;
        Ok(inserted)
    }

    /// Read the persisted boost for a single memory, if any.
    pub fn get_boost(&self, memory_id: &str) -> Result<Option<f32>> {
        self.conn
            .query_row(
                "SELECT score FROM memory_boost WHERE memory_id = ?1",
                [memory_id],
                |row| row.get::<_, f32>(0),
            )
            .optional()
            .map_err(|e| KnowledgeError::Io(std::io::Error::other(e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(query: &str, ids: &[&str]) -> RecallEvent {
        RecallEvent {
            id: None,
            session_id: Some("s1".to_string()),
            query: query.to_string(),
            memory_ids: ids.iter().map(|s| s.to_string()).collect(),
            project: None,
        }
    }

    #[test]
    fn log_and_correlate_signals() {
        let store = RecallStore::open_in_memory().unwrap();
        store
            .log_recall_event(event("rust", &["a.md", "b.md"]))
            .unwrap();

        std::thread::sleep(std::time::Duration::from_millis(5));
        store
            .record_outcome_signal(OutcomeSignal {
                session_id: Some("s1".to_string()),
                signal_type: OutcomeSignalType::SessionSuccess,
                value: 2,
                source: None,
                project: None,
            })
            .unwrap();

        let boosts = store.memory_boosts(Duration::from_secs(60)).unwrap();
        assert_eq!(boosts.len(), 2);
        assert!(boosts.iter().all(|(_, s)| *s > 0.0));
    }

    #[test]
    fn negative_signal_reduces_boost() {
        let store = RecallStore::open_in_memory().unwrap();
        store.log_recall_event(event("map", &["c.md"])).unwrap();

        std::thread::sleep(std::time::Duration::from_millis(5));
        store
            .record_outcome_signal(OutcomeSignal {
                session_id: Some("s1".to_string()),
                signal_type: OutcomeSignalType::Correction,
                value: -3,
                source: None,
                project: None,
            })
            .unwrap();

        let boosts = store.memory_boosts(Duration::from_secs(60)).unwrap();
        assert_eq!(boosts.len(), 1);
        assert!(boosts[0].1 < 0.0);
    }

    #[test]
    fn unrelated_session_is_ignored() {
        let store = RecallStore::open_in_memory().unwrap();
        store
            .log_recall_event(RecallEvent {
                id: None,
                session_id: Some("s1".to_string()),
                query: "x".to_string(),
                memory_ids: vec!["x.md".to_string()],
                project: None,
            })
            .unwrap();

        store
            .record_outcome_signal(OutcomeSignal {
                session_id: Some("s2".to_string()),
                signal_type: OutcomeSignalType::ExplicitBoost,
                value: 3,
                source: None,
                project: None,
            })
            .unwrap();

        let boosts = store.memory_boosts(Duration::from_secs(60)).unwrap();
        assert_eq!(boosts.len(), 1);
        assert_eq!(boosts[0].1, 0.0);
    }
}
