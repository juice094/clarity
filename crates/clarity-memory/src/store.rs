//! SQLite + FTS5 storage layer for facts

use crate::types::{Fact, Result};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use tracing::{debug, info, instrument, warn};

/// SQLite-based fact store with FTS5 full-text search
#[derive(Debug)]
pub struct MemoryStore {
    conn: Connection,
}

impl MemoryStore {
    /// Create a new MemoryStore at the given database path
    /// 
    /// If the database doesn't exist, it will be created with the proper schema.
    #[instrument(skip(db_path))]
    pub fn new(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        
        info!("Initializing MemoryStore at {:?}", db_path);
        
        let store = Self { conn };
        store.init_schema()?;
        
        Ok(store)
    }

    /// Create an in-memory store for testing
    pub fn new_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    /// Initialize the database schema
    fn init_schema(&self) -> Result<()> {
        // Create the facts table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS facts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                fact TEXT NOT NULL,
                tags TEXT NOT NULL DEFAULT '[]',
                time TEXT,
                session_id TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
            [],
        )?;

        // Create the FTS5 virtual table
        self.conn.execute(
            "CREATE VIRTUAL TABLE IF NOT EXISTS facts_fts USING fts5(
                fact,
                content='facts',
                content_rowid='id'
            )",
            [],
        )?;

        // Create triggers to keep FTS5 table in sync
        self.conn.execute(
            "CREATE TRIGGER IF NOT EXISTS facts_fts_insert 
            AFTER INSERT ON facts BEGIN
                INSERT INTO facts_fts(rowid, fact) VALUES (new.id, new.fact);
            END",
            [],
        )?;

        self.conn.execute(
            "CREATE TRIGGER IF NOT EXISTS facts_fts_delete 
            AFTER DELETE ON facts BEGIN
                INSERT INTO facts_fts(facts_fts, rowid, fact) VALUES ('delete', old.id, old.fact);
            END",
            [],
        )?;

        self.conn.execute(
            "CREATE TRIGGER IF NOT EXISTS facts_fts_update 
            AFTER UPDATE ON facts BEGIN
                INSERT INTO facts_fts(facts_fts, rowid, fact) VALUES ('delete', old.id, old.fact);
                INSERT INTO facts_fts(rowid, fact) VALUES (new.id, new.fact);
            END",
            [],
        )?;

        // Create index on tags for faster JSON searching
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_facts_session ON facts(session_id)",
            [],
        )?;

        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_facts_created ON facts(created_at)",
            [],
        )?;

        debug!("Schema initialization complete");
        Ok(())
    }

    /// Save a fact to the store
    /// 
    /// Returns the ID of the newly created fact.
    #[instrument(skip(self, fact, tags, time, session_id))]
    pub fn save_fact(
        &self,
        fact: &str,
        tags: &[String],
        time: Option<&str>,
        session_id: Option<&str>,
    ) -> Result<i64> {
        let tags_json = serde_json::to_string(tags)?;
        let now = Utc::now().to_rfc3339();

        self.conn.execute(
            "INSERT INTO facts (fact, tags, time, session_id, created_at) 
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![fact, tags_json, time, session_id, now],
        )?;

        let id = self.conn.last_insert_rowid();
        info!("Saved fact with id={}, tags={:?}", id, tags);
        
        Ok(id)
    }

    /// Search facts by tags (JSON contains)
    /// 
    /// Matches any fact that has ALL of the specified tags.
    #[instrument(skip(self, tags))]
    pub fn search_by_tags(&self, tags: &[String], limit: usize) -> Result<Vec<Fact>> {
        if tags.is_empty() {
            return Ok(Vec::new());
        }

        // Build a query that checks for all tags using JSON_CONTAINS equivalent
        // SQLite JSON1 extension: json_array_contains or json_each
        let mut facts = Vec::new();
        
        // Use a simpler approach: fetch candidates and filter in Rust
        // For better performance with large datasets, consider using json_each
        let mut stmt = self.conn.prepare(
            "SELECT id, fact, tags, time, session_id, created_at 
             FROM facts 
             ORDER BY created_at DESC 
             LIMIT ?"
        )?;

        let rows = stmt.query_map([limit as i64], |row| {
            let tags_json: String = row.get(2)?;
            let tags: Vec<String> = serde_json::from_str(&tags_json)
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(e)))?;
            
            Ok(Fact {
                id: row.get(0)?,
                fact: row.get(1)?,
                tags,
                time: row.get(3)?,
                session_id: row.get(4)?,
                created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(5)?)
                    .map_err(|e| rusqlite::Error::FromSqlConversionFailure(5, rusqlite::types::Type::Text, Box::new(e)))?
                    .with_timezone(&Utc),
            })
        })?;

        for row in rows {
            let fact = row?;
            // Check if fact has all requested tags
            if tags.iter().all(|tag| fact.tags.contains(tag)) {
                facts.push(fact);
                if facts.len() >= limit {
                    break;
                }
            }
        }

        debug!("Found {} facts matching tags {:?}", facts.len(), tags);
        Ok(facts)
    }

    /// Full-text search using FTS5
    #[instrument(skip(self, query))]
    pub fn search_fulltext(&self, query: &str, limit: usize) -> Result<Vec<Fact>> {
        let mut stmt = self.conn.prepare(
            "SELECT f.id, f.fact, f.tags, f.time, f.session_id, f.created_at 
             FROM facts f
             JOIN facts_fts fts ON f.id = fts.rowid
             WHERE facts_fts MATCH ?
             ORDER BY rank
             LIMIT ?"
        )?;

        let rows = stmt.query_map(params![query, limit as i64], |row| {
            let tags_json: String = row.get(2)?;
            let tags: Vec<String> = serde_json::from_str(&tags_json)
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(e)))?;
            
            Ok(Fact {
                id: row.get(0)?,
                fact: row.get(1)?,
                tags,
                time: row.get(3)?,
                session_id: row.get(4)?,
                created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(5)?)
                    .map_err(|e| rusqlite::Error::FromSqlConversionFailure(5, rusqlite::types::Type::Text, Box::new(e)))?
                    .with_timezone(&Utc),
            })
        })?;

        let mut facts = Vec::new();
        for row in rows {
            facts.push(row?);
        }

        debug!("Found {} facts matching FTS query '{}'", facts.len(), query);
        Ok(facts)
    }

    /// Get facts by session ID
    #[instrument(skip(self, session_id))]
    pub fn get_facts_by_session(&self, session_id: &str, limit: usize) -> Result<Vec<Fact>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, fact, tags, time, session_id, created_at 
             FROM facts 
             WHERE session_id = ?
             ORDER BY created_at DESC
             LIMIT ?"
        )?;

        let rows = stmt.query_map(params![session_id, limit as i64], |row| {
            let tags_json: String = row.get(2)?;
            let tags: Vec<String> = serde_json::from_str(&tags_json)
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(e)))?;
            
            Ok(Fact {
                id: row.get(0)?,
                fact: row.get(1)?,
                tags,
                time: row.get(3)?,
                session_id: row.get(4)?,
                created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(5)?)
                    .map_err(|e| rusqlite::Error::FromSqlConversionFailure(5, rusqlite::types::Type::Text, Box::new(e)))?
                    .with_timezone(&Utc),
            })
        })?;

        let mut facts = Vec::new();
        for row in rows {
            facts.push(row?);
        }

        Ok(facts)
    }

    /// Get facts created after a specific time
    #[instrument(skip(self, since))]
    pub fn get_facts_since(&self, since: chrono::DateTime<Utc>) -> Result<Vec<Fact>> {
        let since_str = since.to_rfc3339();
        
        let mut stmt = self.conn.prepare(
            "SELECT id, fact, tags, time, session_id, created_at 
             FROM facts 
             WHERE created_at > ?
             ORDER BY created_at DESC"
        )?;

        let rows = stmt.query_map([&since_str], |row| {
            let tags_json: String = row.get(2)?;
            let tags: Vec<String> = serde_json::from_str(&tags_json)
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(e)))?;
            
            Ok(Fact {
                id: row.get(0)?,
                fact: row.get(1)?,
                tags,
                time: row.get(3)?,
                session_id: row.get(4)?,
                created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(5)?)
                    .map_err(|e| rusqlite::Error::FromSqlConversionFailure(5, rusqlite::types::Type::Text, Box::new(e)))?
                    .with_timezone(&Utc),
            })
        })?;

        let mut facts = Vec::new();
        for row in rows {
            facts.push(row?);
        }

        Ok(facts)
    }

    /// Delete a fact by ID
    #[instrument(skip(self))]
    pub fn delete_fact(&self, id: i64) -> Result<bool> {
        let rows_affected = self.conn.execute(
            "DELETE FROM facts WHERE id = ?",
            [id],
        )?;

        let deleted = rows_affected > 0;
        if deleted {
            info!("Deleted fact with id={}", id);
        } else {
            warn!("Attempted to delete non-existent fact with id={}", id);
        }
        
        Ok(deleted)
    }

    /// Get total count of facts
    pub fn count_facts(&self) -> Result<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM facts",
            [],
            |row| row.get(0),
        )?;
        
        Ok(count)
    }

    /// Get a fact by ID
    pub fn get_fact(&self, id: i64) -> Result<Option<Fact>> {
        let result = self.conn.query_row(
            "SELECT id, fact, tags, time, session_id, created_at 
             FROM facts 
             WHERE id = ?",
            [id],
            |row| {
                let tags_json: String = row.get(2)?;
                let tags: Vec<String> = serde_json::from_str(&tags_json)
                    .map_err(|e| rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(e)))?;
                
                Ok(Fact {
                    id: row.get(0)?,
                    fact: row.get(1)?,
                    tags,
                    time: row.get(3)?,
                    session_id: row.get(4)?,
                    created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(5)?)
                        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(5, rusqlite::types::Type::Text, Box::new(e)))?
                        .with_timezone(&Utc),
                })
            },
        ).optional()?;

        Ok(result)
    }

    /// Begin a transaction
    pub fn begin_transaction(&mut self) -> Result<rusqlite::Transaction<'_>> {
        Ok(self.conn.transaction()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_save_and_retrieve_fact() {
        let store = MemoryStore::new_in_memory().unwrap();
        
        let id = store.save_fact(
            "User likes Rust programming",
            &["preference".to_string(), "tech".to_string()],
            Some("2024-01-15"),
            Some("session-1"),
        ).unwrap();

        let fact = store.get_fact(id).unwrap().expect("Fact should exist");
        assert_eq!(fact.fact, "User likes Rust programming");
        assert_eq!(fact.tags, vec!["preference", "tech"]);
        assert_eq!(fact.time, Some("2024-01-15".to_string()));
        assert_eq!(fact.session_id, Some("session-1".to_string()));
    }

    #[test]
    fn test_search_by_tags() {
        let store = MemoryStore::new_in_memory().unwrap();
        
        store.save_fact(
            "User likes Rust",
            &["preference".to_string(), "tech".to_string()],
            None,
            None,
        ).unwrap();
        
        store.save_fact(
            "User likes Python",
            &["preference".to_string(), "tech".to_string()],
            None,
            None,
        ).unwrap();
        
        store.save_fact(
            "Meeting at 3pm",
            &["schedule".to_string()],
            None,
            None,
        ).unwrap();

        let results = store.search_by_tags(&["preference".to_string()], 10).unwrap();
        assert_eq!(results.len(), 2);

        let results = store.search_by_tags(&["preference".to_string(), "tech".to_string()], 10).unwrap();
        assert_eq!(results.len(), 2);

        let results = store.search_by_tags(&["schedule".to_string()], 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].fact, "Meeting at 3pm");
    }

    #[test]
    fn test_fulltext_search() {
        let store = MemoryStore::new_in_memory().unwrap();
        
        store.save_fact(
            "Rust is a systems programming language",
            &["tech".to_string()],
            None,
            None,
        ).unwrap();
        
        store.save_fact(
            "Python is great for data science",
            &["tech".to_string(), "data".to_string()],
            None,
            None,
        ).unwrap();

        let results = store.search_fulltext("Rust", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].fact.contains("Rust"));

        let results = store.search_fulltext("programming language", 10).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_delete_fact() {
        let store = MemoryStore::new_in_memory().unwrap();
        
        let id = store.save_fact("Test fact", &[], None, None).unwrap();
        assert!(store.get_fact(id).unwrap().is_some());
        
        assert!(store.delete_fact(id).unwrap());
        assert!(store.get_fact(id).unwrap().is_none());
        
        // Deleting non-existent should return false
        assert!(!store.delete_fact(999).unwrap());
    }

    #[test]
    fn test_count_facts() {
        let store = MemoryStore::new_in_memory().unwrap();
        
        assert_eq!(store.count_facts().unwrap(), 0);
        
        store.save_fact("Fact 1", &[], None, None).unwrap();
        assert_eq!(store.count_facts().unwrap(), 1);
        
        store.save_fact("Fact 2", &[], None, None).unwrap();
        assert_eq!(store.count_facts().unwrap(), 2);
    }
}
