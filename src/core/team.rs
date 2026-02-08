use super::*;
use crate::{
    core::{constants::MAX_CREW_SIZE, utils::is_default},
    game_engine::{tactic::Tactic, types::EnginePlayer, Tournament, TournamentId, TournamentState},
    network::{challenge::Challenge, trade::Trade},
    types::*,
};
use anyhow::anyhow;
use itertools::Itertools;
use libp2p::PeerId;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use std::{
    cmp::min,
    collections::{HashMap, HashSet},
};
use strum::Display;

#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq)]
pub struct CrewRoles {
    pub captain: Option<PlayerId>,
    pub doctor: Option<PlayerId>,
    pub pilot: Option<PlayerId>,
    pub engineer: Option<PlayerId>,
    pub mozzo: Vec<PlayerId>,
}

#[derive(Debug, Default, Display, Serialize, Deserialize, Clone, Copy, PartialEq)]
pub enum TournamentRegistrationState {
    #[default]
    None,
    Pending {
        tournament_id: TournamentId,
    },
    Registered {
        tournament_id: TournamentId,
    },
    Confirmed {
        tournament_id: TournamentId,
    },
}

#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq)]
pub struct Team {
    pub id: TeamId,
    pub version: u64,
    pub name: String,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub creation_time: Tick,
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
    pub tournament_registration_state: TournamentRegistrationState,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub is_organizing_tournament: Option<TournamentId>,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub local_game_rating: GameRating,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub network_game_rating: GameRating,
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
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub autonomous_strategy: AutonomousStrategy,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub honours: HashSet<Honour>,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub space_cove: Option<SpaceCove>,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub tournaments_won: Vec<TournamentId>,
}

impl Team {
    pub fn random(rng: Option<&mut ChaCha8Rng>) -> Self {
        let rng = if let Some(r) = rng {
            r
        } else {
            &mut ChaCha8Rng::from_os_rng()
        };
        let jersey = Jersey::random(rng);
        let ship_color = jersey.color;
        let mut resources = HashMap::new();
        resources.insert(Resource::SATOSHI, INITIAL_TEAM_BALANCE);
        Self {
            id: TeamId::new_v4(),
            creation_time: Tick::now(),
            jersey,
            spaceship: Spaceship::random(rng).with_color_map(ship_color),
            game_tactic: Tactic::random(rng),
            resources,
            ..Default::default()
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    pub fn with_spaceship_name(mut self, name: impl Into<String>) -> Self {
        self.spaceship.name = name.into();
        self
    }

    pub fn with_home_planet(mut self, home_planet_id: PlanetId) -> Self {
        self.home_planet_id = home_planet_id;
        self.current_location = TeamLocation::OnPlanet {
            planet_id: home_planet_id,
        };
        self
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
        let other_team_id = if home_team_id == self.id {
            away_team_id
        } else {
            home_team_id
        };
        self.sent_challenges.remove(&other_team_id);
        self.received_challenges.remove(&other_team_id);
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

    pub fn add_resource(&mut self, resource: Resource, amount: u32) -> AppResult<()> {
        if resource == Resource::FUEL {
            self.resources.add(resource, amount, self.fuel_capacity())?;
        } else {
            self.resources
                .add(resource, amount, self.storage_capacity())?;
        }
        Ok(())
    }

    pub fn saturating_add_resource(&mut self, resource: Resource, amount: u32) {
        if resource == Resource::FUEL {
            self.resources
                .saturating_add(resource, amount, self.fuel_capacity());
        } else {
            self.resources
                .saturating_add(resource, amount, self.storage_capacity());
        }
    }

    pub fn sub_resource(&mut self, resource: Resource, amount: u32) -> AppResult<()> {
        self.resources.sub(resource, amount)
    }

    pub fn saturating_sub_resource(&mut self, resource: Resource, amount: u32) {
        self.resources.saturating_sub(resource, amount);
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

    pub fn available_fuel_capacity(&self) -> u32 {
        self.fuel_capacity() - self.used_fuel_capacity()
    }

    pub fn used_storage_capacity(&self) -> u32 {
        self.resources.used_storage_capacity()
    }

    pub fn storage_capacity(&self) -> u32 {
        self.spaceship.storage_capacity()
    }

    pub fn available_storage_capacity(&self) -> u32 {
        self.storage_capacity() - self.used_storage_capacity()
    }

    pub fn spaceship_speed(&self) -> f32 {
        self.spaceship.speed(self.used_storage_capacity())
    }

    pub fn spaceship_fuel_consumption_per_tick(&self) -> f32 {
        self.spaceship
            .fuel_consumption_per_tick(self.used_storage_capacity())
    }

    pub fn spaceship_fuel_consumption_per_kilometer(&self) -> f32 {
        self.spaceship
            .fuel_consumption_per_kilometer(self.used_storage_capacity())
    }

    pub fn is_on_planet(&self) -> Option<PlanetId> {
        match self.current_location {
            TeamLocation::OnPlanet { planet_id } => Some(planet_id),
            _ => None,
        }
    }

    pub fn playing_in_tournament(&self) -> Option<PlanetId> {
        match self.tournament_registration_state {
            TournamentRegistrationState::None
            | TournamentRegistrationState::Pending { .. }
            | TournamentRegistrationState::Registered { .. } => None,
            TournamentRegistrationState::Confirmed { tournament_id } => Some(tournament_id),
        }
    }

    pub fn committed_to_tournament(&self) -> Option<PlanetId> {
        match self.tournament_registration_state {
            TournamentRegistrationState::None => None,
            TournamentRegistrationState::Pending { tournament_id }
            | TournamentRegistrationState::Registered { tournament_id }
            | TournamentRegistrationState::Confirmed { tournament_id } => Some(tournament_id),
        }
    }

    pub fn average_tiredness(&self, world: &World) -> f32 {
        let tiredness_iter = self
            .player_ids
            .iter()
            .take(MAX_PLAYERS_PER_GAME)
            .map(|&id| {
                if let Ok(player) = world.players.get_or_err(&id) {
                    player.current_tiredness(world)
                } else {
                    0.0
                }
            });

        let n = tiredness_iter.len();
        (tiredness_iter.sum::<f32>() / n as f32).bound()
    }

    pub fn is_on_player_planet(&self, player: &Player) -> bool {
        // Player must be on same planet as team current_location
        self.is_on_planet() == player.is_on_planet()
    }

    pub fn has_space_cove_on(&self) -> Option<PlanetId> {
        self.space_cove.as_ref().map(|cove| cove.planet_id)
    }

    pub fn can_teleport_to(&self, to: &Planet) -> AppResult<()> {
        let has_teleportation_pad = self.home_planet_id == to.id
            || to
                .upgrades
                .contains(&AsteroidUpgradeTarget::TeleportationPad);

        if !has_teleportation_pad {
            return Err(anyhow!("{} has no teleportation pad", to.name));
        }

        // If it has a pad but it's not your own asteroid, cannot teleport
        if self.home_planet_id != to.id && !self.asteroid_ids.contains(&to.id) {
            return Err(anyhow!("Cannot use teleportation pad on {}", to.name));
        }

        let rum_required = self.player_ids.len() as u32;
        let has_rum = self.resources.value(&Resource::RUM) >= rum_required;

        if !has_rum {
            return Err(anyhow!("Not enough Rum! You need at least {rum_required}"));
        }

        Ok(())
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

    // This function is necessary for local teams to consider hiring a player (even if the crew is full).
    pub fn can_consider_hiring_player(&self, player: &Player) -> AppResult<()> {
        let hiring_cost = player.hire_cost(self.reputation);
        if self.balance() < hiring_cost {
            return Err(anyhow!("Not enough money {hiring_cost}"));
        }

        // Check player age is not above limit
        if player.info.relative_age() >= 1.0 {
            return Err(anyhow!("Player is too old"));
        }

        Ok(())
    }

    pub fn can_hire_player(&self, player: &Player) -> AppResult<()> {
        self.can_add_player(player)?;
        self.can_consider_hiring_player(player)?;

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
            return Err(anyhow!("{} is not on a planet", self.name));
        }

        if self.current_game.is_some() {
            return Err(anyhow!("{} is playing", self.name));
        }

        if self.playing_in_tournament().is_some() {
            return Err(anyhow!("{} is in a tournament", self.name));
        }

        Ok(())
    }

    pub fn can_set_crew_role(&self, player: &Player) -> AppResult<()> {
        if player.team.is_none() {
            return Err(anyhow!("Player is not in a team"));
        }

        if self.current_game.is_some() {
            return Err(anyhow!("{} is playing", self.name));
        }

        if self.playing_in_tournament().is_some() {
            return Err(anyhow!("{} is in a tournament", self.name));
        }

        Ok(())
    }

    fn can_play_game_with_team(
        &self,
        team: &Team,
        part_of_tournament: Option<TournamentId>,
    ) -> AppResult<()> {
        if self.playing_in_tournament() != part_of_tournament {
            return Err(anyhow!(
                "{} game and team tournaments not matching",
                self.name
            ));
        }
        if team.playing_in_tournament() != part_of_tournament {
            return Err(anyhow!("Team game and team tournaments not matching"));
        }

        if self.id == team.id {
            return Err(anyhow!("Cannot play alone"));
        }

        if self.is_on_planet().is_none() {
            return Err(anyhow!("{} is in space", self.name));
        }

        if self.is_on_planet() != team.is_on_planet() {
            return Err(anyhow!(
                "{} and {} not on the same planet: {:#?} and {:#?}",
                self.name,
                team.name,
                self.is_on_planet(),
                team.is_on_planet()
            ));
        }

        if self.player_ids.len() < MIN_PLAYERS_PER_GAME {
            return Err(anyhow!("{} does not have enough pirates", self.name));
        }

        if team.player_ids.len() < MIN_PLAYERS_PER_GAME {
            return Err(anyhow!("{} does not have enough pirates", team.name));
        }

        Ok(())
    }

    pub fn can_organize_tournament(&self) -> AppResult<()> {
        if self.current_game.is_some() {
            return Err(anyhow!("{} is playing", self.name));
        }

        if self.committed_to_tournament().is_some() {
            return Err(anyhow!("{} is already commited to a tournament", self.name));
        }

        if self.is_organizing_tournament.is_some() {
            return Err(anyhow!("{} is already organizing a tournament", self.name));
        }

        match self.space_cove.as_ref() {
            None => {
                return Err(anyhow!(
                    "Cannot organize a tournament without a space cove."
                ));
            }
            Some(cove) => match cove.state {
                SpaceCoveState::UnderConstruction => {
                    return Err(anyhow!(
                        "Cannot organize a tournament if space cove is not ready."
                    ))
                }
                SpaceCoveState::Ready => {
                    if !matches!(self.is_on_planet(), Some(id) if id == cove.planet_id) {
                        return Err(anyhow!(
                            "Cannot organize a tournament while not at your space cove planet."
                        ));
                    }
                }
            },
        }

        // FIXME: add conditions on kartoffeln

        Ok(())
    }

    pub fn can_register_to_tournament(
        &self,
        tournament: &Tournament,
        timestamp: Tick,
    ) -> AppResult<()> {
        if !matches!(tournament.state(timestamp), TournamentState::Registration) {
            return Err(anyhow!("Tournament registrations are closed."));
        }

        if tournament.is_team_registered(&self.id) {
            return Err(anyhow!("Team is already registered to this tournament."));
        }

        if self.current_game.is_some() {
            return Err(anyhow!("Team is playing a game."));
        }

        // Only allowed state are None and Pending
        if matches!(
            self.tournament_registration_state,
            TournamentRegistrationState::Registered { tournament_id } if tournament_id != tournament.id
        ) {
            return Err(anyhow!("Team is registered to another tournament."));
        }

        if matches!(
            self.tournament_registration_state,
            TournamentRegistrationState::Registered { tournament_id } if tournament_id == tournament.id
        ) {
            return Err(anyhow!("Team is registered to this tournament."));
        }

        if matches!(
            self.tournament_registration_state,
            TournamentRegistrationState::Confirmed { .. }
        ) {
            return Err(anyhow!("Team is playing in a tournament."));
        }

        if !matches!(self.is_on_planet(), Some(id) if id == tournament.planet_id) {
            return Err(anyhow!("Team is not at the tournament location."));
        }

        Ok(())
    }

    pub fn can_confirm_tournament_registration(
        &self,
        tournament: &Tournament,
        timestamp: Tick,
    ) -> AppResult<()> {
        if !matches!(tournament.state(timestamp), TournamentState::Confirmation) {
            return Err(anyhow!("Tournament confirmations are closed."));
        }

        if !tournament.is_team_registered(&self.id) {
            return Err(anyhow!("Team is not registered to this tournament."));
        }

        if tournament.participants.len() == tournament.max_participants {
            return Err(anyhow!("Tournament is already full."));
        }

        if self.current_game.is_some() {
            return Err(anyhow!("Team is playing a game."));
        }

        if self.id != tournament.organizer_id
            && !matches!(
                self.tournament_registration_state,
                TournamentRegistrationState::Registered { tournament_id } if tournament_id == tournament.id
            )
        {
            return Err(anyhow!(
                "Team {} is not Registered for this tournament.",
                self.name
            ));
        }

        if !matches!(self.is_on_planet(), Some(id) if id == tournament.planet_id) {
            return Err(anyhow!("Team is not at the tournament location."));
        }

        Ok(())
    }

    pub fn can_accept_network_challenge(&self, team: &Team) -> AppResult<()> {
        // This function runs checks similar to can_challenge_local_team,
        // but crucially skips the checks about the current_game.
        // This is to go around a race condition described in the challenge SynAck protocol.

        self.can_play_game_with_team(team, None)
    }

    pub fn can_challenge_local_team(&self, team: &Team) -> AppResult<()> {
        if team.peer_id.is_some() {
            return Err(anyhow!("{} is not local", team.name));
        }

        if self.current_game.is_some() {
            return Err(anyhow!("{} is already playing", self.name));
        }

        if team.current_game.is_some() {
            return Err(anyhow!("{} is already playing", team.name));
        }

        self.can_play_game_with_team(team, None)
    }

    pub fn can_challenge_network_team(&self, team: &Team) -> AppResult<()> {
        if team.peer_id.is_none() {
            return Err(anyhow!("{} is not from network", team.name));
        }

        if self.current_game.is_some() {
            return Err(anyhow!("{} is already playing", self.name));
        }

        if team.current_game.is_some() {
            return Err(anyhow!("{} is already playing", team.name));
        }

        if self.sent_challenges.contains_key(&team.id) {
            return Err(anyhow!("Already challenged {}", team.name));
        }

        self.can_play_game_with_team(team, None)
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
            return Err(anyhow!("{} is playing", self.name));
        }

        if target_team.current_game.is_some() {
            return Err(anyhow!("{} is playing", target_team.name));
        }

        if self.playing_in_tournament().is_some() {
            return Err(anyhow!("{} is playing in a tournament", self.name));
        }

        if target_team.playing_in_tournament().is_some() {
            return Err(anyhow!("{} is playing in a tournament", target_team.name));
        }

        Ok(())
    }

    pub fn can_travel_to_planet(&self, planet: &Planet, duration: Tick) -> AppResult<()> {
        planet.can_be_travelled_to()?;

        if self.player_ids.is_empty() {
            return Err(anyhow!("No pirate to travel"));
        }

        if let Some(current_planet_id) = self.is_on_planet() {
            if planet.id == current_planet_id {
                return Err(anyhow!("Already on planet {}", planet.name));
            }
        } else {
            return Err(anyhow!("Already in space"));
        }

        if self.spaceship.pending_upgrade.is_some() {
            return Err(anyhow!("Upgrading spaceship"));
        }

        if self.current_game.is_some() {
            return Err(anyhow!("{} is playing", self.name));
        }

        if self.playing_in_tournament().is_some() {
            return Err(anyhow!("{} is playing in a tournament", self.name));
        }

        let is_teleporting = duration == TELEPORT_TRAVEL_DURATION;
        if is_teleporting {
            if let Err(e) = self.can_teleport_to(planet) {
                return Err(anyhow!("Cannot teleport to planet {}: {e}", planet.name));
            }
        } else {
            // If we can't get there with full tank, than the planet is too far.
            let max_fuel = self.fuel_capacity();

            let fuel_consumption =
                (duration as f64 * self.spaceship_fuel_consumption_per_tick() as f64).ceil() as u32;

            if fuel_consumption > max_fuel {
                return Err(anyhow!("Planet {} is too far", planet.name));
            }

            // Else we check that we can go there with the current fuel.
            // Note: this check seems wrong because there is a minimal consumption of 1 tonne of fuel for each travel,
            //       regardless of the distance. However, this is only relevant if the current fuel is 0, in which case
            //       any travel duration larger than 0 would fail this check.
            let current_fuel = self.fuel();
            if fuel_consumption > current_fuel {
                return Err(anyhow!("Not enough fuel"));
            }
        }

        Ok(())
    }

    pub fn can_start_space_adventure(&self, average_tiredness: Skill) -> AppResult<()> {
        if self.player_ids.is_empty() {
            return Err(anyhow!("No pirate to explore"));
        }

        if self.is_on_planet().is_none() {
            return Err(anyhow!("Already in space"));
        }

        if self.spaceship.pending_upgrade.is_some() {
            return Err(anyhow!("Upgrading spaceship"));
        }

        if self.current_game.is_some() {
            return Err(anyhow!("{} is playing", self.name));
        }

        if self.playing_in_tournament().is_some() {
            return Err(anyhow!("{} is playing in a tournament", self.name));
        }

        if self.spaceship.current_durability() == 0 {
            return Err(anyhow!("Spaceship needs reparations"));
        }

        if self.fuel() == 0 {
            return Err(anyhow!("Not enough fuel"));
        }

        if average_tiredness > MAX_AVG_TIREDNESS_PER_SPACE_ADVENTURE {
            return Err(anyhow!("Crew is too tired"));
        }

        Ok(())
    }

    pub fn can_explore_around_planet(
        &self,
        planet: &Planet,
        exploration_time: Tick,
    ) -> AppResult<()> {
        // Exploration does not cost tiredness, so we can pretend the crew is at full energy for the check.
        let averate_tiredness_for_exploration = 0.0;
        if let Err(err) = self.can_start_space_adventure(averate_tiredness_for_exploration) {
            return Err(anyhow!(err));
        }

        if self.is_on_planet() != Some(planet.id) {
            return Err(anyhow!("Not on this planet"));
        }

        // If we can't get there with full tank, than the planet is too far.
        let fuel_consumption = (exploration_time as f64
            * self.spaceship_fuel_consumption_per_tick() as f64)
            .ceil() as u32;

        // We check that we can go there with the current fuel.
        let current_fuel = self.fuel();
        if fuel_consumption > current_fuel {
            return Err(anyhow!("Not enough fuel"));
        }

        Ok(())
    }

    pub fn can_change_training_focus(&self) -> AppResult<()> {
        if self.current_game.is_some() {
            return Err(anyhow!("{} is playing", self.name));
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
            if current < amount.unsigned_abs() {
                return Err(anyhow!("Not enough resource"));
            }
        }
        Ok(())
    }

    pub fn can_upgrade_spaceship(
        &self,
        upgrade: &Upgrade<SpaceshipUpgradeTarget>,
    ) -> AppResult<()> {
        if self.is_on_planet().is_none() {
            return Err(anyhow!("Can only upgrade on a planet"));
        }

        for (resource, amount) in upgrade.upgrade_cost().iter() {
            if self.resources.value(resource) < *amount {
                return Err(anyhow!("Insufficient resources"));
            }
        }

        Ok(())
    }

    pub fn can_upgrade_asteroid(
        &self,
        asteroid: &Planet,
        upgrade: &Upgrade<AsteroidUpgradeTarget>,
    ) -> AppResult<()> {
        if asteroid.upgrades.contains(&upgrade.target) {
            return Err(anyhow!("Asteroid already has this upgrade"));
        }

        // Special rules for space cove: it has to be unique across all asteroids.
        if upgrade.target == AsteroidUpgradeTarget::SpaceCove && self.space_cove.is_some() {
            return Err(anyhow!("You already have a space cove"));
        }

        let mut missing_requirements = vec![];
        if let Some(required_upgrade) = upgrade.target.previous() {
            if !asteroid.upgrades.contains(&required_upgrade) {
                missing_requirements.push(required_upgrade);
            }
        }

        if !missing_requirements.is_empty() {
            return Err(anyhow!(
                "Missing requirement{}: {:#?}",
                if missing_requirements.len() > 1 {
                    "s"
                } else {
                    ""
                },
                missing_requirements
            ));
        }

        if let Some(pending_upgrade) = &asteroid.pending_upgrade {
            if pending_upgrade.target == upgrade.target {
                return Err(anyhow!("Already bulding this upgrade"));
            }
            return Err(anyhow!("Already building another upgrade"));
        }

        if self.is_on_planet() != Some(asteroid.id) {
            return Err(anyhow!("Can only build on the asteroid"));
        }

        for (resource, amount) in upgrade.target.upgrade_cost().iter() {
            if self.resources.value(resource) < *amount {
                return Err(anyhow!("Insufficient resources"));
            }
        }

        Ok(())
    }

    pub fn max_resource_buy_amount(&self, resource: Resource, unit_cost: u32) -> u32 {
        if unit_cost == 0 {
            return u32::MAX;
        }

        let max_satoshi_amount = self.balance() / unit_cost;
        let max_storage_amount = if resource == Resource::FUEL {
            self.spaceship.fuel_capacity().saturating_sub(self.fuel())
        } else if resource.to_storing_space() == 0 {
            u32::MAX
        } else {
            let free_storage_capacity = self
                .spaceship
                .storage_capacity()
                .saturating_sub(self.resources.used_storage_capacity());
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

    pub fn best_position_assignment(players: Vec<&Player>) -> Vec<PlayerId> {
        if players.len() < MAX_GAME_POSITION as usize {
            return players.iter().map(|&p| p.id).collect();
        }

        // Create an N-vector of 5-vectors. Each player is mapped to the vector (of length 5) of ratings for each role.
        let all_ratings = players
            .iter()
            .take(MAX_CREW_SIZE) // For performance reasons, we only consider the first MAX_CREW_SIZE players by rating.
            .map(|&p| {
                (0..MAX_GAME_POSITION)
                    .map(|position| p.in_game_rating_at_position(position))
                    .collect::<Vec<f32>>()
            })
            .collect::<Vec<Vec<f32>>>();

        let mut max_team_value = 0.0;
        let mut max_perm_index: usize = 0;

        // Iterate over all 5-permutations of the players. For each permutation assign a value equal to the sum of the ratings
        // when the player is assigned to the role corresponding to the index in the permutation.
        for perm in all_ratings.iter().permutations(5).enumerate() {
            let team_value = (0..MAX_GAME_POSITION as usize)
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
        assert!(new_players.len() == MAX_GAME_POSITION as usize);
        let mut bench = players
            .iter()
            .filter(|&p| !new_players.contains(&p.id))
            .copied()
            .collect::<Vec<&Player>>();
        bench.sort_by(|a, b| {
            b.tiredness_weighted_rating()
                .partial_cmp(&a.tiredness_weighted_rating())
                .expect("Skill value should exist")
        });
        new_players.append(&mut bench.iter().map(|&p| p.id).collect::<Vec<PlayerId>>());

        new_players
    }
}
