//! User, team, organization, and device identity CRUD for `SessionStoreV2`.

use chrono::Utc;
use rusqlite::OptionalExtension;

use crate::session_store_v2::session::SessionStoreV2;
use crate::types::Result as MemoryResult;

impl SessionStoreV2 {
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
