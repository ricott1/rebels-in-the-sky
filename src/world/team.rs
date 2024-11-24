use super::{
    constants::{INITIAL_TEAM_BALANCE, KILOMETER, MIN_PLAYERS_PER_GAME},
    jersey::Jersey,
    planet::Planet,
    player::Player,
    position::MAX_POSITION,
    resources::Resource,
    spaceship::{Spaceship, SpaceshipUpgrade},
    types::{TeamLocation, TrainingFocus},
};
use crate::{
    game_engine::tactic::Tactic,
    network::{challenge::Challenge, trade::Trade},
    types::*,
    world::{constants::MAX_PLAYERS_PER_TEAM, utils::is_default},
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
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub player_ids: Vec<PlayerId>,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub kartoffel_ids: Vec<KartoffelId>,
    pub crew_roles: CrewRoles,
    pub jersey: Jersey,
    pub resources: ResourceMap,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub resources_gathered: ResourceMap,
    pub spaceship: Spaceship,
    pub home_planet_id: PlanetId,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub asteroid_ids: Vec<PlanetId>,
    pub current_location: TeamLocation,
    pub peer_id: Option<PeerId>,
    pub current_game: Option<GameId>,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub game_record: [u32; 3], // Stores game record as wins/losses/draws
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub network_game_record: [u32; 3], // Stores game record as wins/losses/draws
    pub game_tactic: Tactic,
    pub training_focus: Option<TrainingFocus>,
    #[serde(skip)]
    pub sent_trades: HashMap<(PlayerId, PlayerId), Trade>,
    #[serde(skip)]
    pub received_trades: HashMap<(PlayerId, PlayerId), Trade>,
    #[serde(skip)]
    pub sent_challenges: HashMap<TeamId, Challenge>,
    #[serde(skip)]
    pub received_challenges: HashMap<TeamId, Challenge>,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub total_travelled: KILOMETER,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub number_of_space_adventures: usize,
}

impl Team {
    pub fn random(
        id: TeamId,
        home_planet_id: PlanetId,
        name: impl Into<String>,
        ship_name: impl Into<String>,
    ) -> Self {
        let jersey = Jersey::random();
        let ship_color = jersey.color;
        let mut resources = HashMap::new();
        resources.insert(Resource::SATOSHI, INITIAL_TEAM_BALANCE);
        Self {
            id,
            name: name.into(),
            jersey,
            home_planet_id,
            current_location: TeamLocation::OnPlanet {
                planet_id: home_planet_id,
            },
            spaceship: Spaceship::random(ship_name.into()).with_color_map(ship_color),
            game_tactic: Tactic::random(),
            resources,
            ..Default::default()
        }
    }

    pub fn add_sent_challenge(&mut self, challenge: Challenge) {
        self.sent_challenges
            .insert(challenge.away_team_in_game.team_id, challenge);
    }

    pub fn add_received_challenge(&mut self, challenge: Challenge) {
        self.received_challenges
            .insert(challenge.home_team_in_game.team_id, challenge);
    }

    pub fn remove_challenge(&mut self, home_team_id: TeamId, away_team_id: TeamId) {
        let team_id = if home_team_id == self.id {
            away_team_id
        } else {
            home_team_id
        };
        self.sent_challenges.remove(&team_id);
        self.received_challenges.remove(&team_id);
    }

    pub fn clear_challenges(&mut self) {
        self.sent_challenges.clear();
        self.received_challenges.clear();
    }

    pub fn add_sent_trade(&mut self, trade: Trade) {
        self.sent_trades
            .insert((trade.proposer_player.id, trade.target_player.id), trade);
    }

    pub fn add_received_trade(&mut self, trade: Trade) {
        self.received_trades
            .insert((trade.proposer_player.id, trade.target_player.id), trade);
    }

    pub fn remove_trade(&mut self, proposer_player_id: PlayerId, target_player_id: PlayerId) {
        self.sent_trades
            .remove(&(proposer_player_id, target_player_id));
        self.received_trades
            .remove(&(proposer_player_id, target_player_id));
    }

    pub fn clear_trades(&mut self) {
        self.sent_trades.clear();
        self.received_trades.clear();
    }

    pub fn balance(&self) -> u32 {
        self.resources.value(&Resource::SATOSHI)
    }

    pub fn fuel(&self) -> u32 {
        self.resources.value(&Resource::FUEL)
    }

    pub fn used_fuel_capacity(&self) -> u32 {
        self.resources.used_fuel_capacity()
    }

    pub fn fuel_capacity(&self) -> u32 {
        self.spaceship.fuel_capacity()
    }

    pub fn used_storage_capacity(&self) -> u32 {
        self.resources.used_storage_capacity()
    }

    pub fn storage_capacity(&self) -> u32 {
        self.spaceship.storage_capacity()
    }

    pub fn spaceship_speed(&self) -> f32 {
        self.spaceship.speed(self.used_storage_capacity())
    }

    pub fn spaceship_fuel_consumption(&self) -> f32 {
        self.spaceship
            .fuel_consumption(self.used_storage_capacity())
    }

    pub fn is_on_planet(&self) -> Option<PlanetId> {
        match self.current_location {
            TeamLocation::OnPlanet { planet_id } => Some(planet_id),
            _ => None,
        }
    }

    pub fn can_add_player(&self, player: &Player) -> AppResult<()> {
        if player.team.is_some() {
            return Err(anyhow!("Already in a team"));
        }

        if self.player_ids.len() >= self.spaceship.crew_capacity() as usize {
            return Err(anyhow!("Team is full"));
        }

        match self.current_location {
            TeamLocation::Exploring { .. } => {
                return Err(anyhow!("Team is exploring"));
            }

            TeamLocation::Travelling { .. } => {
                return Err(anyhow!("Team is travelling"));
            }

            TeamLocation::OnSpaceAdventure { .. } => {
                return Err(anyhow!("Team is on a space adventure"));
            }

            _ => {}
        }

        // Player must be on same planet as team current_location
        if self.is_on_planet() != player.is_on_planet() {
            return Err(anyhow!("Not on the same planet"));
        }

        Ok(())
    }

    pub fn can_hire_player(&self, player: &Player) -> AppResult<()> {
        self.can_add_player(player)?;
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

        if self.is_on_planet().is_none() {
            return Err(anyhow!("Team is not on a planet"));
        }

        if self.current_game.is_some() {
            return Err(anyhow!("Team is playing"));
        }

        Ok(())
    }

    pub fn can_set_crew_role(&self, player: &Player) -> AppResult<()> {
        if player.team.is_none() {
            return Err(anyhow!("Player is not in a team"));
        }

        if self.current_game.is_some() {
            return Err(anyhow!("Team is playing"));
        }

        Ok(())
    }

    pub fn can_challenge_team_over_network(&self, team: &Team) -> AppResult<()> {
        // This function runs checks similar to can_challenge_team,
        // but crucially skips the checks about the current_game.
        // This is to go around a race condition described in the challenge SynAck protocol.

        if self.id == team.id {
            return Err(anyhow!("Cannot challenge self"));
        }

        if self.is_on_planet().is_none() {
            return Err(anyhow!("Team is in space"));
        }

        if self.is_on_planet() != team.is_on_planet() {
            return Err(anyhow!("Not on the same planet"));
        }

        if self.player_ids.len() < MIN_PLAYERS_PER_GAME {
            return Err(anyhow!("Team does not have enough players"));
        }

        if team.player_ids.len() < MIN_PLAYERS_PER_GAME {
            return Err(anyhow!("Opponent does not have enough players"));
        }

        Ok(())
    }

    pub fn can_challenge_team(&self, team: &Team) -> AppResult<()> {
        if self.current_game.is_some() {
            return Err(anyhow!("Team is already playing"));
        }

        if team.current_game.is_some() {
            return Err(anyhow!("Opponent is already playing"));
        }

        if self.sent_challenges.get(&team.id).is_some() {
            return Err(anyhow!("Already challenged"));
        }

        self.can_challenge_team_over_network(team)
    }

    pub fn can_trade_players(
        &self,
        proposer_player: &Player,
        target_player: &Player,
        target_team: &Team,
    ) -> AppResult<()> {
        // This is always run from the proposer team point of view.
        if self.id == target_team.id {
            return Err(anyhow!("Cannot trade with oneself"));
        }

        if proposer_player.team.is_none() || proposer_player.team.unwrap() != self.id {
            return Err(anyhow!("Proposed player is not part of the team"));
        }

        if target_player.team.is_none() || target_player.team.unwrap() != target_team.id {
            return Err(anyhow!("Target player is not part of the team"));
        }

        if target_player.team.unwrap() == self.id {
            return Err(anyhow!("Target player is in team"));
        }

        if self.is_on_planet() != target_team.is_on_planet() {
            return Err(anyhow!("Not on the same planet"));
        }

        if self.current_game.is_some() {
            return Err(anyhow!("Team is playing"));
        }

        if target_team.current_game.is_some() {
            return Err(anyhow!("Opponent is playing"));
        }

        Ok(())
    }

    pub fn can_travel_to_planet(&self, planet: &Planet, duration: Tick) -> AppResult<()> {
        if planet.peer_id.is_some() {
            return Err(anyhow!("Cannot travel to asteroid"));
        }

        if self.player_ids.len() < 1 {
            return Err(anyhow!("No pirate to travel"));
        }

        if let Some(current_planet_id) = self.is_on_planet() {
            if planet.id == current_planet_id {
                return Err(anyhow!("Already on this planet"));
            }
        } else {
            return Err(anyhow!("Already in space"));
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

    pub fn can_start_space_adventure(&self) -> AppResult<()> {
        if self.player_ids.len() < 1 {
            return Err(anyhow!("No pirate to explore"));
        }

        if self.is_on_planet().is_none() {
            return Err(anyhow!("Already in space"));
        }

        if self.spaceship.pending_upgrade.is_some() {
            return Err(anyhow!("Upgrading spaceship"));
        }

        if self.current_game.is_some() {
            return Err(anyhow!("Team is playing"));
        }

        if self.spaceship.current_durability() == 0 {
            return Err(anyhow!("Spaceship needs reparations"));
        }

        if self.fuel() == 0 {
            return Err(anyhow!("Not enough fuel"));
        }

        Ok(())
    }

    pub fn can_explore_around_planet(
        &self,
        planet: &Planet,
        exploration_time: Tick,
    ) -> AppResult<()> {
        if let Err(err) = self.can_start_space_adventure() {
            return Err(anyhow!(err));
        }

        if self.is_on_planet() != Some(planet.id) {
            return Err(anyhow!("Not on this planet"));
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
                let storage_capacity = self.spaceship.fuel_capacity();
                if current + amount as u32 > storage_capacity {
                    return Err(anyhow!("Not enough storage capacity"));
                }
            } else {
                let current = self.resources.used_storage_capacity();
                let storage_capacity = self.spaceship.storage_capacity();
                if current + resource.to_storing_space() * amount as u32 > storage_capacity {
                    return Err(anyhow!("Not enough storage capacity"));
                }
            }
        } else if amount < 0 {
            // Selling. Check if enough resource
            let current = self.resources.value(&resource);
            if current < amount.abs() as u32 {
                return Err(anyhow!("Not enough resource"));
            }
        }
        Ok(())
    }

    pub fn can_set_upgrade_spaceship(&self, upgrade: SpaceshipUpgrade) -> AppResult<()> {
        if self.is_on_planet().is_none() {
            return Err(anyhow!("Can only upgrade on a planet"));
        }

        for (resource, amount) in upgrade.cost().iter() {
            if self.resources.value(resource) < *amount {
                return Err(anyhow!("Insufficient resources"));
            }
        }

        Ok(())
    }

    pub fn max_resource_storage_capacity(&self, resource: Resource) -> u32 {
        if resource == Resource::FUEL {
            self.spaceship.fuel_capacity() - self.fuel()
        } else {
            let free_storage_capacity =
                self.spaceship.storage_capacity() - self.resources.used_storage_capacity();
            if resource.to_storing_space() == 0 {
                u32::MAX
            } else {
                free_storage_capacity / resource.to_storing_space()
            }
        }
    }

    pub fn max_resource_buy_amount(&self, resource: Resource, unit_cost: u32) -> u32 {
        if unit_cost == 0 {
            return u32::MAX;
        }

        let max_satoshi_amount = self.balance() / unit_cost;
        let max_storage_amount = if resource == Resource::FUEL {
            self.spaceship.fuel_capacity() - self.fuel()
        } else if resource.to_storing_space() == 0 {
            u32::MAX
        } else {
            let free_storage_capacity =
                self.spaceship.storage_capacity() - self.resources.used_storage_capacity();
            free_storage_capacity / resource.to_storing_space()
        };

        max_satoshi_amount.min(max_storage_amount)
    }

    pub fn max_resource_sell_amount(&self, resource: Resource) -> u32 {
        self.resources.value(&resource)
    }

    pub fn is_travelling(&self) -> bool {
        matches!(self.current_location, TeamLocation::Travelling { .. })
    }

    pub fn best_position_assignment(mut players: Vec<&Player>) -> Vec<PlayerId> {
        if players.len() < MAX_POSITION as usize {
            return players.iter().map(|&p| p.id).collect();
        }

        // Sort players in case we need to take only the first MAX_PLAYERS_PER_TEAM
        players.sort_by(|a, b| {
            b.average_skill()
                .partial_cmp(&a.average_skill())
                .expect("Skill value should exist")
        });

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
        bench.sort_by(|a, b| {
            b.average_skill()
                .partial_cmp(&a.average_skill())
                .expect("Skill value should exist")
        });
        new_players.append(&mut bench.iter().map(|&p| p.id).collect::<Vec<PlayerId>>());

        new_players
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        types::TeamId,
        world::{planet::Planet, utils::TEAM_DATA},
    };

    #[test]
    fn test_team_random() {
        let (name, _) = TEAM_DATA[0].clone();
        let team = super::Team::random(TeamId::new_v4(), Planet::default().id, name, "test");
        println!("{:?}", team);
    }
}
