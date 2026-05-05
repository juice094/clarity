use crate::stores::{Team, TeamStore};

pub fn on_team_list(team_store: &mut TeamStore, teams: Vec<Team>) {
    team_store.teams = teams;
}
