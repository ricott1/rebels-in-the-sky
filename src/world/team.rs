use super::{
    constants::MIN_PLAYERS_PER_TEAM,
    jersey::Jersey,
    planet::Planet,
    player::Player,
    position::{GamePosition, MAX_POSITION},
    role::CrewRole,
    spaceship::Spaceship,
    types::{PlayerLocation, TeamLocation},
};
use crate::{
    engine::tactic::Tactic,
    types::{AppResult, GameId, PlanetId, PlayerId, SystemTimeTick, TeamId, Tick},
};
use itertools::Itertools;
use libp2p::PeerId;
use serde::{Deserialize, Serialize};
use std::cmp::min;

#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq)]
pub struct CrewRoles {
    pub captain: Option<PlayerId>,
    pub doctor: Option<PlayerId>,
    pub pilot: Option<PlayerId>,
    pub mozzo: Vec<PlayerId>,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq)]
pub struct Team {
    pub id: TeamId,
    pub version: u64,
    pub name: String,
    pub reputation: f32,
    pub player_ids: Vec<PlayerId>,
    pub crew_roles: CrewRoles,
    pub jersey: Jersey,
    pub balance: u32,
    pub max_jersey_number: u8,
    pub spaceship: Spaceship,
    pub home_planet: PlanetId,
    pub current_location: TeamLocation,
    pub peer_id: Option<PeerId>,
    pub current_game: Option<GameId>,
    pub game_tactic: Tactic,
}

impl Team {
    pub fn random(id: TeamId, home_planet: PlanetId, name: String) -> Self {
        let jersey = Jersey::random();
        let ship_name = format!("{}shipp", name);
        let ship_color = jersey.color;
        Self {
            id,
            name,
            jersey,
            home_planet,
            current_location: TeamLocation::OnPlanet {
                planet_id: home_planet,
            },
            spaceship: Spaceship::random(ship_name, ship_color),
            game_tactic: Tactic::random(),
            ..Default::default()
        }
    }

    pub fn can_hire_player(&self, player: &Player) -> AppResult<()> {
        if player.team.is_some() {
            return Err("Already in a team".into());
        }
        if self.balance < player.hire_cost(self.reputation) {
            return Err("Not enough money".into());
        }
        if self.player_ids.len() >= self.spaceship.capacity() as usize {
            return Err("Team is full".into());
        }

        // Player must be on same planet as team current_location
        match self.current_location {
            TeamLocation::OnPlanet {
                planet_id: team_planet_id,
            } => match player.current_location {
                PlayerLocation::OnPlanet { planet_id } => {
                    if planet_id != team_planet_id {
                        return Err("Not on team planet".into());
                    }
                }
                PlayerLocation::WithTeam => {
                    return Err("Already in a team".into());
                }
            },
            TeamLocation::Travelling { .. } => {
                return Err("Team is travelling".into());
            }
        }
        Ok(())
    }

    pub fn can_release_player(&self, player: &Player) -> AppResult<()> {
        if player.team.is_none() {
            return Err("Player is not in a team".into());
        }

        match self.current_location {
            TeamLocation::Travelling { .. } => {
                return Err("Team is travelling".into());
            }
            _ => {}
        }

        if self.current_game.is_some() {
            return Err("Team is currently playing".into());
        }

        if self.player_ids.len() <= MIN_PLAYERS_PER_TEAM {
            return Err("Team is too small".into());
        }
        Ok(())
    }

    pub fn can_set_crew_role(&self, player: &Player, role: CrewRole) -> AppResult<()> {
        if player.team.is_none() {
            return Err("Player is not in a team".into());
        }

        if self.current_game.is_some() {
            return Err("Team is currently playing".into());
        }

        match role {
            CrewRole::Captain => {
                if self.crew_roles.captain == Some(player.id) {
                    return Err("Player is already captain".into());
                }
            }
            CrewRole::Doctor => {
                if self.crew_roles.doctor == Some(player.id) {
                    return Err("Player is already doctor".into());
                }
            }
            CrewRole::Pilot => {
                if self.crew_roles.pilot == Some(player.id) {
                    return Err("Player is already pilot".into());
                }
            }
            CrewRole::Mozzo => {
                if self.crew_roles.mozzo.contains(&player.id) {
                    return Err("Player is already mozzo".into());
                }
            }
        }
        Ok(())
    }

    pub fn can_challenge_team(&self, team: &Team) -> AppResult<()> {
        if self.id == team.id {
            return Err("Cannot challenge self".into());
        }

        if matches!(self.current_location, TeamLocation::Travelling { .. }) {
            return Err("Team is travelling".into());
        }

        if matches!(team.current_location, TeamLocation::Travelling { .. }) {
            return Err("Opponent is travelling".into());
        }

        if self.current_location != team.current_location {
            return Err("Not on the same planet".into());
        }

        if self.current_game.is_some() {
            return Err("Team is already playing".into());
        }

        if team.current_game.is_some() {
            return Err("Opponent is already playing".into());
        }

        if self.current_location != team.current_location {
            return Err("Not on the same planet".into());
        }

        Ok(())
    }

    pub fn can_travel_to_planet(&self, planet: &Planet, travel_time: Tick) -> AppResult<()> {
        match self.current_location {
            TeamLocation::OnPlanet {
                planet_id: current_planet_id,
            } => {
                if planet.id == current_planet_id {
                    return Err("Already on this planet".into());
                }
            }
            TeamLocation::Travelling {
                from: _from,
                to: _to,
                started,
                duration,
            } => {
                let current = Tick::now();
                if started + duration > current {
                    return Err(format!(
                        "Travelling ({})",
                        (started + duration - current).formatted()
                    )
                    .into());
                } else {
                    return Err("Landing...".into());
                };
            }
        }

        if self.current_game.is_some() {
            return Err("Team is currently playing".into());
        }

        if planet.total_population() == 0 {
            return Err("This place is inhabitable".into());
        }

        let autonomy = self.spaceship.max_travel_time();
        if travel_time > autonomy {
            return Err("This planet is too far".into());
        }

        Ok(())
    }

    pub fn can_change_training_focus(&self) -> AppResult<()> {
        if self.current_game.is_some() {
            return Err("Team is currently playing".into());
        }
        Ok(())
    }

    pub fn is_travelling(&self) -> bool {
        matches!(self.current_location, TeamLocation::Travelling { .. })
    }

    pub fn add_player(&mut self, player: &mut Player) {
        if self.player_ids.contains(&player.id) {
            return;
        }
        player.team = Some(self.id);
        player.current_location = PlayerLocation::WithTeam;
        self.player_ids.push(player.id);
        player.jersey_number = Some(self.max_jersey_number as usize);
        self.max_jersey_number += 1;
        player.set_jersey(&self.jersey);
        player.version += 1;
    }

    pub fn remove_player(&mut self, player: &mut Player) -> AppResult<()> {
        if !self.player_ids.contains(&player.id) {
            return Err("Player is not in team".into());
        }
        player.team = None;
        self.player_ids.retain(|&p| p != player.id);
        player.jersey_number = None;
        player.image.remove_jersey();
        player.compose_image()?;
        match self.current_location {
            TeamLocation::OnPlanet { planet_id } => {
                player.current_location = PlayerLocation::OnPlanet { planet_id };
            }
            _ => return Err("Cannot release player while travelling".into()),
        }
        player.version += 1;
        Ok(())
    }

    pub fn best_position_assignment(mut players: Vec<&Player>) -> Vec<PlayerId> {
        // return players.iter().map(|&p| p.id).collect();
        if players.len() < MAX_POSITION as usize {
            return players.iter().map(|&p| p.id).collect();
        }

        players.sort_by(|a, b| b.total_skills().cmp(&a.total_skills()));

        // Create an N-vector of 5-vectors. Each player is mapped to the vector (of length 5) of ratings for each role.
        let all_ratings = players
            .iter()
            .take(12) // For performance reasons, we only consider the first 12 players by rating.
            .map(|&p| {
                (0..MAX_POSITION)
                    .map(|i| i.player_rating(p.current_skill_array()))
                    .collect::<Vec<f32>>()
            })
            .collect::<Vec<Vec<f32>>>();

        let mut max_team_value = 0.0;
        let mut max_perm_index: usize = 0;

        // Iterate over all 5-permutations of the players. For each permutation assign a value equal to the sum of the ratings
        // when the player is assigned to the role corresponding to the index in the permutation.
        for perm in all_ratings.iter().permutations(5).enumerate() {
            let team_value = (0..MAX_POSITION as usize)
                .map(|i| perm.1[i][i])
                .sum::<f32>();
            if team_value > max_team_value {
                max_team_value = team_value;
                max_perm_index = perm.0;
            }
        }

        let idx_perms = (0..min(players.len(), 12))
            .permutations(5)
            .collect::<Vec<Vec<usize>>>();
        let max_perm = &idx_perms[max_perm_index];
        let mut new_players: Vec<PlayerId> = max_perm.iter().map(|&i| players[i].id).collect();
        assert!(new_players.len() == MAX_POSITION as usize);
        let mut bench = players
            .iter()
            .filter(|&p| !new_players.contains(&p.id))
            .map(|&p| p)
            .collect::<Vec<&Player>>();
        bench.sort_by(|a, b| b.total_skills().cmp(&a.total_skills()));
        new_players.append(&mut bench.iter().map(|&p| p.id).collect::<Vec<PlayerId>>());

        new_players
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        types::{IdSystem, TeamId},
        world::{planet::Planet, utils::TEAM_DATA},
    };

    #[test]
    fn test_team_random() {
        let data = TEAM_DATA.as_ref().unwrap();
        let (name, _) = data.names[0].clone();
        let team = super::Team::random(TeamId::new(), Planet::default().id, name);
        println!("{:?}", team);
    }
}
