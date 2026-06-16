//! Team Store
//!
//! team coordination list, creation modal

/// Holds team member state.
#[derive(Clone, Debug)]
pub struct TeamMember {
    pub name: String,
    pub description: String,
    pub agent_type: String,
}

/// Holds team state.
#[derive(Clone, Debug)]
pub struct Team {
    pub name: String,
    pub goal: String,
    pub members: Vec<TeamMember>,
    pub max_concurrency: usize,
    pub timeout_secs: u64,
}

/// Holds team UI state.
pub struct TeamStore {
    pub teams: Vec<Team>,
    pub create_name: String,
    pub create_goal: String,
    pub create_members: Vec<TeamMember>,
    pub create_max_concurrency: usize,
    pub create_timeout_secs: u64,
}
