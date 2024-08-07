use super::{
    constants::{MIN_PLAYERS_PER_TEAM, MORALE_BONUS_ON_TEAM_ADD, MORALE_DEMOTION_MALUS},
    jersey::Jersey,
    planet::Planet,
    player::Player,
    position::MAX_POSITION,
    resources::Resource,
    role::CrewRole,
    skill::GameSkill,
    spaceship::{Spaceship, SpaceshipUpgrade},
    types::{PlayerLocation, TeamLocation, TrainingFocus},
};
use crate::{
    engine::tactic::Tactic,
    types::{AppResult, GameId, PlanetId, PlayerId, SystemTimeTick, TeamId, Tick},
    world::constants::MAX_PLAYERS_PER_TEAM,
};
use anyhow::anyhow;
use itertools::Itertools;
use libp2p::PeerId;
use serde::{Deserialize, Serialize};
use std::{cmp::min, collections::HashMap};

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
    pub resources: HashMap<Resource, u32>,
    pub spaceship: Spaceship,
    pub home_planet_id: PlanetId,
    pub current_location: TeamLocation,
    pub peer_id: Option<PeerId>,
    pub current_game: Option<GameId>,
    pub game_tactic: Tactic,
    pub training_focus: Option<TrainingFocus>,
}

impl Team {
    pub fn random(id: TeamId, home_planet_id: PlanetId, name: String) -> Self {
        let jersey = Jersey::random();
        let ship_name = format!("{}shipp", name);
        let ship_color = jersey.color;
        Self {
            id,
            name,
            jersey,
            home_planet_id,
            current_location: TeamLocation::OnPlanet {
                planet_id: home_planet_id,
            },
            spaceship: Spaceship::random(ship_name, ship_color),
            game_tactic: Tactic::random(),
            ..Default::default()
        }
    }

    pub fn balance(&self) -> u32 {
        self.resources
            .get(&Resource::SATOSHI)
            .copied()
            .unwrap_or_default()
    }

    pub fn fuel(&self) -> u32 {
        self.resources
            .get(&Resource::FUEL)
            .copied()
            .unwrap_or_default()
    }

    pub fn used_storage_capacity(&self) -> u32 {
        Resource::used_storage_capacity(&self.resources)
    }

    pub fn max_storage_capacity(&self) -> u32 {
        self.spaceship.storage_capacity()
    }

    pub fn add_resource(&mut self, resource: Resource, amount: u32) {
        let max_amount = if resource == Resource::FUEL {
            let current = self.fuel();
            let max_storage_capacity = self.spaceship.fuel_capacity();
            amount.min(max_storage_capacity - current)
        } else {
            if resource.to_storing_space() == 0 {
                amount
            } else {
                let current = Resource::used_storage_capacity(&self.resources);
                let max_storage_capacity = self.spaceship.storage_capacity();
                amount.min((max_storage_capacity - current) / resource.to_storing_space())
            }
        };

        self.resources
            .entry(resource)
            .and_modify(|e| {
                *e = e.saturating_add(max_amount);
            })
            .or_insert(max_amount);
    }

    pub fn remove_resource(&mut self, resource: Resource, amount: u32) -> AppResult<()> {
        self.can_trade_resource(resource, -(amount as i32), 0)?;
        self.resources
            .entry(resource)
            .and_modify(|e| {
                *e = e.saturating_sub(amount);
            })
            .or_insert(0);
        Ok(())
    }

    pub fn can_hire_player(&self, player: &Player) -> AppResult<()> {
        if player.team.is_some() {
            return Err(anyhow!("Already in a team"));
        }

        if self.player_ids.len() >= self.spaceship.crew_capacity() as usize {
            return Err(anyhow!("Team is full"));
        }

        // Player must be on same planet as team current_location
        match self.current_location {
            TeamLocation::OnPlanet {
                planet_id: team_planet_id,
            } => match player.current_location {
                PlayerLocation::OnPlanet { planet_id } => {
                    if planet_id != team_planet_id {
                        return Err(anyhow!("Not on team planet"));
                    }
                }
                PlayerLocation::WithTeam => {
                    return Err(anyhow!("Already in a team"));
                }
            },
            TeamLocation::Travelling { .. } => {
                return Err(anyhow!("Team is travelling"));
            }
            TeamLocation::Exploring { .. } => {
                return Err(anyhow!("Team is exploring"));
            }
        }

        let hiring_cost = player.hire_cost(self.reputation);
        if self.balance() < hiring_cost {
            return Err(anyhow!("Not enough money {}", hiring_cost));
        }

        Ok(())
    }

    pub fn can_release_player(&self, player: &Player) -> AppResult<()> {
        if !self.player_ids.contains(&player.id) {
            return Err(anyhow!("Player is not in team"));
        }

        if player.team.is_none() {
            return Err(anyhow!("Player is not in a team"));
        }

        match self.current_location {
            TeamLocation::Travelling { .. } => {
                return Err(anyhow!("Team is travelling"));
            }
            _ => {}
        }

        if self.current_game.is_some() {
            return Err(anyhow!("Team is playing"));
        }

        if self.player_ids.len() <= MIN_PLAYERS_PER_TEAM {
            return Err(anyhow!("Team is too small"));
        }
        Ok(())
    }

    pub fn can_set_crew_role(&self, player: &Player, role: CrewRole) -> AppResult<()> {
        if player.team.is_none() {
            return Err(anyhow!("Player is not in a team"));
        }

        if self.current_game.is_some() {
            return Err(anyhow!("Team is playing"));
        }

        match role {
            CrewRole::Captain => {
                if self.crew_roles.captain == Some(player.id) {
                    return Err(anyhow!("Player is already captain"));
                }
            }
            CrewRole::Doctor => {
                if self.crew_roles.doctor == Some(player.id) {
                    return Err(anyhow!("Player is already doctor"));
                }
            }
            CrewRole::Pilot => {
                if self.crew_roles.pilot == Some(player.id) {
                    return Err(anyhow!("Player is already pilot"));
                }
            }
            CrewRole::Mozzo => {
                if self.crew_roles.mozzo.contains(&player.id) {
                    return Err(anyhow!("Player is already mozzo"));
                }
            }
        }
        Ok(())
    }

    pub fn can_challenge_team(&self, team: &Team) -> AppResult<()> {
        if self.id == team.id {
            return Err(anyhow!("Cannot challenge self"));
        }

        if matches!(self.current_location, TeamLocation::Travelling { .. }) {
            return Err(anyhow!("Team is travelling"));
        }

        if matches!(team.current_location, TeamLocation::Travelling { .. }) {
            return Err(anyhow!("Opponent is travelling"));
        }

        if self.current_location != team.current_location {
            return Err(anyhow!("Not on the same planet"));
        }

        if self.current_game.is_some() {
            return Err(anyhow!("Team is already playing"));
        }

        if team.current_game.is_some() {
            return Err(anyhow!("Opponent is already playing"));
        }

        if self.current_location != team.current_location {
            return Err(anyhow!("Not on the same planet"));
        }

        Ok(())
    }

    pub fn can_travel_to_planet(&self, planet: &Planet, duration: Tick) -> AppResult<()> {
        if planet.peer_id.is_some() {
            return Err(anyhow!("Cannot travel to asteroid"));
        }

        match self.current_location {
            TeamLocation::OnPlanet {
                planet_id: current_planet_id,
            } => {
                if planet.id == current_planet_id {
                    return Err(anyhow!("Already on this planet"));
                }
            }
            TeamLocation::Travelling {
                from: _from,
                to: _to,
                started,
                duration,
                ..
            } => {
                let current = Tick::now();
                if started + duration > current {
                    return Err(anyhow!(
                        "Travelling ({})",
                        (started + duration - current).formatted()
                    ));
                } else {
                    return Err(anyhow!("Landing..."));
                };
            }
            TeamLocation::Exploring { .. } => {
                return Err(anyhow!("Exploring"));
            }
        }

        if self.spaceship.pending_upgrade.is_some() {
            return Err(anyhow!("Upgrading spaceship"));
        }

        if self.current_game.is_some() {
            return Err(anyhow!("Team is playing"));
        }

        if planet.total_population() == 0 && self.home_planet_id != planet.id {
            return Err(anyhow!("This place is inhabitable"));
        }

        //If we can't get there with full tank, than the planet is too far.
        let max_fuel = self.spaceship.fuel_capacity();
        let max_autonomy = self.spaceship.max_travel_time(max_fuel);
        if duration > max_autonomy {
            return Err(anyhow!("This planet is too far"));
        }

        // Else we check that we can go there with the current fuel.
        let current_fuel = self.fuel();
        let autonomy = self.spaceship.max_travel_time(current_fuel);

        if duration > autonomy {
            return Err(anyhow!("Not enough fuel"));
        }

        Ok(())
    }

    pub fn can_explore_around_planet(
        &self,
        planet: &Planet,
        exploration_time: Tick,
    ) -> AppResult<()> {
        match self.current_location {
            TeamLocation::OnPlanet {
                planet_id: current_planet_id,
            } => {
                if planet.id != current_planet_id {
                    return Err(anyhow!("Not on this planet"));
                }
            }
            TeamLocation::Travelling {
                from: _from,
                to: _to,
                started,
                duration,
                ..
            } => {
                let current = Tick::now();
                if started + duration > current {
                    return Err(anyhow!(
                        "Travelling ({})",
                        (started + duration - current).formatted()
                    ));
                } else {
                    return Err(anyhow!("Landing..."));
                };
            }
            TeamLocation::Exploring { .. } => {
                return Err(anyhow!("Exploring"));
            }
        }

        if self.spaceship.pending_upgrade.is_some() {
            return Err(anyhow!("Upgrading spaceship"));
        }

        if self.current_game.is_some() {
            return Err(anyhow!("Team is playing"));
        }

        //If we can't get there with full tank, than the planet is too far.
        let max_fuel = self.spaceship.fuel_capacity();
        let max_autonomy = self.spaceship.max_travel_time(max_fuel);
        if exploration_time > max_autonomy {
            return Err(anyhow!("This planet is too far"));
        }

        // Else we check that we can go there with the current fuel.
        let current_fuel = self.fuel();
        let autonomy = self.spaceship.max_travel_time(current_fuel);

        if exploration_time > autonomy {
            return Err(anyhow!("Not enough fuel"));
        }

        Ok(())
    }

    pub fn can_change_training_focus(&self) -> AppResult<()> {
        if self.current_game.is_some() {
            return Err(anyhow!("Team is playing"));
        }
        Ok(())
    }

    pub fn can_trade_resource(
        &self,
        resource: Resource,
        amount: i32,
        unit_cost: u32,
    ) -> AppResult<()> {
        // Buying. Check if enough satoshi and if enough storing space
        if amount > 0 {
            let total_cost = amount as u32 * unit_cost;
            if self.balance() < total_cost {
                return Err(anyhow!("Not enough satoshi"));
            }

            if resource == Resource::FUEL {
                let current = self.fuel();
                let max_storage_capacity = self.spaceship.fuel_capacity();
                if current + amount as u32 > max_storage_capacity {
                    return Err(anyhow!("Not enough storage capacity"));
                }
            } else {
                let current = Resource::used_storage_capacity(&self.resources);
                let max_storage_capacity = self.spaceship.storage_capacity();
                if current + resource.to_storing_space() * amount as u32 > max_storage_capacity {
                    return Err(anyhow!("Not enough storage capacity"));
                }
            }
        } else {
            // Selling. Check if enough resource
            let current = self.resources.get(&resource).copied().unwrap_or_default();
            if current < amount.abs() as u32 {
                return Err(anyhow!("Not enough resource"));
            }
        }
        Ok(())
    }

    pub fn can_set_upgrade_spaceship(&self, upgrade: SpaceshipUpgrade) -> AppResult<()> {
        match self.current_location {
            TeamLocation::OnPlanet { .. } => {}
            _ => return Err(anyhow!("Can only upgrade on a planet")),
        }

        for (resource, amount) in upgrade.cost.iter() {
            if self.resources.get(resource).copied().unwrap_or_default() < *amount {
                return Err(anyhow!("Insufficient resources"));
            }
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
        player.morale = (player.morale + MORALE_BONUS_ON_TEAM_ADD).bound();
        player.current_location = PlayerLocation::WithTeam;
        self.player_ids.push(player.id);
        player.set_jersey(&self.jersey);
        player.version += 1;
    }

    pub fn remove_player(&mut self, player: &mut Player) -> AppResult<()> {
        self.can_release_player(&player)?;

        player.team = None;
        self.player_ids.retain(|&p| p != player.id);

        match player.info.crew_role {
            CrewRole::Captain => self.crew_roles.captain = None,
            CrewRole::Doctor => self.crew_roles.doctor = None,
            CrewRole::Pilot => self.crew_roles.pilot = None,
            CrewRole::Mozzo => self.crew_roles.mozzo.retain(|&p| p != player.id),
        }

        player.info.crew_role = CrewRole::Mozzo;
        // Removed player is a bit demoralized :(
        if player.morale > MORALE_DEMOTION_MALUS {
            player.morale -= MORALE_DEMOTION_MALUS;
        } else {
            player.morale = 0.0;
        }

        player.image.remove_jersey();
        player.compose_image()?;
        match self.current_location {
            TeamLocation::OnPlanet { planet_id } => {
                player.current_location = PlayerLocation::OnPlanet { planet_id };
            }
            _ => return Err(anyhow!("Cannot release player while travelling")),
        }
        Ok(())
    }

    pub fn best_position_assignment(mut players: Vec<&Player>) -> Vec<PlayerId> {
        if players.len() < MAX_POSITION as usize {
            return players.iter().map(|&p| p.id).collect();
        }

        // Sort players in case we need to take only the first MAX_PLAYERS_PER_TEAM
        players.sort_by(|a, b| b.total_skills().cmp(&a.total_skills()));

        // Create an N-vector of 5-vectors. Each player is mapped to the vector (of length 5) of ratings for each role.
        let all_ratings = players
            .iter()
            .take(MAX_PLAYERS_PER_TEAM) // For performance reasons, we only consider the first MAX_PLAYERS_PER_TEAM players by rating.
            .map(|&p| {
                (0..MAX_POSITION)
                    .map(|position| p.tiredness_weighted_rating_at_position(position))
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
        let (name, _) = TEAM_DATA[0].clone();
        let team = super::Team::random(TeamId::new(), Planet::default().id, name);
        println!("{:?}", team);
    }
}
