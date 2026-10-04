use super::constants::*;
use super::jersey::{Jersey, JerseyStyle};
use super::planet::{Planet, PlanetType};
use super::player::{Player, Trait};
use super::position::{GamePosition, NUM_GAME_POSITIONS};
use super::resources::Resource;
use super::role::CrewRole;
use super::skill::{GameSkill, MAX_SKILL};
use super::spaceship::Spaceship;
use super::team::Team;
use super::types::{PlayerLocation, TeamBonus, TeamLocation};
use super::utils::{is_default, PLANET_DATA};
use crate::core::{
    AutonomousStrategy, DockListing, GameResult, Honour, Offer, OfferKind, PlanetUpgradeTarget,
    PlayerOpinion, PlayerOpinionMapDescription, Population, Rated, RatedPlayers, ScoutReport,
    Skill, SpaceCove, SpaceCoveUpgradeTarget, Tavern, TournamentRegistrationState, Upgrade,
    MIN_SKILL,
};
use crate::game_engine::game::{Game, GameSummary};
use crate::game_engine::tactic::Tactic;
use crate::game_engine::types::{Possession, TeamInGame};
use crate::game_engine::{
    TournamentId, TournamentState, TournamentSummary, RECOVERING_TIREDNESS_PER_SHORT_TICK,
};
use crate::image::color_map::ColorMap;
use crate::network::network_store_data::NetworkStoreData;
use crate::network::trade::Trade;
use crate::network::types::{NetworkGame, NetworkRequestState, NetworkTeam};
use crate::space_adventure::ControllableSpaceship;
use crate::space_adventure::SpaceAdventure;
use crate::store::{save_game, save_tournament, ASSETS_DIR};
use crate::ui::{PopupMessage, UiCallback};
use crate::{app_version, types::*};
use anyhow::anyhow;
use itertools::Itertools;
use libp2p::PeerId;
use rand::seq::{IteratorRandom, SliceRandom};
use rand::{RngExt, SeedableRng};
use rand_chacha::ChaCha8Rng;
use rand_distr::weighted::WeightedIndex;
use rand_distr::Distribution;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use strum::IntoEnumIterator;

// const GAME_CLEANUP_TIME: Tick = 10 * SECONDS;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct World {
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub app_version: [usize; 3],
    #[serde(skip)]
    pub dirty: bool, // Whether anything relevant for the world state has changed and thus should be stored.
    #[serde(skip)]
    pub dirty_network: bool, // Whether anything relevant for the entwork has changed and thus should be sent over.
    #[serde(skip)]
    pub dirty_ui: bool, // Whether anything relevant for UI has changed and thus should be drawn.
    pub serialized_size: u64,
    pub seed: u64,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub last_tick_min_interval: Tick,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub last_tick_short_interval: Tick,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub last_tick_medium_interval: Tick,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub last_tick_long_interval: Tick,
    pub own_team_id: TeamId,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub teams: TeamMap,
    #[serde(skip)]
    pub network_team_timestamps: HashMap<TeamId, Tick>,
    #[serde(skip)]
    pub network_team_last_heard: HashMap<TeamId, Tick>,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub applied_trades: HashSet<TradeId>,
    #[serde(skip)]
    pub trade_outbox: Vec<Trade>,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub players: PlayerMap,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub players_scouting: HashMap<PlayerId, ScoutReport>,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub planets: PlanetMap,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub games: GameMap, // Holds currently running games.
    #[serde(skip)]
    pub recently_finished_games: GameMap, // Holds finished games for the session, but are not persisted.
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub past_games: GameSummaryMap, // Holds summary of finished games, persisted.
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub kartoffeln: KartoffelMap,
    #[serde(skip)]
    pub space_adventure: Option<SpaceAdventure>,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub tournaments: TournamentMap,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub past_tournaments: TournamentSummaryMap, // Holds summary of finished tournaments, persisted.
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub canceled_tournaments: HashSet<TournamentId>, // Cancelled here, so a peer's rebroadcast is ignored.
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub network_store_data: NetworkStoreData,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfferOutcome {
    Sent,
    Accepted,
    Refused,
}

impl World {
    pub fn new(seed: Option<u64>) -> Self {
        let mut planets = HashMap::new();
        for planet in PLANET_DATA.iter() {
            planets.insert(planet.id, planet.clone());
        }

        Self {
            app_version: app_version(),
            seed: seed.unwrap_or(rand::random()),
            planets,
            ..Default::default()
        }
    }

    pub fn initialize(&mut self, generate_local_world: bool) -> AppResult<()> {
        let rng = &mut ChaCha8Rng::seed_from_u64(self.seed);
        for planet in PLANET_DATA.iter() {
            self.populate_planet(rng, planet, None, Some(MAX_SKILL))?;
        }

        if generate_local_world {
            self.generate_local_world(rng)?;
        }

        let now = Tick::now();

        self.last_tick_min_interval = now;
        self.last_tick_short_interval = now;
        // We round up to the beginning of the next TickInterval::SHORT to ensure
        // that online games don't drift.
        self.last_tick_short_interval -= self.last_tick_short_interval % TickInterval::SHORT;
        self.last_tick_medium_interval = now;
        self.last_tick_long_interval = now;
        Ok(())
    }

    pub fn has_own_team(&self) -> bool {
        self.own_team_id != TeamId::default()
    }

    fn populate_planet(
        &mut self,
        rng: &mut ChaCha8Rng,
        planet: &Planet,
        extra_potential: Option<Skill>,
        extra_scouting: Option<Skill>,
    ) -> AppResult<()> {
        let number_free_pirates = planet.total_population();
        let mut position = rng.random_range(0..NUM_GAME_POSITIONS) as GamePosition;
        for _ in 0..number_free_pirates {
            let pirate_base_level = Some(rng.random_range(-2.0..2.0));
            let population = planet.random_population(rng).unwrap_or_default();

            self.generate_random_pirate(
                rng,
                population,
                planet.id,
                Some(position),
                pirate_base_level,
                extra_potential,
                extra_scouting,
            )?;
            position = (position + 1) % NUM_GAME_POSITIONS;
        }

        Ok(())
    }

    pub fn generate_local_world(&mut self, rng: &mut ChaCha8Rng) -> AppResult<()> {
        let team_data = {
            let file = ASSETS_DIR
                .get_file("data/teams_data.json")
                .expect("Could not find teams_data.json");
            let data = file
                .contents_utf8()
                .expect("Could not read teams_data.json");
            let mut data: Vec<(String, String, PlanetId)> = serde_json::from_str(data)
                .unwrap_or_else(|e| panic!("Could not parse teams_data.json: {e}"));
            data.shuffle(rng);
            data
        };

        for (team_name, ship_name, home_planet_id) in team_data {
            self.generate_random_team(rng, home_planet_id, team_name, ship_name)?;
        }
        Ok(())
    }

    pub fn generate_random_team(
        &mut self,
        rng: &mut ChaCha8Rng,
        home_planet_id: PlanetId,
        team_name: String,
        ship_name: String,
    ) -> AppResult<TeamId> {
        let team = Team::random(Some(rng))
            .with_name(team_name)
            .with_spaceship_name(ship_name)
            .with_home_planet(home_planet_id)
            .with_balance(INITIAL_RANDOM_TEAM_BALANCE)?;
        let team_id = team.id;

        let mut planet = self.planets.get_or_err(&team.home_planet_id)?.clone();
        planet.team_ids.push(team_id);
        let current_tick = team.creation_time;

        self.teams.insert(team.id, team);

        let team_base_level = rng.random_range(2.0..=14.0);
        for position in 0..NUM_GAME_POSITIONS {
            let population = planet.random_population(rng).unwrap_or_default();
            let player_id = self.generate_random_pirate(
                rng,
                population,
                planet.id,
                Some(position),
                Some(team_base_level),
                None,
                None,
            )?;
            self.add_player_to_team(&player_id, &team_id, current_tick)?;
        }

        loop {
            let team = self.teams.get_or_err(&team_id)?;
            if team.player_ids.len() == team.spaceship.crew_capacity() as usize - 1 {
                break;
            }

            let population = planet.random_population(rng).unwrap_or_default();
            let player_id = self.generate_random_pirate(
                rng,
                population,
                planet.id,
                None,
                Some(team_base_level),
                None,
                None,
            )?;
            self.add_player_to_team(&player_id, &team_id, current_tick)?;
        }
        self.planets.insert(planet.id, planet);

        let player_ids = self.teams.get_or_err(&team_id)?.active_player_ids();
        self.auto_assign_crew_roles(player_ids)?;

        // Set reputation so an average game roughly covers a day of salaries.
        let total_salary = self.teams.get_or_err(&team_id)?.total_salary(&self.players) as f32;
        let population = self
            .planets
            .get_or_err(&home_planet_id)?
            .total_population()
            .max(1) as f32;
        let combined_reputation =
            (((total_salary - BASE_INCOME as f32) / INCOME_PER_ATTENDEE as f32 - BASE_ATTENDANCE)
                / population)
                .max(0.0)
                .powf(2.0 / 3.0)
                * 3.5; // less than one game per day
        self.teams.get_mut_or_err(&team_id)?.reputation =
            (3.75 + combined_reputation / 2.0).bound();

        self.dirty = true;
        self.dirty_ui = true;
        Ok(team_id)
    }

    pub fn generate_own_team(
        &mut self,
        name: String,
        home_planet_id: PlanetId,
        jersey_style: JerseyStyle,
        jersey_colors: ColorMap,
        players: Vec<PlayerId>,
        spaceship: Spaceship,
    ) -> AppResult<TeamId> {
        let team_id = TeamId::new_v4();
        self.own_team_id = team_id;

        let current_location = TeamLocation::OnPlanet {
            planet_id: home_planet_id,
        };

        let mut resources = HashMap::default();
        resources.insert(Resource::FUEL, spaceship.fuel_capacity());

        let team = Team {
            id: team_id,
            name,
            creation_time: Tick::now(),
            jersey: Jersey {
                style: jersey_style,
                color: jersey_colors,
            },
            home_planet_id,
            current_location,
            spaceship,
            resources,
            autonomous_strategy: AutonomousStrategy::new_for_own_team(),
            ..Default::default()
        };

        let current_tick = team.creation_time;
        self.teams.insert(team.id, team);

        for player_id in players {
            self.add_player_to_team(&player_id, &team_id, current_tick)?;
        }

        let player_ids = self.teams.get_or_err(&team_id)?.active_player_ids();
        self.auto_assign_crew_roles(player_ids)?;

        self.planets
            .get_mut_or_err(&home_planet_id)?
            .team_ids
            .push(team_id);

        self.dirty = true;
        self.dirty_network = true;
        self.dirty_ui = true;
        Ok(team_id)
    }

    pub fn generate_team_asteroid(
        &mut self,
        name: String,
        filename: String,
        satellite_of: PlanetId,
    ) -> AppResult<PlanetId> {
        let asteroid = Planet::asteroid(name, filename, satellite_of);
        let asteroid_id = asteroid.id;
        self.planets.insert(asteroid_id, asteroid);

        let satellite_of_planet = self.planets.get_mut_or_err(&satellite_of)?;
        satellite_of_planet.satellites.push(asteroid_id);
        satellite_of_planet.version += 1;

        Ok(asteroid_id)
    }

    fn generate_random_pirate(
        &mut self,
        rng: &mut ChaCha8Rng,
        population: Population,
        home_planet_id: PlanetId,
        position: Option<GamePosition>,
        base_level: Option<Skill>,
        extra_potential: Option<Skill>,
        extra_scouting: Option<Skill>,
    ) -> AppResult<PlayerId> {
        let mut build_player = Player::default()
            .with_position(position)
            .with_population(population)
            .with_current_location_on_planet(home_planet_id);

        if let Some(v) = base_level {
            build_player = build_player.with_base_level(v);
        }

        if let Some(v) = extra_potential {
            build_player = build_player.with_extra_potential(v);
        }

        let player = build_player.randomize(Some(rng));

        let player_id = player.id;
        self.players_scouting.insert(
            player.id,
            ScoutReport::new(
                player.id,
                (player.reputation + extra_scouting.unwrap_or_default()).bound(),
            ),
        );
        self.players.insert(player.id, player);

        self.dirty = true;
        self.dirty_ui = true;
        Ok(player_id)
    }

    pub fn auto_assign_crew_roles(&mut self, player_ids: Vec<PlayerId>) -> AppResult<()> {
        if player_ids.len() < 3 {
            return Ok(());
        }

        // Each player has a tuple in the vec, each tuple represents the toal bonus as captain, pilot, and doctor.
        let mut team_bonus: Vec<(f32, f32, f32, f32)> = vec![];
        for player_id in player_ids.iter() {
            let player = self.players.get_or_err(player_id)?;
            let captain_bonus = TeamBonus::Reputation.current_player_bonus(player)
                + TeamBonus::Bargaining.current_player_bonus(player);
            let pilot_bonus = TeamBonus::Scouting.current_player_bonus(player)
                + TeamBonus::SpaceshipSpeed.current_player_bonus(player);
            let doctor_bonus = TeamBonus::Training.current_player_bonus(player)
                + TeamBonus::TirednessRecovery.current_player_bonus(player);
            let engineer_bonus = TeamBonus::Weapons.current_player_bonus(player)
                + TeamBonus::Upgrades.current_player_bonus(player);

            team_bonus.push((captain_bonus, pilot_bonus, doctor_bonus, engineer_bonus));
        }

        // Assign roles to best player for role, starting from captain, then pilot, then doctor, then engineer.
        let mut assigned_idxs = vec![];
        let (captain_idx, _) = team_bonus
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.0.partial_cmp(&b.0).expect("Bonus should exist"))
            .expect("There should be a max");
        assigned_idxs.push(captain_idx);

        self.set_team_crew_role(CrewRole::Captain, player_ids[captain_idx])?;

        let (pilot_idx, _) = team_bonus
            .iter()
            .enumerate()
            .map(|(idx, value)| {
                if assigned_idxs.contains(&idx) {
                    (idx, (0.0, 0.0, 0.0, 0.0))
                } else {
                    (idx, *value)
                }
            })
            .max_by(|(_, a), (_, b)| a.1.partial_cmp(&b.1).expect("Bonus should exist"))
            .expect("There should be a max");

        assigned_idxs.push(pilot_idx);
        self.set_team_crew_role(CrewRole::Pilot, player_ids[pilot_idx])?;

        let (doctor_idx, _) = team_bonus
            .iter()
            .enumerate()
            .map(|(idx, value)| {
                if assigned_idxs.contains(&idx) {
                    (idx, (0.0, 0.0, 0.0, 0.0))
                } else {
                    (idx, *value)
                }
            })
            .max_by(|(_, a), (_, b)| a.2.partial_cmp(&b.2).expect("Bonus should exist"))
            .expect("There should be a max");

        assigned_idxs.push(doctor_idx);
        self.set_team_crew_role(CrewRole::Doctor, player_ids[doctor_idx])?;

        let (engineer_idx, _) = team_bonus
            .iter()
            .enumerate()
            .map(|(idx, value)| {
                if assigned_idxs.contains(&idx) {
                    (idx, (0.0, 0.0, 0.0, 0.0))
                } else {
                    (idx, *value)
                }
            })
            .max_by(|(_, a), (_, b)| a.3.partial_cmp(&b.3).expect("Bonus should exist"))
            .expect("There should be a max");

        assigned_idxs.push(engineer_idx);
        self.set_team_crew_role(CrewRole::Engineer, player_ids[engineer_idx])?;

        Ok(())
    }

    fn remove_player_from_role(&mut self, player_id: PlayerId) -> AppResult<()> {
        let mut player = self.players.get_or_err(&player_id)?.clone();
        let player_previous_role = player.info.crew_role;
        if player_previous_role == CrewRole::Mozzo {
            return Ok(());
        }

        let team_id = if let Some(team_id) = player.team {
            team_id
        } else {
            return Err(anyhow!("Player {player_id:?} is not in a team"));
        };

        let mut team = self.teams.get_or_err(&team_id)?.clone();

        team.can_set_crew_role(&player)?;

        let previous_spaceship_speed_bonus =
            TeamBonus::SpaceshipSpeed.current_team_bonus(&team.id, &self.teams, &self.players)?;
        let previous_upgrade_bonus =
            TeamBonus::Upgrades.current_team_bonus(&team.id, &self.teams, &self.players)?;

        let jersey = if team.is_travelling() {
            Jersey {
                style: JerseyStyle::Pirate,
                color: team.jersey.color,
            }
        } else {
            team.jersey.clone()
        };

        // Empty previous role of player.
        match player_previous_role {
            CrewRole::Captain => {
                team.crew_roles.captain = None;
            }
            CrewRole::Pilot => {
                team.crew_roles.pilot = None;
            }
            CrewRole::Doctor => {
                team.crew_roles.doctor = None;
            }
            CrewRole::Engineer => {
                team.crew_roles.engineer = None;
            }
            CrewRole::Mozzo => unreachable!(),
        }

        // Demote player to mozzo.
        player.info.crew_role = CrewRole::Mozzo;
        team.crew_roles.mozzo.push(player.id);
        // Demoted player is a bit demoralized :(
        player.add_morale(MORALE_DEMOTION_MALUS);
        player.set_jersey(&jersey);
        self.players.insert(player.id, player);
        self.teams.insert(team.id, team);

        if player_previous_role == CrewRole::Pilot {
            // Grab team again. We need to do this to ensure that the speed bonus is calculated correctly.
            let mut team = self.teams.get_or_err(&team_id)?.clone();
            // If team is travelling and pilot was updated recalculate travel duration.
            if let TeamLocation::Travelling {
                from,
                to,
                started,
                duration,
                distance,
            } = team.current_location
            {
                let new_start = Tick::now();
                let time_elapsed = new_start - started;
                let bonus = TeamBonus::SpaceshipSpeed.current_team_bonus(
                    &team_id,
                    &self.teams,
                    &self.players,
                )?;

                let new_duration =
                    (duration - time_elapsed) as f32 * previous_spaceship_speed_bonus / bonus;

                log::debug!(
                    "Update {player_previous_role}: old speed {previous_spaceship_speed_bonus}, new speed {bonus}"
                );

                team.current_location = TeamLocation::Travelling {
                    from,
                    to,
                    started: new_start,
                    duration: new_duration as Tick,
                    distance,
                };
            }
            self.teams.insert(team.id, team);
        } else if player_previous_role == CrewRole::Engineer {
            // Grab team again. We need to do this to ensure that the upgrade bonus is calculated correctly.
            let mut team = self.teams.get_or_err(&team_id)?.clone();
            // If spaceship or any asteroid has a pending upgrade and engineer was updated recalculate upgrade duration.
            if let Some(upgrade) = team.spaceship.pending_upgrade {
                let new_start = Tick::now();
                let time_elapsed = new_start - upgrade.started;
                let bonus =
                    TeamBonus::Upgrades.current_team_bonus(&team_id, &self.teams, &self.players)?;

                let new_duration =
                    (upgrade.duration - time_elapsed) as f32 * previous_upgrade_bonus / bonus;

                log::debug!(
                    "Update {player_previous_role}: old upgrade {previous_upgrade_bonus}, new upgrade {bonus}"
                );
                let new_upgrade =
                    Upgrade::new(upgrade.target, bonus).with_duration(new_duration as Tick);
                team.spaceship.pending_upgrade = Some(new_upgrade);
            }

            for asteroid_id in team.asteroid_ids.iter() {
                let asteroid = self.planets.get_or_err(asteroid_id)?;
                if let Some(upgrade) = asteroid.pending_upgrade {
                    let new_start = Tick::now();
                    let time_elapsed = new_start - upgrade.started;
                    let bonus = TeamBonus::Upgrades.current_team_bonus(
                        &team_id,
                        &self.teams,
                        &self.players,
                    )?;

                    let new_duration =
                        (upgrade.duration - time_elapsed) as f32 * previous_upgrade_bonus / bonus;

                    log::debug!(
                        "Update {player_previous_role}: old upgrade {previous_upgrade_bonus}, new upgrade {bonus}"
                    );

                    let mut asteroid = asteroid.clone();

                    let new_upgrade =
                        Upgrade::new(upgrade.target, bonus).with_duration(new_duration as Tick);

                    asteroid.pending_upgrade = Some(new_upgrade);
                    self.planets.insert(asteroid.id, asteroid);
                }
            }
            self.teams.insert(team.id, team);
        }

        self.dirty = true;
        self.dirty_ui = true;

        Ok(())
    }

    pub fn set_team_crew_role(&mut self, role: CrewRole, player_id: PlayerId) -> AppResult<()> {
        let player_previous_role = self.players.get_or_err(&player_id)?.info.crew_role;
        if player_previous_role == role {
            return Ok(());
        }

        if role == CrewRole::Mozzo {
            return self.remove_player_from_role(player_id);
        }

        let mut player = self.players.get_or_err(&player_id)?.clone();

        let team_id = if let Some(team_id) = player.team {
            team_id
        } else {
            return Err(anyhow!("Player {player_id:?} is not in a team"));
        };

        let mut team = self.teams.get_or_err(&team_id)?.clone();

        team.can_set_crew_role(&player)?;

        let previous_spaceship_speed_bonus =
            TeamBonus::SpaceshipSpeed.current_team_bonus(&team.id, &self.teams, &self.players)?;

        let previous_upgrade_bonus =
            TeamBonus::Upgrades.current_team_bonus(&team.id, &self.teams, &self.players)?;

        let jersey = if team.is_travelling() {
            Jersey {
                style: JerseyStyle::Pirate,
                color: team.jersey.color,
            }
        } else {
            team.jersey.clone()
        };

        let current_role_player_id = team.get_crew_role(role);

        // Empty previous role of player.
        match player_previous_role {
            CrewRole::Captain => {
                team.crew_roles.captain = None;
            }
            CrewRole::Pilot => {
                team.crew_roles.pilot = None;
            }
            CrewRole::Doctor => {
                team.crew_roles.doctor = None;
            }
            CrewRole::Engineer => {
                team.crew_roles.engineer = None;
            }
            CrewRole::Mozzo => {
                team.crew_roles.mozzo.retain(|&id| id != player.id);
            }
        }

        // Demote previous crew role player to mozzo.
        if let Some(crew_player_id) = current_role_player_id {
            let current_role_player = self.players.get_mut_or_err(&crew_player_id)?;
            current_role_player.info.crew_role = CrewRole::Mozzo;
            team.crew_roles.mozzo.push(current_role_player.id);
            // Demoted player is a bit demoralized :(
            current_role_player.add_morale(MORALE_DEMOTION_MALUS);
            current_role_player.set_jersey(&jersey);
        }

        // Set player to new role.
        match role {
            CrewRole::Captain => {
                team.crew_roles.captain = Some(player_id);
            }
            CrewRole::Pilot => {
                team.crew_roles.pilot = Some(player_id);
            }
            CrewRole::Doctor => {
                team.crew_roles.doctor = Some(player_id);
            }
            CrewRole::Engineer => {
                team.crew_roles.engineer = Some(player_id);
            }
            CrewRole::Mozzo => unreachable!(),
        }
        player.info.crew_role = role;
        player.set_jersey(&jersey);

        self.players.insert(player.id, player);
        self.teams.insert(team.id, team);

        if role == CrewRole::Pilot || player_previous_role == CrewRole::Pilot {
            // Grab team again. We need to do this to ensure that the speed bonus is calculated correctly.
            let mut team = self.teams.get_or_err(&team_id)?.clone();
            // If team is travelling and pilot was updated recalculate travel duration.
            if let TeamLocation::Travelling {
                from,
                to,
                started,
                duration,
                distance,
            } = team.current_location
            {
                let new_start = Tick::now();
                let time_elapsed = new_start - started;
                let bonus = TeamBonus::SpaceshipSpeed.current_team_bonus(
                    &team_id,
                    &self.teams,
                    &self.players,
                )?;

                let new_duration =
                    (duration - time_elapsed) as f32 * previous_spaceship_speed_bonus / bonus;

                log::debug!(
                    "Update {role}: old speed {previous_spaceship_speed_bonus}, new speed {bonus}"
                );

                team.current_location = TeamLocation::Travelling {
                    from,
                    to,
                    started: new_start,
                    duration: new_duration as Tick,
                    distance,
                };
            }
            self.teams.insert(team.id, team);
        }
        if role == CrewRole::Engineer || player_previous_role == CrewRole::Engineer {
            // Grab team again. We need to do this to ensure that the upgrade bonus is calculated correctly.
            let mut team = self.teams.get_or_err(&team_id)?.clone();
            // If spaceship or any asteroid has a pending upgrade and engineer was updated recalculate upgrade duration.
            if let Some(upgrade) = team.spaceship.pending_upgrade {
                let new_start = Tick::now();
                let time_elapsed = new_start - upgrade.started;
                let bonus =
                    TeamBonus::Upgrades.current_team_bonus(&team_id, &self.teams, &self.players)?;

                let new_duration =
                    (upgrade.duration - time_elapsed) as f32 * previous_upgrade_bonus / bonus;

                log::debug!(
                    "Update {role}: old upgrade {previous_upgrade_bonus}, new upgrade {bonus}"
                );

                let new_upgrade =
                    Upgrade::new(upgrade.target, bonus).with_duration(new_duration as Tick);
                team.spaceship.pending_upgrade = Some(new_upgrade);
            }

            for asteroid_id in team.asteroid_ids.iter() {
                let asteroid = self.planets.get_or_err(asteroid_id)?;
                if let Some(upgrade) = asteroid.pending_upgrade {
                    let new_start = Tick::now();
                    let time_elapsed = new_start - upgrade.started;
                    let bonus = TeamBonus::Upgrades.current_team_bonus(
                        &team_id,
                        &self.teams,
                        &self.players,
                    )?;

                    let new_duration =
                        (upgrade.duration - time_elapsed) as f32 * previous_upgrade_bonus / bonus;

                    log::debug!(
                        "Update {role}: old upgrade {previous_upgrade_bonus}, new upgrade {bonus}"
                    );

                    let mut asteroid = asteroid.clone();
                    let new_upgrade =
                        Upgrade::new(upgrade.target, bonus).with_duration(new_duration as Tick);
                    asteroid.pending_upgrade = Some(new_upgrade);
                    self.planets.insert(asteroid.id, asteroid);
                }
            }
            self.teams.insert(team.id, team);
        }

        self.dirty = true;
        self.dirty_network = true;
        self.dirty_ui = true;
        Ok(())
    }

    pub fn next_free_pirates_refresh(&self) -> Tick {
        // Returns the time to the next FA refresh in milliseconds
        let next_refresh = self.last_tick_long_interval + TickInterval::LONG;
        next_refresh.saturating_sub(self.last_tick_short_interval)
    }

    fn add_player_to_team(
        &mut self,
        player_id: &PlayerId,
        team_id: &TeamId,
        current_tick: Tick,
    ) -> AppResult<()> {
        let mut player = self.players.get_or_err(player_id)?.clone();
        let mut team = self.teams.get_or_err(team_id)?.clone();
        let is_in_space_cove = self.player_is_in_space_cove_on(&player);
        team.can_add_player(&player, is_in_space_cove)?;

        team.player_ids.push(player.id);
        team.reassign_positions(&self.players);
        team.version += 1;

        player.team = Some(team.id);
        player.joined_team_on = Some(current_tick);
        player.reset_team_satisfaction_on_hire();
        player.current_location = PlayerLocation::WithTeam;
        player.set_jersey(&team.jersey);
        player.peer_id = team.peer_id;
        player.info.crew_role = CrewRole::Mozzo;

        // Set player minimum morale to MORALE_HIRE_BONUS.
        // This makes the player not wanna immediately leave the crew.
        // On the other hand, it allows for a dirty trick of firing and re hiring the pirate
        // to reset the morale to this minimum.
        if player.morale < MORALE_HIRE_BONUS {
            player.add_morale(MORALE_HIRE_BONUS - player.morale);
        }
        player.version += 1;

        self.players.insert(player.id, player);
        self.teams.insert(team.id, team);
        self.dirty = true;
        if *team_id == self.own_team_id {
            self.dirty_network = true;
        }
        self.dirty_ui = true;

        Ok(())
    }

    pub fn hire_player_for_team(
        &mut self,
        player_id: &PlayerId,
        team_id: &TeamId,
        current_tick: Tick,
    ) -> AppResult<()> {
        let player = self.players.get_or_err(player_id)?;
        let mut team = self.teams.get_or_err(team_id)?.clone();
        let is_in_space_cove = self.player_is_in_space_cove_on(player);
        team.can_hire_player(player, is_in_space_cove)?;
        team.sub_resource(Resource::SATOSHI, player.hire_cost())?;
        self.teams.insert(team.id, team);

        self.add_player_to_team(player_id, team_id, current_tick)?;

        Ok(())
    }

    /// Applies a settled trade. A failure leaves the world untouched.
    pub fn apply_trade(&mut self, trade: &Trade, current_tick: Tick) -> AppResult<()> {
        if self.applied_trades.contains(&trade.id) {
            return Ok(());
        }

        let mut proposer_team = self.teams.get_or_err(&trade.proposer_team_id)?.clone();
        let mut target_team = self.teams.get_or_err(&trade.target_team_id)?.clone();

        let mut target_player =
            Self::resolve_traded_player(&self.players, &trade.target_player, trade.target_team_id);
        let mut proposer_player = trade.proposer_player.as_ref().map(|player| {
            Self::resolve_traded_player(&self.players, player, trade.proposer_team_id)
        });

        let held_by_proposer = proposer_team.offer(&trade.id).is_some();
        let held_by_target = target_team
            .pending_accepts
            .iter()
            .any(|pending| pending.id == trade.id);
        Self::validate_trade(
            &proposer_team,
            &target_team,
            proposer_player.as_ref(),
            &target_player,
            trade,
            held_by_proposer,
            held_by_target,
        )?;

        if held_by_proposer {
            proposer_team
                .offers
                .retain(|offer| offer.trade_id != trade.id);
        } else {
            Self::pay(&mut proposer_team, trade.proposer_pays())?;
        }
        if !held_by_target {
            Self::pay(&mut target_team, trade.target_pays())?;
        }
        proposer_team.saturating_add_resource(Resource::SATOSHI, trade.target_pays());
        target_team.saturating_add_resource(Resource::SATOSHI, trade.proposer_pays());
        target_team
            .pending_accepts
            .retain(|pending| pending.id != trade.id);

        let to_dock = matches!(trade.route, OfferKind::Dock);
        Self::stage_traded_player(
            &mut target_player,
            &mut target_team,
            &mut proposer_team,
            current_tick,
            to_dock,
        );
        if let Some(player) = proposer_player.as_mut() {
            Self::stage_traded_player(
                player,
                &mut proposer_team,
                &mut target_team,
                current_tick,
                to_dock,
            );
        }

        proposer_team.version += 1;
        target_team.version += 1;

        self.players.insert(target_player.id, target_player);
        if let Some(player) = proposer_player {
            self.players.insert(player.id, player);
        }
        let (proposer_team_id, target_team_id) = (proposer_team.id, target_team.id);
        self.teams.insert(proposer_team_id, proposer_team);
        self.teams.insert(target_team_id, target_team);

        for team_id in [proposer_team_id, target_team_id] {
            if let Some(mut team) = self.teams.remove(&team_id) {
                team.reassign_positions(&self.players);
                self.teams.insert(team_id, team);
            }
        }

        self.applied_trades.insert(trade.id);
        self.dirty = true;
        self.dirty_network = true;
        self.dirty_ui = true;

        Ok(())
    }

    fn pay(team: &mut Team, amount: u32) -> AppResult<()> {
        if team.peer_id.is_some() {
            team.saturating_sub_resource(Resource::SATOSHI, amount);
            Ok(())
        } else {
            team.sub_resource(Resource::SATOSHI, amount)
        }
    }

    /// Our stored copy of a pirate wins when it is at least as fresh as the one on
    /// the wire; the declared owner always wins over the payload's `team` field.
    fn resolve_traded_player(
        players: &PlayerMap,
        payload: &Player,
        from_team_id: TeamId,
    ) -> Player {
        let mut player = match players.get(&payload.id) {
            Some(local) if local.version >= payload.version => local.clone(),
            _ => payload.clone(),
        };
        player.team = Some(from_team_id);
        player
    }

    fn validate_trade(
        proposer_team: &Team,
        target_team: &Team,
        proposer_player: Option<&Player>,
        target_player: &Player,
        trade: &Trade,
        held_by_proposer: bool,
        held_by_target: bool,
    ) -> AppResult<()> {
        if target_team.peer_id.is_none() {
            if !target_team.player_ids.contains(&target_player.id) {
                return Err(anyhow!(
                    "{} is no longer in {}",
                    target_player.info.short_name(),
                    target_team.name
                ));
            }
            if !held_by_target && target_team.balance() < trade.target_pays() {
                return Err(anyhow!("{} cannot afford that", target_team.name));
            }
        }

        if proposer_team.peer_id.is_none() {
            if let Some(player) = proposer_player {
                if !proposer_team.player_ids.contains(&player.id) {
                    return Err(anyhow!(
                        "{} is no longer in {}",
                        player.info.short_name(),
                        proposer_team.name
                    ));
                }
            }
            if !held_by_proposer && proposer_team.balance() < trade.proposer_pays() {
                return Err(anyhow!("{} cannot afford that", proposer_team.name));
            }
            if !proposer_team.has_seat_for(proposer_player.is_some()) {
                return Err(anyhow!("{} is full", proposer_team.name));
            }
        }

        Ok(())
    }

    fn stage_traded_player(
        player: &mut Player,
        from: &mut Team,
        to: &mut Team,
        current_tick: Tick,
        to_dock: bool,
    ) {
        from.player_ids.retain(|&id| id != player.id);
        from.remove_listing(&player.id);
        from.waiting_at_dock.retain(|&id| id != player.id);
        from.vacate_crew_role(&player.id, player.info.crew_role);

        if !to.player_ids.contains(&player.id) {
            to.player_ids.push(player.id);
        }
        if to_dock && !to.waiting_at_dock.contains(&player.id) {
            to.waiting_at_dock.push(player.id);
        }
        Self::welcome_traded_pirate(player, from.id, to, current_tick);
    }

    /// Everything about a pirate that changes when they join a crew through a
    /// deal rather than a hire: opinions carried over, no firing penalty.
    fn welcome_traded_pirate(
        player: &mut Player,
        old_team_id: TeamId,
        to: &Team,
        current_tick: Tick,
    ) {
        // Remember how they felt about the old crew, then start fresh with the new
        // one - but without the firing penalty, which a trade is not.
        let base = player
            .opinions
            .remove(&PlayerOpinion::OwnTeam)
            .map(|(_, value)| value)
            .unwrap_or(OPINION_NEUTRAL_VALUE);
        player.opinions.insert(
            PlayerOpinion::Team {
                team_id: old_team_id,
            },
            (Tick::now(), base),
        );

        player.team = Some(to.id);
        player.joined_team_on = Some(current_tick);
        // Keeps add_network_team from letting the old crew reclaim this pirate.
        player.peer_id = to.peer_id;
        player.info.crew_role = CrewRole::Mozzo;
        player.current_location = PlayerLocation::WithTeam;
        player.set_jersey(&to.jersey);
        player.reset_team_satisfaction_on_hire();
        player.add_team_satisfaction(SATISFACTION_MALUS_TRADED);
        player.version += 1;
    }

    pub fn release_player_from_team(
        &mut self,
        player_id: PlayerId,
        is_fired: bool,
    ) -> AppResult<()> {
        let mut player = self.players.get_or_err(&player_id)?.clone();

        let team_id = if let Some(team_id) = player.team {
            team_id
        } else {
            return Err(anyhow!("Cannot release player with no team"));
        };
        let mut team = self.teams.get_or_err(&team_id)?.clone();
        team.can_release_player(&player)?;

        // Drop any listing before the pirate leaves, so the dock never names
        // someone who is no longer on the crew.
        let at_dock = team.is_listed(&player.id) || team.is_waiting(&player.id);
        team.remove_listing(&player.id);
        team.waiting_at_dock.retain(|id| *id != player.id);

        team.player_ids.retain(|&p| p != player.id);

        team.reassign_positions(&self.players);
        team.version += 1;

        player.team = None;
        team.vacate_crew_role(&player.id, player.info.crew_role);
        player.info.crew_role = CrewRole::Mozzo;
        player.add_morale(MORALE_RELEASE_MALUS);
        player.image.remove_jersey();
        player.compose_image()?;
        player.current_location = if at_dock {
            PlayerLocation::OnPlanet {
                planet_id: *GALAXY_ROOT_ID,
            }
        } else {
            match team.current_location {
                TeamLocation::OnPlanet { planet_id } => PlayerLocation::OnPlanet { planet_id },
                _ => return Err(anyhow!("Cannot release player while travelling")),
            }
        };

        let satisfaction = player.opinions.remove(&PlayerOpinion::OwnTeam);
        let base = satisfaction
            .map(|(_, value)| value)
            .unwrap_or(OPINION_NEUTRAL_VALUE);
        let value = if is_fired {
            (base + SATISFACTION_MALUS_RELEASE_FROM_TEAM).bound()
        } else {
            base
        };
        player
            .opinions
            .insert(PlayerOpinion::Team { team_id }, (Tick::now(), value));
        player.version += 1;

        self.dirty = true;
        if team.id == self.own_team_id {
            self.dirty_network = true;
        }
        self.dirty_ui = true;

        self.players.insert(player.id, player);
        self.teams.insert(team.id, team);

        Ok(())
    }

    pub fn make_offer(
        &mut self,
        target_player_id: PlayerId,
        pirate: Option<PlayerId>,
        satoshis: i64,
        own_peer_id: PeerId,
    ) -> AppResult<OfferOutcome> {
        let target_player = self.players.get_or_err(&target_player_id)?.clone();
        let name = target_player.info.short_name();
        let target_team_id = target_player
            .team
            .ok_or_else(|| anyhow!("{name} has no crew"))?;
        let target_team = self.teams.get_or_err(&target_team_id)?;
        let own_team = self.get_own_team()?;
        if own_team.offer_on(&target_player_id).is_some() {
            return Err(anyhow!("You already made an offer for {name}"));
        }

        let route = if target_team.is_listed(&target_player_id) {
            OfferKind::Dock
        } else {
            OfferKind::Direct
        };
        let proposer_player = match pirate {
            Some(id) => Some(self.players.get_or_err(&id)?.clone()),
            None => None,
        };
        own_team.can_make_offer(
            target_team,
            route,
            proposer_player.as_ref(),
            &target_player,
            satoshis,
        )?;

        let target_peer_id = target_team.peer_id;
        let trade = Trade::new(
            route,
            own_peer_id,
            target_peer_id.unwrap_or(own_peer_id),
            self.own_team_id,
            target_team_id,
            proposer_player,
            target_player,
            satoshis,
        );

        if target_peer_id.is_none() {
            return self.offer_to_local_crew(&trade);
        }

        let mut own_team = self.get_own_team()?.clone();
        own_team.sub_resource(Resource::SATOSHI, trade.proposer_pays())?;
        own_team.offers.push(Offer {
            trade_id: trade.id,
            kind: route,
            target_team_id,
            target_player_id,
            satoshis,
            pirate,
            placed_on: trade.created_at,
        });
        if let Some(player_id) = pirate {
            let mut player = self.players.get_or_err(&player_id)?.clone();
            own_team.vacate_crew_role(&player_id, player.info.crew_role);
            player.info.crew_role = CrewRole::Mozzo;
            player.version += 1;
            self.players.insert(player_id, player);
        }
        own_team.reassign_positions(&self.players);
        own_team.version += 1;
        self.teams.insert(own_team.id, own_team);
        self.trade_outbox.push(trade);

        self.dirty = true;
        self.dirty_network = true;
        self.dirty_ui = true;
        Ok(OfferOutcome::Sent)
    }

    fn offer_to_local_crew(&mut self, trade: &Trade) -> AppResult<OfferOutcome> {
        let target_team = self.teams.get_or_err(&trade.target_team_id)?;
        if target_team.current_game.is_some() || target_team.playing_in_tournament().is_some() {
            return Err(anyhow!(
                "{} is playing, make your offer after the game",
                target_team.name
            ));
        }

        let offered = trade
            .proposer_player
            .as_ref()
            .map_or(0, |player| i64::from(player.hire_cost()))
            + trade.satoshis;
        if offered < i64::from(trade.target_player.hire_cost()) {
            return Ok(OfferOutcome::Refused);
        }
        self.apply_trade(trade, Tick::now())?;
        Ok(OfferOutcome::Accepted)
    }

    pub fn end_offer(&mut self, trade_id: &TradeId) -> AppResult<Option<Offer>> {
        let mut own_team = self.get_own_team()?.clone();
        let Some(index) = own_team
            .offers
            .iter()
            .position(|offer| offer.trade_id == *trade_id)
        else {
            return Ok(None);
        };

        let offer = own_team.offers.remove(index);
        own_team.saturating_add_resource(Resource::SATOSHI, offer.held_satoshis());
        if let Some(player_id) = offer.pirate {
            if offer.is_dock_offer() && own_team.player_ids.contains(&player_id) {
                own_team.waiting_at_dock.push(player_id);
            }
        }
        own_team.reassign_positions(&self.players);
        own_team.version += 1;
        self.teams.insert(own_team.id, own_team);

        self.dirty = true;
        self.dirty_network = true;
        self.dirty_ui = true;
        Ok(Some(offer))
    }

    pub fn retire_offer(&mut self, trade_id: &TradeId, own_peer_id: PeerId) -> AppResult<()> {
        let offer = self
            .get_own_team()?
            .offer(trade_id)
            .cloned()
            .ok_or_else(|| anyhow!("That offer is no longer open"))?;
        if let Some(trade) = self.trade_for_offer(&offer, own_peer_id) {
            self.push_reply(
                &trade,
                NetworkRequestState::Failed {
                    error_message: "Offer retired".to_string(),
                },
            );
        }
        self.end_offer(trade_id)?;
        Ok(())
    }

    pub fn trade_for_offer(&self, offer: &Offer, own_peer_id: PeerId) -> Option<Trade> {
        let target_team = self.teams.get(&offer.target_team_id)?;
        let target_peer_id = target_team.peer_id?;
        let target_player = self.players.get(&offer.target_player_id)?.clone();
        let proposer_player = match offer.pirate {
            Some(id) => Some(self.players.get(&id)?.clone()),
            None => None,
        };
        Some(Trade {
            id: offer.trade_id,
            state: NetworkRequestState::Syn,
            route: offer.kind,
            app_version: app_version(),
            created_at: offer.placed_on,
            proposer_peer_id: own_peer_id,
            target_peer_id,
            proposer_team_id: self.own_team_id,
            target_team_id: offer.target_team_id,
            proposer_player,
            target_player,
            satoshis: offer.satoshis,
        })
    }

    pub fn push_reply(&mut self, trade: &Trade, state: NetworkRequestState) {
        let mut reply = trade.clone();
        reply.state = state;
        self.trade_outbox.push(reply);
    }

    pub fn open_trades(&self, own_peer_id: PeerId) -> Vec<Trade> {
        let Ok(own_team) = self.get_own_team() else {
            return vec![];
        };
        own_team
            .offers
            .iter()
            .filter_map(|offer| self.trade_for_offer(offer, own_peer_id))
            .chain(own_team.pending_accepts.iter().cloned())
            .collect()
    }

    pub fn is_team_present(&self, team_id: &TeamId, now: Tick) -> bool {
        self.network_team_last_heard
            .get(team_id)
            .is_some_and(|&heard| now.saturating_sub(heard) <= DOCK_PRESENCE_WINDOW)
    }

    pub fn receive_offer(&mut self, trade: Trade, now: Tick) -> AppResult<bool> {
        self.network_team_last_heard
            .insert(trade.proposer_team_id, now);
        let own_team = self.get_own_team()?;
        if own_team
            .pending_accepts
            .iter()
            .any(|pending| pending.id == trade.id)
        {
            return Ok(false);
        }
        if let Err(err) = self.check_incoming_offer(&trade) {
            self.get_own_team_mut()?.received_trades.remove(&trade.id);
            self.dirty_ui = true;
            self.push_reply(
                &trade,
                NetworkRequestState::Failed {
                    error_message: err.to_string(),
                },
            );
            return Ok(false);
        }
        let is_new = self
            .get_own_team_mut()?
            .received_trades
            .insert(trade.id, trade)
            .is_none();
        self.dirty_ui = true;
        Ok(is_new)
    }

    fn check_incoming_offer(&self, trade: &Trade) -> AppResult<()> {
        let own_team = self.get_own_team()?;
        let id = trade.target_player.id;
        let name = trade.target_player.info.short_name();
        if !own_team.player_ids.contains(&id) {
            return Err(anyhow!("{name} is not in {}", own_team.name));
        }
        match trade.route {
            OfferKind::Dock => {
                if !own_team.is_listed(&id) {
                    return Err(anyhow!("{name} is no longer at the dock"));
                }
            }
            OfferKind::Direct => {
                if own_team.is_parked(&id) {
                    return Err(anyhow!("{name} is not aboard"));
                }
                if let Some(proposer) = self.teams.get(&trade.proposer_team_id) {
                    if !proposer.shares_planet_with(own_team) {
                        return Err(anyhow!("Not on the same planet"));
                    }
                }
            }
        }
        Ok(())
    }

    pub fn can_accept_offer(&self, trade_id: &TradeId, now: Tick) -> AppResult<()> {
        let own_team = self.get_own_team()?;
        let trade = own_team
            .received_trades
            .get(trade_id)
            .ok_or_else(|| anyhow!("That offer is no longer open"))?;
        let proposer = self
            .teams
            .get(&trade.proposer_team_id)
            .filter(|_| self.is_team_present(&trade.proposer_team_id, now))
            .ok_or_else(|| anyhow!("The other crew is offline"))?;

        let id = trade.target_player.id;
        let name = trade.target_player.info.short_name();
        if own_team.is_leaving(&id) {
            return Err(anyhow!("{name} is already leaving"));
        }
        match trade.route {
            OfferKind::Dock => {
                if !own_team.is_listed(&id) {
                    return Err(anyhow!("{name} is no longer at the dock"));
                }
            }
            OfferKind::Direct => {
                if !own_team.player_ids.contains(&id) || own_team.is_parked(&id) {
                    return Err(anyhow!("{name} is not aboard"));
                }
                if !own_team.shares_planet_with(proposer) {
                    return Err(anyhow!("Not on the same planet"));
                }
            }
        }
        for team in [own_team, proposer] {
            if team.current_game.is_some() {
                return Err(anyhow!("{} is playing", team.name));
            }
            if team.playing_in_tournament().is_some() {
                return Err(anyhow!("{} is in a tournament", team.name));
            }
        }
        if own_team.balance() < trade.target_pays() {
            return Err(anyhow!("Not enough satoshi"));
        }
        if !proposer.has_seat_for(trade.proposer_player.is_some()) {
            return Err(anyhow!("{} is full", proposer.name));
        }
        Ok(())
    }

    pub fn accept_offer(&mut self, trade_id: &TradeId, now: Tick) -> AppResult<()> {
        self.can_accept_offer(trade_id, now)?;
        let mut team = self.get_own_team()?.clone();
        let mut trade = team
            .received_trades
            .get(trade_id)
            .cloned()
            .ok_or_else(|| anyhow!("That offer is no longer open"))?;
        let mut player = self.players.get_or_err(&trade.target_player.id)?.clone();
        team.sub_resource(Resource::SATOSHI, trade.target_pays())?;
        team.vacate_crew_role(&player.id, player.info.crew_role);
        player.info.crew_role = CrewRole::Mozzo;
        trade.target_player = player.clone();
        trade.state = NetworkRequestState::SynAck;
        team.pending_accepts.push(trade.clone());
        team.reassign_positions(&self.players);
        team.version += 1;
        self.players.insert(player.id, player);
        self.teams.insert(team.id, team);
        self.trade_outbox.push(trade);

        self.dirty = true;
        self.dirty_network = true;
        self.dirty_ui = true;
        Ok(())
    }

    pub fn decline_offer(&mut self, trade_id: &TradeId) -> AppResult<()> {
        let own_team = self.get_own_team_mut()?;
        if own_team
            .pending_accepts
            .iter()
            .any(|pending| pending.id == *trade_id)
        {
            return Err(anyhow!("That offer is being accepted"));
        }
        let trade = own_team
            .received_trades
            .remove(trade_id)
            .ok_or_else(|| anyhow!("That offer is no longer open"))?;
        let name = own_team.name.clone();
        self.push_reply(
            &trade,
            NetworkRequestState::Failed {
                error_message: format!("{name} declined"),
            },
        );
        self.dirty_ui = true;
        Ok(())
    }

    pub fn receive_ack(&mut self, trade: &Trade, now: Tick) -> AppResult<Option<String>> {
        if self
            .get_own_team()?
            .pending_accepts
            .iter()
            .all(|pending| pending.id != trade.id)
        {
            return Ok(None);
        }
        if let Some(player) = trade.proposer_player.as_ref() {
            self.players.insert(player.id, player.clone());
        }
        self.complete_accept(&trade.id, now).map(Some)
    }

    fn complete_accept(&mut self, trade_id: &TradeId, now: Tick) -> AppResult<String> {
        let pending = self
            .get_own_team()?
            .pending_accepts
            .iter()
            .find(|pending| pending.id == *trade_id)
            .cloned()
            .ok_or_else(|| anyhow!("No accept in progress for that offer"))?;
        let proposer_name = self
            .teams
            .get(&pending.proposer_team_id)
            .map_or_else(|| "another crew".to_string(), |team| team.name.clone());
        self.apply_trade(&pending, now)?;

        let player_id = pending.target_player.id;
        let own_team = self.get_own_team_mut()?;
        own_team.received_trades.remove(trade_id);
        let rivals: Vec<Trade> = own_team
            .received_trades
            .values()
            .filter(|trade| trade.target_player.id == player_id)
            .cloned()
            .collect();
        for rival in &rivals {
            own_team.received_trades.remove(&rival.id);
        }
        for rival in rivals {
            self.push_reply(
                &rival,
                NetworkRequestState::Failed {
                    error_message: format!("Signed with {proposer_name}"),
                },
            );
        }

        Ok(format!(
            "{} signed with {proposer_name}.",
            pending.target_player.info.short_name()
        ))
    }

    fn abort_accept(&mut self, trade_id: &TradeId) -> AppResult<()> {
        let mut team = self.get_own_team()?.clone();
        if let Some(index) = team
            .pending_accepts
            .iter()
            .position(|pending| pending.id == *trade_id)
        {
            let pending = team.pending_accepts.remove(index);
            team.saturating_add_resource(Resource::SATOSHI, pending.target_pays());
        }
        team.received_trades.remove(trade_id);
        team.reassign_positions(&self.players);
        team.version += 1;
        self.teams.insert(team.id, team);
        self.dirty = true;
        self.dirty_network = true;
        self.dirty_ui = true;
        Ok(())
    }

    pub fn receive_syn_ack(&mut self, trade: &Trade, now: Tick) -> AppResult<Option<String>> {
        if self.applied_trades.contains(&trade.id) {
            self.push_reply(trade, NetworkRequestState::Ack);
            return Ok(None);
        }
        let Some(offer) = self.get_own_team()?.offer(&trade.id).cloned() else {
            self.push_reply(
                trade,
                NetworkRequestState::Failed {
                    error_message: "Offer retired".to_string(),
                },
            );
            return Ok(None);
        };

        let name = trade.target_player.info.short_name();
        let checked = if trade.target_player.id != offer.target_player_id
            || trade.target_team_id != offer.target_team_id
        {
            Err(anyhow!("That is not the offer we made"))
        } else {
            self.can_complete_offer(&offer)
        };
        if let Err(err) = checked {
            self.end_offer(&trade.id)?;
            self.push_reply(
                trade,
                NetworkRequestState::Failed {
                    error_message: err.to_string(),
                },
            );
            return Ok(Some(format!(
                "Your offer for {name} could not be completed: {err}. It was refunded."
            )));
        }

        let proposer_player = match offer.pirate {
            Some(player_id) => Some(self.players.get_or_err(&player_id)?.clone()),
            None => None,
        };
        let trade = Trade {
            route: offer.kind,
            proposer_team_id: self.own_team_id,
            proposer_player,
            satoshis: offer.satoshis,
            ..trade.clone()
        };
        self.players
            .insert(trade.target_player.id, trade.target_player.clone());
        if let Err(err) = self.apply_trade(&trade, now) {
            self.end_offer(&trade.id)?;
            self.push_reply(
                &trade,
                NetworkRequestState::Failed {
                    error_message: err.to_string(),
                },
            );
            return Err(err);
        }
        self.push_reply(&trade, NetworkRequestState::Ack);
        Ok(Some(format!("{name} signed with you.")))
    }

    fn can_complete_offer(&self, offer: &Offer) -> AppResult<()> {
        let own_team = self.get_own_team()?;
        if own_team.current_game.is_some() {
            return Err(anyhow!("{} is playing", own_team.name));
        }
        if own_team.playing_in_tournament().is_some() {
            return Err(anyhow!("{} is in a tournament", own_team.name));
        }
        if !own_team.has_seat_for(offer.pirate.is_some()) {
            return Err(anyhow!("{} is full", own_team.name));
        }
        if offer.kind == OfferKind::Direct {
            let target_planet = self
                .teams
                .get(&offer.target_team_id)
                .and_then(Team::is_on_planet);
            if own_team.is_on_planet().is_none()
                || (target_planet.is_some() && target_planet != own_team.is_on_planet())
            {
                return Err(anyhow!("Not on the same planet"));
            }
        }
        Ok(())
    }

    pub fn receive_failed(
        &mut self,
        trade: &Trade,
        error_message: &str,
    ) -> AppResult<Option<String>> {
        if trade.proposer_team_id == self.own_team_id {
            return Ok(self.end_offer(&trade.id)?.map(|_| {
                format!(
                    "Your offer for {} ended: {error_message}. It was refunded.",
                    trade.target_player.info.short_name()
                )
            }));
        }
        let own_team = self.get_own_team()?;
        let is_known = own_team.received_trades.contains_key(&trade.id)
            || own_team.pending_accepts.iter().any(|p| p.id == trade.id);
        if trade.target_team_id == self.own_team_id && is_known {
            self.abort_accept(&trade.id)?;
        }
        Ok(None)
    }

    pub fn leave_player_at_dock(
        &mut self,
        player_id: PlayerId,
        current_tick: Tick,
    ) -> AppResult<()> {
        let mut player = self.players.get_or_err(&player_id)?.clone();
        let mut team = self.get_own_team()?.clone();
        team.can_leave_player_at_dock(&player)?;

        team.dock_listings
            .push(DockListing::new(player_id, current_tick));

        team.vacate_crew_role(&player_id, player.info.crew_role);
        player.info.crew_role = CrewRole::Mozzo;

        team.reassign_positions(&self.players);
        team.version += 1;

        self.players.insert(player.id, player);
        self.teams.insert(team.id, team);
        self.dirty = true;
        self.dirty_network = true;
        self.dirty_ui = true;

        Ok(())
    }

    /// The seat was never freed, so recalling cannot fail on crew capacity.
    pub fn recall_player_from_dock(&mut self, player_id: PlayerId) -> AppResult<()> {
        let mut team = self.get_own_team()?.clone();
        team.can_recall_player_from_dock(&player_id)?;

        team.remove_listing(&player_id);
        let dropped: Vec<Trade> = team
            .received_trades
            .values()
            .filter(|trade| trade.target_player.id == player_id)
            .cloned()
            .collect();
        for trade in &dropped {
            team.received_trades.remove(&trade.id);
        }

        let mut player = self.players.get_or_err(&player_id)?.clone();
        player.current_location = PlayerLocation::WithTeam;
        player.set_jersey(&team.jersey);
        player.version += 1;

        team.reassign_positions(&self.players);
        team.version += 1;

        let name = player.info.short_name();
        self.players.insert(player.id, player);
        self.teams.insert(team.id, team);
        for trade in dropped {
            self.push_reply(
                &trade,
                NetworkRequestState::Failed {
                    error_message: format!("{name} was recalled"),
                },
            );
        }
        self.dirty = true;
        self.dirty_network = true;
        self.dirty_ui = true;

        Ok(())
    }

    pub fn abandon_asteroid(&mut self, asteroid_id: PlanetId) -> AppResult<()> {
        let own_team = self.get_own_team_mut()?;
        own_team.asteroid_ids.retain(|&id| id != asteroid_id);

        if matches!(own_team.has_space_cove_on(), Some(id) if id == asteroid_id) {
            own_team.space_cove = None;
        }
        own_team.version += 1;

        self.dirty = true;
        self.dirty_network = true;
        self.dirty_ui = true;
        Ok(())
    }

    pub fn upgrade_asteroid(
        &mut self,
        asteroid_id: PlanetId,
        upgrade: Upgrade<PlanetUpgradeTarget>,
    ) -> AppResult<String> {
        // Validate asteroid exists
        self.planets.get_or_err(&asteroid_id)?;

        if upgrade.target == PlanetUpgradeTarget::SpaceCove {
            let own_team = self.teams.get_mut_or_err(&self.own_team_id)?;
            if let Some(cove) = own_team.space_cove.as_mut() {
                if cove.planet_id != asteroid_id {
                    return Err(anyhow!(
                        "Cannot finalize space cove upgrade: mismatching planet id."
                    ));
                }
                cove.finish_contruction();
                own_team.version += 1;
            } else {
                return Err(anyhow!(
                    "Cannot finalize space cove upgrade: no space cove under construction."
                ));
            }
        }

        let asteroid = self.planets.get_mut_or_err(&asteroid_id)?;
        asteroid.pending_upgrade = None;
        asteroid.version += 1;
        asteroid.upgrades.insert(upgrade.target);

        let message = format!(
            "{} construction on {} completed!",
            upgrade.target, asteroid.name
        );

        self.dirty = true;
        self.dirty_network = true;
        self.dirty_ui = true;
        Ok(message)
    }

    pub fn start_space_adventure(&mut self) -> AppResult<()> {
        let mut own_team = self.get_own_team()?.clone();
        let average_tiredness = own_team.average_tiredness(self);
        own_team.can_start_space_adventure(average_tiredness)?;

        let planet_id = own_team
            .is_on_planet()
            .ok_or_else(|| anyhow!("Team should be on a planet to start a space adventure."))?;

        let current_planet = self.planets.get_or_err(&planet_id)?;
        let should_spawn_asteroid = current_planet.asteroid_probability > 0.0
            && own_team.asteroid_ids.len() < MAX_NUM_ASTEROID_PER_TEAM;
        let gold_fragment_probability = 0.001
            + 0.075 * (current_planet.resources.value(&Resource::GOLD) as f64) / MAX_SKILL as f64;

        let speed_bonus = TeamBonus::SpaceshipSpeed.current_team_bonus(
            &own_team.id,
            &self.teams,
            &self.players,
        )?;
        let weapons_bonus =
            TeamBonus::Weapons.current_team_bonus(&own_team.id, &self.teams, &self.players)?;

        let space = SpaceAdventure::new(should_spawn_asteroid, gold_fragment_probability)?
            .with_player(
                &own_team.spaceship,
                own_team.resources.clone(),
                speed_bonus,
                weapons_bonus,
                own_team.fuel(),
            )?;

        own_team.current_location = TeamLocation::OnSpaceAdventure { around: planet_id };

        for player_id in own_team.active_player_ids().iter() {
            let player = self.players.get_mut_or_err(player_id)?;
            player.add_tiredness(SPACE_ADVENTURE_TIREDNESS_COST);
        }

        self.teams.insert(own_team.id, own_team);
        self.space_adventure = Some(space);
        self.dirty = true;
        self.dirty_network = true;
        self.dirty_ui = true;
        Ok(())
    }

    pub fn return_from_space_adventure(&mut self) -> AppResult<(String, Option<usize>)> {
        let mut own_team = self.get_own_team()?.clone();

        let space_adventure = self
            .space_adventure
            .take()
            .ok_or_else(|| anyhow!("World should have a space adventure"))?;

        let player = space_adventure
            .get_player()
            .ok_or_else(|| anyhow!("Space adventure should have a player entity."))?;

        own_team.number_of_space_adventures += 1;

        let mut resources_gathered = vec![];
        let mut resources_lost = vec![];
        for resource in Resource::iter() {
            let old_amount = own_team.resources.value(&resource);
            let new_amount = player.resources().value(&resource);
            if old_amount < new_amount {
                resources_gathered.push((resource, new_amount - old_amount));
            } else if old_amount > new_amount && resource != Resource::FUEL {
                resources_lost.push((resource, old_amount - new_amount));
            }
        }

        let resources_gathered_text = if resources_gathered.is_empty() {
            "No resources collected!".to_string()
        } else {
            format!(
                "{} collected.",
                resources_gathered
                    .iter()
                    .map(|(res, amount)| format!("{amount} {res}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };

        let resources_lost_text = if !resources_lost.is_empty() {
            format!(
                "{} lost.",
                resources_lost
                    .iter()
                    .map(|(res, amount)| format!("{amount} {res}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        } else {
            String::new()
        };

        own_team.resources = player.resources().clone();
        own_team
            .spaceship
            .set_current_durability(player.current_durability());

        match own_team.current_location {
            TeamLocation::OnSpaceAdventure { around } => {
                own_team.current_location = TeamLocation::OnPlanet { planet_id: around }
            }
            _ => {
                return Err(anyhow!("Team should be on a space adventure."));
            }
        }

        let asteroid_type = space_adventure.asteroid_planet_found();
        for player_id in own_team.active_player_ids().iter() {
            let player = self.players.get_mut_or_err(player_id)?;
            if asteroid_type.is_some() {
                player.satisfy_opinion(PlayerOpinion::Space);
            }
        }

        self.teams.insert(own_team.id, own_team);
        self.dirty = true;
        self.dirty_network = true;
        self.dirty_ui = true;

        let message = format!(
            "Team returned from space adventure:\n{resources_gathered_text}\n{resources_lost_text}"
        );
        Ok((message, asteroid_type))
    }

    fn generate_game_no_checks(
        &mut self,
        mut home_team_in_game: TeamInGame,
        mut away_team_in_game: TeamInGame,
        starting_at: Tick,
        planet_id: PlanetId,
        part_of_tournament: Option<TournamentId>,
    ) -> AppResult<GameId> {
        // Generate deterministic game id from team IDs and starting time.
        // Two games starting at u64::MAX milliseconds apart ~ 584_942_417 years
        // would have the same ID, we assume this can't happen.
        let mut rng_seed = ((home_team_in_game.team_id.as_u64_pair().0 as u128
            + away_team_in_game.team_id.as_u64_pair().0 as u128)
            % (u64::MAX as u128)) as u64;
        rng_seed = (rng_seed as Tick + starting_at) % (u64::MAX as Tick);

        let rng = &mut ChaCha8Rng::seed_from_u64(rng_seed);
        let game_id = GameId::from_u128(rng.random());

        let planet = self.planets.get_or_err(&planet_id)?;

        // Give morale bonus to players based on planet populations
        // Restore tiredness and morale, at least partially if it's a tournament game.
        for (_, player) in home_team_in_game.players.iter_mut() {
            let morale_bonus = planet
                .populations
                .get(&player.info.population)
                .copied()
                .unwrap_or_default() as f32
                * MORALE_GAME_POPULATION_MODIFIER;
            player.add_morale(morale_bonus);

            if part_of_tournament.is_some() {
                player.tiredness =
                    (player.tiredness - TIREDNESS_DECREASE_BEFORE_TOURNAMENT_GAME).bound();
            }
        }

        for (_, player) in away_team_in_game.players.iter_mut() {
            let morale_bonus = planet
                .populations
                .get(&player.info.population)
                .copied()
                .unwrap_or_default() as f32
                * MORALE_GAME_POPULATION_MODIFIER;
            player.add_morale(morale_bonus);

            if part_of_tournament.is_some() {
                player.tiredness =
                    (player.tiredness - TIREDNESS_DECREASE_BEFORE_TOURNAMENT_GAME).bound();
            }
        }

        let game = Game::new(
            game_id,
            home_team_in_game,
            away_team_in_game,
            starting_at,
            planet.id,
            planet.total_population(),
            planet.name.as_str(),
            part_of_tournament,
        );
        self.games.insert(game.id, game);

        Ok(game_id)
    }

    pub fn generate_network_game(
        &mut self,
        home_team_in_game: TeamInGame,
        away_team_in_game: TeamInGame,
        starting_at: Tick,
    ) -> AppResult<GameId> {
        let mut home_team = self.teams.get_or_err(&home_team_in_game.team_id)?.clone();
        let mut away_team = self.teams.get_or_err(&away_team_in_game.team_id)?.clone();

        // For a network game we run different checks.
        // In particular, we check if our game has already a game, not the other.
        // This is necessary because of a race condition when a team can be received
        // over the network before the challenge confirmation message.
        if self.own_team_id == home_team.id {
            if home_team.current_game.is_some() {
                return Err(anyhow!("{} is already playing", home_team.name));
            }
        } else if self.own_team_id == away_team.id && away_team.current_game.is_some() {
            return Err(anyhow!("{} is already playing", away_team.name));
        }

        home_team.can_accept_network_challenge(&away_team)?;

        let location = match home_team.current_location {
            TeamLocation::OnPlanet { planet_id } => planet_id,
            _ => {
                panic!("Should have failed in can_challenge_team")
            }
        };

        let home_initial_rum = home_team_in_game.initial_rum;
        let away_initial_rum = away_team_in_game.initial_rum;

        let game_id = self.generate_game_no_checks(
            home_team_in_game,
            away_team_in_game,
            starting_at,
            location,
            None,
        )?;

        if let Some(previous_game_id) = home_team.current_game {
            if game_id != previous_game_id {
                return Err(anyhow!(
                    "{} is already playing another game",
                    home_team.name
                ));
            }
        }

        if let Some(previous_game_id) = away_team.current_game {
            if game_id != previous_game_id {
                return Err(anyhow!(
                    "{} is already playing another game",
                    away_team.name
                ));
            }
        }

        home_team.enter_game(game_id, home_initial_rum);
        away_team.enter_game(game_id, away_initial_rum);
        self.dirty = true;
        self.dirty_ui = true;

        if home_team.id == self.own_team_id || away_team.id == self.own_team_id {
            // Update network that game has started.
            self.dirty_network = true;
        }

        self.teams.insert(home_team.id, home_team);
        self.teams.insert(away_team.id, away_team);

        Ok(game_id)
    }

    pub fn generate_local_game(
        &mut self,
        home_team_in_game: TeamInGame,
        away_team_in_game: TeamInGame,
    ) -> AppResult<GameId> {
        let starting_at = self.last_tick_short_interval + GAME_START_DELAY;

        let home_team = self.teams.get_or_err(&home_team_in_game.team_id)?;
        let away_team = self.teams.get_or_err(&away_team_in_game.team_id)?;
        home_team.can_challenge_local_team(away_team)?;

        let location = home_team
            .is_on_planet()
            .expect("Should have failed in can_challenge_team");

        let team_ids = [home_team_in_game.team_id, away_team_in_game.team_id];
        let initial_rums = [home_team_in_game.initial_rum, away_team_in_game.initial_rum];

        let game_id = self.generate_game_no_checks(
            home_team_in_game,
            away_team_in_game,
            starting_at,
            location,
            None,
        )?;

        for (team_id, initial_rum) in team_ids.iter().zip(initial_rums) {
            let team = self
                .teams
                .get_mut(team_id)
                .ok_or_else(|| anyhow!("Team {team_id:?} not found"))?;

            team.enter_game(game_id, initial_rum);
            if team.id == self.own_team_id {
                self.dirty_network = true
            }
        }

        self.dirty = true;
        self.dirty_ui = true;

        Ok(game_id)
    }

    pub fn add_network_game(&mut self, network_game: NetworkGame) -> AppResult<()> {
        // Check that the game does not involve the own team (otherwise we would have generated it).
        if network_game.home_team_in_game.team_id == self.own_team_id
            || network_game.away_team_in_game.team_id == self.own_team_id
        {
            return Err(anyhow!(
                "Cannot receive game involving own team over the network."
            ));
        }

        if network_game.timer.has_ended() {
            return Err(anyhow!(
                "Cannot receive game that has ended over the network."
            ));
        }

        // A tournament game only makes sense with its tournament.
        if let Some(tournament_id) = network_game.part_of_tournament {
            if !self.tournaments.contains_key(&tournament_id) {
                return Err(anyhow!(
                    "Cannot receive tournament game whose tournament is not present."
                ));
            }
        }

        if !self.games.contains_key(&network_game.id) {
            let planet = self.planets.get_or_err(&network_game.location)?;
            let mut game = Game::new(
                network_game.id,
                network_game.home_team_in_game,
                network_game.away_team_in_game,
                network_game.starting_at,
                planet.id,
                planet.total_population(),
                planet.name.as_str(),
                network_game.part_of_tournament,
            );

            game.catch_up(Tick::now());

            self.games.insert(game.id, game);
            self.dirty_ui = true;
        }

        Ok(())
    }

    pub fn add_network_team(
        &mut self,
        network_team: NetworkTeam,
        timestamp: Tick,
    ) -> AppResult<bool> {
        let NetworkTeam {
            team,
            players,
            asteroids,
        } = network_team;
        if team.peer_id.is_none() {
            return Err(anyhow!(
                "Cannot receive team without peer_id over the network."
            ));
        }
        if team.id == self.own_team_id {
            return Err(anyhow!("Cannot receive own team over the network."));
        }
        self.network_team_last_heard.insert(team.id, Tick::now());

        // Check if we are receiving a team with which we have an open challenge.
        // Note: there could be a race condition where we receive a team over the network right after
        //       accepting the challenge but before the challenge has been finalized on our side.
        //       In this case, the received team would have current_game set to some (set to the challenge game
        //       they just started) and the challenge would fail on our hand since the challenge team must have no game.

        // Ignore updates not newer than the last one applied for this team; wire delivery is unordered.
        if self
            .network_team_timestamps
            .get(&team.id)
            .is_some_and(|&prev| prev >= timestamp)
        {
            return Ok(false);
        }

        let db_team = self.teams.get(&team.id).cloned();
        let version_changed = db_team
            .as_ref()
            .is_some_and(|previous| previous.version < team.version);

        // Stitch network asteroids into their parent planet's satellites so they show in the
        // galaxy. Dangling satellite ids are pruned later in filter_peer_data.
        for asteroid in asteroids {
            if asteroid.peer_id.is_none() {
                return Err(anyhow!(
                    "Cannot receive planet without peer_id over the network."
                ));
            }
            let satellite_of = asteroid
                .satellite_of
                .ok_or_else(|| anyhow!("Asteroid should have a parent planet"))?;
            if let Some(planet) = self.planets.get_mut(&satellite_of) {
                // A satellite's parent must be a local planet; refuse to attach to a peer one.
                if planet.peer_id.is_some() {
                    return Err(anyhow!(
                        "Cannot receive asteroid whose parent planet has a peer_id set."
                    ));
                }
                if !planet.satellites.contains(&asteroid.id) {
                    planet.satellites.push(asteroid.id);
                }
            }
            self.planets.insert(asteroid.id, asteroid);
        }

        if let Some(previous_version_team) = db_team.as_ref() {
            if let TeamLocation::OnPlanet { planet_id } = previous_version_team.current_location {
                // Remove team from previous planet
                let planet = self.planets.get_mut_or_err(&planet_id)?;
                planet.team_ids.retain(|&id| id != team.id);
            }

            // Remove players from db_team that are not in the new team to clean up fired players
            for player_id in &previous_version_team.player_ids {
                if self
                    .players
                    .get(player_id)
                    .is_some_and(|player| player.peer_id.is_none())
                {
                    continue;
                }

                self.players.remove(player_id);
            }
        }

        // Add team to new planet
        if let TeamLocation::OnPlanet { planet_id } = team.current_location {
            let planet = self.planets.get_mut_or_err(&planet_id)?;
            if !planet.team_ids.contains(&team.id) {
                planet.team_ids.push(team.id);
            }
        }

        for (_, player) in players {
            if player.peer_id.is_none() || player.peer_id.unwrap() != team.peer_id.unwrap() {
                return Err(anyhow!(
                    "Cannot receive player with wrong peer_id over the network."
                ));
            }
            // Our own state takes precedence over anything a peer says about our crew.
            if self
                .players
                .get(&player.id)
                .is_some_and(|ours| ours.team == Some(self.own_team_id))
            {
                continue;
            }
            let known = if team.listing(&player.id).is_some() {
                player.reputation.max(DOCK_LISTING_SCOUTING)
            } else {
                player.reputation
            };
            self.players_scouting
                .entry(player.id)
                .and_modify(|report| report.raise_scouting_to(known))
                .or_insert_with(|| ScoutReport::new(player.id, known));
            self.players.insert(player.id, player);
        }

        self.network_team_timestamps.insert(team.id, timestamp);
        self.teams.insert(team.id, team);
        self.dirty_ui = true;

        Ok(version_changed)
    }

    pub fn space_cove_on(&self, planet_id: PlanetId) -> Option<&SpaceCove> {
        self.team_with_cove_on(planet_id)?.space_cove.as_ref()
    }

    pub fn team_with_cove_on(&self, planet_id: PlanetId) -> Option<&Team> {
        self.teams
            .values()
            .find(|team| team.has_space_cove_on() == Some(planet_id))
    }

    pub fn listing_for(&self, player_id: &PlayerId) -> Option<&DockListing> {
        let team_id = self.players.get(player_id)?.team?;
        self.teams.get(&team_id)?.listing(player_id)
    }

    pub fn player_is_in_space_cove_on(&self, player: &Player) -> Option<PlanetId> {
        player
            .is_on_planet()
            .filter(|&id| self.space_cove_on(id).is_some())
    }

    pub fn upgrade_space_cove(&mut self, target: SpaceCoveUpgradeTarget) -> AppResult<()> {
        let own_team = self.teams.get_mut_or_err(&self.own_team_id)?;
        let cove = own_team
            .space_cove
            .as_mut()
            .ok_or(anyhow!("No space cove to upgrade"))?;

        cove.pending_upgrade = None;
        cove.upgrades.insert(target);
        if target == SpaceCoveUpgradeTarget::Tavern {
            cove.tavern = Some(Tavern::default());
        }
        own_team.version += 1;

        self.dirty = true;
        self.dirty_ui = true;
        self.dirty_network = true;

        Ok(())
    }

    pub fn get_own_team(&self) -> AppResult<&Team> {
        self.teams.get_or_err(&self.own_team_id)
    }

    pub fn get_own_team_mut(&mut self) -> AppResult<&mut Team> {
        self.teams.get_mut_or_err(&self.own_team_id)
    }

    pub fn get_game_players_by_team(players: &PlayerMap, team: &Team) -> AppResult<PlayerMap> {
        let mut team_players = PlayerMap::new();
        for player_id in team
            .active_player_ids()
            .into_iter()
            .take(MAX_PLAYERS_PER_GAME)
        {
            let mut player = players
                .get(&player_id)
                .ok_or_else(|| anyhow!("Player {player_id} not found."))?
                .clone();
            player.peer_id = team.peer_id;
            team_players.insert(player.id, player);
        }
        Ok(team_players)
    }

    pub fn team_rating(&self, team_id: &TeamId) -> AppResult<Skill> {
        let team = self.teams.get_or_err(team_id)?;
        let active = team.active_player_ids();
        if active.is_empty() {
            return Ok(MIN_SKILL);
        }
        Ok(active
            .iter()
            .filter_map(|id| self.players.get(id))
            .map(|player| player.average_skill())
            .sum::<Skill>()
            / active.len().max(MIN_PLAYERS_PER_GAME) as Skill)
    }

    pub fn is_simulating(&self) -> bool {
        if !self.has_own_team() {
            return false;
        }

        // This works if we assume that we can't lag behind more than a SHORT interval (1 second).
        // DEBUG_TIME_MULTIPLIER than cannot be too large or due to finite FPS this condition
        // would always return true.
        Tick::now() > self.last_tick_short_interval + TickInterval::SHORT
    }

    fn resources_found_after_exploration(
        &self,
        bonus: f32,
        planet: &Planet,
    ) -> AppResult<ResourceMap> {
        let mut rng = ChaCha8Rng::from_rng(&mut rand::rng());
        let mut resources = HashMap::new();

        for (&resource, &amount) in planet.resources.iter() {
            let mut found_amount = 0;
            // The exploration bonus makes the random range larger, which is positive in expectation
            // since we clamp at 0.
            let base = ((2.0_f32).powf(amount as f32 / 2.0) * bonus) as i32;
            for _ in 0..8 {
                found_amount += rng.random_range(-base / 2..base).max(0) as u32;
            }
            resources.insert(resource, found_amount);
        }

        Ok(resources)
    }

    fn free_pirates_found_after_exploration(
        &mut self,
        planet: &Planet,
        duration: Tick,
    ) -> AppResult<Vec<PlayerId>> {
        let rng = &mut ChaCha8Rng::from_rng(&mut rand::rng());
        let mut free_pirates = vec![];

        let duration_bonus = (duration as f32 / HOURS as f32).powf(1.3);
        let population_bonus = planet.total_population() as f32;

        let amount = rng
            .random_range((-32 + (population_bonus + duration_bonus) as i32).min(0)..3)
            .max(0);

        if amount > 0 {
            for _ in 0..amount {
                let base_level = Some(rng.random_range(0.0..7.0));
                let extra_potential = Some(0.5 + rng.random_range(0.0..2.0));
                let extra_scouting = Some(2.0 + rng.random_range(0.0..12.0));
                let population = planet.random_population(rng).unwrap_or_default();
                let player_id = self.generate_random_pirate(
                    rng,
                    population,
                    planet.id,
                    None,
                    base_level,
                    extra_potential,
                    extra_scouting,
                )?;

                free_pirates.push(player_id);
            }
        }

        Ok(free_pirates)
    }

    pub fn handle_fast_tick_events(&mut self, current_tick: Tick) -> AppResult<Vec<UiCallback>> {
        if let Some(adventure) = self.space_adventure.as_mut() {
            // deltatime is in seconds.
            let deltatime = (current_tick - self.last_tick_min_interval) as f32 / SECONDS as f32;
            self.last_tick_min_interval = current_tick;
            return adventure.update(deltatime);
        }

        Ok(vec![])
    }

    pub fn handle_slow_tick_events(&mut self, current_tick: Tick) -> AppResult<Vec<UiCallback>> {
        if !self.has_own_team() {
            return Ok(Vec::default());
        }

        let mut callbacks: Vec<UiCallback> = vec![];

        // Round down to keep it in sync across network.
        let current_tick = current_tick - current_tick % TickInterval::SHORT;

        if current_tick >= self.last_tick_short_interval + TickInterval::SHORT {
            self.tick_games(current_tick)?;
            callbacks.append(&mut self.tick_tournaments(current_tick)?);

            if let Some(cb) = self.cleanup_games(current_tick)? {
                callbacks.push(cb);
            }

            callbacks.append(&mut self.tick_travel(current_tick)?);

            if let Some(cb) = self.tick_collect_waiting_pirates(current_tick)? {
                callbacks.push(cb);
            }

            if let Some(callback) = self.tick_spaceship_upgrade(current_tick)? {
                callbacks.push(callback);
            }

            for cb in self.tick_asteroid_upgrade(current_tick)? {
                callbacks.push(cb);
            }

            if let Some(callback) = self.tick_space_cove_upgrade(current_tick)? {
                callbacks.push(callback);
            }

            if self.dirty {
                self.update_own_team_honours()?;
            }

            self.last_tick_short_interval += TickInterval::SHORT;
            // Round up to the TickInterval::SHORT to keep these ticks synchronous across network.
            self.last_tick_short_interval -= self.last_tick_short_interval % TickInterval::SHORT;
        }

        // FIXME: this workls only if we use it for medium intervals...
        let is_simulating = self.is_simulating();

        if current_tick >= self.last_tick_medium_interval + TickInterval::MEDIUM {
            self.tick_tiredness_recovery()?;

            for cb in self.tick_player_leaving_team_for_low_satisfaction(current_tick)? {
                callbacks.push(cb);
            }

            if !is_simulating {
                self.tick_team_position_assignment()?;
            }

            let num_local_games = self
                .games
                .values()
                .filter(|g| g.is_local())
                .collect_vec()
                .len();
            if num_local_games < AUTO_GENERATE_GAMES_NUMBER {
                self.generate_random_games(AUTO_GENERATE_GAMES_NUMBER - num_local_games)?;
            }

            for cb in self.tick_pirates_drinking()? {
                callbacks.push(cb);
            }

            // Once every MEDIUM interval, set dirty_network flag,
            // so that we send our team to the network.
            self.dirty_network = true;

            self.last_tick_medium_interval += TickInterval::MEDIUM;
        }

        if current_tick >= self.last_tick_long_interval + TickInterval::LONG {
            // Local teams try to make money :)
            self.tick_auto_handle_finances()?;
            if let Some(callback) = self.tick_players_salary()? {
                callbacks.push(callback);
            }
            self.tick_players_update();
            self.tick_players_at_dock()?;

            for cb in self.tick_player_retirement(current_tick)? {
                callbacks.push(cb);
            }

            self.tick_teams_reputation()?;
            self.tick_space_coves()?;

            // Local teams hire free pirates just before refreshing team,
            // so own team has had already time to hire them.
            self.tick_auto_hire_free_pirates(current_tick)?;

            // Create free pirates only if this is the last time window to do so.
            // This will run also during a simulation, but only once.
            if Tick::now() < current_tick + TickInterval::LONG {
                callbacks.push(self.tick_free_pirates(current_tick)?);
            }

            self.last_tick_long_interval += TickInterval::LONG;
        }

        Ok(callbacks)
    }

    fn cleanup_games(&mut self, current_tick: Tick) -> AppResult<Option<UiCallback>> {
        let mut own_team_game_notification = None;

        for game in self.games.values() {
            // In this loop we process ended games before they are cleaned up.
            if !game.has_ended() {
                continue;
            }

            log::debug!(
                "Game {} vs {}: started at {}, ended at {} and is being removed at {}",
                game.home_team_in_game.name,
                game.away_team_in_game.name,
                game.starting_at.formatted_as_time(),
                game.ended_at.unwrap().formatted_as_time(),
                current_tick.formatted_as_time()
            );

            // FIXME: Add this check once we add network local games.
            // We skip local games that involve a network team (where the local team also comes from the network).
            // Notice that in local games neither team has a peer_id (not even the own team).
            // if game.is_local_from_network()
            // {
            //     continue;
            // }

            let is_tournament_game = game.part_of_tournament.is_some();

            for team in [&game.home_team_in_game, &game.away_team_in_game] {
                //we do not apply end of game logic to peer teams
                //TODO: once we remove local teams, we can remove this loop and only apply to own_team
                if team.peer_id.is_some() && team.team_id != self.own_team_id {
                    continue;
                }

                for game_player in team.players.values() {
                    // Set tiredness and morale to the value in game.
                    // We do not clone the game_player as other changes may have occured to the player
                    // during the game (such as skill update).
                    let mut player = match self.players.get_or_err(&game_player.id) {
                        Ok(player) => player.clone(),
                        Err(e) => {
                            log::error!(
                                "Can't find player {} in world during game {} cleanup: {}",
                                game_player.id,
                                game.id,
                                e
                            );
                            continue;
                        }
                    };

                    player.tiredness = game_player.tiredness;
                    player.morale = game_player.morale;
                    player.drunkenness = game_player.drunkenness;

                    player.version += 1;
                    player.add_morale(MORALE_INCREASE_PER_GAME);

                    // Restore tiredness and morale, at least partially if it's a tournament game.
                    // FIXME: this shouldnt be applied if the tournament is over, but we dont know it yet here.
                    // if is_tournament_game {
                    //     player.add_morale(MORALE_INCREASE_AFTER_TOURNAMENT_GAME);
                    //     player.tiredness =
                    //         (player.tiredness - TIREDNESS_DECREASE_AFTER_TOURNAMENT_GAME).bound();
                    // }

                    let stats = team
                        .stats
                        .get(&player.id)
                        .ok_or_else(|| anyhow!("Player {:?} not found in team stats", player.id))?;

                    player.satisfy_opinion(PlayerOpinion::Games);

                    // Update player global stats, but remove position, shots and last action shot
                    player.historical_stats.update(stats);
                    player.historical_stats.position = None;
                    player.historical_stats.shots.clear();
                    player.historical_stats.last_action_shot = None;
                    player.historical_stats.extra_morale = 0.0;
                    player.historical_stats.extra_tiredness = 0.0;
                    // Add game to player historical stats
                    match game.winner {
                        Some(winner) => {
                            if winner == team.team_id {
                                player.historical_stats.games[0] += 1;
                            } else {
                                player.historical_stats.games[1] += 1;
                            }
                        }
                        None => {
                            player.historical_stats.games[2] += 1;
                        }
                    }
                    // Plus/minus is not updated automatically and must be updated by hand
                    player.historical_stats.plus_minus += stats.plus_minus;

                    player.reputation = (player.reputation
                        + REPUTATION_PER_EXPERIENCE
                            * stats.seconds_played as f32
                            * TeamBonus::Reputation.current_team_bonus(
                                &player.team.expect("Player should have a team"),
                                &self.teams,
                                &self.players,
                            )?)
                    .bound();
                    self.players_scouting
                        .entry(player.id)
                        .and_modify(|report| report.raise_scouting_to(player.reputation))
                        .or_insert_with(|| ScoutReport::new(player.id, player.reputation));

                    let mut training_bonus = TeamBonus::Training.current_team_bonus(
                        &team.team_id,
                        &self.teams,
                        &self.players,
                    )?;

                    if is_tournament_game {
                        training_bonus *= TOURNAMENT_GAME_TRAINING_BONUS_MODIFIER;
                    }

                    let training_focus = player.training_focus;
                    player.update_skills_training(
                        stats.experience_at_position,
                        training_bonus,
                        training_focus,
                    );
                    self.players.insert(player.id, player);
                }

                if let Some(world_team) = self.teams.get(&team.team_id) {
                    for player_id in world_team
                        .active_player_ids()
                        .iter()
                        .filter(|id| !team.players.contains_key(id))
                    {
                        if let Some(player) = self.players.get_mut(player_id) {
                            player.add_team_satisfaction(SATISFACTION_MALUS_FOR_SITTING_OUT);
                        }
                    }
                }
            }

            let game_summary = GameSummary::from_game(game);
            self.past_games.insert(game_summary.id, game_summary);
            self.recently_finished_games.insert(game.id, game.clone());

            // Special handling for own team games
            if game.home_team_in_game.team_id == self.own_team_id
                || game.away_team_in_game.team_id == self.own_team_id
            {
                // Past games of the own team are persisted in the store.
                save_game(game)?;
                // Update network that game has ended.
                self.dirty_network = true;

                let scouted_players = if game.home_team_in_game.team_id == self.own_team_id {
                    &game.away_team_in_game.players
                } else {
                    &game.home_team_in_game.players
                };

                // Scout opposing team players a bit
                let bonus = TeamBonus::Scouting.current_team_bonus(
                    &self.own_team_id,
                    &self.teams,
                    &self.players,
                )?;
                let increase = bonus * PLAYER_SCOUTING_PER_GAME;
                for (&player_id, player) in scouted_players.iter() {
                    self.players_scouting
                        .entry(player_id)
                        .and_modify(|report| report.add_scouting(increase))
                        .or_insert_with(|| {
                            ScoutReport::new(player_id, (player.reputation + increase).bound())
                        });
                }

                own_team_game_notification = Some(UiCallback::PushUiPopup {
                    popup_message: PopupMessage::Message {
                        message: format!(
                            "Game ended\n{} {}-{} {}",
                            game.home_team_in_game.name,
                            game.get_score().0,
                            game.get_score().1,
                            game.away_team_in_game.name,
                        ),
                        links: vec![(
                            "Game ended".to_string(),
                            UiCallback::GoToGame {
                                game_id: game.id,
                                from_popup: true,
                            },
                        )],
                        level: log::Level::Info,
                        is_skippable: false,
                        timestamp: current_tick,
                    },
                });
            }

            // Update tournament's copy of the game with the ended version,
            // so that network sync and generate_next_games see the correct state.
            if let Some(tournament_id) = game.part_of_tournament {
                if let Some(tournament) = self.tournaments.get_mut(&tournament_id) {
                    if let Some(t_game) = tournament.games.iter_mut().find(|g| g.id == game.id) {
                        *t_game = game.clone();
                    } else {
                        log::error!(
                            "Game {} should be in tournament {} games",
                            game.id,
                            tournament_id
                        );
                    }
                }
            }

            // Teams get money depending on game attendance.
            // If a team is knocked out, money goes to the other team.
            // If both are knocked out, they get no money.
            let mut home_team_income = Game::income(game.attendance);
            let mut away_team_income = Game::income(game.attendance);
            let home_knocked_out = game.is_team_knocked_out(Possession::Home);
            let away_knocked_out = game.is_team_knocked_out(Possession::Away);

            match (home_knocked_out, away_knocked_out) {
                (true, false) => {
                    away_team_income += home_team_income;
                    home_team_income = 0;
                }
                (false, true) => {
                    home_team_income += away_team_income;
                    away_team_income = 0;
                }
                (true, true) => {
                    home_team_income = 0;
                    away_team_income = 0;
                }
                _ => {}
            }

            // Winner team gets 1 rum per player, Loser team gets 1 rum in total
            let (home_team_rum, away_team_rum) = match game.winner {
                Some(winner) => {
                    if winner == game.home_team_in_game.team_id {
                        (game.home_team_in_game.players.len() as u32, 1)
                    } else {
                        (1, game.away_team_in_game.players.len() as u32)
                    }
                }
                None => unreachable!("There should be a winner"),
            };

            // On top of the bonus, teams get back the rum brought to the game that was not drunk.
            // Note: the total is capped by the available storage, so bottles that no longer fit
            // (if storage filled up during the game) are lost.
            let home_team_rum = home_team_rum + game.home_team_in_game.rum;
            let away_team_rum = away_team_rum + game.away_team_in_game.rum;

            // Set playing teams current game to None and assign income, reputation, and rum.
            // Network games allow for not finding the team in the world.teams, local games don't
            let team_ids = [
                game.home_team_in_game.team_id,
                game.away_team_in_game.team_id,
            ];

            if game.is_network() {
                for (idx, team_id) in team_ids.iter().enumerate() {
                    // It is possible not to have a network team in the world.
                    let mut team = if let Some(team) = self.teams.get(team_id) {
                        team.clone()
                    } else {
                        continue;
                    };

                    let other_rating = if idx == 0 {
                        &game.away_team_in_game.network_game_rating
                    } else {
                        &game.home_team_in_game.network_game_rating
                    };

                    match game.winner {
                        Some(winner) => {
                            if winner == *team_id {
                                team.network_game_rating
                                    .update(GameResult::Win, other_rating);
                                team.reputation = (team.reputation
                                    + ReputationModifier::HIGH_BONUS
                                    + ReputationModifier::MEDIUM_BONUS)
                                    .bound();
                            } else {
                                team.network_game_rating
                                    .update(GameResult::Loss, other_rating);
                                team.reputation =
                                    (team.reputation + ReputationModifier::MEDIUM_MALUS).bound();
                            }
                        }
                        None => {
                            team.network_game_rating
                                .update(GameResult::Draw, other_rating);
                            team.reputation =
                                (team.reputation + ReputationModifier::MEDIUM_BONUS).bound()
                        }
                    }

                    team.current_game = None;
                    let income_bonus = if idx == 0 {
                        home_team_income
                    } else {
                        away_team_income
                    };
                    team.saturating_add_resource(Resource::SATOSHI, income_bonus);

                    let rum_bonus = if idx == 0 {
                        home_team_rum
                    } else {
                        away_team_rum
                    };
                    team.saturating_add_resource(Resource::RUM, rum_bonus);

                    self.teams.insert(team.id, team);
                }
            } else {
                // Snapshot both teams' pre-game ratings before updating either
                let home_local_rating = self
                    .teams
                    .get_or_err(&game.home_team_in_game.team_id)?
                    .local_game_rating
                    .clone();
                let away_local_rating = self
                    .teams
                    .get_or_err(&game.away_team_in_game.team_id)?
                    .local_game_rating
                    .clone();

                for (idx, team_id) in team_ids.iter().enumerate() {
                    let mut team = self.teams.get_or_err(team_id)?.clone();

                    let other_rating = if idx == 0 {
                        &away_local_rating
                    } else {
                        &home_local_rating
                    };

                    match game.winner {
                        Some(winner) => {
                            if winner == *team_id {
                                team.local_game_rating.update(GameResult::Win, other_rating);
                                team.reputation =
                                    (team.reputation + ReputationModifier::HIGH_BONUS).bound();
                            } else {
                                team.local_game_rating
                                    .update(GameResult::Loss, other_rating);
                                team.reputation =
                                    (team.reputation + ReputationModifier::MEDIUM_MALUS).bound();
                            }
                        }
                        None => {
                            team.local_game_rating
                                .update(GameResult::Draw, other_rating);
                            team.reputation =
                                (team.reputation + ReputationModifier::MEDIUM_BONUS).bound()
                        }
                    }

                    team.current_game = None;
                    let income_bonus = if idx == 0 {
                        home_team_income
                    } else {
                        away_team_income
                    };
                    team.saturating_add_resource(Resource::SATOSHI, income_bonus);

                    let rum_bonus = if idx == 0 {
                        home_team_rum
                    } else {
                        away_team_rum
                    };
                    team.saturating_add_resource(Resource::RUM, rum_bonus);

                    self.teams.insert(team.id, team);
                }
            }

            self.dirty = true;
            self.dirty_ui = true;
        }

        // We wait an extra GAME_CLEANUP_TIME before removing the game from the games
        // collection so that we can leave it up for the UI to visualize.
        self.games.retain(|_, game| !game.has_ended());

        Ok(own_team_game_notification)
    }

    fn tick_games(&mut self, current_tick: Tick) -> AppResult<()> {
        // NOTE!!: we do not set the world to dirty so we don't save on every tick.
        //         the idea is that the game is completely determined at the beginning,
        //         so we can similuate it through.
        for game in self.games.values_mut() {
            if game.has_started(current_tick) {
                game.catch_up(Tick::now());
            }
        }
        Ok(())
    }

    fn tick_tournaments(&mut self, current_tick: Tick) -> AppResult<Vec<UiCallback>> {
        let mut callbacks = vec![];
        let mut new_games = vec![];
        for (&tournament_id, tournament) in self.tournaments.iter_mut() {
            match tournament.state(current_tick) {
                TournamentState::Canceled => {}
                TournamentState::Registration => {}
                TournamentState::Confirmation => {
                    // Append callback to send Confirmation.
                    // If we are simulating, abort tournament.
                    if tournament.organizer_id == self.own_team_id {
                        if tournament.registered_teams.len() < 2 {
                            let error_message = format!(
                                "Insufficient registered teams ({}).",
                                tournament.registered_teams.len()
                            );
                            log::warn!("Canceling tournament {tournament_id}: {error_message}");
                            callbacks.push(UiCallback::CancelTournament {
                                tournament_id,
                                error_message,
                            });
                            continue;
                        }

                        callbacks.push(UiCallback::ConfirmTournamentParticipants { tournament_id });
                    }
                }
                TournamentState::Syncing => {
                    if tournament.organizer_id == self.own_team_id {
                        if tournament.participants.len() < 2 {
                            let error_message = format!(
                                "Insufficient participants ({}).",
                                tournament.participants.len()
                            );
                            log::warn!("Canceling tournament {tournament_id}: {error_message}");
                            callbacks.push(UiCallback::CancelTournament {
                                tournament_id,
                                error_message,
                            });
                            continue;
                        }

                        if !tournament.is_initialized() {
                            new_games.append(&mut tournament.initialize());
                        }
                        callbacks.push(UiCallback::SendInitializedTournament { tournament_id });
                    }
                }
                TournamentState::Started => {
                    // This can still happen if the client did not receive the tournament cancellation.
                    // In that case however, the tournament will not be initialized.
                    if !tournament.is_initialized() {
                        let error_message = "Tournament not initialized.".to_string();
                        log::warn!("Canceling tournament {tournament_id}: {error_message}");
                        callbacks.push(UiCallback::CancelTournament {
                            tournament_id,
                            error_message,
                        });
                        continue;
                    }

                    // FIXME: this is not very robust. For instance, it relies on retaining all
                    // tournament games when storing, otherwise the hashmap would be incorrect.
                    if tournament.is_team_participating(&self.own_team_id) {
                        // If team is participating, it should have all the necessary games stored.

                        match tournament.generate_next_games(
                            current_tick,
                            &self.games,
                            &self.past_games,
                        ) {
                            Ok(mut t_games) => new_games.append(&mut t_games),
                            Err(err) => {
                                let error_message =
                                    format!("Error while simulating tournament: {err}.");
                                log::warn!("{error_message}");
                                callbacks.push(UiCallback::CancelTournament {
                                    tournament_id,
                                    error_message,
                                });
                                continue;
                            }
                        }
                    }
                    // If we receive a valid initialized tournament from the network where the team is not participating,
                    // the next games cannot be generated correctly as the world does not contain the games.
                    // In this case, we just accept the error.
                    else {
                        match tournament.generate_next_games(
                            current_tick,
                            &self.games,
                            &self.past_games,
                        ) {
                            Ok(mut t_games) => new_games.append(&mut t_games),
                            Err(err) => {
                                let error_message =
                                    format!("Error while simulating network tournament: {err}.");
                                log::warn!("{error_message}");
                                callbacks.push(UiCallback::CancelTournament {
                                    tournament_id,
                                    error_message,
                                });
                                continue;
                            }
                        }
                    }

                    if tournament.has_ended() {
                        let message = format!(
                            "{} has won the tournament. Congrats!",
                            tournament
                                .winner
                                .map(|id| tournament
                                    .participants
                                    .get(&id)
                                    .expect("Winner should be a participant")
                                    .name
                                    .as_str())
                                .expect("Tournament should have a winner")
                        );

                        let callback = UiCallback::PushUiPopup {
                            popup_message: PopupMessage::Message {
                                message,
                                links: vec![(
                                    "tournament".to_string(),
                                    UiCallback::GoToTournaments { from_popup: true },
                                )],
                                level: log::Level::Info,
                                is_skippable: false,
                                timestamp: current_tick,
                            },
                        };
                        callbacks.push(callback);
                    }
                }

                TournamentState::Ended => {
                    unreachable!(
                        "Tournament ended, it should have been removed from the world state."
                    )
                }
            }
        }

        for game in new_games {
            let team_ids = [
                game.home_team_in_game.team_id,
                game.away_team_in_game.team_id,
            ];
            let initial_rums = [
                game.home_team_in_game.initial_rum,
                game.away_team_in_game.initial_rum,
            ];
            for (team_id, initial_rum) in team_ids.iter().zip(initial_rums) {
                let team = if let Some(team) = self.teams.get_mut(team_id) {
                    team
                } else {
                    continue;
                };

                team.enter_game(game.id, initial_rum);
                if team.id == self.own_team_id {
                    self.dirty_network = true
                }
            }
            self.games.insert(game.id, game);
        }

        for tournament in self.tournaments.values() {
            if tournament.is_canceled() {
                continue;
            }

            if tournament.has_ended() {
                if tournament.is_team_participating(&self.own_team_id) {
                    save_tournament(tournament)?;
                }

                self.past_tournaments.insert(
                    tournament.id,
                    TournamentSummary::from_tournament(tournament),
                );

                for team_id in tournament.participants.keys() {
                    let team = if let Some(team) = self.teams.get_mut(team_id) {
                        team
                    } else {
                        continue;
                    };

                    team.tournament_registration_state = TournamentRegistrationState::None;
                    if matches!(team.is_organizing_tournament, Some(id) if id == tournament.id) {
                        team.is_organizing_tournament = None;
                    }
                }

                if let Some(winner) = tournament.winner.as_ref() {
                    if let Some(team) = self.teams.get_mut(winner) {
                        team.tournaments_won.push(tournament.id);
                    }
                }
            }
        }

        self.canceled_tournaments.extend(
            self.tournaments
                .values()
                .filter(|t| t.is_canceled())
                .map(|t| t.id),
        );
        self.tournaments
            .retain(|_, t| !t.has_ended() && !t.is_canceled());

        let own_team = self.teams.get_mut_or_err(&self.own_team_id)?;
        if let Some(tournament_id) = own_team.is_organizing_tournament {
            if !self.tournaments.contains_key(&tournament_id) {
                own_team.is_organizing_tournament = None;
            }
        }

        // FIXME: The following check should not be necessary, but there are still bugs and it is convenient.
        if let Some(tournament_id) = own_team.committed_to_tournament() {
            if !self.tournaments.contains_key(&tournament_id) {
                own_team.tournament_registration_state = TournamentRegistrationState::None;
            }
        }

        Ok(callbacks)
    }

    fn team_reputation_bonus_per_distance(distance: KILOMETER) -> f32 {
        ((distance as f32 + 1.0).ln()).powf(4.0) * ReputationModifier::BONUS_PER_DISTANCE
    }

    pub fn tick_travel(&mut self, current_tick: Tick) -> AppResult<Vec<UiCallback>> {
        let own_team = self
            .teams
            .get_mut(&self.own_team_id)
            .ok_or_else(|| anyhow!("Could not find own team."))?;
        match own_team.current_location {
            TeamLocation::Travelling {
                from: _,
                to,
                started,
                duration,
                distance,
            } => {
                if current_tick > started + duration {
                    own_team.current_location = TeamLocation::OnPlanet { planet_id: to };
                    let planet = self
                        .planets
                        .get_mut(&to)
                        .ok_or_else(|| anyhow!("Could not find planet {to}."))?;
                    planet.team_ids.push(own_team.id);

                    let team_name = own_team.name.clone();
                    let planet_name = planet.name.clone();
                    let planet_filename = planet.filename.clone();
                    let planet_type = planet.planet_type;

                    for player_id in own_team.active_player_ids().iter() {
                        let player = self
                            .players
                            .get_mut(player_id)
                            .ok_or_else(|| anyhow!("Could not find player {player_id}."))?;
                        player.set_jersey(&own_team.jersey);
                    }

                    // Increase team reputation based on the travel distance if the team didn't use a teleport pad.
                    let is_teleporting = duration == TELEPORT_TRAVEL_DURATION;
                    // Note: if the team switches a pilot at the last moment, they lose this bonus as the duration is reset
                    // and is_using_portal would be true.
                    let is_using_portal = duration <= PORTAL_TRAVEL_DURATION;
                    if !is_teleporting && !is_using_portal {
                        own_team.total_travelled += distance;
                        let reputation_bonus = Self::team_reputation_bonus_per_distance(distance);
                        own_team.reputation = (own_team.reputation + reputation_bonus).bound();
                        for player_id in own_team.active_player_ids().iter() {
                            let player = self.players.get_mut_or_err(player_id)?;
                            player.satisfy_opinion(PlayerOpinion::Space);
                        }
                    }

                    self.dirty = true;
                    self.dirty_network = true;
                    self.dirty_ui = true;
                    return Ok(vec![UiCallback::PushUiPopup {
                        popup_message: PopupMessage::TeamLanded {
                            team_name,
                            planet_name,
                            planet_filename,
                            planet_type,
                            timestamp: current_tick,
                        },
                    }]);
                }
            }
            TeamLocation::Exploring {
                around,
                started,
                duration,
            } => {
                if current_tick > started + duration {
                    let mut team = own_team.clone();
                    let mut callbacks = vec![];

                    for player in team.active_player_ids().iter() {
                        let player = self.players.get_mut_or_err(player)?;
                        player.set_jersey(&team.jersey);
                    }
                    let mut around_planet = self.planets.get_or_err(&around)?.clone();
                    team.current_location = TeamLocation::OnPlanet { planet_id: around };

                    let mut rng = ChaCha8Rng::from_rng(&mut rand::rng());

                    // If team has already MAX_NUM_ASTEROID_PER_TEAM, it cannot find another one.
                    // Finding asteroids becomes progressively more difficult.
                    let team_asteroid_modifier =
                        (MAX_NUM_ASTEROID_PER_TEAM.saturating_sub(team.asteroid_ids.len()) as f64)
                            / MAX_NUM_ASTEROID_PER_TEAM as f64;

                    let asteroid_discovery_probability = (ASTEROID_DISCOVERY_PROBABILITY
                        * around_planet.asteroid_probability
                        * team_asteroid_modifier)
                        .min(1.0);

                    if asteroid_discovery_probability > 0.0
                        && rng.random_bool(asteroid_discovery_probability)
                    {
                        // We have temporarily set the team back on the exploration base planet,
                        // until the asteroid is accepted and generated.
                        callbacks.push(UiCallback::PushUiPopup {
                            popup_message: PopupMessage::AsteroidNameDialog {
                                timestamp: current_tick,
                                asteroid_type: rng.random_range(0..30),
                            },
                        });
                    }

                    around_planet.team_ids.push(team.id);

                    let bonus = TeamBonus::Scouting.current_team_bonus(
                        &team.id,
                        &self.teams,
                        &self.players,
                    )?;
                    let found_resources =
                        self.resources_found_after_exploration(bonus, &around_planet)?;

                    let mut collected_resources = ResourceMap::new();
                    // Try to add resources starting from the most expensive one,
                    // but still trying to add the others if they fit (notice that resources occupy a different amount of space).
                    for (&resource, &amount) in found_resources
                        .iter()
                        .sorted_by(|(a, _), (b, _)| b.base_price().total_cmp(&a.base_price()))
                    {
                        let storable_amount = if resource == Resource::FUEL {
                            let available_capacity = team.available_fuel_capacity();
                            // One fuel unit occupies one capacity
                            available_capacity.min(amount)
                        } else {
                            let available_capacity = team.available_storage_capacity();
                            (available_capacity / resource.to_storing_space()).min(amount)
                        };

                        if storable_amount > 0 {
                            // This reduces available_capacity for the next resource
                            team.add_resource(resource, storable_amount)?;
                        }
                        collected_resources.insert(resource, storable_amount);
                    }

                    let found_pirates = self
                        .free_pirates_found_after_exploration(&around_planet, duration)?
                        .iter()
                        .map(|&player_id| {
                            self.players
                                .get_or_err(&player_id)
                                .expect("Player should be part of world")
                                .clone()
                        })
                        .collect_vec();

                    for player_id in team.active_player_ids().iter() {
                        let player = self.players.get_mut_or_err(player_id)?;
                        player.satisfy_opinion(PlayerOpinion::Space);
                    }

                    callbacks.push(UiCallback::PushUiPopup {
                        popup_message: PopupMessage::ExplorationResult {
                            planet_name: around_planet.name.clone(),
                            resources: collected_resources,
                            players: found_pirates,
                            timestamp: current_tick,
                        },
                    });

                    self.planets.insert(around_planet.id, around_planet);
                    self.teams.insert(team.id, team);

                    self.dirty = true;
                    self.dirty_network = true;
                    self.dirty_ui = true;

                    return Ok(callbacks);
                }
            }
            _ => {}
        }
        Ok(vec![])
    }

    fn tick_spaceship_upgrade(&self, current_tick: Tick) -> AppResult<Option<UiCallback>> {
        let own_team = self.get_own_team()?;
        if let Some(upgrade) = own_team.spaceship.pending_upgrade {
            if current_tick > upgrade.started + upgrade.duration {
                return Ok(Some(UiCallback::UpgradeSpaceship { upgrade }));
            }
        }
        Ok(None)
    }

    fn tick_asteroid_upgrade(&self, current_tick: Tick) -> AppResult<Vec<UiCallback>> {
        let own_team = self.get_own_team()?;
        let mut callbacks = vec![];
        for asteroid_id in own_team.asteroid_ids.iter() {
            let asteroid = self.planets.get_or_err(asteroid_id)?;
            if let Some(upgrade) = asteroid.pending_upgrade {
                if current_tick > upgrade.started + upgrade.duration {
                    callbacks.push(UiCallback::UpgradeAsteroid {
                        asteroid_id: *asteroid_id,
                        upgrade,
                    });
                }
            }
        }
        Ok(callbacks)
    }

    fn tick_space_cove_upgrade(&self, current_tick: Tick) -> AppResult<Option<UiCallback>> {
        let own_team = self.get_own_team()?;
        if let Some(cove) = own_team.space_cove.as_ref() {
            if let Some(upgrade) = cove.pending_upgrade {
                if current_tick > upgrade.started + upgrade.duration {
                    return Ok(Some(UiCallback::UpgradeSpaceCove {
                        target: upgrade.target,
                    }));
                }
            }
        }
        Ok(None)
    }

    fn tick_pirates_drinking(&mut self) -> AppResult<Vec<UiCallback>> {
        let rng = &mut ChaCha8Rng::from_rng(&mut rand::rng());
        let mut callbacks = vec![];
        let teams = self
            .teams
            .values()
            .filter(|team| {
                team.current_game.is_none()
                    && team.playing_in_tournament().is_none()
                    && team.peer_id.is_none()
            })
            .collect::<Vec<&Team>>();
        for team in teams {
            // `weights` and the sampled index must index the same vec: bind the
            // active crew once rather than deriving each from `player_ids`.
            let drinkers = team.active_player_ids();
            let weights: Vec<f64> = drinkers
                .iter()
                .map(|id| {
                    let Some(player) = self.players.get(id) else {
                        return 0.0;
                    };
                    if player.can_drink(&self.teams).is_err() {
                        return 0.0;
                    }
                    BASE_TEAM_DRINKING_PROBABILITY
                        * (1.0 + player.opinions.modifier(PlayerOpinion::Drinking) as f64)
                })
                .collect();

            let team_drinks_probability: f64 = weights.iter().sum();
            if rng.random_bool(team_drinks_probability.clamp(0.0, 1.0)) {
                if let Ok(distribution) = WeightedIndex::new(&weights) {
                    let drinker_idx = distribution.sample(rng);
                    let player_id = drinkers[drinker_idx];
                    callbacks.push(UiCallback::Drink { player_id });
                }
            }
        }
        Ok(callbacks)
    }

    fn tick_tiredness_recovery(&mut self) -> AppResult<()> {
        let teams = self
            .teams
            .values()
            .filter(|team| {
                team.current_game.is_none()
                    && team.playing_in_tournament().is_none()
                    && team.peer_id.is_none()
            })
            .collect::<Vec<&Team>>();

        for team in teams {
            let bonus = TeamBonus::TirednessRecovery.current_team_bonus(
                &team.id,
                &self.teams,
                &self.players,
            )?;
            for player_id in team.player_ids.iter() {
                let player = if let Some(player) = self.players.get_mut(player_id) {
                    player
                } else {
                    continue;
                };

                // Negative drunkenness means that the player is drunk.
                if player.drunkenness < 0.0 {
                    player.drunkenness = (player.drunkenness
                        + bonus * RECOVERING_DRUNKENNESS_PER_SHORT_TICK)
                        .min(0.0);
                } else if player.drunkenness > 0.0 {
                    player.drunkenness = (player.drunkenness
                        - bonus * RECOVERING_DRUNKENNESS_PER_SHORT_TICK)
                        .bound();
                }

                // While drunk, tiredness is not recovered, until drunkenness is 0.0.
                if player.tiredness > 0.0 && player.drunkenness >= 0.0 {
                    // Recovery outside of games is slower by a factor TICK_SHORT_INTERVAL/TICK_MEDIUM_INTERVAL
                    // so that it takes 1 minute * 10 * 100 ~ 18 hours to recover from 100% tiredness.
                    player.tiredness =
                        (player.tiredness - bonus * RECOVERING_TIREDNESS_PER_SHORT_TICK).bound();
                }
            }
        }

        Ok(())
    }

    fn tick_team_position_assignment(&mut self) -> AppResult<()> {
        //TODO: once we remove local teams, we can completely remove this function
        for team in self.teams.values_mut() {
            if team.peer_id.is_some() {
                continue;
            }

            if team.id == self.own_team_id {
                continue;
            }

            if team.current_game.is_some() {
                continue;
            }

            if team.is_on_planet().is_none() {
                continue;
            }

            team.reassign_positions(&self.players);

            let rng = &mut ChaCha8Rng::from_rng(&mut rand::rng());
            team.game_tactic = Tactic::random(rng);
        }

        Ok(())
    }

    fn tick_free_pirates(&mut self, current_tick: Tick) -> AppResult<UiCallback> {
        // Remove old unhired free pirates
        self.players.retain(|_, player| player.team.is_some());
        // Clean up scouting for unreachable pirates
        self.players_scouting
            .retain(|k, _| self.players.contains_key(k));

        let rng = &mut ChaCha8Rng::seed_from_u64(rand::random());

        for planet in PLANET_DATA.iter() {
            let extra_scouting = Some(4.0 + rng.random_range(0.0..4.0));
            self.populate_planet(rng, planet, None, extra_scouting)?;
        }

        let cove_asteroids: Vec<Planet> = self
            .teams
            .values()
            .filter(|team| team.peer_id.is_none())
            .filter_map(|team| team.space_cove.as_ref())
            .filter(|cove| cove.is_ready())
            .filter_map(|cove| self.planets.get(&cove.planet_id).cloned())
            .collect();

        let max_extra_potential = self
            .get_own_team()
            .ok()
            .map(|t| t.reputation / 5.0)
            .unwrap_or_default();
        let extra_potential = Some(0.25 + rng.random_range(0.0..max_extra_potential + 0.01));
        let extra_scouting = Some(6.0 + rng.random_range(0.0..8.0));
        for asteroid in &cove_asteroids {
            self.populate_planet(rng, asteroid, extra_potential, extra_scouting)?;
        }

        Ok(UiCallback::PushUiPopup {
            popup_message: PopupMessage::Message {
                message: "Free pirates refreshed".into(),
                links: vec![(
                    "Free pirates".to_string(),
                    UiCallback::GoToFreePirates { from_popup: true },
                )],
                level: log::Level::Info,
                is_skippable: false,
                timestamp: current_tick,
            },
        })
    }

    fn tick_auto_handle_finances(&mut self) -> AppResult<()> {
        let team_ids = self.teams.keys().copied().collect_vec();
        for team_id in team_ids {
            if team_id == self.own_team_id {
                continue;
            }

            let team = self.teams.get_or_err(&team_id)?;
            if team.current_game.is_some() {
                continue;
            }

            if let Some(planet_id) = team.is_on_planet() {
                let planet = self.planets.get_or_err(&planet_id)?;
                if !planet.has_market(None) {
                    continue;
                }

                let merchant_bonus = TeamBonus::Bargaining.current_team_bonus(
                    &team_id,
                    &self.teams,
                    &self.players,
                )?;
                let sell_unit_cost = planet.resource_sell_price(Resource::RUM, merchant_bonus);

                let team = self.teams.get_mut(&team_id).expect("team should exist");
                let amount = team
                    .resources
                    .value(&Resource::RUM)
                    .saturating_sub(team.active_players_count() as u32 * 2);
                if amount > 0 && team.can_sell_resource(Resource::RUM, amount).is_ok() {
                    team.sub_resource(Resource::RUM, amount)?;
                    team.add_resource(Resource::SATOSHI, sell_unit_cost * amount)?;
                }
            } else {
                continue;
            }
        }

        Ok(())
    }

    fn tick_auto_hire_free_pirates(&mut self, current_tick: Tick) -> AppResult<()> {
        const SALARY_MULTIPLIER_FOR_DECISION: u32 = 5;
        let free_pirates = self
            .players
            .values()
            .filter(|p| p.team.is_none() && p.peer_id.is_none())
            .collect_vec()
            .sort_by_rating();

        // Free pirates standing in a space cove can only be hired by that cove's team.
        let pirate_cove_planet: HashMap<PlayerId, PlanetId> = free_pirates
            .iter()
            .filter_map(|&p| {
                self.player_is_in_space_cove_on(p)
                    .map(|planet_id| (p.id, planet_id))
            })
            .collect();

        let mut released_player_ids: Vec<PlayerId> = vec![];
        let mut hired_player_ids: Vec<PlayerId> = vec![];
        let mut hiring_team_ids: Vec<TeamId> = vec![];

        for (&team_id, team) in self.teams.iter() {
            if team_id == self.own_team_id {
                continue;
            }

            if team.is_on_planet().is_none() {
                continue;
            }

            if team.current_game.is_some() {
                continue;
            }

            if team.balance() < SALARY_MULTIPLIER_FOR_DECISION * team.total_salary(&self.players) {
                // Remove most expensive player if enough players
                if team.active_players_count() > MIN_PLAYERS_PER_GAME {
                    let pirates = team
                        .active_player_ids()
                        .iter()
                        .filter_map(|id| self.players.get(id))
                        .collect::<Vec<_>>();
                    if let Some(most_expensive_pirate) = pirates.sort_by_salary().first() {
                        released_player_ids.push(most_expensive_pirate.id);
                    }
                }
                continue;
            }

            let available_free_pirates = free_pirates
                .iter()
                .filter(|&player| {
                    !hired_player_ids.contains(&player.id)
                        && team.can_consider_hiring_player(player).is_ok()
                        && team.balance() >= (team.total_salary(&self.players) + player.salary() )* SALARY_MULTIPLIER_FOR_DECISION // buffer to ensure team can afford player
                        && team.is_on_player_planet(player)
                        && team
                            .can_hire_from_space_cove(pirate_cove_planet.get(&player.id).copied())
                })
                .collect_vec();

            if available_free_pirates.is_empty() {
                continue;
            }

            // Hire as many pirates are needed to reach crew capacity
            let max_hirable_pirates = {
                let mut n = 0;
                let mut running_cost = 0;

                // Loop and keep candidates until the team is out of balance
                for pirate in available_free_pirates.iter() {
                    running_cost += pirate.hire_cost();
                    if running_cost > team.balance() {
                        break;
                    }
                    n += 1;
                }

                // If team is at capactiy we still want to try to hire one (but only 1) pirate.
                let free_spots =
                    (team.spaceship.crew_capacity() as usize).saturating_sub(team.player_ids.len());
                n = n.min(free_spots).max(1);

                n
            };

            let candidates = available_free_pirates
                .iter()
                .take(max_hirable_pirates)
                .collect_vec();

            if team.player_ids.len() == team.spaceship.crew_capacity() as usize {
                // If the team is at capacity, it definetely had at least MIN_PLAYERS_PER_GAME.
                assert!(candidates.len() <= 1);
                // Check if weakest pirate is worse than best free pirate.
                // If not, continue.
                let pirates = team
                    .active_player_ids()
                    .iter()
                    .filter_map(|id| self.players.get(id))
                    .collect::<Vec<_>>();
                if let Some(worst_pirate) = pirates.sort_by_rating().last() {
                    let best_pirate = candidates[0];
                    if worst_pirate.rating() >= best_pirate.rating() {
                        continue;
                    }
                    released_player_ids.push(worst_pirate.id);
                }
            }

            for player in candidates {
                hired_player_ids.push(player.id);
                hiring_team_ids.push(team.id);
            }
        }

        assert!(hired_player_ids.len() == hiring_team_ids.len());

        for &player_id in released_player_ids.iter() {
            self.release_player_from_team(player_id, true)?;
        }

        for idx in 0..hired_player_ids.len() {
            let player_id = hired_player_ids[idx];
            let team_id = hiring_team_ids[idx];
            self.hire_player_for_team(&player_id, &team_id, current_tick)?;
        }

        Ok(())
    }

    fn tick_players_salary(&mut self) -> AppResult<Option<UiCallback>> {
        let mut callback = None;
        let teams = self
            .teams
            .values_mut()
            .filter(|team| team.peer_id.is_none());
        for team in teams {
            let total_salary = team.total_salary(&self.players);
            let balance = team.balance();

            if balance >= total_salary {
                team.sub_resource(Resource::SATOSHI, total_salary)?;
                for player_id in team.player_ids.iter() {
                    let player = self.players.get_mut_or_err(player_id)?;
                    player.satisfy_opinion(PlayerOpinion::Gold); // Gold opinion is always positive
                }
            } else if team.id == self.own_team_id {
                let delta_modifier = (total_salary - balance) as f32 / total_salary as f32;
                team.sub_resource(Resource::SATOSHI, balance)?;
                for player_id in team.player_ids.iter() {
                    let player = self.players.get_mut_or_err(player_id)?;
                    player
                        .add_team_satisfaction(SATISFACTION_MALUS_UNPAID_SALARIES * delta_modifier);
                }

                callback = Some(UiCallback::PushUiPopup {
                    popup_message: PopupMessage::Message {
                        message: "You couldn't pay the full salaries, pirates are getting angry..."
                            .to_string(),
                        links: vec![],
                        level: log::Level::Warn,
                        is_skippable: true,
                        timestamp: Tick::now(),
                    },
                });
            } else {
                // FIXME: i wish we didnt need this, but local teams keep losing pirates, we cant afford that for now
                log::warn!(
                    "{} did not have enough money to keep up with the salaries",
                    team.name
                );
            }
        }

        Ok(callback)
    }

    fn tick_collect_waiting_pirates(
        &mut self,
        current_tick: Tick,
    ) -> AppResult<Option<UiCallback>> {
        let own_team = self.get_own_team()?;
        if own_team.waiting_at_dock.is_empty() || !own_team.is_at_dock() {
            return Ok(None);
        }

        let mut team = own_team.clone();
        let names = team
            .waiting_at_dock
            .iter()
            .filter_map(|id| self.players.get(id))
            .map(|player| player.info.short_name())
            .join(", ");
        team.waiting_at_dock.clear();
        team.reassign_positions(&self.players);
        team.version += 1;
        self.teams.insert(team.id, team);

        self.dirty = true;
        self.dirty_network = true;
        self.dirty_ui = true;

        Ok(Some(UiCallback::PushUiPopup {
            popup_message: PopupMessage::Message {
                message: format!("{names} came aboard from the dock."),
                links: vec![],
                level: log::Level::Info,
                is_skippable: true,
                timestamp: current_tick,
            },
        }))
    }

    fn tick_players_at_dock(&mut self) -> AppResult<()> {
        let listed: Vec<PlayerId> = self
            .teams
            .values()
            .filter(|team| team.peer_id.is_none())
            .flat_map(|team| team.listed_player_ids())
            .collect();

        for player_id in listed {
            if let Some(player) = self.players.get_mut(&player_id) {
                player.add_team_satisfaction(SATISFACTION_MALUS_PER_LONG_TICK_AT_DOCK);
            }
        }

        Ok(())
    }

    fn tick_players_update(&mut self) {
        let rng = &mut ChaCha8Rng::from_rng(&mut rand::rng());
        for (_, player) in self.players.iter_mut() {
            //TODO: once we remove local teams, we can remove this loop and only apply to own_team
            if player.peer_id.is_some() {
                continue;
            }
            player.version += 1;
            // Reset player improvements for UI.
            let current = player.current_skill_array();
            for i in 0..player.previous_skills.len() {
                player.previous_skills[i] = TREND_SMOOTHING * current[i]
                    + (1.0 - TREND_SMOOTHING) * player.previous_skills[i];
            }

            for i in 0..NUM_GAME_POSITIONS as usize {
                player.previous_game_position_fitness[i] = TREND_SMOOTHING
                    * player.game_position_fitness[i]
                    + (1.0 - TREND_SMOOTHING) * player.previous_game_position_fitness[i];
            }
            player.info.age += AGE_INCREASE_PER_LONG_TICK;

            if player.special_trait == Some(Trait::Crumiro) {
                player.skills_training = [0.0; 20];
                player.reputation = 0.0; //fuck crumiris!
                continue;
            }

            // Pirates slightly dislike being part of a team.
            player.add_team_satisfaction(SATISFACTION_DECREASE_PER_LONG_TICK);
            player.reputation = (player.reputation + REPUTATION_DECREASE_PER_LONG_TICK).bound();

            // All game position fitness have decrease by a small amount
            // that depends on the total game fitness and the player intuition.
            //This is planned to counteract the effect of training by playing games.
            let malus = GAME_POSITION_DECREMENT_PER_LONG_TICK
                * ((player.game_position_fitness.iter().sum::<Skill>()
                    / player.game_position_fitness.len() as Skill
                    - 0.25 * player.mental.intuition)
                    / MAX_SKILL)
                    .bound();
            for p in 0..player.game_position_fitness.len() {
                if let Some(value) = player.game_position_fitness.get_mut(p) {
                    *value = (*value + malus).bound();

                    // Increase player game position from training
                    let bonus = player
                        .game_position_fitness_training
                        .get(p)
                        .copied()
                        .unwrap_or_default();
                    *value = (*value + bonus).bound()
                }
            }

            for idx in 0..player.skills_training.len() {
                // Reduce player skills. This is planned to counteract the effect of training by playing games.
                let age_modifier = player.age_modifier_to_skill_update(idx);
                player.modify_skill(idx, SKILL_DECREMENT_PER_LONG_TICK * age_modifier.bound());

                // Increase player skills from training
                let value = (rng.random_range(0.9..1.1) * player.skills_training[idx]).bound();
                player.modify_skill(idx, value);
            }
            player.game_position_fitness_training = [Skill::default(); NUM_GAME_POSITIONS as usize];
            player.skills_training = [Skill::default(); 20];
        }
    }

    fn tick_teams_reputation(&mut self) -> AppResult<()> {
        let mut reputation_update: Vec<(TeamId, f32)> = vec![];
        for (_, team) in self.teams.iter() {
            //TODO: once we remove local teams, we can remove this loop and only apply to own_team
            if team.peer_id.is_some() {
                continue;
            }
            let active = team.active_player_ids();
            let players_reputation = active
                .iter()
                .map(|id| {
                    if let Ok(player) = self.players.get_or_err(id) {
                        player.reputation
                    } else {
                        0.0
                    }
                })
                .sum::<f32>()
                / active.len().max(1) as f32;

            // If team reputation is smaller than players average reputation, it increases.
            // Otherwise, it decreases.
            let mut reputation_modifier =
                ((players_reputation - team.reputation) / MAX_SKILL) / 2.0;
            if reputation_modifier > 0.0 {
                reputation_modifier *= TeamBonus::Reputation.current_team_bonus(
                    &team.id,
                    &self.teams,
                    &self.players,
                )?;
            }
            let new_reputation = (team.reputation * (1.0 + reputation_modifier)).bound();
            let min_reputation = team.honours.len() as f32 * MIN_REPUTATION_PER_HONOUR;
            reputation_update.push((team.id, new_reputation.max(min_reputation)));
        }

        for (team_id, new_reputation) in reputation_update {
            let team = self.teams.get_mut_or_err(&team_id)?;
            team.reputation = new_reputation;
        }
        Ok(())
    }

    fn tick_space_coves(&mut self) -> AppResult<()> {
        let rng = &mut ChaCha8Rng::from_rng(&mut rand::rng());
        for (_, team) in self.teams.iter_mut() {
            //TODO: once we remove local teams, we can remove this loop and only apply to own_team
            if team.peer_id.is_some() {
                continue;
            }

            let cove = if let Some(c) = team.space_cove.as_mut() {
                c
            } else {
                continue;
            };

            if !cove.is_ready() {
                continue;
            }

            let asteroid = self.planets.get_or_err(&cove.planet_id)?;
            let parent_planet_populations = &self
                .planets
                .get_or_err(
                    &asteroid
                        .satellite_of
                        .ok_or(anyhow!("Asteroid {} should have a parent.", asteroid.id))?,
                )?
                .populations;

            let effective_rum = cove.consume_daily_rum();
            if let Some(tavern) = cove.tavern.as_ref() {
                let populations =
                    tavern.refresh_populations(parent_planet_populations, effective_rum, rng);

                let asteroid = self.planets.get_mut_or_err(&cove.planet_id)?;
                asteroid.populations = populations;
            }
        }

        Ok(())
    }

    fn tick_player_leaving_team_for_low_satisfaction(
        &mut self,
        current_tick: Tick,
    ) -> AppResult<Vec<UiCallback>> {
        let mut messages = vec![];

        let mut releasing_player_ids = vec![];

        for &player_id in self.players.keys() {
            let player = self.players.get_or_err(&player_id)?;

            if player.peer_id.is_some() {
                continue;
            }

            if player.team.is_none() {
                continue;
            }

            if player.special_trait == Some(Trait::Crumiro) {
                continue;
            }

            let team = self
                .teams
                .get_or_err(&player.team.expect("Player should have a team"))?;

            if team.can_release_player(player).is_err() {
                continue;
            }

            let rng = &mut ChaCha8Rng::from_rng(&mut rand::rng());
            let Some(satisfaction) = player.team_satisfaction() else {
                continue;
            };
            if satisfaction < SATISFACTION_THRESHOLD_FOR_LEAVING
                && rng.random_bool(
                    (1.0 - satisfaction / MAX_SKILL) as f64
                        * LEAVING_PROBABILITY_SATISFACTION_MODIFIER,
                )
            {
                releasing_player_ids.push(player_id);

                if player.team.expect("Team should be some") == self.own_team_id {
                    messages.push(UiCallback::PushUiPopup {
                        popup_message: PopupMessage::Message {
                            message: format!(
                                "{} {} left the crew!\n{} had enough...",
                                player.info.first_name,
                                player.info.last_name,
                                player.info.pronouns.as_subject(),
                            ),
                            links: vec![],
                            level: log::Level::Info,
                            is_skippable: false,
                            timestamp: current_tick,
                        },
                    })
                }
            }
        }

        for &player_id in releasing_player_ids.iter() {
            self.release_player_from_team(player_id, false)?;
        }

        if !releasing_player_ids.is_empty() {
            self.dirty = true;
            self.dirty_network = true;
            self.dirty_ui = true;
        }

        Ok(messages)
    }

    fn tick_player_retirement(&mut self, current_tick: Tick) -> AppResult<Vec<UiCallback>> {
        let mut messages = vec![];

        let mut releasing_player_ids = vec![];

        for &player_id in self.players.keys() {
            let player = self.players.get_or_err(&player_id)?;

            if player.peer_id.is_some() {
                continue;
            }

            if player.team.is_none() {
                continue;
            }

            if player.special_trait == Some(Trait::Crumiro) {
                continue;
            }

            let team = self
                .teams
                .get_or_err(&player.team.expect("Player should have a team"))?;

            if team.can_release_player(player).is_err() {
                continue;
            }

            let rng = &mut ChaCha8Rng::from_rng(&mut rand::rng());
            if player.info.relative_age() > MIN_RELATIVE_RETIREMENT_AGE {
                // Add extra check to avoid running rng call unnecessarily.
                if player.info.relative_age() > rng.random_range(MIN_RELATIVE_RETIREMENT_AGE..1.0) {
                    releasing_player_ids.push(player_id);

                    if player.team.expect("Team should be some") == self.own_team_id {
                        messages.push(UiCallback::PushUiPopup {
                            popup_message: PopupMessage::Message{
                                message:format!(
                                    "{} {} left the crew and retired to cultivate turnips\n{} {} been a great pirate...",
                                    player.info.first_name,
                                    player.info.last_name,
                                    player.info.pronouns.as_subject(),
                                    player.info.pronouns.to_have(),
                                ),
                                links: vec![],
                                level: log::Level::Info,
                                is_skippable:false,
                                timestamp:current_tick
                            },
                        })
                    }
                }
            }
        }

        for &player_id in releasing_player_ids.iter() {
            self.release_player_from_team(player_id, false)?;
        }

        if !releasing_player_ids.is_empty() {
            self.dirty = true;
            self.dirty_network = true;
            self.dirty_ui = true;
        }

        Ok(messages)
    }

    fn update_own_team_honours(&mut self) -> AppResult<()> {
        let own_team = self
            .teams
            .get_mut(&self.own_team_id)
            .ok_or_else(|| anyhow!("Team {:?} not found", self.own_team_id))?;

        for honour in Honour::iter() {
            if !own_team.honours.contains(&honour)
                && honour.conditions_met(own_team, &self.past_games, &self.players, &self.planets)
            {
                own_team.honours.insert(honour);
            }
        }

        Ok(())
    }

    fn generate_random_games(&mut self, num_games: usize) -> AppResult<()> {
        let rng = &mut ChaCha8Rng::from_rng(&mut rand::rng());

        // Phase 1: collect one matchup per eligible planet (immutable reads only).
        let mut matchups: Vec<(TeamId, TeamId)> = vec![];
        for planet in self.planets.values() {
            if planet.team_ids.len() < 2 {
                continue;
            }
            let candidate_teams = planet
                .team_ids
                .iter()
                .filter_map(|id| self.teams.get(id))
                .filter(|team| {
                    team.active_players_count() >= MIN_PLAYERS_PER_GAME
                        && team.current_game.is_none()
                        && team.autonomous_strategy.challenge_local
                        && team.peer_id.is_none()
                        && team.average_tiredness(self) <= MAX_AVG_TIREDNESS_PER_AUTO_GAME
                })
                .collect::<Vec<&Team>>();

            if candidate_teams.len() < 2 {
                continue;
            }

            let teams = candidate_teams.iter().sample(rng, 2);
            matchups.push((teams[0].id, teams[1].id));
        }

        matchups.shuffle(rng);

        // Phase 2: create games
        for (home_id, away_id) in matchups.iter().take(num_games) {
            let home_team_in_game = TeamInGame::from_team_id(home_id, &self.teams, &self.players)?;
            let away_team_in_game = TeamInGame::from_team_id(away_id, &self.teams, &self.players)?;

            if let Err(err) = self.generate_local_game(home_team_in_game, away_team_in_game) {
                log::error!("Error while generating local game: {err}");
            }
        }

        Ok(())
    }

    pub fn filter_peer_data(&mut self, peer_id: Option<PeerId>) -> AppResult<()> {
        let mut own_team = self.get_own_team()?.clone();
        let own_team_current_location = match own_team.current_location {
            TeamLocation::OnPlanet { planet_id } => Some(planet_id),
            TeamLocation::Exploring { around, .. } | TeamLocation::OnSpaceAdventure { around } => {
                Some(around)
            }
            TeamLocation::Travelling { to, .. } => Some(to),
        };

        if let Some(peer_id) = peer_id {
            // Filter all data that has a specific peer_id
            self.teams
                .retain(|_, team| !matches!(team.peer_id, Some(id) if id == peer_id));
            self.players
                .retain(|_, player| !matches!(player.peer_id, Some(id) if id == peer_id));

            // If team is on peer asteroid, dont filter it
            self.planets.retain(|_, planet| {
                !matches!(planet.peer_id, Some(id) if id == peer_id)
                    || matches!(own_team_current_location, Some(id) if id == planet.id)
            });

            self.tournaments.retain(|_, t| {
                (t.is_team_registered(&self.own_team_id) && !t.has_started(Tick::now()))
                    || t.is_team_participating(&self.own_team_id)
                    || t.organizer_id == self.own_team_id
            });

            self.games.retain(|_, game| {
                game.home_team_in_game.team_id == self.own_team_id
                    || game.away_team_in_game.team_id == self.own_team_id
                    || ((game.home_team_in_game.peer_id.is_none()
                        || game.home_team_in_game.peer_id.unwrap() != peer_id)
                        && (game.away_team_in_game.peer_id.is_none()
                            || game.away_team_in_game.peer_id.unwrap() != peer_id))
                    || game
                        .part_of_tournament
                        .map(|id| self.tournaments.contains_key(&id))
                        .unwrap_or_default()
            });
            own_team
                .sent_challenges
                .retain(|_, challenge| challenge.target_peer_id != peer_id);
            own_team
                .received_challenges
                .retain(|_, challenge| challenge.proposer_peer_id != peer_id);
            own_team
                .sent_trades
                .retain(|_, trade| trade.target_peer_id != peer_id);
            own_team
                .received_trades
                .retain(|_, trade| trade.proposer_peer_id != peer_id);
        } else {
            // Filter all data that has a peer_id (i.e. keep only local data)
            self.teams.retain(|_, team| team.peer_id.is_none());
            self.players.retain(|_, player| player.peer_id.is_none());
            self.planets.retain(|_, planet| {
                planet.peer_id.is_none()
                    || matches!(own_team_current_location, Some(id) if id == planet.id )
            });
            self.tournaments.retain(|_, t| {
                (t.is_team_registered(&self.own_team_id) && !t.has_started(Tick::now()))
                    || t.is_team_participating(&self.own_team_id)
                    || t.organizer_id == self.own_team_id
            });
            self.games.retain(|_, game| {
                game.home_team_in_game.team_id == self.own_team_id
                    || game.away_team_in_game.team_id == self.own_team_id
                    || game.is_local()
                    || game
                        .part_of_tournament
                        .map(|id| self.tournaments.contains_key(&id))
                        .unwrap_or_default()
            });
            own_team.clear_challenges();
            own_team.clear_trades();
        }

        self.teams.insert(own_team.id, own_team);

        // Drop dangling team_ids and satellite ids from each planet.
        let valid_planet_ids: HashSet<PlanetId> = self.planets.keys().copied().collect();
        for (_, planet) in self.planets.iter_mut() {
            planet
                .team_ids
                .retain(|&team_id| self.teams.contains_key(&team_id));
            planet.satellites.retain(|id| valid_planet_ids.contains(id));
        }

        // Set current game to None for teams playing a game not stored in games.
        for team in self.teams.values_mut() {
            if let Some(game_id) = team.current_game {
                if !self.games.contains_key(&game_id) {
                    team.current_game = None;
                }
            }
        }

        // Note: we explicitly do not clean up scouting, as this is local to the own world
        // and can persist data from network teams in the future.
        // FIXME: clean up scouting for past network team players.

        self.dirty = true;
        self.dirty_ui = true;
        Ok(())
    }

    pub fn reset_network_store_peers(&mut self) {
        self.network_store_data.reset_peers();
    }

    pub fn travel_duration_to_planet(&self, team_id: TeamId, to_id: PlanetId) -> AppResult<Tick> {
        let team = self.teams.get_or_err(&team_id)?;

        // Travelling back to planet with teleportation pad is istantaneous.
        let to = self.planets.get_or_err(&to_id)?;
        if team.can_teleport_to(to).is_ok() {
            return Ok(TELEPORT_TRAVEL_DURATION);
        }

        let from_id = match team.current_location {
            TeamLocation::OnPlanet { planet_id } => planet_id,
            TeamLocation::Travelling { .. } => return Err(anyhow!("Team is travelling")),
            TeamLocation::Exploring { .. } => return Err(anyhow!("Team is exploring")),
            TeamLocation::OnSpaceAdventure { .. } => {
                return Err(anyhow!("Team is on space adventure"))
            }
        };

        let distance = self.distance_between_planets(from_id, to_id)?;
        let bonus =
            TeamBonus::SpaceshipSpeed.current_team_bonus(&team.id, &self.teams, &self.players)?;
        Ok(
            ((LANDING_TIME_OVERHEAD as f32 + (distance as f32 / team.spaceship_speed())) / bonus)
                as Tick,
        )
    }

    fn planet_height(&self, planet_id: PlanetId) -> AppResult<usize> {
        let mut planet = self.planets.get_or_err(&planet_id)?;
        let mut height = 0;

        while let Some(parent_id) = planet.satellite_of {
            planet = self.planets.get_or_err(&parent_id)?;
            height += 1;
        }
        Ok(height)
    }

    pub fn fuel_consumption_to_planet(&self, team_id: TeamId, to_id: PlanetId) -> AppResult<u32> {
        let duration = self.travel_duration_to_planet(team_id, to_id)?;
        let team = self.teams.get_or_err(&team_id)?;

        Ok((duration as f64 * team.spaceship_fuel_consumption_per_tick() as f64).ceil() as u32)
    }

    pub fn distance_between_planets(
        &self,
        from_id: PlanetId,
        to_id: PlanetId,
    ) -> AppResult<KILOMETER> {
        // We calculate the distance. 5 cases:
        // 1: from and to are the same planet -> 0
        if from_id == to_id {
            return Ok(0);
        }

        let from = self.planets.get_or_err(&from_id)?;
        let to = self.planets.get_or_err(&to_id)?;

        let from_height: usize = self.planet_height(from_id)?;
        let to_height: usize = self.planet_height(to_id)?;

        // 2: from and to have the same parent -> difference in largest and smallest axes
        if from.satellite_of == to.satellite_of {
            let distance = ((from.axis.0 - to.axis.0).abs()).max((from.axis.1 - to.axis.1).abs())
                / 24.0
                * BASE_DISTANCES[from_height - 1] as f32;

            return Ok(distance as KILOMETER);
        }

        // 3: from is a satellite of to -> largest 'from' axis divided by 24
        if from.satellite_of == Some(to.id) {
            let distance =
                (from.axis.0).max(from.axis.1) / 24.0 * BASE_DISTANCES[from_height - 1] as f32;

            return Ok(distance as KILOMETER);
        }

        // 4: to is a satellite of from -> largest 'to' axis divided by 24
        if to.satellite_of == Some(from.id) {
            let distance = (to.axis.0).max(to.axis.1) / 24.0 * BASE_DISTANCES[to_height - 1] as f32;

            return Ok(distance as KILOMETER);
        }

        // 5: from and to are not related -> find distance recursively and add distance bottom planet to parent (case 3 and 4)
        let (bottom, top) = if from_height > to_height {
            (from, to)
        } else {
            (to, from)
        };
        let bottom_height = if from_height > to_height {
            from_height
        } else {
            to_height
        };

        let parent_id = bottom
            .satellite_of
            .expect("There should be a parent planet"); // This is guaranteed to be some, otherwise we would have matched already

        let distance =
            (bottom.axis.0).max(bottom.axis.1) / 24.0 * BASE_DISTANCES[bottom_height - 1] as f32;

        Ok(self.distance_between_planets(parent_id, top.id)? + distance as KILOMETER)
    }

    pub fn to_store(&self) -> AppResult<World> {
        // FIXME: this can be optimized by not cloning and filtering directly
        let mut w = World {
            app_version: self.app_version,
            seed: self.seed,
            last_tick_short_interval: self.last_tick_short_interval,
            last_tick_medium_interval: self.last_tick_medium_interval,
            last_tick_long_interval: self.last_tick_long_interval,
            own_team_id: self.own_team_id,
            teams: self.teams.clone(),
            players: self.players.clone(),
            players_scouting: self.players_scouting.clone(),
            planets: self.planets.clone(),
            games: self.games.clone(),
            tournaments: self.tournaments.clone(),
            kartoffeln: self.kartoffeln.clone(),
            past_games: self.past_games.clone(),
            past_tournaments: self
                .past_tournaments
                .iter()
                .filter(|(_, t)| t.participant_ids.contains(&self.own_team_id))
                .map(|(id, t)| (*id, t.clone()))
                .collect(),
            canceled_tournaments: self.canceled_tournaments.clone(),
            applied_trades: self.applied_trades.clone(),
            serialized_size: self.serialized_size,
            network_store_data: self.network_store_data.to_store(),
            ..Default::default()
        };

        w.filter_peer_data(None)?;

        // Drop asteroids that are no longer owned by any team. Asteroid abandonment is
        // deferred to save time so that an in-progress visit to the asteroid is not broken:
        // skip asteroids any team is currently located on, and prune dangling satellite ids
        // from each parent planet afterwards.
        let owned_asteroids: HashSet<PlanetId> = w
            .teams
            .values()
            .flat_map(|team| team.asteroid_ids.iter().copied())
            .collect();
        let occupied_planets: HashSet<PlanetId> = w
            .teams
            .values()
            .map(|team| match team.current_location {
                TeamLocation::OnPlanet { planet_id } => planet_id,
                TeamLocation::Exploring { around, .. }
                | TeamLocation::OnSpaceAdventure { around } => around,
                TeamLocation::Travelling { to, .. } => to,
            })
            .collect();
        w.planets.retain(|_, planet| {
            planet.planet_type != PlanetType::Asteroid
                || owned_asteroids.contains(&planet.id)
                || occupied_planets.contains(&planet.id)
        });
        let valid_planet_ids: HashSet<PlanetId> = w.planets.keys().copied().collect();
        for planet in w.planets.values_mut() {
            planet.satellites.retain(|id| valid_planet_ids.contains(id));
        }

        Ok(w)
    }

    pub fn get_team_players<'a>(
        players: &'a PlayerMap,
        team: &'a Team,
    ) -> AppResult<Vec<&'a Player>> {
        let team_players = match team
            .player_ids
            .iter()
            .map(|id| players.get_or_err(id))
            .collect::<AppResult<Vec<_>>>()
        {
            Ok(players) => players,
            Err(err) => {
                log::error!("Error while collecting team players: {err}");
                return Err(anyhow!("Error while collecting team players: {err}"));
            }
        };
        Ok(team_players)
    }
}

#[cfg(test)]
mod test {
    use std::{thread, time::Duration};

    use super::{AppResult, World};
    use crate::{
        app::App,
        core::{
            player::Trait,
            resources::Resource,
            role::CrewRole,
            skill::Rated,
            types::TeamLocation,
            utils::PLANET_DATA,
            world::{TickInterval, AU, EXPLORATION_DURATION},
            RatedPlayers, DEFAULT_PLANET_ID, MAX_SKILL, MIN_PLAYERS_PER_GAME,
            PORTAL_TRAVEL_DURATION, SPUGNA_DRUNKENNESS_ON_GETTING_DRUNK,
        },
        game_engine::{types::TeamInGame, Tournament, TournamentId},
        network::trade::Trade,
        types::{HashMapWithResult, StorableResourceMap, SystemTimeTick, Tick},
        ui::UiCallback,
    };
    use itertools::Itertools;
    use rand::{RngExt, SeedableRng};
    use rand_chacha::ChaCha8Rng;
    use uuid::uuid;

    /// Gives the own team a finished cove with a market on a fresh asteroid and
    /// parks the crew there, which is what listing requires.
    fn give_own_team_a_ready_market(app: &mut App) -> AppResult<crate::types::PlanetId> {
        use crate::core::{SpaceCove, SpaceCoveUpgradeTarget};

        let home_planet_id = app.world.get_own_team()?.home_planet_id;
        let asteroid_id = app.world.generate_team_asteroid(
            "Testeroid".into(),
            "asteroid1".into(),
            home_planet_id,
        )?;

        let mut cove = SpaceCove::under_construction(asteroid_id);
        cove.finish_contruction();
        cove.upgrades.insert(SpaceCoveUpgradeTarget::Market);

        let own_team_id = app.world.own_team_id;
        let team = app.world.teams.get_mut_or_err(&own_team_id)?;
        team.asteroid_ids.push(asteroid_id);
        team.space_cove = Some(cove);
        team.current_location = TeamLocation::OnPlanet {
            planet_id: asteroid_id,
        };

        Ok(asteroid_id)
    }

    fn park_own_team_at_the_dock(app: &mut App) -> AppResult<()> {
        let own_team_id = app.world.own_team_id;
        app.world
            .teams
            .get_mut_or_err(&own_team_id)?
            .current_location = TeamLocation::OnPlanet {
            planet_id: *crate::core::GALAXY_ROOT_ID,
        };
        Ok(())
    }

    /// A peer crew whose pirates are all but unknown, with its first pirate left at the dock.
    fn crew_listing_a_pirate(
        app: &App,
    ) -> AppResult<(
        crate::core::Team,
        crate::types::PlayerMap,
        crate::types::PlayerId,
        crate::types::PlayerId,
    )> {
        use crate::core::DockListing;
        use crate::types::PlayerMap;
        use libp2p::PeerId;

        let peer_id = PeerId::random();
        let mut team = app
            .world
            .teams
            .values()
            .find(|team| team.id != app.world.own_team_id)
            .expect("another crew")
            .clone();
        team.peer_id = Some(peer_id);

        let mut players = PlayerMap::new();
        for player_id in team.player_ids.iter() {
            let mut player = app.world.players.get_or_err(player_id)?.clone();
            player.peer_id = Some(peer_id);
            player.reputation = 1.0;
            players.insert(player.id, player);
        }

        let listed = team.player_ids[0];
        let unlisted = team.player_ids[1];
        team.dock_listings
            .push(DockListing::new(listed, Tick::now()));

        Ok((team, players, listed, unlisted))
    }

    fn scouting_of(app: &App, player_id: &crate::types::PlayerId) -> crate::core::skill::Skill {
        app.world
            .players_scouting
            .get(player_id)
            .expect("a report")
            .scouting()
    }

    #[test]
    fn test_a_pirate_listed_over_the_network_arrives_scouted() -> AppResult<()> {
        use crate::core::DOCK_LISTING_SCOUTING;
        use crate::network::types::NetworkTeam;

        let mut app = App::test_default()?;
        let (team, players, listed, unlisted) = crew_listing_a_pirate(&app)?;

        app.world.players_scouting.remove(&listed);
        app.world.players_scouting.remove(&unlisted);
        app.world
            .add_network_team(NetworkTeam::new(team, players, vec![]), Tick::now())?;

        assert_eq!(
            scouting_of(&app, &listed),
            DOCK_LISTING_SCOUTING,
            "a pirate left at the dock is shown off"
        );
        assert_eq!(
            scouting_of(&app, &unlisted),
            1.0,
            "the rest of the crew is no better known than before"
        );
        Ok(())
    }

    /// The listing sets a floor, it does not pay out. A crew that lists the same
    /// pirate over and over must not scout them to MAX_SKILL by repetition.
    #[test]
    fn test_relisting_a_pirate_does_not_stack_scouting() -> AppResult<()> {
        use crate::core::DOCK_LISTING_SCOUTING;
        use crate::network::types::NetworkTeam;

        let mut app = App::test_default()?;
        let (team, players, listed, _) = crew_listing_a_pirate(&app)?;
        app.world.players_scouting.remove(&listed);

        let mut delisted = team.clone();
        delisted.dock_listings.clear();

        let now = Tick::now();
        // Listed, taken back off the market, then listed again.
        for (round, crew) in [&team, &delisted, &team].into_iter().enumerate() {
            app.world.add_network_team(
                NetworkTeam::new(crew.clone(), players.clone(), vec![]),
                now + round as Tick + 1,
            )?;
            assert_eq!(
                scouting_of(&app, &listed),
                DOCK_LISTING_SCOUTING,
                "round {round} moved the floor"
            );
        }

        Ok(())
    }

    /// The lost-Ack split-brain, closed: a peer's stale roster cannot claw back
    /// a pirate we have since made ours.
    #[test]
    fn test_a_peer_cannot_take_back_a_pirate_we_own() -> AppResult<()> {
        use crate::network::types::NetworkTeam;

        let mut app = App::test_default()?;
        let ours = app.world.get_own_team()?.player_ids[0];
        let (mut team, mut players, _, _) = crew_listing_a_pirate(&app)?;

        // Their gossip still says our pirate is theirs.
        let mut stale = app.world.players.get_or_err(&ours)?.clone();
        stale.team = Some(team.id);
        stale.peer_id = team.peer_id;
        players.insert(ours, stale);
        team.player_ids.push(ours);
        team.dock_listings.clear();

        app.world
            .add_network_team(NetworkTeam::new(team, players, vec![]), Tick::now())?;

        assert_eq!(
            app.world.players.get_or_err(&ours)?.team,
            Some(app.world.own_team_id),
            "ours stays ours"
        );
        Ok(())
    }

    #[test]
    fn test_free_pirates_gather_at_the_dock() -> AppResult<()> {
        use crate::core::GALAXY_ROOT_ID;

        let mut world = World::new(Some(1));
        world.initialize(false)?;
        assert!(world
            .players
            .values()
            .any(|player| player.team.is_none() && player.is_on_planet() == Some(*GALAXY_ROOT_ID)));
        Ok(())
    }

    #[test]
    fn test_list_and_recall_round_trip() -> AppResult<()> {
        let mut app = App::test_default()?;
        park_own_team_at_the_dock(&mut app)?;

        let player_id = app.world.get_own_team()?.player_ids[0];
        let crew_size = app.world.get_own_team()?.player_ids.len();

        app.world.leave_player_at_dock(player_id, Tick::now())?;

        let team = app.world.get_own_team()?;
        assert!(team.is_listed(&player_id));
        // Still crew and still aboard: same seat count, one fewer able to play.
        assert_eq!(team.player_ids.len(), crew_size);
        assert_eq!(team.active_players_count(), crew_size - 1);
        assert!(matches!(
            app.world.players.get_or_err(&player_id)?.current_location,
            crate::core::types::PlayerLocation::WithTeam
        ));
        assert_eq!(
            app.world.players.get_or_err(&player_id)?.info.crew_role,
            CrewRole::Mozzo
        );

        app.world.recall_player_from_dock(player_id)?;

        let team = app.world.get_own_team()?;
        assert!(!team.is_listed(&player_id));
        assert_eq!(team.active_players_count(), crew_size);
        assert!(matches!(
            app.world.players.get_or_err(&player_id)?.current_location,
            crate::core::types::PlayerLocation::WithTeam
        ));

        Ok(())
    }

    #[test]
    fn test_listing_vacates_the_crew_role() -> AppResult<()> {
        let mut app = App::test_default()?;
        park_own_team_at_the_dock(&mut app)?;

        let player_id = app.world.get_own_team()?.player_ids[0];
        app.world.set_team_crew_role(CrewRole::Captain, player_id)?;
        assert_eq!(
            app.world.get_own_team()?.crew_roles.captain,
            Some(player_id)
        );

        app.world.leave_player_at_dock(player_id, Tick::now())?;

        assert_eq!(app.world.get_own_team()?.crew_roles.captain, None);
        Ok(())
    }

    #[test]
    fn test_listed_pirate_is_left_out_of_the_game_roster() -> AppResult<()> {
        let mut app = App::test_default()?;
        park_own_team_at_the_dock(&mut app)?;

        let own_team_id = app.world.own_team_id;
        let player_id = app.world.get_own_team()?.player_ids[0];
        app.world.leave_player_at_dock(player_id, Tick::now())?;

        let team_in_game =
            TeamInGame::from_team_id(&own_team_id, &app.world.teams, &app.world.players)
                .expect("should build a roster");
        assert!(
            !team_in_game.players.contains_key(&player_id),
            "a pirate at the dock must never be picked for a game"
        );

        Ok(())
    }

    #[test]
    fn test_abandoning_the_cove_leaves_the_dock_alone() -> AppResult<()> {
        let mut app = App::test_default()?;
        let asteroid_id = give_own_team_a_ready_market(&mut app)?;
        park_own_team_at_the_dock(&mut app)?;
        let player_id = app.world.get_own_team()?.player_ids[0];
        app.world.leave_player_at_dock(player_id, Tick::now())?;

        // The dock is not on the rock: dropping the cove from far away is fine
        // and changes nothing about the listing.
        app.world.abandon_asteroid(asteroid_id)?;

        let team = app.world.get_own_team()?;
        assert!(team.space_cove.is_none());
        assert!(team.is_listed(&player_id), "the listing stays");
        assert!(team.player_ids.contains(&player_id));

        Ok(())
    }

    #[test]
    fn test_a_listed_pirate_can_still_be_released() -> AppResult<()> {
        let mut app = App::test_default()?;
        park_own_team_at_the_dock(&mut app)?;
        let player_id = app.world.get_own_team()?.player_ids[0];
        app.world.leave_player_at_dock(player_id, Tick::now())?;

        app.world.release_player_from_team(player_id, true)?;

        let team = app.world.get_own_team()?;
        assert!(!team.player_ids.contains(&player_id));
        assert!(!team.is_listed(&player_id), "the listing must go with them");
        // Released where the crew is docked, like any other firing.
        assert_eq!(
            app.world.players.get_or_err(&player_id)?.current_location,
            crate::core::types::PlayerLocation::OnPlanet {
                planet_id: *crate::core::GALAXY_ROOT_ID
            }
        );
        assert!(app.world.players.get_or_err(&player_id)?.team.is_none());

        Ok(())
    }

    #[test]
    fn test_waiting_pirates_board_when_the_crew_reaches_the_dock() -> AppResult<()> {
        let mut app = App::test_default()?;
        let own_team_id = app.world.own_team_id;
        app.world
            .teams
            .get_mut_or_err(&own_team_id)?
            .current_location = TeamLocation::OnPlanet {
            planet_id: *DEFAULT_PLANET_ID,
        };
        let player_id = app.world.get_own_team()?.player_ids[0];
        app.world
            .get_own_team_mut()?
            .waiting_at_dock
            .push(player_id);

        assert!(app
            .world
            .tick_collect_waiting_pirates(Tick::now())?
            .is_none());
        assert!(app.world.get_own_team()?.is_waiting(&player_id));

        park_own_team_at_the_dock(&mut app)?;
        assert!(app
            .world
            .tick_collect_waiting_pirates(Tick::now())?
            .is_some());
        let team = app.world.get_own_team()?;
        assert!(!team.is_waiting(&player_id));
        assert!(team.active_player_ids().contains(&player_id));
        Ok(())
    }

    #[test]
    fn test_a_released_waiting_pirate_stays_at_the_dock() -> AppResult<()> {
        let mut app = App::test_default()?;
        let own_team_id = app.world.own_team_id;
        app.world
            .teams
            .get_mut_or_err(&own_team_id)?
            .current_location = TeamLocation::OnPlanet {
            planet_id: *DEFAULT_PLANET_ID,
        };
        let player_id = app.world.get_own_team()?.player_ids[0];
        app.world
            .get_own_team_mut()?
            .waiting_at_dock
            .push(player_id);

        app.world.release_player_from_team(player_id, true)?;

        assert!(!app.world.get_own_team()?.is_waiting(&player_id));
        assert_eq!(
            app.world.players.get_or_err(&player_id)?.current_location,
            crate::core::types::PlayerLocation::OnPlanet {
                planet_id: *crate::core::GALAXY_ROOT_ID
            }
        );
        Ok(())
    }

    #[test]
    fn test_deterministic_randomness() {
        let seed = rand::random::<u64>();
        let rng = &mut ChaCha8Rng::seed_from_u64(seed);
        let mut v1 = vec![];
        let mut v2 = vec![];
        for _ in 0..10 {
            v1.push(rng.random::<u8>());
        }
        let rng = &mut ChaCha8Rng::seed_from_u64(seed);
        for _ in 0..10 {
            v2.push(rng.random::<u8>());
        }
        assert_eq!(v1, v2);
    }

    #[test]
    fn test_distance_to_earth() -> AppResult<()> {
        let world = World::new(None);
        let earth = world.planets.values().find(|p| p.name == "Earth").unwrap();

        for planet in world.planets.values() {
            let distance = world.distance_between_planets(earth.id, planet.id)?;
            let reputation_bonus = World::team_reputation_bonus_per_distance(distance);
            println!(
                "Earth to {} = {} Km = {:.4} AU\nReputation bonus = {}\n",
                planet.name,
                distance,
                distance as f32 / AU as f32,
                reputation_bonus
            );
        }

        Ok(())
    }

    #[test]
    fn test_exploration_result() -> AppResult<()> {
        let mut world = World::new(None);
        let rng = &mut ChaCha8Rng::from_rng(&mut rand::rng());
        let jupiter_id = uuid!("71a43700-0000-0000-0002-000000000002");
        let planet = world.planets.get(&jupiter_id).unwrap().clone();
        println!(
            "Around planet {} - Population {} - Asteroid probability {}",
            planet.name,
            planet.total_population(),
            planet.asteroid_probability
        );
        println!(
            "Planet resources:
    Satoshi {}
    Gold {}
    Scraps {}
    Fuel {}
    Rum {}",
            planet.resources.value(&Resource::SATOSHI),
            planet.resources.value(&Resource::GOLD),
            planet.resources.value(&Resource::SCRAPS),
            planet.resources.value(&Resource::FUEL),
            planet.resources.value(&Resource::RUM)
        );

        let team_id =
            world.generate_random_team(rng, planet.id, "test".into(), "testship".into())?;

        world.own_team_id = team_id;

        let mut own_team = world.get_own_team()?.clone();

        println!("\nResources before exploration: {:#?}", own_team.resources);
        println!(
            "Storage: {}/{}",
            own_team.used_storage_capacity(),
            own_team.storage_capacity()
        );
        println!(
            "Tank: {}/{}",
            own_team.used_fuel_capacity(),
            own_team.fuel_capacity()
        );
        let now = Tick::now();
        let duration = EXPLORATION_DURATION;
        own_team.current_location = TeamLocation::Exploring {
            around: planet.id,
            started: now.saturating_sub(duration),
            duration,
        };
        assert!(own_team.is_on_planet() == None);
        world.teams.insert(own_team.id, own_team);

        let callbacks = world.tick_travel(now + TickInterval::SHORT)?;

        let own_team = world.get_own_team()?;
        assert!(own_team.is_on_planet() == Some(planet.id));

        println!("\nTeam found {} asteroids", callbacks.len() - 1);

        println!("\nResources after exploration: {:#?}", own_team.resources);
        println!(
            "Storage: {}/{}",
            own_team.used_storage_capacity(),
            own_team.storage_capacity()
        );
        println!(
            "Tank: {}/{}",
            own_team.used_fuel_capacity(),
            own_team.fuel_capacity()
        );

        assert!(own_team.used_storage_capacity() <= own_team.storage_capacity());
        assert!(own_team.used_fuel_capacity() <= own_team.fuel_capacity());

        Ok(())
    }

    #[test]
    fn test_exploration_result_capping() -> AppResult<()> {
        let mut world = World::new(None);
        let rng = &mut ChaCha8Rng::from_rng(&mut rand::rng());
        let jupiter_id = uuid!("71a43700-0000-0000-0002-000000000002");
        let planet = world.planets.get(&jupiter_id).unwrap().clone();
        println!(
            "Around planet {} - Population {} - Asteroid probability {}",
            planet.name,
            planet.total_population(),
            planet.asteroid_probability
        );
        println!(
            "Planet resources:
    Satoshi {}
    Gold {}
    Scraps {}
    Fuel {}
    Rum {}",
            planet.resources.value(&Resource::SATOSHI),
            planet.resources.value(&Resource::GOLD),
            planet.resources.value(&Resource::SCRAPS),
            planet.resources.value(&Resource::FUEL),
            planet.resources.value(&Resource::RUM)
        );

        let team_id =
            world.generate_random_team(rng, planet.id, "test".into(), "testship".into())?;

        world.own_team_id = team_id;

        let mut own_team = world.get_own_team()?.clone();

        let available_storage = own_team.available_storage_capacity();
        let available_tank = own_team.available_fuel_capacity();

        own_team.add_resource(Resource::FUEL, available_tank - 2)?;
        own_team.add_resource(
            Resource::SCRAPS,
            available_storage / Resource::SCRAPS.to_storing_space() - 8,
        )?;

        println!("\nResources before exploration: {:#?}", own_team.resources);
        println!(
            "Storage: {}/{}",
            own_team.used_storage_capacity(),
            own_team.storage_capacity()
        );
        println!(
            "Tank: {}/{}",
            own_team.used_fuel_capacity(),
            own_team.fuel_capacity()
        );
        let now = Tick::now();
        let duration = EXPLORATION_DURATION;
        own_team.current_location = TeamLocation::Exploring {
            around: planet.id,
            started: now.saturating_sub(duration),
            duration,
        };
        assert!(own_team.is_on_planet() == None);
        world.teams.insert(own_team.id, own_team);

        let callbacks = world.tick_travel(now + TickInterval::SHORT)?;

        let own_team = world.get_own_team()?;
        assert!(own_team.is_on_planet() == Some(planet.id));

        println!("\nTeam found {} asteroids", callbacks.len() - 1);

        println!("\nResources after exploration: {:#?}", own_team.resources);
        println!(
            "Storage: {}/{}",
            own_team.used_storage_capacity(),
            own_team.storage_capacity()
        );
        println!(
            "Tank: {}/{}",
            own_team.used_fuel_capacity(),
            own_team.fuel_capacity()
        );

        assert!(own_team.used_storage_capacity() <= own_team.storage_capacity());
        assert!(own_team.used_fuel_capacity() <= own_team.fuel_capacity());

        Ok(())
    }

    #[test]
    fn test_spugna_portal() -> AppResult<()> {
        let mut app = App::test_default()?;
        app.new_world();

        let rng = &mut ChaCha8Rng::from_rng(&mut rand::rng());
        let planet = PLANET_DATA[0].clone();
        let team_id =
            app.world
                .generate_random_team(rng, planet.id, "test".into(), "testship".into())?;

        // Add rum to team
        let mut team = app.world.teams.get_or_err(&team_id)?.clone();
        team.add_resource(Resource::RUM, 20)?;

        // Give player Spugna skill and set it as pilot.
        // Max drunkenness and zero stamina maximize the chance each drink
        // triggers the drunk event.
        let mut spugna = app.world.players.get_or_err(&team.player_ids[0])?.clone();
        let spugna_id = spugna.id.clone();
        spugna.special_trait = Some(Trait::Spugna);
        spugna.drunkenness = MAX_SKILL;
        spugna.athletics.stamina = 0.0;
        app.world.players.insert(spugna.id, spugna);
        if app.world.players.get_or_err(&spugna_id)?.info.crew_role != CrewRole::Pilot {
            app.world.set_team_crew_role(CrewRole::Pilot, spugna_id)?;
        }

        // Travel to a random planet
        let target = PLANET_DATA[1].clone();
        team.current_location = TeamLocation::Travelling {
            from: planet.id,
            to: target.id,
            started: 0,
            duration: app.world.travel_duration_to_planet(team.id, target.id)?,
            distance: app.world.distance_between_planets(planet.id, target.id)?,
        };

        println!("Team resources {:?}", team.resources);
        println!("Team location {:?}", team.current_location);
        println!("Travelled distance {}", team.total_travelled);
        assert!(team.total_travelled == 0);
        app.world.teams.insert(team.id, team);

        // Each drink only triggers the drunk event with probability < 1, so drink
        // until it does. Morale is captured right before the successful drink: sober
        // drinks raise morale, but getting drunk must leave it untouched.
        let mut morale_before;
        loop {
            morale_before = app.world.players.get_or_err(&spugna_id)?.morale;
            UiCallback::Drink {
                player_id: spugna_id,
            }
            .call(&mut app)?;
            if app.world.players.get_or_err(&spugna_id)?.is_knocked_out() {
                break;
            }
        }

        let spugna = app.world.players.get_or_err(&spugna_id)?;
        // Getting drunk wastes the player, makes drunkenness negative and leaves morale untouched.
        assert!(spugna.is_knocked_out());
        assert!(spugna.drunkenness == SPUGNA_DRUNKENNESS_ON_GETTING_DRUNK);
        assert!(spugna.morale == morale_before);

        // The team is now travelling through the portal.
        let team = app.world.teams.get_or_err(&team_id)?;
        match team.current_location {
            TeamLocation::Travelling { duration, .. } => {
                assert!(duration == PORTAL_TRAVEL_DURATION)
            }
            _ => panic!("Team should be travelling through the portal"),
        }

        app.world
            .handle_slow_tick_events(app.world.last_tick_short_interval + TickInterval::SHORT)?;

        let team = app.world.teams.get_or_err(&team_id)?;
        println!("Team resources {:?}", team.resources);
        println!("Team location {:?}", team.current_location);
        println!("Travelled distance {}", team.total_travelled);
        // Teleportation does not add to total_travelled
        assert!(team.total_travelled == 0);

        Ok(())
    }

    #[test]
    fn test_in_game_drinking_rum_accounting() -> AppResult<()> {
        let mut app = App::test_default()?;
        app.new_world();

        let rng = &mut ChaCha8Rng::from_rng(&mut rand::rng());
        let planet = PLANET_DATA[0].clone();
        let home_id =
            app.world
                .generate_random_team(rng, planet.id, "home".into(), "homeship".into())?;
        let away_id =
            app.world
                .generate_random_team(rng, planet.id, "away".into(), "awayship".into())?;

        // Give both teams a known amount of rum.
        const STARTING_RUM: u32 = 20;
        for team_id in [home_id, away_id] {
            let mut team = app.world.teams.get_or_err(&team_id)?.clone();
            let current_rum = team.resources.value(&Resource::RUM);
            team.saturating_sub_resource(Resource::RUM, current_rum);
            team.add_resource(Resource::RUM, STARTING_RUM)?;
            app.world.teams.insert(team.id, team);
        }

        let home_team_in_game =
            TeamInGame::from_team_id(&home_id, &app.world.teams, &app.world.players)?;
        let away_team_in_game =
            TeamInGame::from_team_id(&away_id, &app.world.teams, &app.world.players)?;
        let initial_rums = [home_team_in_game.initial_rum, away_team_in_game.initial_rum];
        assert!(initial_rums[0] > 0);
        assert!(initial_rums[1] > 0);

        let game_id = app
            .world
            .generate_local_game(home_team_in_game, away_team_in_game)?;

        // The rum brought to the game is debited upfront.
        for (team_id, initial_rum) in [home_id, away_id].iter().zip(initial_rums) {
            let team = app.world.teams.get_or_err(team_id)?;
            assert!(team.resources.value(&Resource::RUM) == STARTING_RUM - initial_rum);
        }

        // Play the game to the end.
        {
            let game = app
                .world
                .games
                .get_mut(&game_id)
                .expect("Game should exist");
            while !game.has_ended() {
                game.tick();
            }
        }

        let game = app.world.games.get_or_err(&game_id)?.clone();
        app.world.cleanup_games(Tick::now())?;

        // After the game, the remaining brought rum is returned along with the rum bonus.
        for (idx, (team_id, initial_rum)) in [home_id, away_id].iter().zip(initial_rums).enumerate()
        {
            let team_in_game = if idx == 0 {
                &game.home_team_in_game
            } else {
                &game.away_team_in_game
            };

            let rum_bonus = match game.winner {
                Some(winner) if winner == *team_id => team_in_game.players.len() as u32,
                _ => 1,
            };

            let total_drunk: u16 = team_in_game.stats.values().map(|s| s.rum_drunk).sum();
            assert!(total_drunk as u32 == initial_rum - team_in_game.rum);

            let team = app.world.teams.get_or_err(team_id)?;
            assert!(
                team.resources.value(&Resource::RUM)
                    == STARTING_RUM - initial_rum + team_in_game.rum + rum_bonus
            );
        }

        Ok(())
    }

    #[test]
    fn test_tick_players_update() -> AppResult<()> {
        let mut app = App::test_default()?;

        let world = &mut app.world;

        let player_id = world
            .players
            .values()
            .next()
            .expect("There should be at least one player")
            .id;

        for _ in 0..16 {
            let player = world.players.get_mut(&player_id).unwrap();
            player.info.age = player.info.population.min_age();

            println!(
                "Age {:.2} - Overall {:.2} {} - Potential {:.2} {}",
                player.info.relative_age(),
                player.average_skill(),
                player.average_skill().stars(),
                player.potential,
                player.potential.stars(),
            );
            world.tick_players_update();
        }

        for _ in 0..16 {
            let player = world.players.get_mut(&player_id).unwrap();
            player.info.age = player.info.population.max_age();
            println!(
                "Age {:.2} - Overall {:.2} {} - Potential {:.2} {}",
                player.info.relative_age(),
                player.average_skill(),
                player.average_skill().stars(),
                player.potential,
                player.potential.stars(),
            );
            world.tick_players_update();
        }
        Ok(())
    }

    #[test]
    fn test_players_training() -> AppResult<()> {
        let mut app = App::test_default()?;

        let world = &mut app.world;

        let player_id = world
            .players
            .values()
            .sorted_by(|a, b| b.potential.partial_cmp(&a.potential).unwrap())
            .next()
            .expect("There should be at least one player")
            .id;

        let mut overalls = vec![];

        let player = world.players.get_or_err(&player_id)?;

        let mut current_max_average_skill = player.current_skill_array();
        for _ in 0..300 {
            let mut player = world.players.get_or_err(&player_id)?.clone();
            if player.average_skill()
                > current_max_average_skill.iter().sum::<f32>()
                    / current_max_average_skill.len() as f32
            {
                current_max_average_skill = player.current_skill_array();
            }
            overalls.push(player.average_skill());
            assert!(player.skills_training == [0.0; 20]);
            println!(
                "Age {:.2} - Overall {:.2} {} - Potential {:.2} {}",
                player.info.relative_age(),
                player.average_skill(),
                player.average_skill().stars(),
                player.potential,
                player.potential.stars(),
            );
            if player.info.relative_age() > 1.0 {
                break;
            }

            // 32 minutes equally split between 5 positions
            let experience_at_position = [384; 5];
            player.update_skills_training(experience_at_position, 1.5, None);
            world.players.insert(player.id, player);
            world.tick_players_update();
        }

        let player = world.players.get_or_err(&player_id)?;
        println!(
            "Top skills: {:?}",
            current_max_average_skill
                .iter()
                .map(|v| (v * 100.0).round() / 100.0)
                .collect_vec()
        );
        println!(
            "Final skills: {:?}",
            player
                .current_skill_array()
                .iter()
                .map(|v| (v * 100.0).round() / 100.0)
                .collect_vec()
        );

        Ok(())
    }

    #[test]
    fn test_tick_player_leaving_own_team_for_age() -> AppResult<()> {
        let mut app = App::test_default()?;

        let world = &mut app.world;

        let own_team = world.get_own_team()?;
        let player_id = own_team.player_ids[0];
        let mut player = world.players.get_or_err(&player_id)?.clone();
        assert!(player.team.is_some());

        player.info.age = player.info.population.max_age();
        world.players.insert(player_id, player);
        world.tick_player_retirement(Tick::now())?;

        let player = world.players.get_or_err(&player_id)?;
        assert!(player.team.is_none());

        Ok(())
    }

    #[test]
    fn test_tick_player_leaving_own_team_for_satisfaction() -> AppResult<()> {
        let mut app = App::test_default()?;

        let world = &mut app.world;

        let own_team = world.get_own_team()?;
        let player_id = own_team.player_ids[0];
        let player = world.players.get_mut_or_err(&player_id)?;
        assert!(player.team.is_some());

        player.add_team_satisfaction(-MAX_SKILL);

        // Players with low satisfaction quit a team randomly
        let mut idx = 0;
        loop {
            world.tick_player_leaving_team_for_low_satisfaction(Tick::now())?;
            let player: &crate::core::player::Player = world.players.get_or_err(&player_id)?;
            if player.team.is_none() {
                break;
            }
            idx += 1;
        }
        println!("Player left team after {idx} iterations");
        Ok(())
    }

    #[test]
    fn test_auto_hiring() -> AppResult<()> {
        let mut app = App::test_default()?;

        for team in app.world.teams.values_mut() {
            team.add_resource(Resource::SATOSHI, 200_000)?;
        }

        let rng = &mut ChaCha8Rng::seed_from_u64(app.world.seed);
        let team_id = app.world.generate_random_team(
            rng,
            DEFAULT_PLANET_ID.clone(),
            "Testen".to_string(),
            "Tosten".to_string(),
        )?;

        let team = app.world.teams.get_or_err(&team_id)?;

        assert!(team.player_ids.len() <= team.spaceship.crew_capacity() as usize);

        let players = team
            .player_ids
            .iter()
            .map(|id| app.world.players.get(id).unwrap())
            .collect_vec()
            .sort_by_rating();

        let worst_pirate = players.last().unwrap();
        let prev_worst_rating = worst_pirate.average_skill();
        println!(
            "Worst player {} rating {}",
            worst_pirate.info.short_name(),
            prev_worst_rating
        );

        app.world.tick_auto_hire_free_pirates(Tick::now())?;

        let team = app.world.teams.get_or_err(&team_id)?;
        let players = team
            .player_ids
            .iter()
            .map(|id| app.world.players.get(id).unwrap())
            .collect_vec()
            .sort_by_rating();
        let worst_pirate = players.last().unwrap();
        let new_worst_rating = worst_pirate.average_skill();
        println!(
            "Worst player {} rating {}",
            worst_pirate.info.short_name(),
            new_worst_rating
        );

        assert!(prev_worst_rating <= new_worst_rating);

        Ok(())
    }

    #[test]
    fn test_auto_hiring_multiple() -> AppResult<()> {
        let mut app = App::test_default()?;

        for team in app.world.teams.values_mut() {
            team.add_resource(Resource::SATOSHI, 200_000)?;
        }

        let rng = &mut ChaCha8Rng::seed_from_u64(app.world.seed);
        let team_id = app.world.generate_random_team(
            rng,
            DEFAULT_PLANET_ID.clone(),
            "Testen".to_string(),
            "Tosten".to_string(),
        )?;

        let mut team = app.world.teams.get_or_err(&team_id)?.clone();

        while team.player_ids.len() >= MIN_PLAYERS_PER_GAME {
            let player_id = team.player_ids[0];
            team.player_ids.retain(|&p| p != player_id);
        }

        assert!(team.player_ids.len() < MIN_PLAYERS_PER_GAME);

        app.world.teams.insert(team.id, team);

        app.world.tick_auto_hire_free_pirates(Tick::now())?;

        let team = app.world.teams.get_or_err(&team_id)?;
        assert!(team.player_ids.len() >= MIN_PLAYERS_PER_GAME);

        Ok(())
    }

    #[test]
    fn test_is_simulating() -> AppResult<()> {
        let mut app = App::test_default()?;

        let world = &mut app.world;
        assert!(world.is_simulating() == false);

        let now = Tick::now();
        world.last_tick_min_interval = now;

        let cycles = 3;
        // Sleep for at least `cycles` ticks. System scheduling may add extra
        // time beyond the buffer, so we assert runs >= cycles rather than ==.
        thread::sleep(Duration::from_millis(cycles * TickInterval::SHORT + 20));

        let mut runs = 0;

        while world.is_simulating() {
            world.handle_slow_tick_events(Tick::now())?;
            runs += 1;
        }

        assert!(runs >= cycles);

        Ok(())
    }

    #[test]
    fn test_generate_random_games() -> AppResult<()> {
        let mut app = App::test_default()?;

        let world = &mut app.world;

        for i in 0..11 {
            assert!(world.games.len() == i);
            world.generate_random_games(1)?;
        }

        for game in world.games.values() {
            println!(
                "{} vs {}",
                game.home_team_in_game.name, game.away_team_in_game.name
            );
        }

        Ok(())
    }

    #[test]
    fn test_canceled_tournament_is_remembered_once_dropped() -> AppResult<()> {
        let mut app = App::test_default()?;
        let mut tournament = Tournament::test(2, 4);
        tournament.cancel();
        app.world
            .tournaments
            .insert(tournament.id, tournament.clone());

        app.world.tick_tournaments(Tick::now())?;

        assert!(!app.world.tournaments.contains_key(&tournament.id));
        assert!(app.world.canceled_tournaments.contains(&tournament.id));
        Ok(())
    }

    #[test]
    fn test_to_store_keeps_canceled_tournaments() -> AppResult<()> {
        let mut app = App::test_default()?;
        let tournament_id = TournamentId::from_u128(7);
        app.world.canceled_tournaments.insert(tournament_id);

        assert!(app
            .world
            .to_store()?
            .canceled_tournaments
            .contains(&tournament_id));
        Ok(())
    }

    fn network_crew_on(
        app: &mut App,
        planet_id: crate::types::PlanetId,
    ) -> AppResult<crate::types::TeamId> {
        let peer_id = libp2p::PeerId::random();
        let team_id = app
            .world
            .teams
            .values()
            .find(|team| team.id != app.world.own_team_id && team.peer_id.is_none())
            .expect("another crew")
            .id;
        let team = app.world.teams.get_mut_or_err(&team_id)?;
        team.peer_id = Some(peer_id);
        team.current_location = TeamLocation::OnPlanet { planet_id };
        for player_id in team.player_ids.clone() {
            app.world.players.get_mut_or_err(&player_id)?.peer_id = Some(peer_id);
        }
        Ok(team_id)
    }

    fn park_own_team_on(app: &mut App, planet_id: crate::types::PlanetId) -> AppResult<()> {
        let own_team_id = app.world.own_team_id;
        app.world
            .teams
            .get_mut_or_err(&own_team_id)?
            .current_location = TeamLocation::OnPlanet { planet_id };
        Ok(())
    }

    #[test]
    fn test_held_satoshis_pay_for_the_trade_exactly_once() -> AppResult<()> {
        use crate::core::{Offer, OfferKind};

        let mut app = App::test_default()?;
        park_own_team_on(&mut app, *DEFAULT_PLANET_ID)?;
        let target_team_id = network_crew_on(&mut app, *DEFAULT_PLANET_ID)?;
        let target_id = app.world.teams.get_or_err(&target_team_id)?.player_ids[0];
        let target_player = app.world.players.get_or_err(&target_id)?.clone();
        let own_team_id = app.world.own_team_id;

        let trade = Trade::new(
            OfferKind::Direct,
            libp2p::PeerId::random(),
            libp2p::PeerId::random(),
            own_team_id,
            target_team_id,
            None,
            target_player,
            5_000,
        );
        let own_team = app.world.get_own_team_mut()?;
        own_team.sub_resource(Resource::SATOSHI, 5_000)?;
        own_team.offers.push(Offer {
            trade_id: trade.id,
            kind: OfferKind::Direct,
            target_team_id,
            target_player_id: target_id,
            satoshis: 5_000,
            pirate: None,
            placed_on: trade.created_at,
        });

        let own_before = app.world.get_own_team()?.balance();
        let target_before = app.world.teams.get_or_err(&target_team_id)?.balance();
        app.world.apply_trade(&trade, Tick::now())?;

        let own_team = app.world.get_own_team()?;
        assert_eq!(
            own_team.balance(),
            own_before,
            "the held satoshis are the payment"
        );
        assert!(own_team.offers.is_empty());
        assert!(own_team.player_ids.contains(&target_id));
        assert_eq!(
            app.world.teams.get_or_err(&target_team_id)?.balance(),
            target_before + 5_000
        );
        Ok(())
    }

    #[test]
    fn test_a_dock_trade_leaves_both_pirates_waiting_at_the_dock() -> AppResult<()> {
        use crate::core::{DockListing, OfferKind};

        let mut app = App::test_default()?;
        park_own_team_on(&mut app, *DEFAULT_PLANET_ID)?;
        let target_team_id = network_crew_on(&mut app, *DEFAULT_PLANET_ID)?;
        let target_id = app.world.teams.get_or_err(&target_team_id)?.player_ids[0];
        app.world
            .teams
            .get_mut_or_err(&target_team_id)?
            .dock_listings
            .push(DockListing::new(target_id, 5));
        let own_id = app.world.get_own_team()?.player_ids[0];

        let trade = Trade::new(
            OfferKind::Dock,
            libp2p::PeerId::random(),
            libp2p::PeerId::random(),
            app.world.own_team_id,
            target_team_id,
            Some(app.world.players.get_or_err(&own_id)?.clone()),
            app.world.players.get_or_err(&target_id)?.clone(),
            0,
        );
        app.world.apply_trade(&trade, Tick::now())?;

        let own_team = app.world.get_own_team()?;
        let target_team = app.world.teams.get_or_err(&target_team_id)?;
        assert!(own_team.is_waiting(&target_id));
        assert!(target_team.is_waiting(&own_id));
        assert!(!target_team.is_listed(&target_id));
        Ok(())
    }

    #[test]
    fn test_only_local_crews_are_checked_when_applying() -> AppResult<()> {
        use crate::core::OfferKind;

        let mut app = App::test_default()?;
        park_own_team_on(&mut app, *DEFAULT_PLANET_ID)?;
        let target_team_id = network_crew_on(&mut app, *DEFAULT_PLANET_ID)?;
        let target_team = app.world.teams.get_mut_or_err(&target_team_id)?;
        let mirror_balance = target_team.balance();
        target_team.saturating_sub_resource(Resource::SATOSHI, mirror_balance);
        let target_id = target_team.player_ids[0];

        let trade = Trade::new(
            OfferKind::Direct,
            libp2p::PeerId::random(),
            libp2p::PeerId::random(),
            app.world.own_team_id,
            target_team_id,
            None,
            app.world.players.get_or_err(&target_id)?.clone(),
            -1_000,
        );
        let own_before = app.world.get_own_team()?.balance();
        app.world.apply_trade(&trade, Tick::now())?;

        assert_eq!(app.world.get_own_team()?.balance(), own_before + 1_000);
        Ok(())
    }

    #[test]
    fn test_applied_trades_survive_a_save() -> AppResult<()> {
        let mut app = App::test_default()?;
        let trade_id = crate::types::TradeId::new_v4();
        app.world.applied_trades.insert(trade_id);

        let json = serde_json::to_string(&app.world.to_store()?)?;
        let restored: World = serde_json::from_str(&json)?;
        assert!(restored.applied_trades.contains(&trade_id));
        Ok(())
    }

    #[test]
    fn test_held_target_satoshis_are_not_charged_again() -> AppResult<()> {
        use crate::core::OfferKind;

        let mut app = App::test_default()?;
        park_own_team_on(&mut app, *DEFAULT_PLANET_ID)?;
        let proposer_team_id = network_crew_on(&mut app, *DEFAULT_PLANET_ID)?;
        let own_team_id = app.world.own_team_id;
        let own_player_id = app.world.get_own_team()?.player_ids[0];
        let own_player = app.world.players.get_or_err(&own_player_id)?.clone();

        let trade = Trade::new(
            OfferKind::Direct,
            libp2p::PeerId::random(),
            libp2p::PeerId::random(),
            proposer_team_id,
            own_team_id,
            None,
            own_player,
            -2_000,
        );
        let own_team = app.world.get_own_team_mut()?;
        own_team.sub_resource(Resource::SATOSHI, 2_000)?;
        own_team.pending_accepts.push(trade.clone());

        let own_before = app.world.get_own_team()?.balance();
        let proposer_before = app.world.teams.get_or_err(&proposer_team_id)?.balance();
        app.world.apply_trade(&trade, Tick::now())?;

        let own_team = app.world.get_own_team()?;
        assert_eq!(own_team.balance(), own_before, "no second charge");
        assert!(own_team.pending_accepts.is_empty());
        assert!(!own_team.player_ids.contains(&own_player_id));
        assert_eq!(
            app.world.teams.get_or_err(&proposer_team_id)?.balance(),
            proposer_before + 2_000
        );
        Ok(())
    }

    #[test]
    fn test_a_trade_is_recorded_only_once_it_commits() -> AppResult<()> {
        use crate::core::OfferKind;

        let mut app = App::test_default()?;
        park_own_team_on(&mut app, *DEFAULT_PLANET_ID)?;
        let target_team_id = network_crew_on(&mut app, *DEFAULT_PLANET_ID)?;
        let target_id = app.world.teams.get_or_err(&target_team_id)?.player_ids[0];
        let own_team_id = app.world.own_team_id;

        let trade = Trade::new(
            OfferKind::Direct,
            libp2p::PeerId::random(),
            libp2p::PeerId::random(),
            own_team_id,
            target_team_id,
            None,
            app.world.players.get_or_err(&target_id)?.clone(),
            1_000,
        );
        let own_team = app.world.get_own_team_mut()?;
        let own_balance = own_team.balance();
        own_team.saturating_sub_resource(Resource::SATOSHI, own_balance);

        assert!(app.world.apply_trade(&trade, Tick::now()).is_err());
        assert!(!app.world.applied_trades.contains(&trade.id));

        app.world
            .get_own_team_mut()?
            .saturating_add_resource(Resource::SATOSHI, 1_000);
        app.world.apply_trade(&trade, Tick::now())?;

        assert!(app.world.applied_trades.contains(&trade.id));
        assert!(app.world.get_own_team()?.player_ids.contains(&target_id));
        Ok(())
    }

    fn crew_with_a_listed_pirate(
        app: &mut App,
    ) -> AppResult<(crate::types::TeamId, crate::types::PlayerId)> {
        use crate::core::DockListing;

        let team_id = network_crew_on(app, *DEFAULT_PLANET_ID)?;
        let team = app.world.teams.get_mut_or_err(&team_id)?;
        let player_id = team.player_ids[0];
        team.dock_listings.push(DockListing::new(player_id, 5));
        Ok((team_id, player_id))
    }

    #[test]
    fn test_an_offer_holds_its_satoshis_and_parks_its_pirate() -> AppResult<()> {
        use super::OfferOutcome;

        let mut app = App::test_default()?;
        park_own_team_on(&mut app, *DEFAULT_PLANET_ID)?;
        let target_team_id = network_crew_on(&mut app, *DEFAULT_PLANET_ID)?;
        let target_id = app.world.teams.get_or_err(&target_team_id)?.player_ids[0];
        let own_id = app.world.get_own_team()?.player_ids[0];
        app.world.set_team_crew_role(CrewRole::Captain, own_id)?;
        let before = app.world.get_own_team()?.balance();

        let outcome =
            app.world
                .make_offer(target_id, Some(own_id), 2_000, libp2p::PeerId::random())?;

        assert_eq!(outcome, OfferOutcome::Sent);
        let own_team = app.world.get_own_team()?;
        assert_eq!(own_team.balance(), before - 2_000);
        assert!(own_team.is_offered(&own_id));
        assert_eq!(own_team.crew_roles.captain, None);
        assert_eq!(app.world.trade_outbox.len(), 1);
        assert_eq!(app.world.trade_outbox[0].id, own_team.offers[0].trade_id);
        assert!(app
            .world
            .make_offer(target_id, None, 100, libp2p::PeerId::random())
            .unwrap_err()
            .to_string()
            .contains("already made an offer"));
        Ok(())
    }

    #[test]
    fn test_a_dock_offer_leaves_the_offered_pirate_at_the_dock() -> AppResult<()> {
        use crate::core::OfferKind;

        let mut app = App::test_default()?;
        park_own_team_at_the_dock(&mut app)?;
        let (_, listed_id) = crew_with_a_listed_pirate(&mut app)?;
        let own_id = app.world.get_own_team()?.player_ids[0];

        app.world
            .make_offer(listed_id, Some(own_id), 0, libp2p::PeerId::random())?;

        let own_team = app.world.get_own_team()?;
        assert_eq!(own_team.offers[0].kind, OfferKind::Dock);
        assert!(own_team.is_offered(&own_id));
        Ok(())
    }

    #[test]
    fn test_retiring_refunds_and_a_dock_offered_pirate_waits_at_the_dock() -> AppResult<()> {
        use crate::network::types::NetworkRequestState;

        let mut app = App::test_default()?;
        park_own_team_at_the_dock(&mut app)?;
        let (_, listed_id) = crew_with_a_listed_pirate(&mut app)?;
        let own_id = app.world.get_own_team()?.player_ids[0];
        let before = app.world.get_own_team()?.balance();
        let peer_id = libp2p::PeerId::random();
        app.world
            .make_offer(listed_id, Some(own_id), 1_500, peer_id)?;
        let trade_id = app.world.get_own_team()?.offers[0].trade_id;
        app.world.trade_outbox.clear();

        park_own_team_on(&mut app, *DEFAULT_PLANET_ID)?;
        app.world.retire_offer(&trade_id, peer_id)?;

        let own_team = app.world.get_own_team()?;
        assert_eq!(own_team.balance(), before);
        assert!(own_team.offers.is_empty());
        assert!(own_team.is_waiting(&own_id));
        assert!(matches!(
            app.world.trade_outbox[0].state,
            NetworkRequestState::Failed { .. }
        ));

        park_own_team_at_the_dock(&mut app)?;
        app.world.tick_collect_waiting_pirates(Tick::now())?;
        assert!(app
            .world
            .get_own_team()?
            .active_player_ids()
            .contains(&own_id));
        Ok(())
    }

    #[test]
    fn test_a_local_crew_decides_on_the_spot() -> AppResult<()> {
        use super::OfferOutcome;

        let mut app = App::test_default()?;
        park_own_team_on(&mut app, *DEFAULT_PLANET_ID)?;
        let local_team_id = app
            .world
            .teams
            .values()
            .find(|team| team.id != app.world.own_team_id)
            .expect("another crew")
            .id;
        app.world
            .teams
            .get_mut_or_err(&local_team_id)?
            .current_location = TeamLocation::OnPlanet {
            planet_id: *DEFAULT_PLANET_ID,
        };
        let target_id = app.world.teams.get_or_err(&local_team_id)?.player_ids[0];
        let hire_cost = app.world.players.get_or_err(&target_id)?.hire_cost();
        app.world
            .get_own_team_mut()?
            .add_resource(Resource::SATOSHI, hire_cost)?;
        let peer_id = libp2p::PeerId::random();

        assert_eq!(
            app.world.make_offer(target_id, None, 0, peer_id)?,
            OfferOutcome::Refused
        );
        let before = app.world.get_own_team()?.balance();
        assert_eq!(
            app.world
                .make_offer(target_id, None, hire_cost as i64, peer_id)?,
            OfferOutcome::Accepted
        );
        let own_team = app.world.get_own_team()?;
        assert!(own_team.player_ids.contains(&target_id));
        assert!(own_team.offers.is_empty());
        assert_eq!(own_team.balance(), before - hire_cost);
        assert!(app.world.trade_outbox.is_empty());
        Ok(())
    }

    #[test]
    fn test_an_offer_to_a_playing_local_crew_is_refused() -> AppResult<()> {
        let mut app = App::test_default()?;
        park_own_team_on(&mut app, *DEFAULT_PLANET_ID)?;
        let local_team_id = app
            .world
            .teams
            .values()
            .find(|team| team.id != app.world.own_team_id)
            .expect("another crew")
            .id;
        let local_team = app.world.teams.get_mut_or_err(&local_team_id)?;
        local_team.current_location = TeamLocation::OnPlanet {
            planet_id: *DEFAULT_PLANET_ID,
        };
        local_team.current_game = Some(crate::types::GameId::new_v4());
        let target_id = local_team.player_ids[0];
        let hire_cost = app.world.players.get_or_err(&target_id)?.hire_cost();
        app.world
            .get_own_team_mut()?
            .add_resource(Resource::SATOSHI, hire_cost)?;
        let before = app.world.get_own_team()?.balance();

        let error = app
            .world
            .make_offer(target_id, None, hire_cost as i64, libp2p::PeerId::random())
            .unwrap_err();

        assert!(error
            .to_string()
            .contains("is playing, make your offer after the game"));
        let own_team = app.world.get_own_team()?;
        assert_eq!(own_team.balance(), before);
        assert!(own_team.offers.is_empty());
        assert!(app.world.trade_outbox.is_empty());
        Ok(())
    }

    #[test]
    fn test_trade_for_offer_rebuilds_the_trade_that_was_sent() -> AppResult<()> {
        let mut app = App::test_default()?;
        park_own_team_on(&mut app, *DEFAULT_PLANET_ID)?;
        let target_team_id = network_crew_on(&mut app, *DEFAULT_PLANET_ID)?;
        let target_id = app.world.teams.get_or_err(&target_team_id)?.player_ids[0];
        let peer_id = libp2p::PeerId::random();
        app.world.make_offer(target_id, None, 1_000, peer_id)?;

        let sent = app.world.trade_outbox[0].clone();
        let offer = app.world.get_own_team()?.offers[0].clone();
        let rebuilt = app
            .world
            .trade_for_offer(&offer, peer_id)
            .expect("the trade");
        assert_eq!(
            serde_json::to_string(&rebuilt)?,
            serde_json::to_string(&sent)?
        );
        Ok(())
    }

    #[test]
    fn test_open_trades_resend_every_offer() -> AppResult<()> {
        let mut app = App::test_default()?;
        park_own_team_on(&mut app, *DEFAULT_PLANET_ID)?;
        let target_team_id = network_crew_on(&mut app, *DEFAULT_PLANET_ID)?;
        let target_id = app.world.teams.get_or_err(&target_team_id)?.player_ids[0];
        let peer_id = libp2p::PeerId::random();
        app.world.make_offer(target_id, None, 1_000, peer_id)?;

        let open = app.world.open_trades(peer_id);
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].id, app.world.get_own_team()?.offers[0].trade_id);
        Ok(())
    }

    fn receive_offer_from_a_new_crew(
        app: &mut App,
        target_id: crate::types::PlayerId,
        route: crate::core::OfferKind,
        satoshis: i64,
    ) -> AppResult<(Trade, crate::types::TeamId)> {
        let proposer_id = network_crew_on(app, *DEFAULT_PLANET_ID)?;
        let proposer_peer_id = app
            .world
            .teams
            .get_or_err(&proposer_id)?
            .peer_id
            .expect("a peer crew");
        let trade = Trade::new(
            route,
            proposer_peer_id,
            libp2p::PeerId::random(),
            proposer_id,
            app.world.own_team_id,
            None,
            app.world.players.get_or_err(&target_id)?.clone(),
            satoshis,
        );
        app.world.receive_offer(trade.clone(), Tick::now())?;
        Ok((trade, proposer_id))
    }

    fn first_own_pirate(app: &App) -> AppResult<crate::types::PlayerId> {
        Ok(app.world.get_own_team()?.player_ids[0])
    }

    fn with_state(trade: &Trade, state: crate::network::types::NetworkRequestState) -> Trade {
        let mut trade = trade.clone();
        trade.state = state;
        trade
    }

    #[test]
    fn test_an_offer_from_an_absent_crew_cannot_be_accepted() -> AppResult<()> {
        use crate::core::{OfferKind, MINUTES};

        let mut app = App::test_default()?;
        park_own_team_on(&mut app, *DEFAULT_PLANET_ID)?;
        let target_id = first_own_pirate(&app)?;
        let (trade, proposer_id) =
            receive_offer_from_a_new_crew(&mut app, target_id, OfferKind::Direct, 1_000)?;

        app.world
            .network_team_last_heard
            .insert(proposer_id, Tick::now() - 10 * MINUTES);
        assert!(app
            .world
            .can_accept_offer(&trade.id, Tick::now())
            .unwrap_err()
            .to_string()
            .contains("offline"));

        app.world
            .network_team_last_heard
            .insert(proposer_id, Tick::now());
        assert!(app.world.can_accept_offer(&trade.id, Tick::now()).is_ok());
        Ok(())
    }

    #[test]
    fn test_an_offer_cannot_be_accepted_or_completed_while_in_a_tournament() -> AppResult<()> {
        use crate::core::{OfferKind, TournamentRegistrationState};
        use crate::network::types::NetworkRequestState;

        let mut app = App::test_default()?;
        park_own_team_on(&mut app, *DEFAULT_PLANET_ID)?;
        let target_id = first_own_pirate(&app)?;
        let (trade, _) =
            receive_offer_from_a_new_crew(&mut app, target_id, OfferKind::Direct, 1_000)?;
        let other_team_id = network_crew_on(&mut app, *DEFAULT_PLANET_ID)?;
        let other_id = app.world.teams.get_or_err(&other_team_id)?.player_ids[0];
        let peer_id = libp2p::PeerId::random();
        app.world.make_offer(other_id, None, 500, peer_id)?;
        let syn_ack = with_state(&app.world.trade_outbox[0], NetworkRequestState::SynAck);
        app.world.trade_outbox.clear();

        app.world.get_own_team_mut()?.tournament_registration_state =
            TournamentRegistrationState::Confirmed {
                tournament_id: TournamentId::new_v4(),
            };

        assert!(app
            .world
            .can_accept_offer(&trade.id, Tick::now())
            .unwrap_err()
            .to_string()
            .contains("is in a tournament"));
        assert!(app
            .world
            .receive_syn_ack(&syn_ack, Tick::now())?
            .expect("a message")
            .contains("is in a tournament"));
        assert!(app.world.get_own_team()?.offers.is_empty());
        assert!(!app.world.get_own_team()?.player_ids.contains(&other_id));
        Ok(())
    }

    #[test]
    fn test_accepting_locks_the_pirate_and_sends_a_syn_ack() -> AppResult<()> {
        use crate::core::OfferKind;
        use crate::network::types::NetworkRequestState;

        let mut app = App::test_default()?;
        park_own_team_on(&mut app, *DEFAULT_PLANET_ID)?;
        let target_id = first_own_pirate(&app)?;
        let (trade, _) =
            receive_offer_from_a_new_crew(&mut app, target_id, OfferKind::Direct, 1_000)?;
        app.world.trade_outbox.clear();

        app.world.accept_offer(&trade.id, Tick::now())?;

        let own_team = app.world.get_own_team()?;
        assert!(own_team.is_leaving(&target_id));
        assert!(own_team
            .can_release_player(app.world.players.get_or_err(&target_id)?)
            .is_err());
        assert_eq!(app.world.trade_outbox[0].state, NetworkRequestState::SynAck);
        assert!(app
            .world
            .accept_offer(&trade.id, Tick::now())
            .unwrap_err()
            .to_string()
            .contains("already leaving"));
        Ok(())
    }

    #[test]
    fn test_an_ack_completes_the_accept_and_declines_the_rest() -> AppResult<()> {
        use crate::core::OfferKind;
        use crate::network::types::NetworkRequestState;

        let mut app = App::test_default()?;
        park_own_team_on(&mut app, *DEFAULT_PLANET_ID)?;
        let target_id = first_own_pirate(&app)?;
        let (winner, winner_team_id) =
            receive_offer_from_a_new_crew(&mut app, target_id, OfferKind::Direct, 1_000)?;
        let (rival, _) =
            receive_offer_from_a_new_crew(&mut app, target_id, OfferKind::Direct, 500)?;
        let before = app.world.get_own_team()?.balance();
        app.world.accept_offer(&winner.id, Tick::now())?;
        app.world.trade_outbox.clear();

        let message = app
            .world
            .receive_ack(&with_state(&winner, NetworkRequestState::Ack), Tick::now())?;

        assert!(message.is_some());
        let own_team = app.world.get_own_team()?;
        assert!(!own_team.player_ids.contains(&target_id));
        assert!(own_team.pending_accepts.is_empty());
        assert!(own_team.received_trades.is_empty());
        assert_eq!(own_team.balance(), before + 1_000);
        assert!(app
            .world
            .teams
            .get_or_err(&winner_team_id)?
            .player_ids
            .contains(&target_id));
        assert!(app
            .world
            .trade_outbox
            .iter()
            .any(|trade| trade.id == rival.id
                && matches!(trade.state, NetworkRequestState::Failed { .. })));
        Ok(())
    }

    #[test]
    fn test_a_syn_ack_applies_once_and_a_duplicate_gets_another_ack() -> AppResult<()> {
        use crate::network::types::NetworkRequestState;

        let mut app = App::test_default()?;
        park_own_team_on(&mut app, *DEFAULT_PLANET_ID)?;
        let target_team_id = network_crew_on(&mut app, *DEFAULT_PLANET_ID)?;
        let target_id = app.world.teams.get_or_err(&target_team_id)?.player_ids[0];
        app.world
            .make_offer(target_id, None, 2_000, libp2p::PeerId::random())?;
        let syn_ack = with_state(&app.world.trade_outbox[0], NetworkRequestState::SynAck);
        app.world.trade_outbox.clear();
        let balance = app.world.get_own_team()?.balance();

        assert!(app.world.receive_syn_ack(&syn_ack, Tick::now())?.is_some());
        let own_team = app.world.get_own_team()?;
        assert!(own_team.player_ids.contains(&target_id));
        assert!(own_team.offers.is_empty());
        assert_eq!(own_team.balance(), balance);
        assert_eq!(app.world.trade_outbox[0].state, NetworkRequestState::Ack);

        app.world.trade_outbox.clear();
        app.world.receive_syn_ack(&syn_ack, Tick::now())?;
        assert_eq!(app.world.trade_outbox[0].state, NetworkRequestState::Ack);
        assert_eq!(app.world.get_own_team()?.balance(), balance);
        Ok(())
    }

    #[test]
    fn test_a_syn_ack_after_retiring_gets_failed() -> AppResult<()> {
        use crate::network::types::NetworkRequestState;

        let mut app = App::test_default()?;
        park_own_team_on(&mut app, *DEFAULT_PLANET_ID)?;
        let target_team_id = network_crew_on(&mut app, *DEFAULT_PLANET_ID)?;
        let target_id = app.world.teams.get_or_err(&target_team_id)?.player_ids[0];
        let peer_id = libp2p::PeerId::random();
        app.world.make_offer(target_id, None, 2_000, peer_id)?;
        let syn_ack = with_state(&app.world.trade_outbox[0], NetworkRequestState::SynAck);
        app.world.retire_offer(&syn_ack.id, peer_id)?;
        app.world.trade_outbox.clear();

        app.world.receive_syn_ack(&syn_ack, Tick::now())?;

        assert!(matches!(
            app.world.trade_outbox[0].state,
            NetworkRequestState::Failed { .. }
        ));
        assert!(!app.world.get_own_team()?.player_ids.contains(&target_id));
        Ok(())
    }

    #[test]
    fn test_open_trades_resend_a_pending_accept() -> AppResult<()> {
        use crate::core::OfferKind;
        use crate::network::types::NetworkRequestState;

        let mut app = App::test_default()?;
        park_own_team_on(&mut app, *DEFAULT_PLANET_ID)?;
        let target_id = first_own_pirate(&app)?;
        let (trade, _) =
            receive_offer_from_a_new_crew(&mut app, target_id, OfferKind::Direct, 1_000)?;
        app.world.accept_offer(&trade.id, Tick::now())?;

        let open = app.world.open_trades(libp2p::PeerId::random());
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].id, trade.id);
        assert_eq!(open[0].state, NetworkRequestState::SynAck);
        Ok(())
    }

    #[test]
    fn test_asked_satoshis_are_held_from_accept_until_it_ends() -> AppResult<()> {
        use crate::core::OfferKind;
        use crate::network::types::NetworkRequestState;

        let mut app = App::test_default()?;
        park_own_team_on(&mut app, *DEFAULT_PLANET_ID)?;
        let target_id = first_own_pirate(&app)?;
        let before = app.world.get_own_team()?.balance();
        let (asked, _) =
            receive_offer_from_a_new_crew(&mut app, target_id, OfferKind::Direct, -2_000)?;

        app.world.accept_offer(&asked.id, Tick::now())?;
        assert_eq!(app.world.get_own_team()?.balance(), before - 2_000);

        let failed = with_state(
            &asked,
            NetworkRequestState::Failed {
                error_message: "Offer retired".to_string(),
            },
        );
        app.world.receive_failed(&failed, "Offer retired")?;
        assert_eq!(app.world.get_own_team()?.balance(), before);

        let (asked_again, _) =
            receive_offer_from_a_new_crew(&mut app, target_id, OfferKind::Direct, -2_000)?;
        app.world.accept_offer(&asked_again.id, Tick::now())?;
        app.world.receive_ack(
            &with_state(&asked_again, NetworkRequestState::Ack),
            Tick::now(),
        )?;
        assert_eq!(
            app.world.get_own_team()?.balance(),
            before - 2_000,
            "the held satoshis are the payment"
        );
        Ok(())
    }

    #[test]
    fn test_a_resent_offer_that_is_no_longer_valid_is_dropped() -> AppResult<()> {
        use crate::core::OfferKind;
        use crate::network::types::NetworkRequestState;

        let mut app = App::test_default()?;
        park_own_team_on(&mut app, *DEFAULT_PLANET_ID)?;
        let target_id = first_own_pirate(&app)?;
        let (trade, _) =
            receive_offer_from_a_new_crew(&mut app, target_id, OfferKind::Direct, 1_000)?;
        app.world.trade_outbox.clear();

        park_own_team_on(&mut app, *crate::core::GALAXY_ROOT_ID)?;
        assert!(!app.world.receive_offer(trade.clone(), Tick::now())?);

        assert!(app.world.get_own_team()?.received_trades.is_empty());
        assert!(matches!(
            app.world.trade_outbox[0].state,
            NetworkRequestState::Failed { .. }
        ));
        Ok(())
    }

    #[test]
    fn test_a_resend_of_the_offer_being_accepted_is_ignored() -> AppResult<()> {
        use crate::core::OfferKind;

        let mut app = App::test_default()?;
        park_own_team_on(&mut app, *DEFAULT_PLANET_ID)?;
        let target_id = first_own_pirate(&app)?;
        let (trade, _) =
            receive_offer_from_a_new_crew(&mut app, target_id, OfferKind::Direct, 1_000)?;
        app.world.accept_offer(&trade.id, Tick::now())?;
        app.world.trade_outbox.clear();

        assert!(!app.world.receive_offer(trade.clone(), Tick::now())?);
        assert!(app.world.trade_outbox.is_empty());
        assert!(app.world.get_own_team()?.is_leaving(&target_id));
        Ok(())
    }

    #[test]
    fn test_a_failed_from_the_proposer_clears_the_lock() -> AppResult<()> {
        use crate::core::OfferKind;
        use crate::network::types::NetworkRequestState;

        let mut app = App::test_default()?;
        park_own_team_on(&mut app, *DEFAULT_PLANET_ID)?;
        let target_id = first_own_pirate(&app)?;
        let (trade, _) =
            receive_offer_from_a_new_crew(&mut app, target_id, OfferKind::Direct, 1_000)?;
        app.world.accept_offer(&trade.id, Tick::now())?;

        let failed = with_state(
            &trade,
            NetworkRequestState::Failed {
                error_message: "Offer retired".to_string(),
            },
        );
        app.world.receive_failed(&failed, "Offer retired")?;

        let own_team = app.world.get_own_team()?;
        assert!(own_team.player_ids.contains(&target_id));
        assert!(!own_team.is_leaving(&target_id));
        assert!(own_team.received_trades.is_empty());
        Ok(())
    }

    #[test]
    fn test_a_failed_from_the_target_refunds_the_offer() -> AppResult<()> {
        use crate::network::types::NetworkRequestState;

        let mut app = App::test_default()?;
        park_own_team_on(&mut app, *DEFAULT_PLANET_ID)?;
        let target_team_id = network_crew_on(&mut app, *DEFAULT_PLANET_ID)?;
        let target_id = app.world.teams.get_or_err(&target_team_id)?.player_ids[0];
        let before = app.world.get_own_team()?.balance();
        app.world
            .make_offer(target_id, None, 2_000, libp2p::PeerId::random())?;
        let failed = with_state(
            &app.world.trade_outbox[0],
            NetworkRequestState::Failed {
                error_message: "declined".to_string(),
            },
        );

        assert!(app.world.receive_failed(&failed, "declined")?.is_some());
        assert!(app.world.get_own_team()?.offers.is_empty());
        assert_eq!(app.world.get_own_team()?.balance(), before);
        Ok(())
    }

    #[test]
    fn test_presence_reads_our_own_clock() -> AppResult<()> {
        use crate::network::types::NetworkTeam;

        let mut app = App::test_default()?;
        let (team, players, _, _) = crew_listing_a_pirate(&app)?;
        let team_id = team.id;
        app.world
            .add_network_team(NetworkTeam::new(team, players, vec![]), 1)?;
        assert!(app.world.is_team_present(&team_id, Tick::now()));
        Ok(())
    }

    #[test]
    fn test_an_offer_for_a_pirate_no_longer_at_the_dock_is_refused() -> AppResult<()> {
        use crate::core::OfferKind;
        use crate::network::types::NetworkRequestState;

        let mut app = App::test_default()?;
        park_own_team_on(&mut app, *DEFAULT_PLANET_ID)?;
        let target_id = first_own_pirate(&app)?;
        receive_offer_from_a_new_crew(&mut app, target_id, OfferKind::Dock, 1_000)?;

        assert!(app.world.get_own_team()?.received_trades.is_empty());
        assert!(matches!(
            app.world.trade_outbox.last().expect("a reply").state,
            NetworkRequestState::Failed { .. }
        ));
        Ok(())
    }

    #[test]
    fn test_recalling_declines_the_offers_on_that_pirate() -> AppResult<()> {
        use crate::core::OfferKind;
        use crate::network::types::NetworkRequestState;

        let mut app = App::test_default()?;
        park_own_team_at_the_dock(&mut app)?;
        let target_id = first_own_pirate(&app)?;
        app.world.leave_player_at_dock(target_id, Tick::now())?;
        receive_offer_from_a_new_crew(&mut app, target_id, OfferKind::Dock, 1_000)?;
        app.world.trade_outbox.clear();

        app.world.recall_player_from_dock(target_id)?;

        assert!(app.world.get_own_team()?.received_trades.is_empty());
        assert!(matches!(
            app.world.trade_outbox[0].state,
            NetworkRequestState::Failed { .. }
        ));
        Ok(())
    }
}
