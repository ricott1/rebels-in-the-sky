use super::constants::*;
use super::jersey::{Jersey, JerseyStyle};
use super::planet::Planet;
use super::player::{Player, Trait};
use super::position::{GamePosition, MAX_GAME_POSITION};
use super::resources::Resource;
use super::role::CrewRole;
use super::skill::{GameSkill, MAX_SKILL};
use super::spaceship::Spaceship;
use super::team::Team;
use super::types::{PlayerLocation, TeamBonus, TeamLocation};
use super::utils::{is_default, PLANET_DATA, TEAM_DATA};
use crate::core::{
    AutonomousStrategy, GameResult, Honour, Rated, RatedPlayers, Skill,
    TournamentRegistrationState, Upgrade, MIN_SKILL,
};
use crate::game_engine::game::{Game, GameSummary};
use crate::game_engine::tactic::Tactic;
use crate::game_engine::types::{Possession, TeamInGame};
use crate::game_engine::{
    TournamentId, TournamentState, TournamentSummary, RECOVERING_TIREDNESS_PER_SHORT_TICK,
};
use crate::image::color_map::ColorMap;
use crate::network::types::{NetworkGame, NetworkTeam};
use crate::space_adventure::SpaceAdventure;
use crate::store::{save_game, save_tournament};
use crate::types::*;
use crate::ui::{PopupMessage, UiCallback};
use anyhow::anyhow;
use itertools::Itertools;
use libp2p::PeerId;
use rand::seq::{IteratorRandom, SliceRandom};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use strum::IntoEnumIterator;

// const GAME_CLEANUP_TIME: Tick = 10 * SECONDS;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct World {
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
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub players: PlayerMap,
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
    pub network_keypair: Option<Vec<u8>>, // Allows to re-establish the same PeerId across sessions.
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub tournaments: TournamentMap,
    #[serde(skip)]
    pub recently_finished_tournaments: TournamentMap, // Holds finished tournaments for the session, but are not persisted.
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub past_tournaments: TournamentSummaryMap, // Holds summary of finished tournaments, persisted.
}

impl World {
    pub fn new(seed: Option<u64>) -> Self {
        let mut planets = HashMap::new();
        let data_planets: Vec<Planet> = PLANET_DATA.iter().cloned().collect();
        for planet in data_planets.iter() {
            planets.insert(planet.id, planet.clone());
        }

        Self {
            seed: seed.unwrap_or(rand::random()),
            planets,
            ..Default::default()
        }
    }

    pub fn initialize(&mut self, generate_local_world: bool) -> AppResult<()> {
        let rng = &mut ChaCha8Rng::seed_from_u64(self.seed);
        for planet in PLANET_DATA.iter() {
            self.populate_planet(rng, planet)?;
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

    fn populate_planet(&mut self, rng: &mut ChaCha8Rng, planet: &Planet) -> AppResult<()> {
        // generate free pirates per each planet
        let number_free_pirates = planet.total_population();
        let mut position = 0 as GamePosition;
        let own_team_base_level = if let Ok(own_team) = self.get_own_team() {
            own_team.reputation / 5.0
        } else {
            0.0
        };

        for _ in 0..number_free_pirates {
            let base_level = own_team_base_level + rng.random_range(0.0..4.0);
            self.generate_random_player(rng, Some(position), planet, base_level)?;
            position = (position + 1) % MAX_GAME_POSITION;
        }

        Ok(())
    }

    pub fn generate_local_world(&mut self, rng: &mut ChaCha8Rng) -> AppResult<()> {
        let mut team_data = TEAM_DATA.clone();
        team_data.shuffle(rng);

        let home_planet_ids = self
            .planets
            .values()
            .filter(|planet| planet.total_population() > 0)
            .map(|p| p.id)
            .collect::<Vec<PlanetId>>();

        for idx in 0..team_data.len() {
            let (team_name, ship_name) = team_data[idx].clone();
            // Assign 2 teams to each planet
            let home_planet_id = home_planet_ids[(idx / 2) % home_planet_ids.len()];
            self.generate_random_team(rng, home_planet_id, team_name, ship_name, None)?;
        }
        Ok(())
    }

    pub fn generate_random_team(
        &mut self,
        rng: &mut ChaCha8Rng,
        home_planet_id: PlanetId,
        team_name: String,
        ship_name: String,
        team_base_level: Option<f32>,
    ) -> AppResult<TeamId> {
        let team = Team::random(Some(rng))
            .with_name(team_name)
            .with_spaceship_name(ship_name)
            .with_home_planet(home_planet_id);
        let team_id = team.id;

        let mut planet = self.get_planet_or_err(&team.home_planet_id)?.clone();
        planet.team_ids.push(team_id);

        self.teams.insert(team.id, team);

        let team_base_level = team_base_level.unwrap_or(rng.random_range(2..=14) as f32);
        for position in 0..MAX_GAME_POSITION {
            let player_id =
                self.generate_random_player(rng, Some(position), &planet, team_base_level)?;
            self.add_player_to_team(&player_id, &team_id)?;
        }

        loop {
            let player_id = self.generate_random_player(rng, None, &planet, team_base_level)?;
            self.add_player_to_team(&player_id, &team_id)?;
            let team = self.get_team_or_err(&team_id)?;
            if team.player_ids.len() == team.spaceship.crew_capacity() as usize {
                break;
            }
        }
        self.planets.insert(planet.id, planet);

        let player_ids = self.get_team_or_err(&team_id)?.player_ids.clone();
        self.auto_assign_crew_roles(player_ids)?;

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
        self.teams.insert(team.id, team);

        for player_id in players {
            self.add_player_to_team(&player_id, &team_id)?;
        }

        let player_ids = self.get_team_or_err(&team_id)?.player_ids.clone();
        self.auto_assign_crew_roles(player_ids)?;

        let mut planet = self.get_planet_or_err(&home_planet_id)?.clone();
        planet.team_ids.push(team_id);
        self.planets.insert(planet.id, planet);

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

        let mut satellite_of_planet = self.get_planet_or_err(&satellite_of)?.clone();
        satellite_of_planet.satellites.push(asteroid_id);
        satellite_of_planet.version += 1;
        self.planets.insert(satellite_of, satellite_of_planet);

        Ok(asteroid_id)
    }

    fn generate_random_player(
        &mut self,
        rng: &mut ChaCha8Rng,
        position: Option<GamePosition>,
        home_planet: &Planet,
        base_level: f32,
    ) -> AppResult<PlayerId> {
        let player = Player::default()
            .with_position(position)
            .with_home_planet(home_planet.id)
            .with_base_level(base_level)
            .randomize(Some(rng));

        // random(rng, position, home_planet, base_level);
        let player_id = player.id;
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
            let player = self.get_player_or_err(player_id)?;
            let captain_bonus = TeamBonus::Reputation.current_player_bonus(player)
                + TeamBonus::TradePrice.current_player_bonus(player);
            let pilot_bonus = TeamBonus::Exploration.current_player_bonus(player)
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

    fn remove_player_from_role(&mut self, role: CrewRole, player_id: PlayerId) -> AppResult<()> {
        // cannot be demoted from Mozzo
        if role == CrewRole::Mozzo {
            return Ok(());
        }

        let mut player = self.get_player_or_err(&player_id)?.clone();
        let player_previous_role = player.info.crew_role;

        let team_id = if let Some(team_id) = player.team {
            team_id
        } else {
            return Err(anyhow!("Player {player_id:?} is not in a team"));
        };

        let mut team = self.get_team_or_err(&team_id)?.clone();

        team.can_set_crew_role(&player)?;

        let previous_spaceship_speed_bonus =
            TeamBonus::SpaceshipSpeed.current_team_bonus(self, &team.id)?;
        let previous_upgrade_bonus = TeamBonus::Upgrades.current_team_bonus(self, &team.id)?;

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

        if role == CrewRole::Pilot {
            // Grab team again. We need to do this to ensure that the speed bonus is calculated correctly.
            let mut team = self.get_team_or_err(&team_id)?.clone();
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
                let bonus = TeamBonus::SpaceshipSpeed.current_team_bonus(self, &team_id)?;

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
        } else if role == CrewRole::Engineer {
            // Grab team again. We need to do this to ensure that the upgrade bonus is calculated correctly.
            let mut team = self.get_team_or_err(&team_id)?.clone();
            // If spaceship or any asteroid has a pending upgrade and engineer was updated recalculate upgrade duration.
            if let Some(upgrade) = team.spaceship.pending_upgrade {
                let new_start = Tick::now();
                let time_elapsed = new_start - upgrade.started;
                let bonus = TeamBonus::Upgrades.current_team_bonus(self, &team_id)?;

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
                let asteroid = self.get_planet_or_err(asteroid_id)?;
                if let Some(upgrade) = asteroid.pending_upgrade {
                    let new_start = Tick::now();
                    let time_elapsed = new_start - upgrade.started;
                    let bonus = TeamBonus::Upgrades.current_team_bonus(self, &team_id)?;

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
        self.dirty_ui = true;

        Ok(())
    }

    pub fn set_team_crew_role(&mut self, role: CrewRole, player_id: PlayerId) -> AppResult<()> {
        let mut player = self.get_player_or_err(&player_id)?.clone();
        // If role is current player role, than we are removing the player from that role.
        if player.info.crew_role == role {
            return self.remove_player_from_role(role, player_id);
        }

        let player_previous_role = player.info.crew_role;

        let team_id = if let Some(team_id) = player.team {
            team_id
        } else {
            return Err(anyhow!("Player {player_id:?} is not in a team"));
        };

        let mut team = self.get_team_or_err(&team_id)?.clone();

        team.can_set_crew_role(&player)?;

        let previous_spaceship_speed_bonus =
            TeamBonus::SpaceshipSpeed.current_team_bonus(self, &team.id)?;

        let previous_upgrade_bonus = TeamBonus::Upgrades.current_team_bonus(self, &team.id)?;

        let jersey = if team.is_travelling() {
            Jersey {
                style: JerseyStyle::Pirate,
                color: team.jersey.color,
            }
        } else {
            team.jersey.clone()
        };

        let current_role_player_id = match role {
            CrewRole::Captain => team.crew_roles.captain,
            CrewRole::Pilot => team.crew_roles.pilot,
            CrewRole::Doctor => team.crew_roles.doctor,
            CrewRole::Engineer => team.crew_roles.engineer,
            //We don't need to check for mozzo because we can have several mozzos.
            CrewRole::Mozzo => None,
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
            CrewRole::Mozzo => {
                team.crew_roles.mozzo.retain(|&id| id != player.id);
            }
        }

        // Demote previous crew role player to mozzo.
        if let Some(crew_player_id) = current_role_player_id {
            let mut current_role_player = self.get_player_or_err(&crew_player_id)?.clone();
            current_role_player.info.crew_role = CrewRole::Mozzo;
            team.crew_roles.mozzo.push(current_role_player.id);
            // Demoted player is a bit demoralized :(
            current_role_player.morale =
                (current_role_player.morale + MORALE_DEMOTION_MALUS).bound();
            current_role_player.set_jersey(&jersey);

            self.players.insert(crew_player_id, current_role_player);
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
            CrewRole::Mozzo => {
                team.crew_roles.mozzo.push(player_id);
            }
        }
        player.info.crew_role = role;
        player.set_jersey(&jersey);

        self.players.insert(player.id, player);
        self.teams.insert(team.id, team);

        if role == CrewRole::Pilot || player_previous_role == CrewRole::Pilot {
            // Grab team again. We need to do this to ensure that the speed bonus is calculated correctly.
            let mut team = self.get_team_or_err(&team_id)?.clone();
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
                let bonus = TeamBonus::SpaceshipSpeed.current_team_bonus(self, &team_id)?;

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
            let mut team = self.get_team_or_err(&team_id)?.clone();
            // If spaceship or any asteroid has a pending upgrade and engineer was updated recalculate upgrade duration.
            if let Some(upgrade) = team.spaceship.pending_upgrade {
                let new_start = Tick::now();
                let time_elapsed = new_start - upgrade.started;
                let bonus = TeamBonus::Upgrades.current_team_bonus(self, &team_id)?;

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
                let asteroid = self.get_planet_or_err(asteroid_id)?;
                if let Some(upgrade) = asteroid.pending_upgrade {
                    let new_start = Tick::now();
                    let time_elapsed = new_start - upgrade.started;
                    let bonus = TeamBonus::Upgrades.current_team_bonus(self, &team_id)?;

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
        self.dirty_ui = true;
        Ok(())
    }

    pub fn next_free_pirates_refresh(&self) -> Tick {
        // Returns the time to the next FA refresh in milliseconds
        let next_refresh = self.last_tick_long_interval + TickInterval::LONG;
        next_refresh.saturating_sub(self.last_tick_short_interval)
    }

    fn add_player_to_team(&mut self, player_id: &PlayerId, team_id: &TeamId) -> AppResult<()> {
        let mut player = self.get_player_or_err(player_id)?.clone();
        let mut team = self.get_team_or_err(team_id)?.clone();
        team.can_add_player(&player)?;

        team.player_ids.push(player.id);
        team.player_ids = Team::best_position_assignment(
            team.player_ids
                .iter()
                .map(|&id| self.get_player(&id).unwrap())
                .collect(),
        );
        team.version += 1;

        player.team = Some(team.id);
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
    ) -> AppResult<()> {
        let player = self.get_player_or_err(player_id)?;
        let mut team = self.get_team_or_err(team_id)?.clone();
        team.can_hire_player(player)?;
        team.sub_resource(Resource::SATOSHI, player.hire_cost(team.reputation))?;
        self.teams.insert(team.id, team);

        self.add_player_to_team(player_id, team_id)?;

        Ok(())
    }

    pub fn swap_players_team(
        &mut self,
        player_id1: PlayerId,
        player_id2: PlayerId,
    ) -> AppResult<()> {
        let team_id1 = self
            .get_player_or_err(&player_id1)?
            .team
            .ok_or(anyhow!("Player swapped should have a team"))?;
        let team_id2 = self
            .get_player_or_err(&player_id2)?
            .team
            .ok_or(anyhow!("Player swapped should have a team"))?;

        self.release_player_from_team(player_id1)?;
        self.release_player_from_team(player_id2)?;
        self.add_player_to_team(&player_id1, &team_id2)?;
        self.add_player_to_team(&player_id2, &team_id1)?;
        Ok(())
    }

    pub fn release_player_from_team(&mut self, player_id: PlayerId) -> AppResult<()> {
        let mut player = self.get_player_mut_or_err(&player_id)?.clone();

        let team_id = if let Some(team_id) = player.team {
            team_id
        } else {
            return Err(anyhow!("Cannot release player with no team"));
        };
        let mut team = self.get_team_or_err(&team_id)?.clone();
        team.can_release_player(&player)?;

        team.player_ids.retain(|&p| p != player.id);
        team.player_ids = Team::best_position_assignment(
            team.player_ids
                .iter()
                .map(|&id| self.get_player(&id).unwrap())
                .collect(),
        );
        team.version += 1;

        player.team = None;
        match player.info.crew_role {
            CrewRole::Pilot => team.crew_roles.pilot = None,
            CrewRole::Captain => team.crew_roles.captain = None,
            CrewRole::Doctor => team.crew_roles.doctor = None,
            CrewRole::Engineer => team.crew_roles.engineer = None,
            CrewRole::Mozzo => team.crew_roles.mozzo.retain(|&p| p != player.id),
        }
        player.info.crew_role = CrewRole::Mozzo;
        player.add_morale(MORALE_RELEASE_MALUS);
        player.image.remove_jersey();
        player.compose_image()?;
        match team.current_location {
            TeamLocation::OnPlanet { planet_id } => {
                player.current_location = PlayerLocation::OnPlanet { planet_id };
            }
            _ => return Err(anyhow!("Cannot release player while travelling")),
        }
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

        let planet = self.get_planet_or_err(&planet_id)?;

        // Give morale bonus to players based on planet populations
        for (_, player) in home_team_in_game.players.iter_mut() {
            let morale_bonus = planet
                .populations
                .get(&player.info.population)
                .copied()
                .unwrap_or_default() as f32
                * MORALE_GAME_POPULATION_MODIFIER;
            player.add_morale(morale_bonus);
        }

        for (_, player) in away_team_in_game.players.iter_mut() {
            let morale_bonus = planet
                .populations
                .get(&player.info.population)
                .copied()
                .unwrap_or_default() as f32
                * MORALE_GAME_POPULATION_MODIFIER;
            player.add_morale(morale_bonus);
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
        let mut home_team = self.get_team_or_err(&home_team_in_game.team_id)?.clone();
        let mut away_team = self.get_team_or_err(&away_team_in_game.team_id)?.clone();

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

        home_team.current_game = Some(game_id);
        away_team.current_game = Some(game_id);
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

        let home_team = self.get_team_or_err(&home_team_in_game.team_id)?;
        let away_team = self.get_team_or_err(&away_team_in_game.team_id)?;
        home_team.can_challenge_local_team(away_team)?;

        let location = home_team
            .is_on_planet()
            .expect("Should have failed in can_challenge_team");

        let team_ids = [home_team_in_game.team_id, away_team_in_game.team_id];

        let game_id = self.generate_game_no_checks(
            home_team_in_game,
            away_team_in_game,
            starting_at,
            location,
            None,
        )?;

        for team_id in team_ids.iter() {
            let team = self
                .teams
                .get_mut(team_id)
                .ok_or(anyhow!("Team {team_id:?} not found"))?;

            team.current_game = Some(game_id);
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

        if self.get_game(&network_game.id).is_none() {
            let planet = self.get_planet_or_err(&network_game.location)?;
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

            while game.timer.value < network_game.timer.value && !game.timer.has_ended() {
                game.tick(Tick::now());
            }

            self.games.insert(game.id, game);
            self.dirty_ui = true;
        }

        Ok(())
    }

    pub fn add_network_team(&mut self, network_team: NetworkTeam) -> AppResult<bool> {
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

        // Check if we are receiving a team with which we have an open challenge.
        // Note: there could be a race condition where we receive a team over the network right after
        //       accepting the challenge but before the challenge has been finalized on our side.
        //       In this case, the received team would have current_game set to some (set to the challenge game
        //       they just started) and the challenge would fail on our hand since the challenge team must have no game.
        let own_team = self.get_own_team()?;
        for player_id in players.keys() {
            // Check if any player in the team is part of own team, in which case fail.
            // This check guarantees that the own team state gets precedence over
            // what we receive from the network.
            // Note: finalizing a trade in handle_trade_topic assumes that this check is in place
            //       to ensure that there is no race condition between receiving the trade
            //       syn_ack state and the network team from the trade proposer.
            if own_team.player_ids.contains(player_id) {
                return Err(anyhow!(
                    "Cannot receive over the network a player which is part of own team."
                ));
            }
        }

        let db_team = self.get_team(&team.id).cloned();

        // When adding a network planet, we do not update the parent planet satellites on purpose,
        // to avoid complications when cleaning up to store the world.
        // This means that the network satellite will not appear in the galaxy.
        for asteroid in asteroids {
            if asteroid.peer_id.is_none() {
                return Err(anyhow!(
                    "Cannot receive planet without peer_id over the network."
                ));
            }
            self.planets.insert(asteroid.id, asteroid);
        }

        let mut team_version_updated = false;

        if let Some(previous_version_team) = db_team.as_ref() {
            if let TeamLocation::OnPlanet { planet_id } = previous_version_team.current_location {
                // Remove team from previous planet
                let planet = self.get_planet_mut_or_err(&planet_id)?;
                planet.team_ids.retain(|&id| id != team.id);
            }

            // Remove players from db_team that are not in the new team to clean up fired players
            for player_id in &previous_version_team.player_ids {
                self.players.remove(player_id);
            }

            if previous_version_team.version < team.version {
                team_version_updated = true;
            }
        }

        // Add team to new planet
        if let TeamLocation::OnPlanet { planet_id } = team.current_location {
            let planet = self.get_planet_mut_or_err(&planet_id)?;
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
            self.players.insert(player.id, player);
        }

        self.teams.insert(team.id, team);
        self.dirty_ui = true;

        Ok(team_version_updated)
    }

    pub fn get_team(&self, id: &TeamId) -> Option<&Team> {
        self.teams.get(id)
    }

    pub fn get_team_or_err(&self, id: &TeamId) -> AppResult<&Team> {
        self.teams.get(id).ok_or(anyhow!("Team {id:?} not found"))
    }

    pub fn get_team_mut_or_err(&mut self, id: &TeamId) -> AppResult<&mut Team> {
        self.teams
            .get_mut(id)
            .ok_or(anyhow!("Team {id:?} not found"))
    }

    pub fn get_own_team(&self) -> AppResult<&Team> {
        self.get_team_or_err(&self.own_team_id)
    }

    pub fn get_own_team_mut(&mut self) -> AppResult<&mut Team> {
        self.teams
            .get_mut(&self.own_team_id)
            .ok_or(anyhow!("Team {:?} not found", self.own_team_id))
    }

    pub fn get_planet(&self, id: &PlanetId) -> Option<&Planet> {
        self.planets.get(id)
    }

    pub fn get_planet_or_err(&self, id: &PlanetId) -> AppResult<&Planet> {
        self.get_planet(id)
            .ok_or(anyhow!("Planet {id:?} not found"))
    }

    pub fn get_planet_mut_or_err(&mut self, id: &PlanetId) -> AppResult<&mut Planet> {
        self.planets
            .get_mut(id)
            .ok_or(anyhow!("Planet {id:?} not found"))
    }

    pub fn get_player(&self, id: &PlayerId) -> Option<&Player> {
        self.players.get(id)
    }

    pub fn get_player_or_err(&self, id: &PlayerId) -> AppResult<&Player> {
        self.get_player(id)
            .ok_or(anyhow!("Player {id:?} not found"))
    }

    pub fn get_player_mut_or_err(&mut self, id: &PlayerId) -> AppResult<&mut Player> {
        self.players
            .get_mut(id)
            .ok_or(anyhow!("Player {id:?} not found"))
    }

    pub fn get_players_by_team(players: &PlayerMap, team: &Team) -> AppResult<PlayerMap> {
        let mut team_players = PlayerMap::new();
        for player_id in team.player_ids.iter() {
            let player = players
                .get(player_id)
                .ok_or(anyhow!("Player {player_id} not found."))?
                .clone();
            team_players.insert(player.id, player);
        }
        Ok(team_players)
    }

    pub fn get_game(&self, id: &GameId) -> Option<&Game> {
        self.games.get(id)
    }

    pub fn get_game_or_err(&self, id: &GameId) -> AppResult<&Game> {
        self.get_game(id).ok_or(anyhow!("Game {id:?} not found"))
    }

    pub fn team_rating(&self, team_id: &TeamId) -> AppResult<Skill> {
        let team = self.get_team_or_err(team_id)?;
        if team.player_ids.is_empty() {
            return Ok(MIN_SKILL);
        }
        Ok(team
            .player_ids
            .iter()
            .filter(|&id| self.get_player(id).is_some())
            .map(|id| self.get_player(id).unwrap().average_skill())
            .sum::<Skill>()
            / team.player_ids.len().max(MIN_PLAYERS_PER_GAME) as Skill)
    }

    pub fn tournament_rating(&self, tournament_id: &TournamentId) -> AppResult<Skill> {
        let tournament = self
            .tournaments
            .get(tournament_id)
            .ok_or(anyhow!("Cannot find tournament {tournament_id}."))?;

        let teams = match tournament.state(Tick::now()) {
            TournamentState::Registration => &tournament.registered_teams,
            TournamentState::Confirmation
            | TournamentState::Started
            | TournamentState::Ended
            | TournamentState::Syncing => &tournament.participants,
            TournamentState::Canceled => &HashMap::default(),
        };

        if teams.is_empty() {
            return Ok(MIN_SKILL);
        }

        Ok(teams.values().map(|team| team.rating()).sum::<Skill>() / teams.len() as Skill)
    }

    pub fn is_simulating(&self) -> bool {
        if !self.has_own_team() {
            return false;
        }

        // This works if we assume that we can't lag behind more than a SHORT interval (1 second).
        // DEBUG_TIME_MULTIPLIER than cannot be too large or due to finite FPS this condition
        // would always return true.
        Tick::now() > self.last_tick_min_interval + TickInterval::SHORT
    }

    fn resources_found_after_exploration(
        &self,
        bonus: f32,
        planet: &Planet,
    ) -> AppResult<ResourceMap> {
        let mut rng = ChaCha8Rng::from_os_rng();
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
        let rng = &mut ChaCha8Rng::from_os_rng();
        let mut free_pirates = vec![];

        let duration_bonus = (duration as f32 / HOURS as f32).powf(1.3);
        let population_bonus = planet.total_population() as f32;

        let amount = rng
            .random_range((-32 + (population_bonus + duration_bonus) as i32).min(0)..3)
            .max(0);

        if amount > 0 {
            for _ in 0..amount {
                let base_level = rng.random_range(0.0..7.0);
                let player_id = self.generate_random_player(rng, None, planet, base_level)?;

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
        // FIXME: this workls only if we use it for medium intervals...
        let is_simulating = self.is_simulating();
        if is_simulating {
            // If we are simulating, we update last_tick_min_interval by hand,
            // as we do not call handle_fast_tick_events to update it and
            // the is_simulating bool depends on it.
            self.last_tick_min_interval = current_tick;
        }
        log::info!("World slow ticks: is simulating? {is_simulating}");

        if current_tick >= self.last_tick_short_interval + TickInterval::SHORT {
            self.tick_games(current_tick)?;
            callbacks.append(&mut self.tick_tournaments(current_tick, is_simulating)?);

            if let Some(cb) = self.cleanup_games(current_tick)? {
                callbacks.push(cb);
            }

            callbacks.append(&mut self.tick_travel(current_tick)?);

            if let Some(callback) = self.tick_spaceship_upgrade(current_tick)? {
                callbacks.push(callback);
            }

            for cb in self.tick_asteroid_upgrade(current_tick)? {
                callbacks.push(cb);
            }

            if self.dirty {
                self.update_own_team_honours()?;
            }

            self.last_tick_short_interval += TickInterval::SHORT;
            // Round up to the TickInterval::SHORT to keep these ticks synchronous across network.
            self.last_tick_short_interval -= self.last_tick_short_interval % TickInterval::SHORT;
        }

        if current_tick >= self.last_tick_medium_interval + TickInterval::MEDIUM {
            self.tick_tiredness_recovery()?;

            for cb in self.tick_player_leaving_team_for_low_morale(current_tick)? {
                callbacks.push(cb);
            }

            if !is_simulating {
                self.tick_team_position_assignment()?;
            }

            if self.games.len() < AUTO_GENERATE_GAMES_NUMBER {
                self.generate_random_games()?;
            }

            // Once every MEDIUM interval, set dirty_network flag,
            // so that we send our team to the network.
            self.dirty_network = true;

            self.last_tick_medium_interval += TickInterval::MEDIUM;
        }

        if current_tick >= self.last_tick_long_interval + TickInterval::LONG {
            self.tick_players_update();

            for cb in self.tick_player_retirement(current_tick)? {
                callbacks.push(cb);
            }

            self.tick_teams_reputation()?;

            // Local teams hire free pirates just before refreshing team,
            // so own team has had already time to hire them.
            self.tick_auto_hire_free_pirates()?;

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
            // let home_team_peer_id = &game.home_team_in_game.peer_id;
            // let away_team_peer_id = &game.away_team_in_game.peer_id;
            // if home_team_peer_id.is_some() && away_team_peer_id.is_none()
            //     || away_team_peer_id.is_some() && home_team_peer_id.is_none()
            // {
            //     continue;
            // }

            for team in [&game.home_team_in_game, &game.away_team_in_game] {
                //we do not apply end of game logic to peer teams
                //TODO: once we remove local teams, we can remove this loop and only apply to own_team
                if team.peer_id.is_some() && team.team_id != self.own_team_id {
                    continue;
                }

                let is_tournament_game = game.part_of_tournament.is_some();
                for game_player in team.players.values() {
                    // Set tiredness and morale to the value in game.
                    // We do not clone the game_player as other changes may have occured to the player
                    // during the game (such as skill update).
                    let mut player = match self.get_player_or_err(&game_player.id) {
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

                    player.version += 1;
                    player.add_morale(MORALE_INCREASE_PER_GAME);

                    // Restore tiredness and morale, at least partially if it's a tournament game.
                    if is_tournament_game {
                        player.add_morale(MORALE_INCREASE_AFTER_TOURNAMENT_GAME);
                        player.tiredness =
                            (player.tiredness - TIREDNESS_DECREASE_AFTER_TOURNAMENT_GAME).bound();
                    }

                    let stats = team
                        .stats
                        .get(&player.id)
                        .ok_or(anyhow!("Player {:?} not found in team stats", player.id))?;

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
                                self,
                                &player.team.expect("Player should have a team"),
                            )?)
                    .bound();

                    let training_bonus =
                        TeamBonus::Training.current_team_bonus(self, &team.team_id)?;
                    let training_focus = team.training_focus;
                    player.update_skills_training(
                        stats.experience_at_position,
                        training_bonus,
                        training_focus,
                    );
                    self.players.insert(player.id, player);
                }
            }

            let game_summary = GameSummary::from_game(game);
            self.past_games.insert(game_summary.id, game_summary);
            self.recently_finished_games.insert(game.id, game.clone());

            // Past games of the own team are persisted in the store.
            if game.home_team_in_game.team_id == self.own_team_id
                || game.away_team_in_game.team_id == self.own_team_id
            {
                save_game(game)?;
                // Update network that game has ended.
                self.dirty_network = true;

                let mut tournament_text = "".to_string();
                if let Some(id) = game.part_of_tournament.as_ref() {
                    if let Some(tournament) = self.tournaments.get(id) {
                        if tournament.has_ended() {
                            tournament_text = format!(
                                "{} has won the tournament. Congrats!",
                                game.winner
                                    .map(|w| if w == game.home_team_in_game.team_id {
                                        game.home_team_in_game.name.as_str()
                                    } else {
                                        game.away_team_in_game.name.as_str()
                                    })
                                    .unwrap_or("Who knows who")
                            );
                        } else {
                            tournament_text = format!(
                                "{} advances to the next round.",
                                game.winner
                                    .map(|w| if w == game.home_team_in_game.team_id {
                                        game.home_team_in_game.name.as_str()
                                    } else {
                                        game.away_team_in_game.name.as_str()
                                    })
                                    .unwrap_or("Who knows who")
                            );
                        }
                    }
                }
                own_team_game_notification = Some(UiCallback::PushUiPopup {
                    popup_message: PopupMessage::Ok {
                        message: format!(
                            "Game ended\n{} {}-{} {}\n{}",
                            game.home_team_in_game.name,
                            game.get_score().0,
                            game.get_score().1,
                            game.away_team_in_game.name,
                            tournament_text
                        ),
                        is_skippable: false,
                        tick: current_tick,
                    },
                });
            }

            // Teams get money depending on game attendance.
            // Home team gets a bonus for playing at home.
            // If a team is knocked out, money goes to the other team.
            // If both are knocked out, they get no money.
            let mut home_team_income = 500 + game.attendance * INCOME_PER_ATTENDEE_HOME;
            let mut away_team_income = 500 + game.attendance * INCOME_PER_ATTENDEE_AWAY;
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
                None => (1, 1),
            };

            // Set playing teams current game to None and assign income, reputation, and rum.
            // Network games allow for not finding the team in the world.teams, local games don't
            let team_ids = [
                game.home_team_in_game.team_id,
                game.away_team_in_game.team_id,
            ];

            if game.is_network() {
                for (idx, team_id) in team_ids.iter().enumerate() {
                    // It is possible not to have a network team in the world.
                    let mut team = if let Some(team) = self.get_team(team_id) {
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
                for (idx, team_id) in team_ids.iter().enumerate() {
                    let mut team = self.get_team_or_err(team_id)?.clone();

                    let other_rating = if idx == 0 {
                        &self
                            .get_team_or_err(&game.away_team_in_game.team_id)?
                            .local_game_rating
                    } else {
                        &self
                            .get_team_or_err(&game.home_team_in_game.team_id)?
                            .local_game_rating
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
                game.tick(current_tick);
            }
        }
        Ok(())
    }

    fn tick_tournaments(
        &mut self,
        current_tick: Tick,
        is_simulating: bool,
    ) -> AppResult<Vec<UiCallback>> {
        let mut callbacks = vec![];
        let mut new_games = vec![];
        for (&tournament_id, tournament) in self.tournaments.iter_mut() {
            match tournament.state(current_tick) {
                TournamentState::Registration => {}
                TournamentState::Canceled => {}
                TournamentState::Confirmation => {
                    // Append callback to send Confirmation.
                    // If we are simulating, abort tournament.
                    if tournament.organizer_id == self.own_team_id && is_simulating {
                        log::warn!("Canceling tournament {tournament_id}: Confirmation cannot be run while simulating.");
                        callbacks.push(UiCallback::CancelTournament { tournament_id });
                        continue;
                    }

                    if tournament.registered_teams.len() < 2 {
                        log::warn!(
                            "Canceling tournament {tournament_id}: Insufficient registered teams ({}).", tournament.registered_teams.len()
                        );
                        callbacks.push(UiCallback::CancelTournament { tournament_id });
                        continue;
                    }
                    if tournament.organizer_id == self.own_team_id {
                        callbacks.push(UiCallback::ConfirmTournamentParticipants { tournament_id });
                    }
                }
                TournamentState::Syncing => {
                    if tournament.organizer_id == self.own_team_id {
                        if is_simulating {
                            log::warn!("Canceling tournament {tournament_id}: Syncing cannot be run while simulating.");
                            callbacks.push(UiCallback::CancelTournament { tournament_id });
                            continue;
                        }
                        callbacks.push(UiCallback::SendConfirmedTournament { tournament_id });
                    }
                }
                TournamentState::Started => {
                    if tournament.participants.len() < 2 {
                        log::warn!(
                            "Canceling tournament {tournament_id}: Insufficient participants ({}).",
                            tournament.participants.len()
                        );
                        callbacks.push(UiCallback::CancelTournament { tournament_id });
                        continue;
                    }

                    // FIXME: this is not very robust. For instance, it relies on retaining all
                    // tournament games when storing, otherwise the hashmap would be incorrect.
                    let tournament_games = self
                        .games
                        .iter()
                        .filter(
                            |(_, game)| matches!(game.part_of_tournament, Some(id) if id == tournament_id),
                        )
                        .collect::<HashMap<&GameId, &Game>>();

                    new_games.append(
                        &mut tournament.generate_next_games(current_tick, tournament_games),
                    );
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
            for team_id in team_ids.iter() {
                let team = if let Some(team) = self.teams.get_mut(team_id) {
                    team
                } else {
                    continue;
                };

                team.current_game = Some(game.id);
                if team.id == self.own_team_id {
                    self.dirty_network = true
                }
            }
            self.games.insert(game.id, game);
        }

        for tournament in self.tournaments.values() {
            if tournament.has_ended() {
                if tournament.is_team_participating(&self.own_team_id) {
                    save_tournament(tournament)?;
                    let summary = TournamentSummary::from_tournament(tournament);
                    self.past_tournaments.insert(summary.id, summary);
                    self.recently_finished_tournaments
                        .insert(tournament.id, tournament.clone());
                }
                for team_id in tournament.participants.keys() {
                    let team = if let Some(team) = self.teams.get_mut(team_id) {
                        team
                    } else {
                        continue;
                    };

                    if team.peer_id.is_some() {
                        continue;
                    }

                    team.tournament_registration_state = TournamentRegistrationState::None;
                }

                if let Some(winner) = tournament.winner.as_ref() {
                    if let Some(team) = self.teams.get_mut(winner) {
                        team.tournaments_won.push(tournament.id);
                    }
                }
            }
        }

        self.tournaments
            .retain(|_, t| !t.has_ended() && !t.is_canceled());

        // FIXME: This should not be necessary, but there are still bugs and it is convenient.
        let own_team = self
            .teams
            .get_mut(&self.own_team_id)
            .expect("Own team should exist");
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
            .ok_or(anyhow!("Could not find own team."))?;
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
                        .ok_or(anyhow!("Could not find planet {to}."))?;
                    planet.team_ids.push(own_team.id);

                    let team_name = own_team.name.clone();
                    let planet_name = planet.name.clone();
                    let planet_filename = planet.filename.clone();
                    let planet_type = planet.planet_type;

                    for player_id in own_team.player_ids.iter() {
                        let player = self
                            .players
                            .get_mut(player_id)
                            .ok_or(anyhow!("Could not find player {player_id}."))?;
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
                            tick: current_tick,
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

                    for player in team.player_ids.iter() {
                        let player = self.get_player_mut_or_err(player)?;
                        player.set_jersey(&team.jersey);
                    }
                    let mut around_planet = self.get_planet_or_err(&around)?.clone();
                    team.current_location = TeamLocation::OnPlanet { planet_id: around };

                    let mut rng = ChaCha8Rng::from_os_rng();

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
                                tick: current_tick,
                                asteroid_type: rng.random_range(0..30),
                            },
                        });
                    }

                    around_planet.team_ids.push(team.id);

                    let bonus = TeamBonus::Exploration.current_team_bonus(self, &team.id)?;
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
                            self.get_player_or_err(&player_id)
                                .expect("Player should be part of world")
                                .clone()
                        })
                        .collect_vec();

                    callbacks.push(UiCallback::PushUiPopup {
                        popup_message: PopupMessage::ExplorationResult {
                            planet_name: around_planet.name.clone(),
                            resources: collected_resources,
                            players: found_pirates,
                            tick: current_tick,
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

    fn tick_spaceship_upgrade(&mut self, current_tick: Tick) -> AppResult<Option<UiCallback>> {
        let own_team = self.get_own_team()?;
        if let Some(upgrade) = own_team.spaceship.pending_upgrade {
            if current_tick > upgrade.started + upgrade.duration {
                return Ok(Some(UiCallback::UpgradeSpaceship { upgrade }));
            }
        }
        Ok(None)
    }

    fn tick_asteroid_upgrade(&mut self, current_tick: Tick) -> AppResult<Vec<UiCallback>> {
        let own_team = self.get_own_team()?;
        let mut callbacks = vec![];
        for asteroid_id in own_team.asteroid_ids.iter() {
            let asteroid = self.get_planet_or_err(asteroid_id)?;
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

    fn tick_tiredness_recovery(&mut self) -> AppResult<()> {
        let teams = self
            .teams
            .values()
            .filter(|team| team.current_game.is_none() && team.peer_id.is_none())
            .collect::<Vec<&Team>>();

        for team in teams {
            let bonus = TeamBonus::TirednessRecovery.current_team_bonus(self, &team.id)?;
            for player_id in team.player_ids.iter() {
                let player = if let Some(player) = self.players.get_mut(player_id) {
                    player
                } else {
                    continue;
                };
                if player.tiredness > 0.0 {
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

            team.player_ids = Team::best_position_assignment(
                team.player_ids
                    .iter()
                    .map(|&id| self.players.get(&id).unwrap())
                    .collect(),
            );

            let rng = &mut ChaCha8Rng::from_os_rng();
            team.game_tactic = Tactic::random(rng);
        }

        Ok(())
    }

    fn tick_free_pirates(&mut self, current_tick: Tick) -> AppResult<UiCallback> {
        self.players.retain(|_, player| player.team.is_some());

        let rng = &mut ChaCha8Rng::seed_from_u64(rand::random());
        for planet in PLANET_DATA.iter() {
            self.populate_planet(rng, planet)?;
        }
        Ok(UiCallback::PushUiPopup {
            popup_message: PopupMessage::Ok {
                message: "Free pirates refreshed".into(),
                is_skippable: false,
                tick: current_tick,
            },
        })
    }

    fn tick_auto_hire_free_pirates(&mut self) -> AppResult<()> {
        let free_pirates = self
            .players
            .values()
            .filter(|p| p.team.is_none())
            .collect_vec()
            .sort_by_rating();

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

            let available_free_pirates = free_pirates
                .iter()
                .filter(|&player| {
                    !hired_player_ids.contains(&player.id)
                        && team.can_consider_hiring_player(player).is_ok()
                        && team.is_on_player_planet(player)
                })
                .collect_vec();

            if available_free_pirates.is_empty() {
                continue;
            }

            // Hire as many pirates are needed to reach MIN_PLAYERS_PER_GAME
            let needed_pirates = MIN_PLAYERS_PER_GAME
                .saturating_sub(team.player_ids.len())
                .max(1);

            let candidates = available_free_pirates
                .iter()
                .take(needed_pirates)
                .collect_vec();

            if team.player_ids.len() == team.spaceship.crew_capacity() as usize {
                // If the team is at capacity, it definetely had at least MIN_PLAYERS_PER_GAME.
                assert!(candidates.len() <= 1);
                // Check if weakest pirate is worse than best free pirate.
                // If not, continue.
                let worst_pirate = *team
                    .player_ids
                    .iter()
                    .map(|id| self.get_player(id).unwrap())
                    .collect_vec()
                    .sort_by_rating()
                    .last()
                    .expect("There should be at least one pirate in the crew.");
                let best_pirate = candidates[0];
                if worst_pirate.rating() >= best_pirate.rating() {
                    continue;
                }
                released_player_ids.push(worst_pirate.id);
            }

            for player in candidates {
                hired_player_ids.push(player.id);
                hiring_team_ids.push(team.id);
            }
        }

        assert!(hired_player_ids.len() == hiring_team_ids.len());

        for &player_id in released_player_ids.iter() {
            self.release_player_from_team(player_id)?;
        }

        for idx in 0..hired_player_ids.len() {
            let player_id = hired_player_ids[idx];
            let team_id = hiring_team_ids[idx];
            self.hire_player_for_team(&player_id, &team_id)?;
        }

        Ok(())
    }

    fn tick_players_update(&mut self) {
        for (_, player) in self.players.iter_mut() {
            //TODO: once we remove local teams, we can remove this loop and only apply to own_team
            if player.peer_id.is_some() {
                continue;
            }
            player.version += 1;
            // Reset player improvements for UI.
            player.previous_skills = player.current_skill_array();
            player.info.age += AGE_INCREASE_PER_LONG_TICK;

            if player.special_trait == Some(Trait::Crumiro) {
                player.skills_training = [0.0; 20];
                player.reputation = 0.0; //fuck crumiris!
                continue;
            }

            // Pirates slightly dislike being part of a team.
            // This is counteracted by the morale boost pirates get by playing games.
            // We do not apply this to computer teams since no games are played during simulation,
            // and hence it would result in all computer players to be completely demoralized.
            player.add_morale(MORALE_DECREASE_PER_LONG_TICK);
            player.reputation = (player.reputation + REPUTATION_DECREASE_PER_LONG_TICK).bound();

            for idx in 0..player.skills_training.len() {
                // Reduce player skills. This is planned to counteract the effect of training by playing games.

                let factor = (1.0 - PEAK_PERFORMANCE_RELATIVE_AGE)
                    / (1.0 - 0.5 * (player.info.relative_age() + PEAK_PERFORMANCE_RELATIVE_AGE))
                        .max(0.01);
                let age_modifier =
                    // Age modifier increseas linearly from (0,2/3) to (PEAK_PERFORMANCE_RELATIVE_AGE, 1),
                    // then increseas linearly from (PEAK_PERFORMANCE_RELATIVE_AGE, 1) to (1, 4).
                    if PEAK_PERFORMANCE_RELATIVE_AGE >= player.info.relative_age() {
                        1.0 / (1.5
                            - player.info.relative_age() / (2.0 * PEAK_PERFORMANCE_RELATIVE_AGE)).max(1.0)
                    }
                    // Mental abilities decrease slowly for mature players.
                    else if idx > 15 {
                         factor
                    }
                    // Reduce athletics skills even more if relative_age is more than PEAK_PERFORMANCE_RELATIVE_AGE.
                    else if idx < 4 {
                        4.0 * factor
                    } else {
                      2.0 *  factor
                    };

                player.modify_skill(idx, SKILL_DECREMENT_PER_LONG_TICK * age_modifier.bound());

                // Increase player skills from training
                player.modify_skill(idx, player.skills_training[idx]);
            }
            player.skills_training = [0.0; 20];
        }
    }

    fn tick_teams_reputation(&mut self) -> AppResult<()> {
        let mut reputation_update: Vec<(TeamId, f32)> = vec![];
        for (_, team) in self.teams.iter() {
            //TODO: once we remove local teams, we can remove this loop and only apply to own_team
            if team.peer_id.is_some() {
                continue;
            }
            let players_reputation = team
                .player_ids
                .iter()
                .map(|id| {
                    if let Ok(player) = self.get_player_or_err(id) {
                        player.reputation
                    } else {
                        0.0
                    }
                })
                .sum::<f32>()
                / team.player_ids.len().max(1) as f32;

            // If team reputation is smaller than players average reputation, it increases.
            // Otherwise, it decreases.
            let mut reputation_modifier =
                ((players_reputation - team.reputation) / MAX_SKILL) / 2.0;
            if reputation_modifier > 0.0 {
                reputation_modifier *= TeamBonus::Reputation.current_team_bonus(self, &team.id)?;
            }
            let new_reputation = (team.reputation * (1.0 + reputation_modifier)).bound();
            let min_reputation = team.honours.len() as f32 * MIN_REPUTATION_PER_HONOUR;
            reputation_update.push((team.id, new_reputation.max(min_reputation)));
        }

        for (team_id, new_reputation) in reputation_update {
            let team = self.get_team_mut_or_err(&team_id)?;
            team.reputation = new_reputation;
        }
        Ok(())
    }

    fn tick_player_leaving_team_for_low_morale(
        &mut self,
        current_tick: Tick,
    ) -> AppResult<Vec<UiCallback>> {
        let mut messages = vec![];

        let mut releasing_player_ids = vec![];

        for &player_id in self.players.keys() {
            let player = self.get_player_or_err(&player_id)?;
            if player.team.is_none() {
                continue;
            }

            if player.special_trait == Some(Trait::Crumiro) {
                continue;
            }

            let team = self.get_team_or_err(&player.team.expect("Player should have a team"))?;

            if team.can_release_player(player).is_err() {
                continue;
            }

            let rng = &mut ChaCha8Rng::from_os_rng();
            if player.morale < MORALE_THRESHOLD_FOR_LEAVING
                && rng.random_bool(
                    (1.0 - player.morale / MAX_SKILL) as f64 * LEAVING_PROBABILITY_MORALE_MODIFIER,
                )
            {
                releasing_player_ids.push(player_id);

                if player.team.expect("Team should be some") == self.own_team_id {
                    messages.push(UiCallback::PushUiPopup {
                        popup_message: PopupMessage::Ok {
                            message: format!(
                                "{} {} left the crew!\n{} morale was too low...",
                                player.info.first_name,
                                player.info.last_name,
                                player.info.pronouns.as_possessive()
                            ),
                            is_skippable: false,
                            tick: current_tick,
                        },
                    })
                }
            }
        }

        for &player_id in releasing_player_ids.iter() {
            self.release_player_from_team(player_id)?;
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
            let player = self.get_player_or_err(&player_id)?;
            if player.team.is_none() {
                continue;
            }

            if player.special_trait == Some(Trait::Crumiro) {
                continue;
            }

            let team = self.get_team_or_err(&player.team.expect("Player should have a team"))?;

            if team.can_release_player(player).is_err() {
                continue;
            }

            let rng = &mut ChaCha8Rng::from_os_rng();
            if player.info.relative_age() > MIN_RELATIVE_RETIREMENT_AGE {
                // Add extra check to avoid running rng call unnecessarily.
                if player.info.relative_age() > rng.random_range(MIN_RELATIVE_RETIREMENT_AGE..1.0) {
                    releasing_player_ids.push(player_id);

                    if player.team.expect("Team should be some") == self.own_team_id {
                        messages.push(UiCallback::PushUiPopup {
                            popup_message: PopupMessage::Ok{
                                message:format!(
                                    "{} {} left the crew and retired to cultivate turnips\n{} {} been a great pirate...",
                                    player.info.first_name,
                                    player.info.last_name,
                                    player.info.pronouns.as_subject(),
                                    player.info.pronouns.to_have(),
                                ),
                                is_skippable:false,
                                tick:current_tick
                            },
                        })
                    }
                }
            }
        }

        for &player_id in releasing_player_ids.iter() {
            self.release_player_from_team(player_id)?;
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
            .ok_or(anyhow!("Team {:?} not found", self.own_team_id))?;

        for honour in Honour::iter() {
            if !own_team.honours.contains(&honour)
                && honour.conditions_met(own_team, &self.past_games, &self.players, &self.planets)
            {
                own_team.honours.insert(honour);
            }
        }

        Ok(())
    }

    fn generate_random_games(&mut self) -> AppResult<()> {
        let rng = &mut ChaCha8Rng::from_os_rng();
        for planet in self.planets.values() {
            if planet.team_ids.len() < 2 {
                continue;
            }

            let candidate_teams = planet
                .team_ids
                .iter()
                .map(|&id| self.get_team_or_err(&id))
                .filter(|team_res| {
                    if let Ok(team) = team_res {
                        if team.player_ids.len() < MIN_PLAYERS_PER_GAME {
                            return false;
                        }

                        let average_tiredness = team.average_tiredness(self);
                        return team.current_game.is_none()
                            && team.autonomous_strategy.challenge_local
                            && team.peer_id.is_none()
                            && average_tiredness <= MAX_AVG_TIREDNESS_PER_AUTO_GAME;
                    }
                    false
                })
                .collect::<AppResult<Vec<&Team>>>()?;

            if candidate_teams.len() < 2 {
                continue;
            }
            let teams = candidate_teams.iter().choose_multiple(rng, 2);
            let home_team_in_game =
                TeamInGame::from_team_id(&teams[0].id, &self.teams, &self.players)?;

            let away_team_in_game =
                TeamInGame::from_team_id(&teams[1].id, &self.teams, &self.players)?;

            if let Err(err) = self.generate_local_game(home_team_in_game, away_team_in_game) {
                log::error!("Error while generating local game: {err}");
            }
            return Ok(()); // Generate only one game per call
        }

        Ok(())
    }

    pub fn filter_peer_data(&mut self, peer_id: Option<PeerId>) -> AppResult<()> {
        let mut own_team = self.get_own_team()?.clone();
        let own_team_current_location = own_team.is_on_planet();
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

            self.games.retain(|_, game| {
                game.home_team_in_game.team_id == self.own_team_id
                    || game.away_team_in_game.team_id == self.own_team_id
                    || ((game.home_team_in_game.peer_id.is_none()
                        || game.home_team_in_game.peer_id.unwrap() != peer_id)
                        && (game.away_team_in_game.peer_id.is_none()
                            || game.away_team_in_game.peer_id.unwrap() != peer_id))
                    || game.part_of_tournament.is_some()
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

            self.games.retain(|_, game| {
                game.home_team_in_game.team_id == self.own_team_id
                    || game.away_team_in_game.team_id == self.own_team_id
                    || game.is_local()
                    || game.part_of_tournament.is_some()
            });
            own_team.clear_challenges();
            own_team.clear_trades();
        }

        self.tournaments.retain(|_, t| {
            t.is_team_registered(&self.own_team_id) || t.is_team_participating(&self.own_team_id)
        });

        self.teams.insert(own_team.id, own_team);

        // Remove teams from planet teams vector.
        for (_, planet) in self.planets.iter_mut() {
            planet
                .team_ids
                .retain(|&team_id| self.teams.contains_key(&team_id));
        }

        // Set current game to None for teams playing a game not stored in games.
        for team in self.teams.values_mut() {
            if let Some(game_id) = team.current_game {
                if !self.games.contains_key(&game_id) {
                    team.current_game = None;
                }
            }
        }

        self.dirty = true;
        self.dirty_ui = true;
        Ok(())
    }

    pub fn travel_duration_to_planet(&self, team_id: TeamId, to_id: PlanetId) -> AppResult<Tick> {
        let team = self.get_team_or_err(&team_id)?;

        // Travelling back to planet with teleportation pad is istantaneous.
        let to = self.get_planet_or_err(&to_id)?;
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
        let bonus = TeamBonus::SpaceshipSpeed.current_team_bonus(self, &team.id)?;
        Ok(
            ((LANDING_TIME_OVERHEAD as f32 + (distance as f32 / team.spaceship_speed())) / bonus)
                as Tick,
        )
    }

    fn planet_height(&self, planet_id: PlanetId) -> AppResult<usize> {
        let mut planet = self.get_planet_or_err(&planet_id)?;
        let mut height = 0;

        while let Some(parent_id) = planet.satellite_of {
            planet = self.get_planet_or_err(&parent_id)?;
            height += 1;
        }
        Ok(height)
    }

    pub fn fuel_consumption_to_planet(&self, team_id: TeamId, to_id: PlanetId) -> AppResult<u32> {
        let duration = self.travel_duration_to_planet(team_id, to_id)?;
        let team = self.get_team_or_err(&team_id)?;

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

        let from = self.get_planet_or_err(&from_id)?;
        let to = self.get_planet_or_err(&to_id)?;

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
            seed: self.seed,
            last_tick_short_interval: self.last_tick_short_interval,
            last_tick_medium_interval: self.last_tick_medium_interval,
            last_tick_long_interval: self.last_tick_long_interval,
            own_team_id: self.own_team_id,
            teams: self.teams.clone(),
            players: self.players.clone(),
            planets: self.planets.clone(),
            games: self.games.clone(),
            tournaments: self.tournaments.clone(),
            kartoffeln: self.kartoffeln.clone(),
            past_games: self.past_games.clone(),
            serialized_size: self.serialized_size,
            network_keypair: self.network_keypair.clone(),
            ..Default::default()
        };
        w.filter_peer_data(None)?;

        Ok(w)
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
            RatedPlayers, DEFAULT_PLANET_ID, MIN_PLAYERS_PER_GAME,
        },
        types::{StorableResourceMap, SystemTimeTick, Tick},
        ui::UiCallback,
    };
    use itertools::Itertools;
    use rand::{Rng, SeedableRng};
    use rand_chacha::ChaCha8Rng;
    use uuid::uuid;

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
        let rng = &mut ChaCha8Rng::from_os_rng();
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
            world.generate_random_team(rng, planet.id, "test".into(), "testship".into(), None)?;

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
        let rng = &mut ChaCha8Rng::from_os_rng();
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
            world.generate_random_team(rng, planet.id, "test".into(), "testship".into(), None)?;

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
        // To actually test this, set PORTAL_DISCOVERY_PROBABILITY to 1.0
        let mut app = App::test_default()?;
        app.new_world();

        // let world = &mut app.world;
        let rng = &mut ChaCha8Rng::from_os_rng();
        let planet = PLANET_DATA[0].clone();
        let team_id = app.world.generate_random_team(
            rng,
            planet.id,
            "test".into(),
            "testship".into(),
            None,
        )?;

        // Add rum to team
        let mut team = app.world.get_team_or_err(&team_id)?.clone();
        team.add_resource(Resource::RUM, 20)?;

        // Give player Spugna skill and set it as pilot
        let mut spugna = app.world.get_player_or_err(&team.player_ids[0])?.clone();
        let spugna_id = spugna.id.clone();
        spugna.special_trait = Some(Trait::Spugna);
        if spugna.info.crew_role != CrewRole::Pilot {
            app.world.set_team_crew_role(CrewRole::Pilot, spugna.id)?;
        }
        app.world.players.insert(spugna.id, spugna);

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

        // Drink to trigger portal discovery
        UiCallback::Drink {
            player_id: spugna_id,
        }
        .call(&mut app)?;

        app.world
            .handle_slow_tick_events(app.world.last_tick_short_interval + TickInterval::SHORT)?;

        let team = app.world.get_team_or_err(&team_id)?;
        println!("Team resources {:?}", team.resources);
        println!("Team location {:?}", team.current_location);
        println!("Travelled distance {}", team.total_travelled);
        // Teleportation does not add to total_travelled
        assert!(team.total_travelled == 0);

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

        let player = world.get_player_or_err(&player_id)?;

        let mut current_max_average_skill = player.current_skill_array();
        for _ in 0..300 {
            let mut player = world.get_player_or_err(&player_id)?.clone();
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

        let player = world.get_player_or_err(&player_id)?;
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
        let mut player = world.get_player_or_err(&player_id)?.clone();
        assert!(player.team.is_some());

        player.info.age = player.info.population.max_age();
        world.players.insert(player_id, player);
        world.tick_player_retirement(Tick::now())?;

        let player = world.get_player_or_err(&player_id)?;
        assert!(player.team.is_none());

        Ok(())
    }

    #[test]
    fn test_tick_player_leaving_own_team_for_morale() -> AppResult<()> {
        let mut app = App::test_default()?;

        let world = &mut app.world;

        let own_team = world.get_own_team()?;
        let player_id = own_team.player_ids[0];
        let mut player = world.get_player_or_err(&player_id)?.clone();
        assert!(player.team.is_some());

        player.morale = 0.0;
        world.players.insert(player_id, player);

        // Players with low morale quit a team randomly
        let mut idx = 0;
        loop {
            world.tick_player_leaving_team_for_low_morale(Tick::now())?;
            let player: &crate::core::player::Player = world.get_player_or_err(&player_id)?;
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
            Some(0.0),
        )?;

        let team = app.world.get_team_or_err(&team_id)?;

        assert!(team.player_ids.len() <= team.spaceship.crew_capacity() as usize);

        let players = team
            .player_ids
            .iter()
            .map(|id| app.world.get_player(id).unwrap())
            .collect_vec()
            .sort_by_rating();

        let worst_pirate = players.last().unwrap();
        let prev_worst_rating = worst_pirate.average_skill();
        println!(
            "Worst player {} rating {}",
            worst_pirate.info.short_name(),
            prev_worst_rating
        );

        app.world.tick_auto_hire_free_pirates()?;

        let team = app.world.get_team_or_err(&team_id)?;
        let players = team
            .player_ids
            .iter()
            .map(|id| app.world.get_player(id).unwrap())
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
            Some(0.0),
        )?;

        let mut team = app.world.get_team_or_err(&team_id)?.clone();

        while team.player_ids.len() >= MIN_PLAYERS_PER_GAME {
            let player_id = team.player_ids[0];
            team.player_ids.retain(|&p| p != player_id);
        }

        assert!(team.player_ids.len() < MIN_PLAYERS_PER_GAME);

        app.world.teams.insert(team.id, team);

        app.world.tick_auto_hire_free_pirates()?;

        let team = app.world.get_team_or_err(&team_id)?;
        assert!(team.player_ids.len() == MIN_PLAYERS_PER_GAME);

        Ok(())
    }

    #[test]
    fn test_is_simulating() -> AppResult<()> {
        let mut app = App::test_default()?;

        let world = &mut app.world;
        assert!(world.is_simulating() == false);

        let now = Tick::now();
        let mut current_tick = now;
        world.last_tick_min_interval = now;

        let cycles = 3;
        // After waiting the world should be exactly two ticks behind.
        // We add a small buffer to ensure there is no race condition.
        thread::sleep(Duration::from_millis(cycles * TickInterval::SHORT - 350));

        let mut runs = 0;

        while world.is_simulating() {
            world.handle_slow_tick_events(current_tick)?;
            current_tick += TickInterval::SHORT;
            runs += 1;
        }

        println!("{} {}", runs, cycles);
        assert!(runs == cycles);
        assert!(world.is_simulating() == false);

        Ok(())
    }

    #[test]
    fn test_generate_random_games() -> AppResult<()> {
        let mut app = App::test_default()?;

        let world = &mut app.world;

        for i in 0..11 {
            assert!(world.games.len() == i);
            world.generate_random_games()?;
        }

        for game in world.games.values() {
            println!(
                "{} vs {}",
                game.home_team_in_game.name, game.away_team_in_game.name
            );
        }

        Ok(())
    }
}
