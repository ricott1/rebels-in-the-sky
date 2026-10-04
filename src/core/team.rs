use super::*;
use crate::{
    core::{constants::MAX_CREW_SIZE, utils::is_default},
    game_engine::{tactic::Tactic, types::*, Tournament, TournamentId, TournamentState},
    network::{
        challenge::Challenge,
        trade::{Trade, TradeRoute},
    },
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
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub peer_id: Option<PeerId>,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
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
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub substitution_tendency: SubstitutionTendency,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub game_position_fluidity: GamePositionFluidity,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub in_game_drinking: InGameDrinking,
    #[serde(skip)]
    pub sent_trades: HashMap<TradeId, Trade>,
    #[serde(skip)]
    pub received_trades: HashMap<TradeId, Trade>,
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
    pub dock_listings: Vec<DockListing>,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub tournaments_won: Vec<TournamentId>,
}

impl Team {
    pub fn random(rng: Option<&mut ChaCha8Rng>) -> Self {
        let rng = if let Some(r) = rng {
            r
        } else {
            &mut ChaCha8Rng::from_rng(&mut rand::rng())
        };
        let jersey = Jersey::random(rng);
        let ship_color = jersey.color;
        Self {
            id: TeamId::new_v4(),
            creation_time: Tick::now(),
            jersey,
            spaceship: Spaceship::random(rng).with_color_map(ship_color),
            game_tactic: Tactic::random(rng),
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

    pub fn with_reputation(mut self, reputation: Skill) -> Self {
        self.reputation = reputation;
        self
    }

    pub fn with_balance(mut self, amount: u32) -> AppResult<Self> {
        self.add_resource(Resource::SATOSHI, amount)?;

        Ok(self)
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
        self.sent_trades.insert(trade.id, trade);
    }

    pub fn add_received_trade(&mut self, trade: Trade) {
        self.received_trades.insert(trade.id, trade);
    }

    pub fn remove_trade(&mut self, trade_id: &TradeId) {
        self.sent_trades.remove(trade_id);
        self.received_trades.remove(trade_id);
    }

    pub fn trade(&self, trade_id: &TradeId) -> Option<&Trade> {
        self.sent_trades
            .get(trade_id)
            .or_else(|| self.received_trades.get(trade_id))
    }

    pub fn clear_trades(&mut self) {
        self.sent_trades.clear();
        self.received_trades.clear();
    }

    pub fn balance(&self) -> u32 {
        self.resources.value(&Resource::SATOSHI)
    }

    pub fn total_salary(&self, players: &PlayerMap) -> u32 {
        self.player_ids
            .iter()
            .map(|id| players.get(id).map(|p| p.salary()).unwrap_or_default())
            .sum()
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

    // Register the team to a game. The rum brought to the game is debited upfront and (if not drunk) returned at the end,
    // so that trading rum during the game cannot influence the game.
    pub fn enter_game(&mut self, game_id: GameId, brought_rum: u32) {
        self.current_game = Some(game_id);
        self.saturating_sub_resource(Resource::RUM, brought_rum);
    }

    pub fn get_crew_role(&self, role: CrewRole) -> Option<PlayerId> {
        match role {
            CrewRole::Captain => self.crew_roles.captain,
            CrewRole::Doctor => self.crew_roles.doctor,
            CrewRole::Pilot => self.crew_roles.pilot,
            CrewRole::Mozzo => None, // we should not get_crew_role for the Mozzo
            CrewRole::Engineer => self.crew_roles.engineer,
        }
    }

    pub fn is_on_planet(&self) -> Option<PlanetId> {
        match self.current_location {
            TeamLocation::OnPlanet { planet_id } => Some(planet_id),
            _ => None,
        }
    }

    pub fn is_at_dock(&self) -> bool {
        self.is_on_planet() == Some(*GALAXY_ROOT_ID)
    }

    pub fn playing_in_tournament(&self) -> Option<TournamentId> {
        match self.tournament_registration_state {
            TournamentRegistrationState::None
            | TournamentRegistrationState::Pending { .. }
            | TournamentRegistrationState::Registered { .. } => None,
            TournamentRegistrationState::Confirmed { tournament_id } => Some(tournament_id),
        }
    }

    pub fn committed_to_tournament(&self) -> Option<TournamentId> {
        match self.tournament_registration_state {
            TournamentRegistrationState::None => None,
            TournamentRegistrationState::Pending { tournament_id }
            | TournamentRegistrationState::Registered { tournament_id }
            | TournamentRegistrationState::Confirmed { tournament_id } => Some(tournament_id),
        }
    }

    pub fn average_tiredness(&self, world: &World) -> f32 {
        let tiredness_iter = self
            .active_player_ids()
            .into_iter()
            .take(MAX_PLAYERS_PER_GAME)
            .map(|id| {
                if let Ok(player) = world.players.get_or_err(&id) {
                    player.current_tiredness(world)
                } else {
                    0.0
                }
            });

        let n = tiredness_iter.len();
        if n == 0 {
            return 0.0;
        }
        (tiredness_iter.sum::<f32>() / n as f32).bound()
    }

    pub fn is_on_player_planet(&self, player: &Player) -> bool {
        // Player must be on same planet as team current_location
        self.is_on_planet() == player.is_on_planet()
    }

    pub fn has_space_cove_on(&self) -> Option<PlanetId> {
        self.space_cove.as_ref().map(|cove| cove.planet_id)
    }

    /// Empties whichever crew-role slot this pirate occupies.
    pub fn vacate_crew_role(&mut self, player_id: &PlayerId, role: CrewRole) {
        match role {
            CrewRole::Captain => self.crew_roles.captain = None,
            CrewRole::Doctor => self.crew_roles.doctor = None,
            CrewRole::Pilot => self.crew_roles.pilot = None,
            CrewRole::Engineer => self.crew_roles.engineer = None,
            CrewRole::Mozzo => self.crew_roles.mozzo.retain(|id| id != player_id),
        }
    }

    pub fn has_listings(&self) -> bool {
        !self.dock_listings.is_empty()
    }

    pub fn is_listed(&self, player_id: &PlayerId) -> bool {
        self.dock_listings
            .iter()
            .any(|listing| listing.player_id == *player_id)
    }

    pub fn listing(&self, player_id: &PlayerId) -> Option<&DockListing> {
        self.dock_listings
            .iter()
            .find(|listing| listing.player_id == *player_id)
    }

    pub fn remove_listing(&mut self, player_id: &PlayerId) -> Option<DockListing> {
        let index = self
            .dock_listings
            .iter()
            .position(|listing| listing.player_id == *player_id)?;
        Some(self.dock_listings.remove(index))
    }

    pub fn is_parked(&self, player_id: &PlayerId) -> bool {
        self.is_listed(player_id)
    }

    fn has_parked_pirates(&self) -> bool {
        self.has_listings()
    }

    /// Crew that sails, plays, holds roles and drinks rum. Keeps `player_ids`
    /// order, so `.take(MAX_PLAYERS_PER_GAME)` still yields starters then bench.
    pub fn active_player_ids(&self) -> Vec<PlayerId> {
        if !self.has_parked_pirates() {
            return self.player_ids.clone();
        }
        self.player_ids
            .iter()
            .copied()
            .filter(|id| !self.is_parked(id))
            .collect()
    }

    /// The counterpart to `active_player_ids`: together they cover `player_ids`.
    pub fn parked_player_ids(&self) -> Vec<PlayerId> {
        if !self.has_parked_pirates() {
            return vec![];
        }
        self.player_ids
            .iter()
            .copied()
            .filter(|id| self.is_parked(id))
            .collect()
    }

    pub fn listed_player_ids(&self) -> Vec<PlayerId> {
        if !self.has_listings() {
            return vec![];
        }
        self.player_ids
            .iter()
            .copied()
            .filter(|id| self.is_listed(id))
            .collect()
    }

    pub fn active_players_count(&self) -> usize {
        if !self.has_parked_pirates() {
            return self.player_ids.len();
        }
        self.player_ids
            .iter()
            .filter(|id| !self.is_parked(id))
            .count()
    }

    pub fn can_teleport_to(&self, to: &Planet) -> AppResult<()> {
        let is_dock = to.id == *GALAXY_ROOT_ID;
        let has_teleportation_pad = is_dock
            || self.home_planet_id == to.id
            || to.upgrades.contains(&PlanetUpgradeTarget::TeleportationPad);

        if !has_teleportation_pad {
            return Err(anyhow!("{} has no teleportation pad", to.name));
        }

        if !is_dock
            && self.home_planet_id != to.id
            && !self.asteroid_ids.contains(&to.id)
            && !to.allow_external_teleport
        {
            return Err(anyhow!("Cannot use teleportation pad"));
        }

        let rum_required = self.teleport_rum_cost(to.id);
        let has_rum = self.resources.value(&Resource::RUM) >= rum_required;

        if !has_rum {
            return Err(anyhow!("Not enough Rum! You need at least {rum_required}"));
        }

        Ok(())
    }

    pub fn can_add_player(
        &self,
        player: &Player,
        is_in_space_cove_on: Option<PlanetId>,
    ) -> AppResult<()> {
        if player.team.is_some() {
            return Err(anyhow!("Already in a team"));
        }

        if self.current_game.is_some() {
            return Err(anyhow!("Can't hire during a game"));
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

        if !self.can_hire_from_space_cove(is_in_space_cove_on) {
            return Err(anyhow!("Not in team space cove"));
        }

        Ok(())
    }

    /// A free pirate standing in a space cove can only be hired by that cove's team.
    pub fn can_hire_from_space_cove(&self, player_space_cove: Option<PlanetId>) -> bool {
        player_space_cove.is_none()
            || self.space_cove.as_ref().map(|cove| cove.planet_id) == player_space_cove
    }

    // This function is necessary for local teams to consider hiring a player (even if the crew is full).
    pub fn can_consider_hiring_player(&self, player: &Player) -> AppResult<()> {
        let hiring_cost = player.hire_cost();
        if self.balance() < hiring_cost {
            return Err(anyhow!("Not enough money {hiring_cost}"));
        }

        // Check player age is not above limit
        if player.info.relative_age() >= 1.0 {
            return Err(anyhow!("Player is too old"));
        }

        Ok(())
    }

    pub fn can_hire_player(
        &self,
        player: &Player,
        is_in_space_cove_on: Option<PlanetId>,
    ) -> AppResult<()> {
        self.can_add_player(player, is_in_space_cove_on)?;
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

        if self.is_listed(&player.id) {
            return Ok(());
        }

        self.crew_is_ashore_and_idle()
    }

    /// The crew is docked somewhere and not committed to a game or a tournament -
    /// the precondition for any change to who is aboard.
    pub fn crew_is_ashore_and_idle(&self) -> AppResult<()> {
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

    pub fn can_leave_player_at_dock(&self, player: &Player) -> AppResult<()> {
        if !self.is_at_dock() {
            return Err(anyhow!("{} is not at the dock", self.name));
        }

        if !self.player_ids.contains(&player.id) || player.team != Some(self.id) {
            return Err(anyhow!("Player is not in team"));
        }

        if self.is_listed(&player.id) {
            return Err(anyhow!(
                "{} is already at the dock",
                player.info.short_name()
            ));
        }

        self.crew_is_ashore_and_idle()?;

        // Deliberately 1, not MIN_PLAYERS_PER_GAME: a crew may list itself down to
        // where it cannot field a game, and simply cannot play. An empty active
        // roster is the real problem - it soft-locks travel and space adventures.
        if self.active_players_count() <= 1 {
            return Err(anyhow!("Someone has to sail the ship"));
        }

        Ok(())
    }

    pub fn can_recall_player_from_dock(&self, player_id: &PlayerId) -> AppResult<()> {
        if !self.is_listed(player_id) {
            return Err(anyhow!("Pirate is not at the dock"));
        }

        if !self.is_at_dock() {
            return Err(anyhow!("{} is not at the dock", self.name));
        }

        self.crew_is_ashore_and_idle()
    }

    /// Orders `player_ids` as `[active in best-position order.., parked..]`.
    /// The tail is what keeps a parked pirate out of the game roster,
    /// since `TeamInGame::from_team_id` just takes the first MAX_PLAYERS_PER_GAME.
    ///
    /// Ids whose player is missing from `players` are parked in the tail rather
    /// than dropped: silently shrinking `player_ids` here would lose the pirate.
    pub fn reassign_positions(&mut self, players: &PlayerMap) {
        let mut active: Vec<&Player> = vec![];
        let mut tail: Vec<PlayerId> = vec![];
        for &id in self.player_ids.iter() {
            match players.get(&id) {
                Some(player) if !self.is_parked(&id) => active.push(player),
                _ => tail.push(id),
            }
        }

        let mut ids = Self::best_position_assignment(active, self.game_position_fluidity);
        ids.extend(tail);
        self.player_ids = ids;
    }

    pub fn can_set_crew_role(&self, player: &Player) -> AppResult<()> {
        if player.team.is_none() {
            return Err(anyhow!("Player is not in a team"));
        }

        if self.is_parked(&player.id) {
            return Err(anyhow!("{} is at the dock", player.info.short_name()));
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
                "{} and {} not on the same planet",
                self.name,
                team.name,
            ));
        }

        if self.active_players_count() < MIN_PLAYERS_PER_GAME {
            return Err(anyhow!("{} does not have enough pirates", self.name));
        }

        if team.active_players_count() < MIN_PLAYERS_PER_GAME {
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
                return Err(anyhow!("Cannot organize a tournament without a space cove"));
            }
            Some(cove) => {
                if cove.is_ready() {
                    if !matches!(self.is_on_planet(), Some(id) if id == cove.planet_id) {
                        return Err(anyhow!(
                            "Cannot organize a tournament while not at your space cove"
                        ));
                    }

                    if !cove.has_stadium() {
                        return Err(anyhow!("Cannot organize a tournament whitout a stadium"));
                    }
                } else {
                    return Err(anyhow!(
                        "Cannot organize a tournament if space cove is not ready"
                    ));
                }
            }
        }

        if self.resources.value(&Resource::GOLD) < TOURNAMENT_ORGANIZATION_GOLD_COST {
            return Err(anyhow!(
                "To organize a tournament you need {TOURNAMENT_ORGANIZATION_GOLD_COST} gold"
            ));
        }

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

        // Checked last so it does not pre-empt the errors above: without it a crew
        // that has listed itself short could register and then fail to field five.
        if self.active_players_count() < MIN_PLAYERS_PER_GAME {
            return Err(anyhow!("Team does not have enough pirates."));
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
        if self.sent_challenges.contains_key(&team.id) {
            return Err(anyhow!("Already challenged {}", team.name));
        }

        self.can_still_challenge_network_team(team)
    }

    // Same as can_challenge_network_team but without the already-challenged guard, so a challenge
    // being resent is not dropped just because it is still in sent_challenges.
    pub fn can_still_challenge_network_team(&self, team: &Team) -> AppResult<()> {
        if team.peer_id.is_none() {
            return Err(anyhow!("{} is not from network", team.name));
        }

        if self.current_game.is_some() {
            return Err(anyhow!("{} is already playing", self.name));
        }

        if team.current_game.is_some() {
            return Err(anyhow!("{} is already playing", team.name));
        }

        self.can_play_game_with_team(team, None)
    }

    /// Always evaluated from the proposer's point of view.
    pub fn can_trade_with_team(
        &self,
        target_team: &Team,
        route: TradeRoute,
        proposer_player: Option<&Player>,
        target_player: &Player,
        proposer_satoshis: u32,
        target_satoshis: u32,
    ) -> AppResult<()> {
        if self.id == target_team.id {
            return Err(anyhow!("Cannot trade with oneself"));
        }

        if target_player.team.is_none() || target_player.team.unwrap() != target_team.id {
            return Err(anyhow!("Target player is not part of the team"));
        }

        if self.balance() < proposer_satoshis {
            return Err(anyhow!("Not enough satoshi"));
        }

        if target_team.balance() < target_satoshis {
            return Err(anyhow!("{} cannot afford that", target_team.name));
        }

        match route {
            TradeRoute::CrewSwap => {
                let proposer_player = proposer_player
                    .ok_or_else(|| anyhow!("A crew swap needs a pirate on both sides"))?;

                if proposer_player.team.is_none() || proposer_player.team.unwrap() != self.id {
                    return Err(anyhow!("Proposed player is not part of the team"));
                }

                if self.is_on_planet() != target_team.is_on_planet() {
                    return Err(anyhow!("Not on the same planet"));
                }

                if self.is_listed(&proposer_player.id) {
                    return Err(anyhow!("Recall your pirate from the dock first"));
                }
                if target_team.is_listed(&target_player.id) {
                    return Err(anyhow!(
                        "{} is at the dock",
                        target_player.info.short_name()
                    ));
                }

                self.can_release_player(proposer_player)?;
                target_team.can_release_player(target_player)?;
            }
        }

        Ok(())
    }

    pub fn can_travel_to_planet(&self, planet: &Planet, duration: Tick) -> AppResult<()> {
        planet.can_be_travelled_to()?;

        if self.active_players_count() == 0 {
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
        if self.active_players_count() == 0 {
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

    pub fn can_change_team_settings(&self) -> AppResult<()> {
        if self.current_game.is_some() {
            return Err(anyhow!("{} is playing", self.name));
        }

        if self.playing_in_tournament().is_some() {
            return Err(anyhow!("{} is in a tournament", self.name));
        }

        Ok(())
    }

    pub fn can_sell_resource(&self, resource: Resource, amount: u32) -> AppResult<()> {
        // Selling. Check if enough resource
        let current = self.resources.value(&resource);
        if current < amount {
            return Err(anyhow!("Not enough resource"));
        }
        Ok(())
    }

    pub fn can_buy_resource(
        &self,
        resource: Resource,
        amount: u32,
        unit_cost: u32,
    ) -> AppResult<()> {
        // Buying. Check if enough satoshi and if enough storing space

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
        upgrade: &Upgrade<PlanetUpgradeTarget>,
    ) -> AppResult<()> {
        if asteroid.upgrades.contains(&upgrade.target) {
            return Err(anyhow!("Asteroid already has this upgrade"));
        }

        // Special rules for space cove: it has to be unique across all asteroids.
        if upgrade.target == PlanetUpgradeTarget::SpaceCove && self.space_cove.is_some() {
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

    pub fn can_upgrade_space_cove(&self, target: SpaceCoveUpgradeTarget) -> AppResult<()> {
        let cove = self
            .space_cove
            .as_ref()
            .ok_or(anyhow!("You don't have a space cove"))?;

        if !cove.is_ready() {
            return Err(anyhow!("The space cove is still under construction"));
        }

        if cove.upgrades.contains(&target) {
            return Err(anyhow!("{target} is already built"));
        }

        if cove.pending_upgrade.is_some() {
            return Err(anyhow!("Already building something in the cove"));
        }

        if self.is_on_planet() != Some(cove.planet_id) {
            return Err(anyhow!("Can only build at the space cove"));
        }

        for (resource, amount) in target.upgrade_cost().iter() {
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

    pub fn best_position_assignment(
        players: Vec<&Player>,
        game_position_fluidity: GamePositionFluidity,
    ) -> Vec<PlayerId> {
        if players.len() < NUM_GAME_POSITIONS as usize {
            return players.iter().map(|&p| p.id).collect();
        }

        // Create an N-vector of 5-vectors. Each player is mapped to the vector (of length 5) of ratings for each role.
        let all_ratings = players
            .iter()
            .take(MAX_CREW_SIZE) // For performance reasons, we only consider the first MAX_CREW_SIZE players by rating.
            .map(|&p| {
                (0..NUM_GAME_POSITIONS)
                    .map(|position| p.in_game_rating_at_position(position, game_position_fluidity))
                    .collect::<Vec<f32>>()
            })
            .collect::<Vec<Vec<f32>>>();

        let mut max_team_value = 0.0;
        let mut max_perm_index: usize = 0;

        // Iterate over all 5-permutations of the players. For each permutation assign a value equal to the sum of the ratings
        // when the player is assigned to the role corresponding to the index in the permutation.
        for perm in all_ratings.iter().permutations(5).enumerate() {
            let team_value = (0..NUM_GAME_POSITIONS as usize)
                .map(|i| perm.1[i][i])
                .sum::<f32>();
            if team_value > max_team_value {
                max_team_value = team_value;
                max_perm_index = perm.0;
            }
        }

        let idx_perms = (0..min(players.len(), MAX_CREW_SIZE))
            .permutations(NUM_GAME_POSITIONS as usize)
            .collect::<Vec<Vec<usize>>>();
        let max_perm = &idx_perms[max_perm_index];
        let mut new_players: Vec<PlayerId> = max_perm.iter().map(|&i| players[i].id).collect();
        assert!(new_players.len() == NUM_GAME_POSITIONS as usize);
        let mut bench = players
            .iter()
            .filter(|&p| !new_players.contains(&p.id))
            .copied()
            .collect::<Vec<&Player>>();
        bench.sort_by(|a, b| {
            (b.average_skill() * (1.0 - b.tiredness / MAX_SKILL))
                .partial_cmp(&(a.average_skill() * (1.0 - a.tiredness / MAX_SKILL)))
                .expect("Skill value should exist")
        });
        new_players.append(&mut bench.iter().map(|&p| p.id).collect::<Vec<PlayerId>>());

        new_players
    }

    pub fn teleport_rum_cost(&self, planet_id: PlanetId) -> u32 {
        if self.home_planet_id == planet_id {
            0
        } else {
            self.active_players_count() as u32
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A crew of `n` pirates with a ready cove that has a market, so listings can
    /// be pushed directly - list/recall does not exist yet at this layer.
    fn team_with_cove(n: usize) -> (Team, PlayerMap) {
        let planet_id = PlanetId::new_v4();
        let mut cove = SpaceCove::under_construction(planet_id);
        cove.finish_contruction();
        cove.upgrades.insert(SpaceCoveUpgradeTarget::Market);

        let mut team = Team::random(None);
        team.space_cove = Some(cove);
        team.current_location = TeamLocation::OnPlanet {
            planet_id: *GALAXY_ROOT_ID,
        };

        let mut players = PlayerMap::new();
        for _ in 0..n {
            let mut player = Player::default().randomize(None);
            player.team = Some(team.id);
            team.player_ids.push(player.id);
            players.insert(player.id, player);
        }
        (team, players)
    }

    fn list(team: &mut Team, player_id: PlayerId) {
        team.dock_listings
            .push(DockListing::new(player_id, Tick::now()));
    }

    fn dock_planet() -> Planet {
        PLANET_DATA
            .iter()
            .find(|planet| planet.id == *GALAXY_ROOT_ID)
            .expect("the black hole")
            .clone()
    }

    #[test]
    fn test_every_crew_can_teleport_to_the_dock_for_rum() {
        let (mut team, _) = team_with_cove(5);
        team.current_location = TeamLocation::OnPlanet {
            planet_id: PlanetId::new_v4(),
        };
        let mut dock = dock_planet();
        dock.upgrades.clear();
        dock.allow_external_teleport = false;

        assert!(team
            .can_teleport_to(&dock)
            .unwrap_err()
            .to_string()
            .contains("Not enough Rum"));

        team.add_resource(Resource::RUM, 5).unwrap();
        assert!(team.can_teleport_to(&dock).is_ok());
    }

    #[test]
    fn test_leaving_and_recalling_need_the_dock() {
        let (mut team, players) = team_with_cove(5);
        let player = players.get(&team.player_ids[0]).expect("player").clone();
        team.current_location = TeamLocation::OnPlanet {
            planet_id: *GALAXY_ROOT_ID,
        };
        assert!(team.can_leave_player_at_dock(&player).is_ok());
        list(&mut team, player.id);
        assert!(team.can_recall_player_from_dock(&player.id).is_ok());

        team.current_location = TeamLocation::OnPlanet {
            planet_id: PlanetId::new_v4(),
        };
        let other = players.get(&team.player_ids[1]).expect("player").clone();
        assert!(team
            .can_leave_player_at_dock(&other)
            .unwrap_err()
            .to_string()
            .contains("not at the dock"));
        assert!(team
            .can_recall_player_from_dock(&player.id)
            .unwrap_err()
            .to_string()
            .contains("not at the dock"));
    }

    #[test]
    fn test_listed_pirate_leaves_the_active_roster() {
        let (mut team, _) = team_with_cove(7);
        let listed = team.player_ids[0];
        assert_eq!(team.active_players_count(), 7);
        assert!(!team.is_listed(&listed));

        list(&mut team, listed);

        assert!(team.is_listed(&listed));
        assert_eq!(team.active_players_count(), 6);
        assert_eq!(team.active_player_ids().len(), 6);
        assert!(!team.active_player_ids().contains(&listed));
        assert_eq!(team.listed_player_ids(), vec![listed]);
        // Still on the crew: they keep their seat and keep drawing pay.
        assert!(team.player_ids.contains(&listed));
        assert_eq!(team.player_ids.len(), 7);
    }

    #[test]
    fn test_reassign_positions_parks_listed_pirates_at_the_tail() {
        let (mut team, players) = team_with_cove(7);
        let listed = team.player_ids[0];
        list(&mut team, listed);

        team.reassign_positions(&players);

        assert_eq!(team.player_ids.len(), 7);
        assert_eq!(
            team.player_ids.last(),
            Some(&listed),
            "a pirate at the dock must sort behind every active pirate"
        );
        // The game roster cut can therefore never reach them.
        assert!(!team.player_ids[..MAX_PLAYERS_PER_GAME.min(6)].contains(&listed));
    }

    #[test]
    fn test_reassign_positions_keeps_ids_whose_player_is_missing() {
        let (mut team, mut players) = team_with_cove(6);
        let ghost = team.player_ids[2];
        players.remove(&ghost);

        team.reassign_positions(&players);

        assert_eq!(team.player_ids.len(), 6, "no id may be silently dropped");
        assert!(team.player_ids.contains(&ghost));
    }

    #[test]
    fn test_listing_below_five_blocks_games_but_not_the_roster() {
        let (mut team, _) = team_with_cove(6);
        let mut other = Team::random(None);
        other.current_location = team.current_location;
        for _ in 0..MIN_PLAYERS_PER_GAME {
            other.player_ids.push(PlayerId::new_v4());
        }
        assert!(team.can_play_game_with_team(&other, None).is_ok());

        let (first, second) = (team.player_ids[0], team.player_ids[1]);
        list(&mut team, first);
        list(&mut team, second);

        assert_eq!(team.active_players_count(), 4);
        assert!(team
            .can_play_game_with_team(&other, None)
            .unwrap_err()
            .to_string()
            .contains("does not have enough pirates"));
    }

    #[test]
    fn test_listed_pirate_does_not_cost_teleport_rum() {
        let (mut team, _) = team_with_cove(5);
        let elsewhere = PlanetId::new_v4();
        assert_eq!(team.teleport_rum_cost(elsewhere), 5);

        let first = team.player_ids[0];
        list(&mut team, first);

        assert_eq!(team.teleport_rum_cost(elsewhere), 4);
    }

    #[test]
    fn test_can_leave_player_requires_the_dock_and_being_ashore() {
        let (mut team, players) = team_with_cove(5);
        let player = players.get(&team.player_ids[0]).expect("player").clone();
        assert!(team.can_leave_player_at_dock(&player).is_ok());

        team.current_location = TeamLocation::Travelling {
            from: PlanetId::new_v4(),
            to: PlanetId::new_v4(),
            started: 0,
            duration: DAYS,
            distance: 1,
        };
        assert!(team
            .can_leave_player_at_dock(&player)
            .unwrap_err()
            .to_string()
            .contains("not at the dock"));
    }

    // Being on a planet does not imply being idle: games are played on a planet,
    // and a crew can organise a tournament at its own cove.
    #[test]
    fn test_can_list_player_is_blocked_by_a_game_or_a_tournament() {
        let (team, players) = team_with_cove(5);
        let player = players.get(&team.player_ids[0]).expect("player").clone();
        assert!(team.can_leave_player_at_dock(&player).is_ok());

        let mut playing = team.clone();
        playing.current_game = Some(GameId::new_v4());
        assert!(playing
            .can_leave_player_at_dock(&player)
            .unwrap_err()
            .to_string()
            .contains("is playing"));

        let mut in_tournament = team.clone();
        in_tournament.tournament_registration_state = TournamentRegistrationState::Confirmed {
            tournament_id: TournamentId::new_v4(),
        };
        assert!(in_tournament
            .can_leave_player_at_dock(&player)
            .unwrap_err()
            .to_string()
            .contains("in a tournament"));
    }

    #[test]
    fn test_cannot_list_the_last_active_pirate() {
        let (mut team, players) = team_with_cove(2);
        let (first, second) = (team.player_ids[0], team.player_ids[1]);
        let second_player = players.get(&second).expect("player").clone();

        list(&mut team, first);

        assert_eq!(team.active_players_count(), 1);
        assert!(team
            .can_leave_player_at_dock(&second_player)
            .unwrap_err()
            .to_string()
            .contains("sail the ship"));
    }

    #[test]
    fn test_removing_a_listing_unparks_the_pirate() {
        let (mut team, _) = team_with_cove(3);
        let listed = team.player_ids[0];
        list(&mut team, listed);
        assert!(team.is_parked(&listed));

        team.remove_listing(&listed);

        assert!(!team.is_parked(&listed));
        assert!(team.dock_listings.is_empty());
    }

    #[test]
    fn test_cannot_list_the_same_pirate_twice() {
        let (mut team, players) = team_with_cove(5);
        let first = team.player_ids[0];
        let player = players.get(&first).expect("player").clone();

        list(&mut team, first);

        assert!(team
            .can_leave_player_at_dock(&player)
            .unwrap_err()
            .to_string()
            .contains("already at the dock"));
    }

    #[test]
    fn test_cannot_recall_a_pirate_who_is_not_listed() {
        let (team, _) = team_with_cove(5);
        assert!(team
            .can_recall_player_from_dock(&team.player_ids[0])
            .unwrap_err()
            .to_string()
            .contains("not at the dock"));
    }

    #[test]
    fn test_listed_pirate_cannot_take_a_crew_role() {
        let (mut team, players) = team_with_cove(5);
        let listed = team.player_ids[0];
        let player = players.get(&listed).expect("player").clone();
        assert!(team.can_set_crew_role(&player).is_ok());

        list(&mut team, listed);

        assert!(team
            .can_set_crew_role(&player)
            .unwrap_err()
            .to_string()
            .contains("at the dock"));
    }

    // Builds a player with uniform skills - so `position_skill_rating` is identical for
    // every position - and the given per-position fitness. `fitness` entries are in
    // [0, 1]; `game_position_fitness` is stored on the 0..MAX_SKILL scale.
    fn make_player(skill: f32, fitness: [f32; NUM_GAME_POSITIONS as usize]) -> Player {
        let mut p = Player::default();
        p.athletics = Athletics {
            quickness: skill,
            vertical: skill,
            strength: skill,
            stamina: skill,
        };
        p.offense = Offense {
            brawl: skill,
            close_range: skill,
            medium_range: skill,
            long_range: skill,
        };
        p.defense = Defense {
            steal: skill,
            block: skill,
            perimeter_defense: skill,
            interior_defense: skill,
        };
        p.technical = Technical {
            passing: skill,
            ball_handling: skill,
            post_moves: skill,
            rebounds: skill,
        };
        p.mental = Mental {
            vision: skill,
            aggression: skill,
            intuition: skill,
            charisma: skill,
        };
        p.game_position_fitness = fitness.map(|f| f * MAX_SKILL);
        p
    }

    // The team's game position fluidity reshapes how strongly position fitness is
    // weighted (`fitness.powf(exponent)`: Low=1.5 widens the gap between good and bad
    // fits, High=0.6 narrows it), which can flip the optimal starting-five assignment.
    //
    // P2/P3/P4 are locked to positions 2/3/4 (they have nonzero fitness only there).
    // P0 and P1 compete for positions 0 and 1:
    //   P0 fitness: pos0=1.0, pos1=0.6     P1 fitness: pos0=0.7, pos1=0.2
    // With uniform skills every `position_skill_rating` is equal, so each player's
    // in-game rating is `roll + 2 * skill * fitness^exponent` and the comparison
    // reduces to the sum of `fitness^exponent`. The two candidate sub-assignments:
    //   "extreme"  P0->0, P1->1:  1.0^e + 0.2^e
    //   "balanced" P0->1, P1->0:  0.6^e + 0.7^e
    // Low fluidity (e=1.5, convex) prefers the extreme assignment; High fluidity
    // (e=0.6, concave) prefers the balanced one.
    #[test]
    fn test_fluidity_changes_starting_five() {
        let skill = 10.0;
        let p0 = make_player(skill, [1.0, 0.6, 0.0, 0.0, 0.0]);
        let p1 = make_player(skill, [0.7, 0.2, 0.0, 0.0, 0.0]);
        let p2 = make_player(skill, [0.0, 0.0, 1.0, 0.0, 0.0]);
        let p3 = make_player(skill, [0.0, 0.0, 0.0, 1.0, 0.0]);
        let p4 = make_player(skill, [0.0, 0.0, 0.0, 0.0, 1.0]);
        let players: Vec<&Player> = vec![&p0, &p1, &p2, &p3, &p4];

        let low = Team::best_position_assignment(players.clone(), GamePositionFluidity::Low);
        let high = Team::best_position_assignment(players.clone(), GamePositionFluidity::High);

        // Low fluidity -> "extreme" sub-assignment: P0 at pos0, P1 at pos1.
        assert_eq!(low, vec![p0.id, p1.id, p2.id, p3.id, p4.id]);
        // High fluidity -> "balanced" sub-assignment: P1 at pos0, P0 at pos1.
        assert_eq!(high, vec![p1.id, p0.id, p2.id, p3.id, p4.id]);

        // The headline: changing fluidity actually changed the starting five.
        assert_ne!(low, high);
    }
}
