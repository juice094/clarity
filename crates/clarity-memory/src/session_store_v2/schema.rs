//! Database schema and path helpers for `SessionStoreV2`.

use std::path::PathBuf;

// ============================================================================
// Schema
// ============================================================================

pub(crate) const TABLES_SQL: &[&str] = &[
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

pub(crate) const INDEXES_SQL: &[&str] = &[
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

pub(crate) fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}
