//! Session V2 — unified SQLite-backed session storage.
//!
//! Replaces the JSON+JSONL dual system with a single SQLite schema that
//! supports append-only event logs, compacted contexts, and handoff lineage.
//!
//! # Schema
//!
//! ```sql
//! CREATE TABLE sessions_v2 (
//!     id              TEXT PRIMARY KEY,
//!     title           TEXT,
//!     soul_id         TEXT,
//!     created_at      INTEGER NOT NULL,
//!     updated_at      INTEGER,
//!     parent_session_id TEXT,
//!     state           TEXT CHECK(state IN ('active','archived','handoff_pending','compacting')),
//!     config_hash     TEXT
//! );
//!
//! CREATE TABLE event_log (
//!     id          INTEGER PRIMARY KEY AUTOINCREMENT,
//!     session_id  TEXT NOT NULL,
//!     turn_id     INTEGER NOT NULL,
//!     event_id    INTEGER NOT NULL,
//!     timestamp   INTEGER NOT NULL,
//!     event_type  TEXT NOT NULL,
//!     payload     BLOB NOT NULL,
//!     payload_hash TEXT,
//!     FOREIGN KEY (session_id) REFERENCES sessions_v2(id)
//! );
//!
//! CREATE TABLE compacted_context (
//!     session_id          TEXT PRIMARY KEY,
//!     turn_id             INTEGER NOT NULL,
//!     event_id            INTEGER NOT NULL,
//!     context_json        BLOB NOT NULL,
//!     compression_method  TEXT,
//!     source_hash         TEXT,
//!     created_at          INTEGER,
//!     FOREIGN KEY (session_id) REFERENCES sessions_v2(id)
//! );
//! ```

use std::path::{Path, PathBuf};

use chrono::Utc;
use rusqlite::{Connection, OptionalExtension};

use crate::types::Result as MemoryResult;

// ============================================================================
// SessionStoreV2
// ============================================================================

/// Unified SQLite session store (V2).
pub struct SessionStoreV2 {
    conn: Connection,
}

impl SessionStoreV2 {
    /// Open or create a V2 session store at the given path.
    pub fn new(path: impl AsRef<Path>) -> MemoryResult<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(&path)?;
        // SAFETY: PRAGMA journal_mode may return a result row on some SQLite builds.
        // We use execute_batch to silently ignore it; failure is non-fatal.
        let _ = conn.execute_batch("PRAGMA journal_mode=WAL;");
        let _ = conn.execute_batch("PRAGMA foreign_keys=ON;");

        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    /// Default path: `~/.clarity/sessions_v2.sqlite`.
    pub fn default_path() -> PathBuf {
        home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".clarity")
            .join("sessions_v2.sqlite")
    }

    fn init_schema(&self) -> MemoryResult<()> {
        for sql in TABLES_SQL {
            self.conn.execute(sql, [])?;
        }
        for sql in INDEXES_SQL {
            self.conn.execute(sql, [])?;
        }
        Ok(())
    }

    // ------------------------------------------------------------------
    // Session CRUD
    // ------------------------------------------------------------------

    /// Create a new session.
    pub fn create_session(
        &self,
        id: &str,
        title: Option<&str>,
        soul_id: Option<&str>,
        config_hash: Option<&str>,
    ) -> MemoryResult<()> {
        let now = Utc::now().timestamp_millis();
        self.conn.execute(
            "INSERT INTO sessions_v2 (id, title, soul_id, created_at, updated_at, state, config_hash)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                id,
                title,
                soul_id,
                now,
                now,
                "active",
                config_hash,
            ],
        )?;
        Ok(())
    }

    /// Get session metadata.
    pub fn get_session(&self, id: &str) -> MemoryResult<Option<SessionV2>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, soul_id, created_at, updated_at, parent_session_id, state, config_hash
             FROM sessions_v2 WHERE id = ?1"
        )?;
        let row = stmt
            .query_row([id], |row| {
                Ok(SessionV2 {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    soul_id: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                    parent_session_id: row.get(5)?,
                    state: row
                        .get::<_, String>(6)?
                        .parse()
                        .unwrap_or(SessionState::Active),
                    config_hash: row.get(7)?,
                })
            })
            .optional()?;
        Ok(row)
    }

    /// Update session state.
    pub fn set_session_state(&self, id: &str, state: SessionState) -> MemoryResult<()> {
        let now = Utc::now().timestamp_millis();
        self.conn.execute(
            "UPDATE sessions_v2 SET state = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![state.as_str(), now, id],
        )?;
        Ok(())
    }

    /// List all session IDs.
    pub fn list_sessions(&self, state_filter: Option<SessionState>) -> MemoryResult<Vec<String>> {
        let sql = match state_filter {
            Some(_) => "SELECT id FROM sessions_v2 WHERE state = ?1 ORDER BY updated_at DESC",
            None => "SELECT id FROM sessions_v2 ORDER BY updated_at DESC",
        };
        let mut stmt = self.conn.prepare(sql)?;
        let ids: Vec<String> = match state_filter {
            Some(s) => stmt
                .query_map([s.as_str()], |row| row.get(0))?
                .collect::<Result<_, _>>()?,
            None => stmt
                .query_map([], |row| row.get(0))?
                .collect::<Result<_, _>>()?,
        };
        Ok(ids)
    }

    /// Set parent session (handoff lineage).
    pub fn set_parent(&self, session_id: &str, parent_id: &str) -> MemoryResult<()> {
        self.conn.execute(
            "UPDATE sessions_v2 SET parent_session_id = ?1 WHERE id = ?2",
            [parent_id, session_id],
        )?;
        Ok(())
    }

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
        let payload_hash = format!("{:016x}", seahash::hash(&payload_bytes));
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

    // ------------------------------------------------------------------
    // Identity CRUD (Phase 6)
    // ------------------------------------------------------------------

    /// Insert or update a user.
    pub fn upsert_user(&self, user: &clarity_contract::User) -> MemoryResult<()> {
        self.conn.execute(
            "INSERT INTO users (id, display_name, avatar_url, email, provider, provider_user_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(id) DO UPDATE SET
                 display_name = excluded.display_name,
                 avatar_url = excluded.avatar_url,
                 email = excluded.email,
                 provider = excluded.provider,
                 provider_user_id = excluded.provider_user_id,
                 updated_at = excluded.updated_at",
            rusqlite::params![
                user.id,
                user.display_name,
                user.avatar_url,
                user.email,
                user.provider,
                user.provider_user_id,
                user.created_at as i64,
                user.updated_at as i64,
            ],
        )?;
        Ok(())
    }

    /// Get a user by ID.
    pub fn get_user(&self, id: &str) -> MemoryResult<Option<clarity_contract::User>> {
        let row = self
            .conn
            .query_row(
                "SELECT id, display_name, avatar_url, email, provider, provider_user_id, created_at, updated_at
                 FROM users WHERE id = ?1",
                [id],
                |row| {
                    Ok(clarity_contract::User {
                        id: row.get(0)?,
                        display_name: row.get(1)?,
                        avatar_url: row.get(2)?,
                        email: row.get(3)?,
                        provider: row.get(4)?,
                        provider_user_id: row.get(5)?,
                        created_at: row.get::<_, i64>(6)? as u64,
                        updated_at: row.get::<_, i64>(7)? as u64,
                    })
                },
            )
            .optional()?;
        Ok(row)
    }

    /// Look up a user by provider + provider_user_id.
    pub fn get_user_by_provider(
        &self,
        provider: &str,
        provider_user_id: &str,
    ) -> MemoryResult<Option<clarity_contract::User>> {
        let row = self
            .conn
            .query_row(
                "SELECT id, display_name, avatar_url, email, provider, provider_user_id, created_at, updated_at
                 FROM users WHERE provider = ?1 AND provider_user_id = ?2",
                rusqlite::params![provider, provider_user_id],
                |row| {
                    Ok(clarity_contract::User {
                        id: row.get(0)?,
                        display_name: row.get(1)?,
                        avatar_url: row.get(2)?,
                        email: row.get(3)?,
                        provider: row.get(4)?,
                        provider_user_id: row.get(5)?,
                        created_at: row.get::<_, i64>(6)? as u64,
                        updated_at: row.get::<_, i64>(7)? as u64,
                    })
                },
            )
            .optional()?;
        Ok(row)
    }

    /// Delete a user (cascades to memberships).
    pub fn delete_user(&self, id: &str) -> MemoryResult<()> {
        self.conn.execute("DELETE FROM users WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Insert or update a team.
    pub fn upsert_team(&self, team: &clarity_contract::Team) -> MemoryResult<()> {
        self.conn.execute(
            "INSERT INTO teams (id, org_id, name, description, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET
                 org_id = excluded.org_id,
                 name = excluded.name,
                 description = excluded.description",
            rusqlite::params![
                team.id,
                team.org_id,
                team.name,
                team.description,
                team.created_at as i64,
            ],
        )?;
        Ok(())
    }

    /// Get a team by ID.
    pub fn get_team(&self, id: &str) -> MemoryResult<Option<clarity_contract::Team>> {
        let row = self
            .conn
            .query_row(
                "SELECT id, org_id, name, description, created_at FROM teams WHERE id = ?1",
                [id],
                |row| {
                    Ok(clarity_contract::Team {
                        id: row.get(0)?,
                        org_id: row.get(1)?,
                        name: row.get(2)?,
                        description: row.get(3)?,
                        created_at: row.get::<_, i64>(4)? as u64,
                    })
                },
            )
            .optional()?;
        Ok(row)
    }

    /// List teams in an organization.
    pub fn list_teams_for_org(&self, org_id: &str) -> MemoryResult<Vec<clarity_contract::Team>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, org_id, name, description, created_at FROM teams WHERE org_id = ?1",
        )?;
        let teams = stmt
            .query_map([org_id], |row| {
                Ok(clarity_contract::Team {
                    id: row.get(0)?,
                    org_id: row.get(1)?,
                    name: row.get(2)?,
                    description: row.get(3)?,
                    created_at: row.get::<_, i64>(4)? as u64,
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(teams)
    }

    /// Add a member to a team.
    pub fn add_team_member(&self, member: &clarity_contract::TeamMember) -> MemoryResult<()> {
        let role_str = team_role_to_str(&member.role);
        self.conn.execute(
            "INSERT OR REPLACE INTO team_members (user_id, team_id, role, joined_at)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                member.user_id,
                member.team_id,
                role_str,
                member.joined_at as i64
            ],
        )?;
        Ok(())
    }

    /// List all members of a team.
    pub fn list_team_members(
        &self,
        team_id: &str,
    ) -> MemoryResult<Vec<clarity_contract::TeamMember>> {
        let mut stmt = self.conn.prepare(
            "SELECT user_id, team_id, role, joined_at FROM team_members WHERE team_id = ?1",
        )?;
        let members = stmt
            .query_map([team_id], |row| {
                let role_str: String = row.get(2)?;
                Ok(clarity_contract::TeamMember {
                    user_id: row.get(0)?,
                    team_id: row.get(1)?,
                    role: str_to_team_role(&role_str),
                    joined_at: row.get::<_, i64>(3)? as u64,
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(members)
    }

    /// Get a member's role in a team.
    pub fn get_team_member_role(
        &self,
        user_id: &str,
        team_id: &str,
    ) -> MemoryResult<Option<clarity_contract::TeamRole>> {
        let row: Option<String> = self
            .conn
            .query_row(
                "SELECT role FROM team_members WHERE user_id = ?1 AND team_id = ?2",
                rusqlite::params![user_id, team_id],
                |row| row.get(0),
            )
            .optional()?;
        Ok(row.map(|r| str_to_team_role(&r)))
    }

    /// Insert or update an organization.
    pub fn upsert_org(&self, org: &clarity_contract::Organization) -> MemoryResult<()> {
        self.conn.execute(
            "INSERT INTO organizations (id, name, description, created_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET
                 name = excluded.name,
                 description = excluded.description",
            rusqlite::params![org.id, org.name, org.description, org.created_at as i64],
        )?;
        Ok(())
    }

    /// Get an organization by ID.
    pub fn get_org(&self, id: &str) -> MemoryResult<Option<clarity_contract::Organization>> {
        let row = self
            .conn
            .query_row(
                "SELECT id, name, description, created_at FROM organizations WHERE id = ?1",
                [id],
                |row| {
                    Ok(clarity_contract::Organization {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        description: row.get(2)?,
                        created_at: row.get::<_, i64>(3)? as u64,
                    })
                },
            )
            .optional()?;
        Ok(row)
    }

    /// Add a member to an organization.
    pub fn add_org_member(&self, member: &clarity_contract::OrgMember) -> MemoryResult<()> {
        let role_str = team_role_to_str(&member.role);
        self.conn.execute(
            "INSERT OR REPLACE INTO org_members (user_id, org_id, role, joined_at)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                member.user_id,
                member.org_id,
                role_str,
                member.joined_at as i64
            ],
        )?;
        Ok(())
    }

    /// List all members of an organization.
    pub fn list_org_members(&self, org_id: &str) -> MemoryResult<Vec<clarity_contract::OrgMember>> {
        let mut stmt = self.conn.prepare(
            "SELECT user_id, org_id, role, joined_at FROM org_members WHERE org_id = ?1",
        )?;
        let members = stmt
            .query_map([org_id], |row| {
                let role_str: String = row.get(2)?;
                Ok(clarity_contract::OrgMember {
                    user_id: row.get(0)?,
                    org_id: row.get(1)?,
                    role: str_to_team_role(&role_str),
                    joined_at: row.get::<_, i64>(3)? as u64,
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(members)
    }

    // ------------------------------------------------------------------
    // Team policy CRUD (Phase 8)
    // ------------------------------------------------------------------

    /// Store a team permission policy.
    pub fn upsert_team_policy(&self, policy: &clarity_contract::TeamPolicy) -> MemoryResult<()> {
        let json = serde_json::to_vec(policy)?;
        let now = Utc::now().timestamp_millis();
        self.conn.execute(
            "INSERT OR REPLACE INTO team_policies (team_id, policy_json, updated_at)
             VALUES (?1, ?2, ?3)",
            rusqlite::params![policy.team_id, json, now],
        )?;
        Ok(())
    }

    /// Retrieve a team permission policy.
    pub fn get_team_policy(
        &self,
        team_id: &str,
    ) -> MemoryResult<Option<clarity_contract::TeamPolicy>> {
        let row = self
            .conn
            .query_row(
                "SELECT policy_json FROM team_policies WHERE team_id = ?1",
                [team_id],
                |row| {
                    let json: Vec<u8> = row.get(0)?;
                    Ok(json)
                },
            )
            .optional()?;
        match row {
            Some(json) => Ok(Some(serde_json::from_slice(&json)?)),
            None => Ok(None),
        }
    }

    /// Delete a team permission policy.
    pub fn delete_team_policy(&self, team_id: &str) -> MemoryResult<()> {
        self.conn
            .execute("DELETE FROM team_policies WHERE team_id = ?1", [team_id])?;
        Ok(())
    }

    // ------------------------------------------------------------------
    // Device identity CRUD (Phase 7)
    // ------------------------------------------------------------------

    /// Bind a device to a user.
    pub fn upsert_device_identity(&self, rec: &DeviceIdentityRecord) -> MemoryResult<()> {
        self.conn.execute(
            "INSERT INTO device_identities (device_id, user_id, device_name, public_key, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(device_id) DO UPDATE SET
                 user_id = excluded.user_id,
                 device_name = excluded.device_name,
                 public_key = excluded.public_key,
                 updated_at = excluded.updated_at",
            rusqlite::params![
                rec.device_id,
                rec.user_id,
                rec.device_name,
                rec.public_key,
                rec.created_at,
                rec.updated_at,
            ],
        )?;
        Ok(())
    }

    /// Get device identity by device ID.
    pub fn get_device_identity(
        &self,
        device_id: &str,
    ) -> MemoryResult<Option<DeviceIdentityRecord>> {
        let row = self
            .conn
            .query_row(
                "SELECT device_id, user_id, device_name, public_key, created_at, updated_at
                 FROM device_identities WHERE device_id = ?1",
                [device_id],
                |row| {
                    Ok(DeviceIdentityRecord {
                        device_id: row.get(0)?,
                        user_id: row.get(1)?,
                        device_name: row.get(2)?,
                        public_key: row.get(3)?,
                        created_at: row.get(4)?,
                        updated_at: row.get(5)?,
                    })
                },
            )
            .optional()?;
        Ok(row)
    }

    /// List all devices bound to a user.
    pub fn list_user_devices(&self, user_id: &str) -> MemoryResult<Vec<DeviceIdentityRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT device_id, user_id, device_name, public_key, created_at, updated_at
             FROM device_identities WHERE user_id = ?1",
        )?;
        let devices = stmt
            .query_map([user_id], |row| {
                Ok(DeviceIdentityRecord {
                    device_id: row.get(0)?,
                    user_id: row.get(1)?,
                    device_name: row.get(2)?,
                    public_key: row.get(3)?,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(devices)
    }

    /// Delete a device identity binding.
    pub fn delete_device_identity(&self, device_id: &str) -> MemoryResult<()> {
        self.conn.execute(
            "DELETE FROM device_identities WHERE device_id = ?1",
            [device_id],
        )?;
        Ok(())
    }
}

// ------------------------------------------------------------------
// DeviceIdentityRecord (Phase 7)
// ------------------------------------------------------------------

/// A device-to-user identity binding.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceIdentityRecord {
    /// Device identifier (SHA-256 of Ed25519 public key).
    pub device_id: String,
    /// Bound user identifier.
    pub user_id: String,
    /// Human-readable device name.
    pub device_name: String,
    /// Optional Ed25519 public key (PEM or hex).
    pub public_key: Option<String>,
    /// Creation timestamp (milliseconds since epoch).
    pub created_at: i64,
    /// Last update timestamp (milliseconds since epoch).
    pub updated_at: i64,
}

// ------------------------------------------------------------------
// TeamRole ↔ string helpers (ponytail: local; avoid pulling serde into SessionStoreV2)
// ------------------------------------------------------------------

fn team_role_to_str(role: &clarity_contract::TeamRole) -> &'static str {
    match role {
        clarity_contract::TeamRole::Owner => "owner",
        clarity_contract::TeamRole::Admin => "admin",
        clarity_contract::TeamRole::Member => "member",
        clarity_contract::TeamRole::Viewer => "viewer",
    }
}

fn str_to_team_role(s: &str) -> clarity_contract::TeamRole {
    match s {
        "owner" => clarity_contract::TeamRole::Owner,
        "admin" => clarity_contract::TeamRole::Admin,
        "member" => clarity_contract::TeamRole::Member,
        "viewer" => clarity_contract::TeamRole::Viewer,
        _ => clarity_contract::TeamRole::Member,
    }
}

// ============================================================================
// Data types
// ============================================================================

/// Session metadata (V2).
#[derive(Debug, Clone, PartialEq)]
pub struct SessionV2 {
    /// Session identifier
    pub id: String,
    /// Optional session title
    pub title: Option<String>,
    /// Optional associated soul identifier
    pub soul_id: Option<String>,
    /// Creation timestamp (milliseconds since epoch)
    pub created_at: i64,
    /// Last update timestamp (milliseconds since epoch)
    pub updated_at: Option<i64>,
    /// Parent session identifier for handoff lineage
    pub parent_session_id: Option<String>,
    /// Current lifecycle state
    pub state: SessionState,
    /// Hash of the configuration active when the session was created
    pub config_hash: Option<String>,
}

/// Session lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SessionState {
    /// Session is active and accepting events
    Active,
    /// Session has been archived
    Archived,
    /// Session is waiting to be handed off
    HandoffPending,
    /// Session is being compacted
    Compacting,
}

impl SessionState {
    /// Return the string representation stored in the database
    pub fn as_str(&self) -> &'static str {
        match self {
            SessionState::Active => "active",
            SessionState::Archived => "archived",
            SessionState::HandoffPending => "handoff_pending",
            SessionState::Compacting => "compacting",
        }
    }
}

impl std::str::FromStr for SessionState {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "active" => Ok(SessionState::Active),
            "archived" => Ok(SessionState::Archived),
            "handoff_pending" => Ok(SessionState::HandoffPending),
            "compacting" => Ok(SessionState::Compacting),
            _ => Err(format!("unknown session state: {s}")),
        }
    }
}

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

// ============================================================================
// Schema
// ============================================================================

const TABLES_SQL: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS sessions_v2 (\
     id TEXT PRIMARY KEY, \
     title TEXT, \
     soul_id TEXT, \
     created_at INTEGER NOT NULL, \
     updated_at INTEGER, \
     parent_session_id TEXT REFERENCES sessions_v2(id), \
     state TEXT CHECK(state IN ('active','archived','handoff_pending','compacting')) NOT NULL DEFAULT 'active', \
     config_hash TEXT)",
    "CREATE TABLE IF NOT EXISTS event_log (\
     id INTEGER PRIMARY KEY AUTOINCREMENT, \
     session_id TEXT NOT NULL REFERENCES sessions_v2(id) ON DELETE CASCADE, \
     turn_id INTEGER NOT NULL, \
     event_id INTEGER NOT NULL, \
     timestamp INTEGER NOT NULL, \
     event_type TEXT NOT NULL, \
     payload BLOB NOT NULL, \
     payload_hash TEXT, \
     UNIQUE(session_id, turn_id, event_id))",
    "CREATE TABLE IF NOT EXISTS compacted_context (\
     session_id TEXT PRIMARY KEY REFERENCES sessions_v2(id) ON DELETE CASCADE, \
     turn_id INTEGER NOT NULL, \
     event_id INTEGER NOT NULL, \
     context_json BLOB NOT NULL, \
     compression_method TEXT, \
     source_hash TEXT, \
     created_at INTEGER)",
    "CREATE TABLE IF NOT EXISTS rollout_index (\
     thread_id TEXT PRIMARY KEY REFERENCES sessions_v2(id) ON DELETE CASCADE, \
     rollout_path TEXT NOT NULL, \
     last_seq INTEGER NOT NULL DEFAULT 0, \
     updated_at INTEGER NOT NULL)",
    // Identity tables (Phase 6).
    "CREATE TABLE IF NOT EXISTS users (\
     id TEXT PRIMARY KEY, \
     display_name TEXT NOT NULL, \
     avatar_url TEXT, \
     email TEXT, \
     provider TEXT NOT NULL, \
     provider_user_id TEXT, \
     created_at INTEGER NOT NULL, \
     updated_at INTEGER NOT NULL)",
    "CREATE TABLE IF NOT EXISTS teams (\
     id TEXT PRIMARY KEY, \
     org_id TEXT NOT NULL, \
     name TEXT NOT NULL, \
     description TEXT, \
     created_at INTEGER NOT NULL)",
    "CREATE TABLE IF NOT EXISTS team_members (\
     user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE, \
     team_id TEXT NOT NULL REFERENCES teams(id) ON DELETE CASCADE, \
     role TEXT NOT NULL DEFAULT 'member', \
     joined_at INTEGER NOT NULL, \
     PRIMARY KEY (user_id, team_id))",
    "CREATE TABLE IF NOT EXISTS organizations (\
     id TEXT PRIMARY KEY, \
     name TEXT NOT NULL, \
     description TEXT, \
     created_at INTEGER NOT NULL)",
    "CREATE TABLE IF NOT EXISTS org_members (\
     user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE, \
     org_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE, \
     role TEXT NOT NULL DEFAULT 'member', \
     joined_at INTEGER NOT NULL, \
     PRIMARY KEY (user_id, org_id))",
    // Phase 8: team permission policies.
    "CREATE TABLE IF NOT EXISTS team_policies (\
     team_id TEXT PRIMARY KEY REFERENCES teams(id) ON DELETE CASCADE, \
     policy_json BLOB NOT NULL, \
     updated_at INTEGER NOT NULL)",
    // Phase 7: device-to-user identity binding.
    "CREATE TABLE IF NOT EXISTS device_identities (\
     device_id TEXT PRIMARY KEY, \
     user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE, \
     device_name TEXT NOT NULL DEFAULT '', \
     public_key TEXT, \
     created_at INTEGER NOT NULL, \
     updated_at INTEGER NOT NULL)",
];

const INDEXES_SQL: &[&str] = &[
    "CREATE INDEX IF NOT EXISTS idx_sessions_v2_state ON sessions_v2(state)",
    "CREATE INDEX IF NOT EXISTS idx_sessions_v2_soul ON sessions_v2(soul_id)",
    "CREATE INDEX IF NOT EXISTS idx_sessions_v2_parent ON sessions_v2(parent_session_id)",
    "CREATE INDEX IF NOT EXISTS idx_event_log_session ON event_log(session_id)",
    "CREATE INDEX IF NOT EXISTS idx_event_log_session_turn_event ON event_log(session_id, turn_id, event_id)",
    "CREATE INDEX IF NOT EXISTS idx_event_log_timestamp ON event_log(timestamp)",
    "CREATE INDEX IF NOT EXISTS idx_rollout_index_thread ON rollout_index(thread_id)",
    // Identity indexes (Phase 6).
    "CREATE INDEX IF NOT EXISTS idx_users_provider ON users(provider, provider_user_id)",
    "CREATE INDEX IF NOT EXISTS idx_team_members_team ON team_members(team_id)",
    "CREATE INDEX IF NOT EXISTS idx_org_members_org ON org_members(org_id)",
    // Phase 8: team policy lookup.
    "CREATE INDEX IF NOT EXISTS idx_team_policies_team ON team_policies(team_id)",
    // Phase 7: device identity lookup by user.
    "CREATE INDEX IF NOT EXISTS idx_device_identities_user ON device_identities(user_id)",
];

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

// Simple hash for payload integrity (avoids adding heavy crypto deps).
mod seahash {
    pub fn hash(bytes: &[u8]) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        bytes.hash(&mut hasher);
        hasher.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store() -> SessionStoreV2 {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        SessionStoreV2::new(tmp.path()).unwrap()
    }

    #[test]
    fn test_create_and_get_session() {
        let store = temp_store();
        store
            .create_session(
                "sess-1",
                Some("Test Session"),
                Some("soul-a"),
                Some("abc123"),
            )
            .unwrap();

        let sess = store.get_session("sess-1").unwrap().unwrap();
        assert_eq!(sess.id, "sess-1");
        assert_eq!(sess.title, Some("Test Session".to_string()));
        assert_eq!(sess.soul_id, Some("soul-a".to_string()));
        assert_eq!(sess.state, SessionState::Active);
    }

    #[test]
    fn test_event_log_append_and_read() {
        let store = temp_store();
        store.create_session("sess-1", None, None, None).unwrap();

        let payload = serde_json::json!({"role": "user", "content": "hello"});
        store
            .append_event("sess-1", 1, EventType::UserMessage, &payload)
            .unwrap();
        store
            .append_event(
                "sess-1",
                1,
                EventType::AssistantMessage,
                &serde_json::json!({"content": "hi"}),
            )
            .unwrap();
        store
            .append_event(
                "sess-1",
                2,
                EventType::UserMessage,
                &serde_json::json!({"content": "world"}),
            )
            .unwrap();

        let events = store.read_events("sess-1").unwrap();
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].event_type, EventType::UserMessage);
        assert_eq!(events[0].event_id, 1);
        assert_eq!(events[1].event_id, 2);
        assert_eq!(events[2].turn_id, 2);
    }

    #[test]
    fn test_read_events_until() {
        let store = temp_store();
        store.create_session("sess-1", None, None, None).unwrap();

        for i in 1..=5 {
            store
                .append_event(
                    "sess-1",
                    i,
                    EventType::UserMessage,
                    &serde_json::json!({"i": i}),
                )
                .unwrap();
        }

        // turn_id 3 has event_id 3 (auto-assigned by append_event).
        let events = store.read_events_until("sess-1", 3, 3).unwrap();
        assert_eq!(events.len(), 3); // turns 1,2,3
    }

    #[test]
    fn test_compacted_context_roundtrip() {
        let store = temp_store();
        store.create_session("sess-1", None, None, None).unwrap();

        let context = serde_json::json!([{"role": "system", "content": "compacted"}]);
        store
            .store_compacted_context("sess-1", 5, 12, &context, "tier2", "hash-abc")
            .unwrap();

        let loaded = store.load_compacted_context("sess-1").unwrap().unwrap();
        assert_eq!(loaded.turn_id, 5);
        assert_eq!(loaded.event_id, 12);
        assert_eq!(loaded.compression_method, "tier2");
        assert_eq!(loaded.context_json, context);
    }

    #[test]
    fn test_session_state_transition() {
        let store = temp_store();
        store.create_session("sess-1", None, None, None).unwrap();

        store
            .set_session_state("sess-1", SessionState::Compacting)
            .unwrap();
        let sess = store.get_session("sess-1").unwrap().unwrap();
        assert_eq!(sess.state, SessionState::Compacting);
    }

    #[test]
    fn test_parent_session_lineage() {
        let store = temp_store();
        store.create_session("parent", None, None, None).unwrap();
        store.create_session("child", None, None, None).unwrap();
        store.set_parent("child", "parent").unwrap();

        let child = store.get_session("child").unwrap().unwrap();
        assert_eq!(child.parent_session_id, Some("parent".to_string()));
    }

    #[test]
    fn test_delete_session_cascades() {
        let store = temp_store();
        store.create_session("sess-1", None, None, None).unwrap();
        store
            .append_event("sess-1", 1, EventType::UserMessage, &serde_json::json!({}))
            .unwrap();
        store
            .store_compacted_context("sess-1", 1, 1, &serde_json::json!([]), "tier1", "h")
            .unwrap();

        store.delete_session("sess-1").unwrap();
        assert!(store.get_session("sess-1").unwrap().is_none());
        assert!(store.read_events("sess-1").unwrap().is_empty());
        assert!(store.load_compacted_context("sess-1").unwrap().is_none());
    }

    #[test]
    fn test_rollout_index_roundtrip() {
        let store = temp_store();
        store.create_session("thread-1", None, None, None).unwrap();

        let path = std::path::PathBuf::from("/tmp/rollouts/thread-1.jsonl");
        store.register_rollout("thread-1", &path, 42).unwrap();

        let (got_path, seq) = store.get_rollout("thread-1").unwrap().unwrap();
        assert_eq!(got_path, path);
        assert_eq!(seq, 42);

        store.update_rollout_seq("thread-1", 99).unwrap();
        let (_, seq) = store.get_rollout("thread-1").unwrap().unwrap();
        assert_eq!(seq, 99);

        store.delete_session("thread-1").unwrap();
        assert!(store.get_rollout("thread-1").unwrap().is_none());
    }

    // ------------------------------------------------------------------
    // Identity CRUD tests
    // ------------------------------------------------------------------

    fn make_user(id: &str, name: &str, provider: &str) -> clarity_contract::User {
        clarity_contract::User {
            id: id.into(),
            display_name: name.into(),
            avatar_url: None,
            email: None,
            provider: provider.into(),
            provider_user_id: None,
            created_at: 1700000000,
            updated_at: 1700000001,
        }
    }

    #[test]
    fn test_user_upsert_get_delete() {
        let store = temp_store();
        let user = make_user("u-1", "Alice", "local");
        store.upsert_user(&user).unwrap();

        let got = store.get_user("u-1").unwrap().unwrap();
        assert_eq!(got.id, "u-1");
        assert_eq!(got.display_name, "Alice");
        assert_eq!(got.provider, "local");

        store.delete_user("u-1").unwrap();
        assert!(store.get_user("u-1").unwrap().is_none());
    }

    #[test]
    fn test_user_by_provider_lookup() {
        let store = temp_store();
        let mut user = make_user("u-2", "Bob", "wechat");
        user.provider_user_id = Some("wx-openid-123".into());
        store.upsert_user(&user).unwrap();

        let got = store
            .get_user_by_provider("wechat", "wx-openid-123")
            .unwrap()
            .unwrap();
        assert_eq!(got.id, "u-2");

        // Missing provider/user combo returns None.
        assert!(
            store
                .get_user_by_provider("github", "nobody")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn test_team_upsert_get_list() {
        let store = temp_store();
        let team = clarity_contract::Team {
            id: "t-1".into(),
            org_id: "o-1".into(),
            name: "Engineering".into(),
            description: None,
            created_at: 1700000000,
        };
        store.upsert_team(&team).unwrap();

        let got = store.get_team("t-1").unwrap().unwrap();
        assert_eq!(got.name, "Engineering");

        let all = store.list_teams_for_org("o-1").unwrap();
        assert_eq!(all.len(), 1);
    }

    #[test]
    fn test_team_member_add_list() {
        let store = temp_store();
        let user = make_user("u-3", "Carol", "local");
        store.upsert_user(&user).unwrap();
        let team = clarity_contract::Team {
            id: "t-2".into(),
            org_id: "o-1".into(),
            name: "Design".into(),
            description: None,
            created_at: 1700000000,
        };
        store.upsert_team(&team).unwrap();

        let member = clarity_contract::TeamMember {
            user_id: "u-3".into(),
            team_id: "t-2".into(),
            role: clarity_contract::TeamRole::Admin,
            joined_at: 1700000000,
        };
        store.add_team_member(&member).unwrap();

        let members = store.list_team_members("t-2").unwrap();
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].user_id, "u-3");
        assert_eq!(members[0].role, clarity_contract::TeamRole::Admin);

        let role = store.get_team_member_role("u-3", "t-2").unwrap().unwrap();
        assert_eq!(role, clarity_contract::TeamRole::Admin);
    }

    #[test]
    fn test_org_upsert_get() {
        let store = temp_store();
        let org = clarity_contract::Organization {
            id: "o-1".into(),
            name: "Acme Corp".into(),
            description: Some("Enterprise".into()),
            created_at: 1700000000,
        };
        store.upsert_org(&org).unwrap();

        let got = store.get_org("o-1").unwrap().unwrap();
        assert_eq!(got.name, "Acme Corp");
    }

    #[test]
    fn test_org_member_add_list() {
        let store = temp_store();
        let user = make_user("u-4", "Dave", "local");
        store.upsert_user(&user).unwrap();
        let org = clarity_contract::Organization {
            id: "o-2".into(),
            name: "Startup Inc".into(),
            description: None,
            created_at: 1700000000,
        };
        store.upsert_org(&org).unwrap();

        let member = clarity_contract::OrgMember {
            user_id: "u-4".into(),
            org_id: "o-2".into(),
            role: clarity_contract::TeamRole::Owner,
            joined_at: 1700000000,
        };
        store.add_org_member(&member).unwrap();

        let members = store.list_org_members("o-2").unwrap();
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].user_id, "u-4");
        assert_eq!(members[0].role, clarity_contract::TeamRole::Owner);
    }

    #[test]
    fn test_cascade_delete_user_removes_memberships() {
        let store = temp_store();
        let user = make_user("u-del", "Eve", "local");
        store.upsert_user(&user).unwrap();
        let org = clarity_contract::Organization {
            id: "o-del".into(),
            name: "Temp Org".into(),
            description: None,
            created_at: 1700000000,
        };
        store.upsert_org(&org).unwrap();
        let team = clarity_contract::Team {
            id: "t-del".into(),
            org_id: "o-del".into(),
            name: "Temp Team".into(),
            description: None,
            created_at: 1700000000,
        };
        store.upsert_team(&team).unwrap();

        store
            .add_org_member(&clarity_contract::OrgMember {
                user_id: "u-del".into(),
                org_id: "o-del".into(),
                role: clarity_contract::TeamRole::Member,
                joined_at: 1700000000,
            })
            .unwrap();
        store
            .add_team_member(&clarity_contract::TeamMember {
                user_id: "u-del".into(),
                team_id: "t-del".into(),
                role: clarity_contract::TeamRole::Member,
                joined_at: 1700000000,
            })
            .unwrap();

        // Verify memberships exist.
        assert_eq!(store.list_org_members("o-del").unwrap().len(), 1);
        assert_eq!(store.list_team_members("t-del").unwrap().len(), 1);

        // Delete user → cascade deletes memberships.
        store.delete_user("u-del").unwrap();
        assert!(store.list_org_members("o-del").unwrap().is_empty());
        assert!(store.list_team_members("t-del").unwrap().is_empty());
    }
}
