use crate::stores::{Team, TeamStore};

/// Handles the team list event.
#[allow(dead_code)]
pub fn on_team_list(team_store: &mut TeamStore, teams: Vec<Team>) {
    team_store.teams = teams;
}
