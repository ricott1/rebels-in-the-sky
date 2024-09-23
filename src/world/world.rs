use super::constants::*;
use super::jersey::{Jersey, JerseyStyle};
use super::planet::{Planet, PlanetType};
use super::player::Player;
use super::position::{Position, MAX_POSITION};
use super::resources::Resource;
use super::role::CrewRole;
use super::skill::{GameSkill, Rated, MAX_SKILL};
use super::spaceship::Spaceship;
use super::team::Team;
use super::types::{PlayerLocation, TeamBonus, TeamLocation};
use super::utils::{PLANET_DATA, TEAM_DATA};
use crate::engine::constants::RECOVERING_TIREDNESS_PER_SHORT_TICK;
use crate::engine::game::{Game, GameSummary};
use crate::engine::types::{Possession, TeamInGame};
use crate::image::color_map::ColorMap;
use crate::network::types::{NetworkGame, NetworkTeam};
use crate::store::save_game;
use crate::types::*;
use crate::ui::popup_message::PopupMessage;
use crate::ui::ui_callback::UiCallbackPreset;
use crate::world::utils::is_default;
use anyhow::anyhow;
use itertools::Itertools;
use libp2p::PeerId;
use log::info;
use rand::seq::{IteratorRandom, SliceRandom};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::u64;

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct World {
    pub dirty: bool,
    pub dirty_network: bool,
    pub dirty_ui: bool,
    pub serialized_size: u64,
    pub seed: u64,
    pub last_tick_short_interval: Tick,
    pub last_tick_medium_interval: Tick,
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
    pub games: GameMap,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub kartoffeln: KartoffelMap,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub past_games: GameSummaryMap,
}

impl World {
    pub fn new(seed: Option<u64>) -> Self {
        let mut planets = HashMap::new();
        let data_planets: Vec<Planet> = PLANET_DATA.iter().map(|p| p.clone()).collect();
        for planet in data_planets.iter() {
            planets.insert(planet.id, planet.clone());
        }

        Self {
            seed: seed.unwrap_or(rand::random()),
            last_tick_short_interval: Tick::now(),
            planets,
            ..Default::default()
        }
    }
    pub fn initialize(&mut self, generate_local_world: bool) -> AppResult<()> {
        let rng = &mut ChaCha8Rng::seed_from_u64(self.seed);
        for planet in PLANET_DATA.iter() {
            self.populate_planet(rng, planet);
        }
        if generate_local_world {
            self.generate_local_world(rng)?;
        }

        let now = Tick::now();

        self.last_tick_short_interval = now;
        // We round up to the beginning of the next TickInterval::SHORT to ensure
        // that online games don't drift too much.
        self.last_tick_short_interval -= self.last_tick_short_interval % TickInterval::SHORT;
        self.last_tick_medium_interval = now;
        self.last_tick_long_interval = now;
        Ok(())
    }

    pub fn has_own_team(&self) -> bool {
        self.own_team_id != TeamId::default()
    }

    fn populate_planet(&mut self, rng: &mut ChaCha8Rng, planet: &Planet) {
        // generate free pirates per each planet
        let number_free_pirates = planet.total_population();
        let mut position = 0 as Position;
        let own_team_base_level = if let Ok(own_team) = self.get_own_team() {
            own_team.reputation / 4.0
        } else {
            0.0
        };
        let base_level = rng.gen_range(0.0..4.0) + own_team_base_level;
        for _ in 0..number_free_pirates {
            self.generate_random_player(rng, None, Some(position), planet.id, base_level);
            position = (position + 1) % MAX_POSITION;
        }
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
        let team_id = TeamId::new_v4();
        let mut team = Team::random(team_id, home_planet_id, team_name);
        team.spaceship.name = ship_name;
        let home_planet_id = team.home_planet_id.clone();
        let team_base_level = rng.gen_range(0..=5) as f32;
        for position in 0..MAX_POSITION {
            self.generate_random_player(
                rng,
                Some(&mut team),
                Some(position),
                home_planet_id,
                team_base_level,
            );
        }
        while team.player_ids.len() < team.spaceship.crew_capacity() as usize {
            self.generate_random_player(
                rng,
                Some(&mut team),
                None,
                home_planet_id,
                team_base_level,
            );
        }

        let players = team
            .player_ids
            .iter()
            .map(|id| self.players.get(id).unwrap())
            .collect::<Vec<&Player>>();
        team.player_ids = Team::best_position_assignment(players.clone());

        let mut planet = self.get_planet_or_err(home_planet_id)?.clone();
        planet.team_ids.push(team_id);
        self.planets.insert(planet.id, planet);

        self.teams.insert(team.id, team.clone());
        self.auto_set_team_roles(&mut team)?;
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
        balance: u32,
        spaceship: Spaceship,
    ) -> AppResult<TeamId> {
        let team_id = TeamId::new_v4();
        let current_location = TeamLocation::OnPlanet {
            planet_id: home_planet_id,
        };

        let mut resources = HashMap::default();
        // resources.satoshi = balance;
        resources.insert(Resource::SATOSHI, balance);
        resources.insert(Resource::FUEL, spaceship.fuel_capacity());
        let mut team = Team {
            id: team_id,
            name,
            jersey: Jersey {
                style: jersey_style,
                color: jersey_colors,
            },
            home_planet_id,
            current_location,
            spaceship,
            resources,
            ..Default::default()
        };

        for player_id in players {
            let mut player = self.get_player_or_err(player_id)?.clone();
            team.add_player(&mut player);
            self.players.insert(player.id, player);
        }

        team.player_ids = Team::best_position_assignment(
            team.player_ids
                .iter()
                .map(|id| self.players.get(id).unwrap())
                .collect(),
        );

        self.own_team_id = team.id;
        self.teams.insert(team.id, team.clone());
        self.auto_set_team_roles(&mut team)?;

        let mut planet = self.get_planet_or_err(home_planet_id)?.clone();
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

        let mut satellite_of_planet = self.get_planet_or_err(satellite_of)?.clone();
        satellite_of_planet.satellites.push(asteroid_id);
        satellite_of_planet.version += 1;
        self.planets.insert(satellite_of, satellite_of_planet);

        Ok(asteroid_id)
    }

    fn generate_random_player(
        &mut self,
        rng: &mut ChaCha8Rng,
        team: Option<&mut Team>,
        position: Option<Position>,
        home_planet_id: PlanetId,
        base_level: f32,
    ) -> PlayerId {
        let player_id = PlayerId::new_v4();
        let planet = self.get_planet_or_err(home_planet_id).unwrap();
        let mut player = Player::random(rng, player_id, position, &planet, base_level);
        if team.is_some() {
            team.unwrap().add_player(&mut player);
        }
        self.players.insert(player.id, player);
        self.dirty = true;
        self.dirty_ui = true;
        player_id
    }

    fn auto_set_team_roles(&mut self, team: &mut Team) -> AppResult<()> {
        let rng = &mut ChaCha8Rng::seed_from_u64(self.seed);
        let mut shuffled_players = team.player_ids.clone();
        shuffled_players.shuffle(rng);

        self.set_team_crew_role(CrewRole::Captain, shuffled_players[0])?;
        self.set_team_crew_role(CrewRole::Pilot, shuffled_players[1])?;
        self.set_team_crew_role(CrewRole::Doctor, shuffled_players[2])?;
        for player_id in shuffled_players.iter().skip(3) {
            self.set_team_crew_role(CrewRole::Mozzo, *player_id)?;
        }

        Ok(())
    }

    pub fn set_team_crew_role(&mut self, role: CrewRole, player_id: PlayerId) -> AppResult<()> {
        let mut player = self.get_player_or_err(player_id)?.clone();
        let mut team = if let Some(team_id) = player.team {self.get_team_or_err(team_id)?.clone()}else {
            return Err(anyhow!("Player {:?} is not in a team", player_id));
        };

        team.can_set_crew_role(&player, role)?;

        let previous_spaceship_speed_bonus = TeamBonus::SpaceshipSpeed
            .current_team_bonus(&self, team.id)?
            .clone();

        let current_role_player = match role {
            CrewRole::Captain => team.crew_roles.captain,
            CrewRole::Pilot => team.crew_roles.pilot,
            CrewRole::Doctor => team.crew_roles.doctor,
            //We don't need to check for mozzo because we can have several mozzos.
            CrewRole::Mozzo => None,
        };

        let jersey = if team.is_travelling() {
            Jersey {
                style: JerseyStyle::Pirate,
                color: team.jersey.color.clone(),
            }
        } else {
            team.jersey.clone()
        };

        info!("Setting {} to {}", player.info.shortened_name(), role);
        // Demote previous crew role player to mozzo.
        if let Some(crew_player_id) = current_role_player {
            if crew_player_id == player_id {
                return Err(anyhow!("Player {:?} is already {}", player_id, role));
            }

            let mut current_role_player = self
                .get_player_or_err(crew_player_id)?
                .clone();

            info!("Removing {} from {}", current_role_player.info.shortened_name(), role);
            current_role_player.info.crew_role = CrewRole::Mozzo;
            team.crew_roles.mozzo.push(current_role_player.id);
            // Demoted player is a bit demoralized :(

            current_role_player.morale =
                (current_role_player.morale + MORALE_DEMOTION_MALUS).bound();

            current_role_player.set_jersey(&jersey);
            self.players
                .insert(crew_player_id, current_role_player);
        }

        // Empty previous role of player.
        match player.info.crew_role {
            CrewRole::Captain => {
                team.crew_roles.captain = None;
            }
            CrewRole::Pilot => {
                team.crew_roles.pilot = None;
            }
            CrewRole::Doctor => {
                team.crew_roles.doctor = None;
            }
            CrewRole::Mozzo => {
                team.crew_roles.mozzo.retain(|&id| id != player.id);
            }
        }

        // Set new crew role player.
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
            CrewRole::Mozzo => {
                team.crew_roles.mozzo.push(player_id);
            }
        }
        player.info.crew_role = role;
        player.set_jersey(&jersey);

        // If team is travelling and pilot was updated recalculate travel duration.
        match team.current_location {
            TeamLocation::Travelling {
                from,
                to,
                started,
                duration,
                distance,
            } => {
                let new_start = Tick::now();
                let time_elapsed = new_start - started;
                let bonus = TeamBonus::SpaceshipSpeed.current_player_bonus(&player)?;

                let new_duration =
                    (duration - time_elapsed) as f32 * previous_spaceship_speed_bonus / bonus;

                info!(
                    "Update {role}: old speed {previous_spaceship_speed_bonus}, new speed {bonus}"
                );

                let old_location = team.current_location.clone();
                team.current_location = TeamLocation::Travelling {
                    from,
                    to,
                    started: new_start,
                    duration: new_duration as Tick,
                    distance,
                };

                info!(
                    "Update {role}: old location{:?}, new location {:?}",
                    old_location, team.current_location
                );
            }
            _ => {}
        }

        self.players.insert(player.id, player);
        self.teams.insert(team.id, team);
        self.dirty = true;
        self.dirty_ui = true;
        Ok(())
    }

    pub fn next_free_pirates_refresh(&self) -> Tick {
        // Returns the time to the next FA refresh in milliseconds
        let next_refresh = self.last_tick_long_interval + TickInterval::LONG;
        if next_refresh > self.last_tick_short_interval {
            next_refresh - self.last_tick_short_interval
        } else {
            0
        }
    }

    pub fn add_player_to_team(&mut self, player_id: PlayerId, team_id: TeamId) -> AppResult<()> {
        let mut player = self.get_player(player_id).unwrap().clone();
        let mut team = self.get_team_or_err(team_id)?.clone();
        team.can_add_player(&player)?;

        team.add_player(&mut player);
        team.player_ids = Team::best_position_assignment(
            team.player_ids
                .iter()
                .map(|&id| self.get_player(id).unwrap())
                .collect(),
        );
        player.current_location = PlayerLocation::WithTeam;
        player.version += 1;
        self.players.insert(player.id, player);
        self.teams.insert(team.id, team);
        self.dirty = true;
        if team_id == self.own_team_id {
            self.dirty_network = true;
        }
        self.dirty_ui = true;

        Ok(())
    }

    pub fn hire_player_for_team(&mut self, player_id: PlayerId, team_id: TeamId) -> AppResult<()> {
        let mut player = self.get_player(player_id).unwrap().clone();
        let mut team = self.get_team_or_err(team_id)?.clone();
        team.can_hire_player(&player)?;

        team.remove_resource(Resource::SATOSHI, player.hire_cost(team.reputation))?;
        team.add_player(&mut player);
        team.player_ids = Team::best_position_assignment(
            team.player_ids
                .iter()
                .map(|&id| self.get_player(id).unwrap())
                .collect(),
        );
        player.current_location = PlayerLocation::WithTeam;
        player.version += 1;
        self.players.insert(player.id, player);
        self.teams.insert(team.id, team);
        self.dirty = true;
        if team_id == self.own_team_id {
            self.dirty_network = true;
        }
        self.dirty_ui = true;

        Ok(())
    }

    pub fn swap_players_team(
        &mut self,
        player_id1: PlayerId,
        player_id2: PlayerId,
    ) -> AppResult<()> {
        let team_id1 = self
            .get_player_or_err(player_id1)?
            .team
            .ok_or(anyhow!("Player swapped should have a team"))?;
        let team_id2 = self
            .get_player_or_err(player_id2)?
            .team
            .ok_or(anyhow!("Player swapped should have a team"))?;

        self.release_player_from_team(player_id1)?;
        self.release_player_from_team(player_id2)?;
        self.add_player_to_team(player_id1, team_id2)?;
        self.add_player_to_team(player_id2, team_id1)?;
        Ok(())
    }

    pub fn release_player_from_team(&mut self, player_id: PlayerId) -> AppResult<()> {
        let mut player = self.get_player_or_err(player_id)?.clone();
        if player.team.is_none() {
            return Err(anyhow!("Cannot release player with no team"));
        }
        let mut team = self.get_team_or_err(player.team.unwrap())?.clone();
        team.can_release_player(&player)?;
        team.remove_player(&mut player)?;
        team.player_ids = Team::best_position_assignment(
            team.player_ids
                .iter()
                .map(|&id| self.get_player(id).unwrap())
                .collect(),
        );

        player.version += 1;
        self.players.insert(player.id, player.clone());
        team.version += 1;
        self.teams.insert(team.id, team.clone());

        self.dirty = true;
        if team.id == self.own_team_id {
            self.dirty_network = true;
        }
        self.dirty_ui = true;

        Ok(())
    }

     fn generate_game_no_checks(
        &mut self,
        mut home_team_in_game: TeamInGame,
        mut away_team_in_game: TeamInGame,
        starting_at:Tick,
        location: PlanetId

    ) -> AppResult<GameId> {
        // Generate deterministic game id from team IDs and starting time.
        // Two games starting at u64::MAX milliseconds apart ~ 584_942_417 years
        // would have the same ID, we assume this can't happen.
        let mut rng_seed = ((home_team_in_game.team_id.as_u64_pair().0 as u128
            + away_team_in_game.team_id.as_u64_pair().0 as u128)
            % (u64::MAX as u128)) as u64;
        rng_seed = ((rng_seed as u128 + starting_at) % (u64::MAX as u128)) as u64;
        let rng = &mut ChaCha8Rng::seed_from_u64(rng_seed);
        let game_id = GameId::from_u128(rng.gen());

        let planet = self.get_planet_or_err(location)?;

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
            planet,
        );
        self.games.insert(game.id, game);

        Ok(game_id)
    }


    pub fn generate_network_game(
        &mut self,
        home_team_in_game: TeamInGame,
        away_team_in_game: TeamInGame,
        starting_at: Tick
    ) -> AppResult<GameId> {
        let mut home_team = self.get_team_or_err(home_team_in_game.team_id)?.clone();
        let mut away_team = self.get_team_or_err(away_team_in_game.team_id)?.clone();

        // For a network game we run different checks.
        home_team.can_challenge_team_over_network(&away_team)?;

        let location = match home_team.current_location {
            TeamLocation::OnPlanet { planet_id } => planet_id,
            _ => {
                panic!("Should have failed in can_challenge_team")
            }
        };

       let game_id =  self.generate_game_no_checks(home_team_in_game,away_team_in_game,starting_at, location)?;

       if let Some(previous_game_id) = home_team.current_game {
        if game_id != previous_game_id {
            return Err(anyhow!("Team is already playing another game"));
        }
    }

    if let Some(previous_game_id) = away_team.current_game {
        if game_id != previous_game_id {
            return Err(anyhow!("Opponent is already playing another game"));
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


    pub fn generate_game(
        &mut self,
        home_team_in_game: TeamInGame,
        away_team_in_game: TeamInGame,

    ) -> AppResult<GameId> {
        let starting_at = Tick::now() + GAME_START_DELAY;
        let mut home_team = self.get_team_or_err(home_team_in_game.team_id)?.clone();
        let mut away_team = self.get_team_or_err(away_team_in_game.team_id)?.clone();

        home_team.can_challenge_team(&away_team)?;

        let location = match home_team.current_location {
            TeamLocation::OnPlanet { planet_id } => planet_id,
            _ => {
                panic!("Should have failed in can_challenge_team")
            }
        };

       let game_id =  self.generate_game_no_checks(home_team_in_game,away_team_in_game,starting_at, location)?;

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

        let db_game = self.get_game(network_game.id);
        if db_game.is_none() {
            let mut game = Game::new(
                network_game.id,
                network_game.home_team_in_game,
                network_game.away_team_in_game,
                network_game.starting_at,
                self.get_planet_or_err(network_game.location)?,
            );

            while game.timer.value < network_game.timer.value && !game.timer.has_ended() {
                game.tick(Tick::now());
            }

            self.games.insert(game.id, game);
            self.dirty_ui = true;
        }

        Ok(())
    }

    pub fn add_network_team(&mut self, network_team: NetworkTeam) -> AppResult<()> {
        let NetworkTeam {
            team,
            players,
            home_planet,
        } = network_team;
        if team.peer_id.is_none() {
            return Err(anyhow!(
                "Cannot receive team without peer_id over the network."
            ));
        }
        if team.id == self.own_team_id {
            return Err(anyhow!(
                "Cannot receive own team over the network."
            ));
        }

        // Check if we are receiving a team with which we have an open challenge.
        // Note: there could be a race condition where we receive a team over the network right after
        //       accepting the challenge but before the challenge has been finalized on our side.
        //       In this case, the received team would have current_game set to some (set to the challenge game
        //       they just started) and the challenge would fail on our hand since the challenge team must have no game.
        let own_team = self.get_own_team()?;
        for player in &players {
            // Check if any player in the team is part of own team, in which case fail.
            // This check guarantees that the own team state gets precedence over
            // what we receive from the network.
            // Note: finalizing a trade in handle_trade_topic assumes that this check is in place
            //       to ensure that there is no race condition between receiving the trade
            //       syn_ack state and the network team from the trade proposer.
            if own_team.player_ids.contains(&player.id) {
                return Err(anyhow!(
                    "Cannot receive over the network a player which is part of own team."
                ));
            }
        }

        let db_team = self.get_team(team.id).cloned();

        // Remove team from previous planet
        if let Some(previous_version_team) = db_team.as_ref() {
            match previous_version_team.current_location {
                TeamLocation::OnPlanet { planet_id } => {
                    let mut planet = self.get_planet_or_err(planet_id)?.clone();
                    planet.team_ids.retain(|&id| id != team.id);
                    self.planets.insert(planet.id, planet);
                }
                _ => {}
            }

            // Remove players from db_team that are not in the new team to clean up fired players
            for player_id in &previous_version_team.player_ids {
                self.players.remove(&player_id);
            }
        }

        // When adding a network planet, we do not update the parent planet satellites on purpose,
        // to avoid complications when cleaning up to store the world.
        // This means that the network satellite will not appear in the galaxy.
        if let Some(planet) = home_planet {
            if planet.peer_id.is_none() {
                return Err(anyhow!(
                    "Cannot receive planet without peer_id over the network."
                ));
            }
            self.planets.insert(planet.id, planet);
        }

        // Add team to new planet
        match team.current_location {
            TeamLocation::OnPlanet { planet_id } => {
                let mut planet = self.get_planet_or_err(planet_id)?.clone();
                planet.team_ids.push(team.id);
                self.planets.insert(planet.id, planet);
            }
            _ => {}
        }

        for player in players {
            if player.peer_id.is_none() || player.peer_id.unwrap() != team.peer_id.unwrap() {
                return Err(anyhow!(
                    "Cannot receive player with wrong peer_id over the network."
                ));
            }
            self.players.insert(player.id, player);
        }

        self.teams.insert(team.id, team);
        self.dirty_ui = true;

        Ok(())
    }

    pub fn get_team(&self, id: TeamId) -> Option<&Team> {
        self.teams.get(&id)
    }

    pub fn get_team_or_err(&self, id: TeamId) -> AppResult<&Team> {
        self.get_team(id)
            .ok_or(anyhow!("Team {:?} not found", id).into())
    }

    pub fn get_own_team(&self) -> AppResult<&Team> {
        self.get_team_or_err(self.own_team_id)
    }

    pub fn get_own_team_mut(&mut self) -> AppResult<&mut Team> {
        self.teams
            .get_mut(&self.own_team_id)
            .ok_or(anyhow!("Team {:?} not found", self.own_team_id))
    }

    pub fn get_planet(&self, id: PlanetId) -> Option<&Planet> {
        self.planets.get(&id)
    }

    pub fn get_planet_or_err(&self, id: PlanetId) -> AppResult<&Planet> {
        self.get_planet(id)
            .ok_or(anyhow!("Planet {:?} not found", id))
    }

    pub fn get_player(&self, id: PlayerId) -> Option<&Player> {
        self.players.get(&id)
    }

    pub fn get_player_or_err(&self, id: PlayerId) -> AppResult<&Player> {
        self.get_player(id)
            .ok_or(anyhow!("Player {:?} not found", id))
    }

    pub fn get_players_by_team(&self, team: &Team) -> AppResult<Vec<Player>> {
        Ok(team
            .player_ids
            .iter()
            .map(|&id| {
                self.get_player(id)
                    .ok_or(anyhow!("Player {:?} not found", id))
            })
            .collect::<Result<Vec<&Player>, _>>()?
            .iter()
            .map(|&p| p.clone())
            .collect::<Vec<Player>>())
    }

    pub fn get_game(&self, id: GameId) -> Option<&Game> {
        self.games.get(&id)
    }

    pub fn get_game_or_err(&self, id: GameId) -> AppResult<&Game> {
        self.get_game(id).ok_or(anyhow!("Game {:?} not found", id))
    }

    pub fn team_rating(&self, team_id: TeamId) -> AppResult<f32> {
        let team = self.get_team_or_err(team_id)?;
        if team.player_ids.len() == 0 {
            return Ok(0.0);
        }
        Ok(team.player_ids
            .iter()
            .filter(|&&id| self.get_player(id).is_some())
            .map(|&id| self.get_player(id).unwrap().rating())
            .sum::<u8>() as f32
            / team.player_ids.len() as f32)
    }

    pub fn simulate_until_now(&mut self) -> AppResult<Vec<UiCallbackPreset>> {
        if !self.has_own_team() {
            return Ok(vec![]);
        }

        let mut callbacks = vec![];
        let now = Tick::now();
        let last_tick_long = self.last_tick_long_interval;

        while self.last_tick_short_interval + TickInterval::SHORT < now {
            let mut tick_callbacks =
                self.handle_tick_events(self.last_tick_short_interval + TickInterval::SHORT, true)?;
            callbacks.append(&mut tick_callbacks);
        }

        // Workaround to ensure we only generate FAs at most once.
        if now - last_tick_long >= TickInterval::LONG {
            let callback = self.tick_free_pirates()?;
            callbacks.push(callback);
        }

        Ok(callbacks)
    }

    fn resources_found_after_exploration(
        &self,
        team: &Team,
        planet: &Planet,
        duration: u128,
    ) -> AppResult<HashMap<Resource, u32>> {
        let mut rng = ChaCha8Rng::from_entropy();
        let mut resources = HashMap::new();

        let bonus = TeamBonus::Exploration.current_team_bonus(&self, team.id)?;

        for (&resource, &amount) in planet.resources.iter() {
            let mut found_amount = 0;
            // The exploration bonus makes the random range larger, which is positive in expectation
            // since we clamp at 0.
            let base = ((2.0_f32).powf(amount as f32 / 2.0) * bonus) as i32;
            for _ in 0..(duration / QUICK_EXPLORATION_TIME) {
                found_amount += rng.gen_range(-base..base).max(0) as u32;
            }
            resources.insert(resource, found_amount);
        }

        Ok(resources)
    }

    fn free_pirates_found_after_exploration(
        &mut self,
        planet: &Planet,
        duration: u128,
    ) -> Vec<PlayerId> {
        let rng = &mut ChaCha8Rng::from_entropy();
        let mut free_pirates = vec![];

        let duration_bonus = (duration as f32 / (1 * HOURS) as f32).powf(1.3);
        let population_bonus = planet.total_population() as f32;

        let amount = rng
            .gen_range((-32 + (population_bonus + duration_bonus) as i32).min(0)..3)
            .max(0);

        if amount > 0 {
            for _ in 0..amount {
                let base_level = rng.gen_range(0.0..7.0);
                let player_id = self.generate_random_player(rng, None, None, planet.id, base_level);

                free_pirates.push(player_id);
            }
        }

        free_pirates
    }

    pub fn handle_tick_events(
        &mut self,
        current_timestamp: Tick,
        is_simulating: bool,
    ) -> AppResult<Vec<UiCallbackPreset>> {
        let mut callbacks: Vec<UiCallbackPreset> = vec![];

        if current_timestamp >= self.last_tick_short_interval + TickInterval::SHORT {
            if self.games.len() > 0 {
                self.tick_games(current_timestamp)?;
                self.cleanup_games(current_timestamp)?;
            }

            if let Some(callback) = self.tick_travel(current_timestamp, is_simulating)? {
                callbacks.push(callback);
            }
            if let Some(callback) = self.tick_spaceship_upgrade(current_timestamp, is_simulating)? {
                callbacks.push(callback);
            }
            self.last_tick_short_interval += TickInterval::SHORT;
            // Round up to the TickInterval::SHORT to keep these ticks synchronous across network.
            self.last_tick_short_interval -= self.last_tick_short_interval % TickInterval::SHORT;
        }

        if current_timestamp >= self.last_tick_medium_interval + TickInterval::MEDIUM {
            self.tick_tiredness_recovery()?;
            self.tick_player_leaving_own_team()?;

            if !is_simulating && self.games.len() < AUTO_GENERATE_GAMES_NUMBER {
                self.generate_random_games()?;
            }

            // Once every MEDIUM interval, set dirty_network flag,
            // so that we send our team to the network.
            if !is_simulating {
                self.dirty_network = true;
            }

            self.last_tick_medium_interval += TickInterval::MEDIUM;
            
        }

        if current_timestamp >= self.last_tick_long_interval + TickInterval::LONG {
            if !is_simulating {
                callbacks.push(self.tick_free_pirates()?);
            }

            self.tick_players_update();
            self.tick_teams_reputation()?;
            self.last_tick_long_interval += TickInterval::LONG;
        }

        Ok(callbacks)
    }

    fn cleanup_games(&mut self, current_timestamp: Tick) -> AppResult<()> {
        for (_, game) in self.games.iter() {
            if game.ended_at.is_some()
                && current_timestamp > game.ended_at.unwrap() + GAME_CLEANUP_TIME
            {
                info!(
                    "Game {} vs {}: started at {}, ended at {} and is being removed at {}",
                    game.home_team_in_game.name,
                    game.away_team_in_game.name,
                    game.starting_at.formatted_as_time(),
                    game.ended_at.unwrap().formatted_as_time(),
                    current_timestamp.formatted_as_time()
                );
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
                        let mut player = self.get_player_or_err(game_player.id)?.clone();
                        player.tiredness = game_player.tiredness;
                        player.morale = game_player.morale;

                        player.version += 1;
                        player.add_morale(MORALE_INCREASE_PER_GAME);

                        let stats = team
                            .stats
                            .get(&player.id)
                            .ok_or(anyhow!("Player {:?} not found in team stats", player.id))?;

                        player.reputation = (player.reputation
                            + REPUTATION_PER_EXPERIENCE
                                * stats.seconds_played as f32
                                * TeamBonus::Reputation.current_team_bonus(
                                    self,
                                    player.team.expect("Player should have a team"),
                                )?)
                        .bound();

                        let training_bonus =
                            TeamBonus::Training.current_team_bonus(&self, team.team_id)?;
                        let training_focus = team.training_focus;
                        player.update_skills_training(
                            stats.experience_at_position,
                            training_bonus,
                            training_focus,
                        );
                        self.players.insert(player.id, player);
                    }
                }

                // Past games of the own team are persisted in the store.
                if game.home_team_in_game.team_id == self.own_team_id
                    || game.away_team_in_game.team_id == self.own_team_id
                {
                    let game_summary = GameSummary::from_game(&game);
                    self.past_games.insert(game_summary.id, game_summary);
                    save_game(&game)?;
                    // Update network that game has ended.
                    self.dirty_network = true;
                }

                // Teams get money depending on game attendance.
                // Home team gets a bonus for playing at home.
                // If a team is knocked out, money goes to the other team.
                // If both are knocked out, they get no money.

                let mut home_team_income = 100 + game.attendance * INCOME_PER_ATTENDEE_HOME;
                let mut away_team_income = 100 + game.attendance * INCOME_PER_ATTENDEE_AWAY;
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

                // Winner team gets reputation bonus
                let (home_team_reputation, away_team_reputation) = match game.winner {
                    Some(winner) => {
                        if winner == game.home_team_in_game.team_id {
                            (REPUTATION_BONUS_WINNER, REPUTATION_BONUS_LOSER)
                        } else {
                            (REPUTATION_BONUS_LOSER, REPUTATION_BONUS_WINNER)
                        }
                    }
                    None => (REPUTATION_BONUS_DRAW, REPUTATION_BONUS_DRAW),
                };

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

                // Update to team game records.
                let (home_team_record, away_team_record) = match game.winner {
                    Some(winner) => {
                        if winner == game.home_team_in_game.team_id {
                            ([1,0,0], [0,1,0])
                        } else {
                            ([0,1,0], [1,0,0])
                        }
                    }
                    None => ([0,0,1], [0,0,1])
                };

                // Set playing teams current game to None and assign income, reputation, and rum.
                if let Ok(res) = self.get_team_or_err(game.home_team_in_game.team_id) {
                    let mut home_team = res.clone();
                    home_team.current_game = None;
                    home_team.add_resource(Resource::SATOSHI, home_team_income);
                    home_team.reputation = (home_team.reputation + home_team_reputation).bound();
                    home_team.add_resource(Resource::RUM, home_team_rum);
                    if game.home_team_in_game.peer_id.is_some() &&game.away_team_in_game.peer_id.is_some() {
                        // If it's a network game, update the network game record,
                        home_team.network_game_record = [home_team.network_game_record[0] + home_team_record[0],home_team.network_game_record[1] + home_team_record[1],home_team.network_game_record[2] + home_team_record[2]];
                    } else {
                        // else update the game record.
                        home_team.game_record = [home_team.game_record[0] + home_team_record[0],home_team.game_record[1] + home_team_record[1],home_team.game_record[2] + home_team_record[2]];
                    }
                    self.teams.insert(home_team.id, home_team.clone());
                }

                if let Ok(res) = self.get_team_or_err(game.away_team_in_game.team_id) {
                    let mut away_team = res.clone();
                    away_team.current_game = None;
                    away_team.add_resource(Resource::SATOSHI, away_team_income);
                    away_team.reputation = (away_team.reputation + away_team_reputation).bound();
                    away_team.add_resource(Resource::RUM, away_team_rum);

                    if game.home_team_in_game.peer_id.is_some() &&game.away_team_in_game.peer_id.is_some() {
                        // If it's a network game, update the network game record,
                        away_team.network_game_record = [away_team.network_game_record[0] + away_team_record[0],away_team.network_game_record[1] + away_team_record[1],away_team.network_game_record[2] + away_team_record[2]];
                    } else {
                        // else update the game record.
                        away_team.game_record = [away_team.game_record[0] + away_team_record[0],away_team.game_record[1] + away_team_record[1],away_team.game_record[2] + away_team_record[2]];
                    }
                    self.teams.insert(away_team.id, away_team.clone());
                }

                self.dirty = true;
                self.dirty_ui = true;
            }
        }
        self.games.retain(|_, game| {
            game.ended_at.is_none()
                || (game.ended_at.is_some()
                    && current_timestamp <= game.ended_at.unwrap() + GAME_CLEANUP_TIME)
        });
        Ok(())
    }

    fn tick_games(&mut self, current_timestamp: Tick) -> AppResult<()> {
        // NOTE!!: we do not set the world to dirty so we don't save on every tick.
        //         the idea is that the game is completely determined at the beginning,
        //         so we can similuate it through.
        for (_, game) in self.games.iter_mut() {
            if game.has_started(current_timestamp) && !game.has_ended() {
                game.tick(current_timestamp);
            }
        }
        Ok(())
    }

    fn team_reputation_bonus_per_distance(distance: u128) -> f32 {
        ((distance as f32 + 1.0).ln()).powf(4.0) * TEAM_REPUTATION_BONUS_MODIFIER
    }

    pub fn tick_travel(
        &mut self,
        current_timestamp: Tick,
        _is_simulating: bool,
    ) -> AppResult<Option<UiCallbackPreset>> {
        let own_team = self.get_own_team()?;

        match own_team.current_location {
            TeamLocation::Travelling {
                from: _,
                to,
                started,
                duration,
                distance,
            } => {
                if current_timestamp > started + duration {
                    let mut team = own_team.clone();
                    let team_name = team.name.clone();
                    team.current_location = TeamLocation::OnPlanet { planet_id: to };
                    let mut planet = self.get_planet_or_err(to)?.clone();
                    let planet_name = planet.name.clone();
                    let planet_filename = planet.filename.clone();

                    planet.team_ids.push(team.id);

                    for player in team.player_ids.iter() {
                        let mut player = self.get_player_or_err(*player)?.clone();
                        player.set_jersey(&team.jersey);
                        self.players.insert(player.id, player);
                    }

                    team.spaceship.total_travelled += distance;

                    // Increase team reputation based on the travel distance
                    let reputation_bonus = Self::team_reputation_bonus_per_distance(distance);
                    team.reputation = (team.reputation + reputation_bonus).bound();

                    self.teams.insert(team.id, team);
                    self.planets.insert(planet.id, planet);
                    self.dirty = true;
                    self.dirty_network = true;
                    self.dirty_ui = true;
                    return Ok(Some(UiCallbackPreset::PushUiPopup {
                        popup_message: PopupMessage::TeamLanded(
                            team_name,
                            planet_name,
                            planet_filename,
                            Tick::now(),
                        ),
                    }));
                }
            }
            TeamLocation::Exploring {
                around,
                started,
                duration,
            } => {
                if current_timestamp > started + duration {
                    let mut team = own_team.clone();

                    for player in team.player_ids.iter() {
                        let mut player = self.get_player_or_err(*player)?.clone();
                        player.set_jersey(&team.jersey);
                        self.players.insert(player.id, player);
                    }

                    let home_planet = self.get_planet_or_err(team.home_planet_id)?;
                    let mut around_planet = self.get_planet_or_err(around)?.clone();

                    let mut rng = ChaCha8Rng::from_entropy();

                    // If the home planet is an asteroid, it means an asteroid has already been found.
                    let duration_bonus = duration as f64 / (1.0 * HOURS as f64);
                    // If team has already MAX_NUM_ASTEROID_PER_TEAM, it cannot find another one. Finding asteroids
                    // becomes progressively more difficult.
                    let team_asteroid_modifier = (MAX_NUM_ASTEROID_PER_TEAM - team.asteroid_ids.len()) as f64 / MAX_NUM_ASTEROID_PER_TEAM as f64;
                    if home_planet.planet_type != PlanetType::Asteroid
                        && rng.gen_bool(
                            (ASTEROID_DISCOVERY_PROBABILITY
                                * around_planet.asteroid_probability 
                                * duration_bonus
                                *team_asteroid_modifier)
                                .min(1.0),
                        )
                    {
                        // We temporarily set the team back on the exploration base planet,
                        // until the asteroid is accepted and generated.
                        team.current_location = TeamLocation::OnPlanet { planet_id: around };
                        self.teams.insert(team.id, team);
                        self.dirty = true;
                        self.dirty_network = true;
                        self.dirty_ui = true;

                        return Ok(Some(UiCallbackPreset::PushUiPopup {
                            popup_message: PopupMessage::AsteroidNameDialog(Tick::now()),
                        }));
                    }

                    team.current_location = TeamLocation::OnPlanet { planet_id: around };
                    around_planet.team_ids.push(team.id);

                    let found_resources =
                        self.resources_found_after_exploration(&team, &around_planet, duration)?;
                    // Try to add resources starting from the most expensive one,
                    // but still trying to add the others if they fit (notice that resources occupy a different amount of space).
                    for (resource, &amount) in found_resources
                        .iter()
                        .sorted_by(|(a, _), (b, _)| b.base_price().total_cmp(&a.base_price()))
                    {
                        team.add_resource(resource.clone(), amount);
                    }

                    let found_pirates = self
                        .free_pirates_found_after_exploration(&around_planet, duration)
                        .iter()
                        .map(|&player_id| {
                            self.get_player_or_err(player_id)
                                .expect("Player should be part of world")
                                .clone()
                        })
                        .collect_vec();

                    self.planets.insert(around_planet.id, around_planet);
                    self.teams.insert(team.id, team);

                    self.dirty = true;
                    self.dirty_network = true;
                    self.dirty_ui = true;

                    return Ok(Some(UiCallbackPreset::PushUiPopup {
                        popup_message: PopupMessage::ExplorationResult(
                            found_resources,
                            found_pirates,
                            Tick::now(),
                        ),
                    }));
                }
            }
            _ => {}
        }
        Ok(None)
    }

    pub fn tick_spaceship_upgrade(
        &mut self,
        current_timestamp: Tick,
        _is_simulating: bool,
    ) -> AppResult<Option<UiCallbackPreset>> {
        let own_team = self.get_own_team()?;
        if let Some(upgrade) = own_team.spaceship.pending_upgrade.clone() {
            if current_timestamp > upgrade.started + upgrade.duration {
                return Ok(Some(UiCallbackPreset::UpgradeSpaceship { upgrade }));
            }
        }
        Ok(None)
    }

    fn tick_tiredness_recovery(&mut self) -> AppResult<()> {
        let teams = self
            .teams
            .values()
            .filter(|team| team.current_game.is_none() && team.peer_id.is_none())
            .collect::<Vec<&Team>>();

        for team in teams {
            let bonus = TeamBonus::TirednessRecovery.current_team_bonus(&self, team.id)?;
            for player_id in team.player_ids.iter() {
                let db_player = self
                    .get_player(*player_id)
                    .ok_or(anyhow!("Player {:?} not found", player_id))?;
                if db_player.tiredness > 0.0 && db_player.tiredness <= MAX_TIREDNESS {
                    let mut player = db_player.clone();
                    // Recovery outside of games is slower by a factor TICK_SHORT_INTERVAL/TICK_MEDIUM_INTERVAL
                    // so that it takes 1 minute * 10 * 100 ~ 18 hours to recover from 100% tiredness.
                    player.tiredness =
                        (player.tiredness - bonus * RECOVERING_TIREDNESS_PER_SHORT_TICK).max(0.0);
                    self.players.insert(player.id, player);
                }
            }
        }

        Ok(())
    }

    fn tick_free_pirates(&mut self) -> AppResult<UiCallbackPreset> {
        self.players.retain(|_, player| player.team.is_some());

        let rng = &mut ChaCha8Rng::seed_from_u64(rand::random());
        for planet in PLANET_DATA.iter() {
            self.populate_planet(rng, planet);
        }
        Ok(UiCallbackPreset::PushUiPopup {
            popup_message: PopupMessage::Ok("Free pirates refreshed".into(), false, Tick::now()),
        })
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
            player.info.age = player.info.age + AGE_INCREASE_PER_LONG_TICK;
            if player.team.is_some() {
                // Pirates slightly dislike being part of a team.
                // This is counteracted by the morale boost pirates get by playing games.
                player.morale = (player.morale + MORALE_DECREASE_PER_LONG_TICK).bound();
            }
            player.reputation = (player.reputation - REPUTATION_DECREASE_PER_LONG_TICK).bound();

            for idx in 0..player.skills_training.len() {
                // Reduce player skills. This is planned to counteract the effect of training by playing games.
                // Older players get worse faster.
                player.modify_skill(
                    idx,
                    SKILL_DECREMENT_PER_LONG_TICK * player.info.relative_age().max(0.1),
                );
                // Reduce athletics skills even more if relative_age is more than 0.75.
                if idx < 4 && player.info.relative_age() > 0.75 {
                    player.modify_skill(
                        idx,
                        SKILL_DECREMENT_PER_LONG_TICK * player.info.relative_age(),
                    );
                }
                // Increase player skills from training
                player.modify_skill(idx, player.skills_training[idx]);
                player.skills_training[idx] = 0.0;
            }
            player.previous_skills = player.current_skill_array();
        }
    }

    fn tick_teams_reputation(&mut self) -> AppResult<()> {
        let mut reputation_update: Vec<(TeamId, f32)> = vec![];
        for (_, team) in self.teams.iter() {
            //TODO: once we remove local teams, we can remove this loop and only apply to own_team
            if team.peer_id.is_some() {
                continue;
            }
            let bonus = TeamBonus::Reputation.current_team_bonus(&self, team.id)?;
            let players_reputation = team
                .player_ids
                .iter()
                .map(|id| self.players.get(id).unwrap().reputation)
                .sum::<f32>()
                / MAX_POSITION as f32
                / 100.0
                * bonus;

            let new_reputation = if team.reputation > players_reputation {
                (team.reputation - players_reputation).bound()
            } else if team.reputation < players_reputation {
                (team.reputation + players_reputation).bound()
            } else {
                team.reputation
            };
            reputation_update.push((team.id, new_reputation));
        }

        for (team_id, new_reputation) in reputation_update {
            let mut team = self.get_team_or_err(team_id)?.clone();
            team.reputation = new_reputation;
            self.teams.insert(team.id, team);
        }
        Ok(())
    }

    fn tick_player_leaving_own_team(&mut self) -> AppResult<Vec<UiCallbackPreset>> {
        let mut messages = vec![];
        // Only players in own team can leave. In this way, we can run this tick often
        // and just skip it in case the own team is playing a game (which would give problems
        // if releasing the player).
        let own_team = self.get_own_team()?;
        if own_team.current_game.is_some() {
            return Ok(messages);
        }

        let mut releasing_player_ids = vec![];
        for &player_id in own_team.player_ids.iter() {
            let player = self.get_player_or_err(player_id)?;
            let mut rng = ChaCha8Rng::from_entropy();

            if player.morale < MORALE_THRESHOLD_FOR_LEAVING {
                if rng.gen_bool(
                    (1.0 - player.morale / MAX_SKILL) as f64 * LEAVING_PROBABILITY_MORALE_MODIFIER,
                ) {
                    releasing_player_ids.push(player_id);
                    messages.push(UiCallbackPreset::PushUiPopup {
                        popup_message: PopupMessage::Ok(
                            format!(
                                "{} {} left the crew!\n{} morale was too low...",
                                player.info.first_name,
                                player.info.last_name,
                                player.info.pronouns.as_possessive()
                            ),false, 
                            Tick::now(),
                        ),
                    })
                }
            } else if player.info.relative_age() > rng.gen_range(MIN_RELATIVE_RETIREMENT_AGE..1.0) {
                releasing_player_ids.push(player_id);
                messages.push(UiCallbackPreset::PushUiPopup {
                    popup_message: PopupMessage::Ok(
                        format!(
                            "{} {} left the crew and retired to cultivate turnips\n{} {} been a great pirate...",
                            player.info.first_name,
                            player.info.last_name,
                            player.info.pronouns.as_subject(),
                            player.info.pronouns.to_have(),
                        ),false, 
                        Tick::now(),
                    ),
                })
            }
        }

        for &player_id in releasing_player_ids.iter() {
            self.release_player_from_team(player_id)?;
        }

        Ok(messages)
    }

    fn generate_random_games(&mut self) -> AppResult<()> {
        let rng = &mut rand::thread_rng();
        for planet in self.planets.values() {
            if planet.team_ids.len() < 2 {
                continue;
            }

            let candidate_teams = planet
                .team_ids
                .iter()
                .map(|&id| self.get_team_or_err(id))
                .filter(|team_res| {
                    if let Ok(team) = team_res {
                        let avg_tiredness = team
                            .player_ids
                            .iter()
                            .map(|&id| {
                                if let Ok(player) = self.get_player_or_err(id) {
                                    player.tiredness
                                } else {
                                    0.0
                                }
                            })
                            .sum::<f32>()
                            / team.player_ids.len() as f32;
                        return team.current_game.is_none()
                            && team.id != self.own_team_id
                            && team.peer_id.is_none()
                            && avg_tiredness <= MAX_AVG_TIREDNESS_PER_AUTO_GAME;
                    }
                    false
                })
                .collect::<AppResult<Vec<&Team>>>()?;

            if candidate_teams.len() < 2 {
                continue;
            }
            let teams = candidate_teams.iter().choose_multiple(rng, 2);
            let home_team_in_game =
                TeamInGame::from_team_id(teams[0].id, &self.teams, &self.players)
                    .ok_or(anyhow!("Team {:?} not found in world", teams[0].id))?;

            let away_team_in_game =
                TeamInGame::from_team_id(teams[1].id, &self.teams, &self.players)
                    .ok_or(anyhow!("Team {:?} not found in world", teams[1].id))?;


            self.generate_game(home_team_in_game, away_team_in_game)?;
            return Ok(());
        }

        Ok(())
    }

    pub fn filter_peer_data(&mut self, peer_id: Option<PeerId>) -> AppResult<()> {
        let mut own_team = self.get_own_team()?.clone();
        if let Some(peer_id) = peer_id {
            // Filter all data that has a specific peer_id
            self.teams
                .retain(|_, team| team.peer_id.is_none() || team.peer_id.unwrap() != peer_id);
            self.players
                .retain(|_, player| player.peer_id.is_none() || player.peer_id.unwrap() != peer_id);
            self.planets
                .retain(|_, planet| planet.peer_id.is_none() || planet.peer_id.unwrap() != peer_id);
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
            self.planets.retain(|_, planet| planet.peer_id.is_none());
            own_team.clear_challenges();
            own_team.clear_trades();
        }
        // Remove teams from planet teams vector.
        for (_, planet) in self.planets.iter_mut() {
            planet
                .team_ids
                .retain(|&team_id| self.teams.contains_key(&team_id));
        }

        // Set current game to None for teams playing a game not stored in games.
        for team in self.teams.values_mut() {
            if team.current_game.is_some() && !self.games.contains_key(&team.current_game.unwrap())
            {
                team.current_game = None;
            }
        }

        self.teams.insert(own_team.id, own_team);
        self.dirty = true;
        self.dirty_ui = true;
        Ok(())
    }

    pub fn travel_time_to_planet(&self, team_id: TeamId, to: PlanetId) -> AppResult<Tick> {
        let team = self.get_team_or_err(team_id)?;
        let from = match team.current_location {
            TeamLocation::OnPlanet { planet_id } => planet_id,
            TeamLocation::Travelling { .. } => {
                return Err(anyhow!("Team is travelling"))
            }
            TeamLocation::Exploring { .. } => {
                return Err(anyhow!("Team is exploring"))
            }
        };

        let distance = self.distance_between_planets(from, to)?;
        let bonus = TeamBonus::SpaceshipSpeed.current_team_bonus(&self, team.id)?;
        Ok(
            ((LANDING_TIME_OVERHEAD as f32 + (distance as f32 / team.spaceship_speed())) / bonus)
                as Tick,
        )
    }

    fn planet_height(&self, planet_id: PlanetId) -> AppResult<usize> {
        let mut planet = self.get_planet_or_err(planet_id)?;

        let mut height = 0;

        while planet.satellite_of.is_some() {
            planet = self.get_planet_or_err(planet.satellite_of.unwrap())?;
            height += 1;
        }
        Ok(height)
    }

    pub fn distance_between_planets(&self, from_id: PlanetId, to_id: PlanetId) -> AppResult<u128> {
        // We calculate the distance. 5 cases:

        // 1: from and to are the same planet -> 0
        if from_id == to_id {
            return Ok(0);
        }

        let from = self.get_planet_or_err(from_id)?;
        let to = self.get_planet_or_err(to_id)?;

        let from_height: usize = self.planet_height(from_id)?;
        let to_height: usize = self.planet_height(to_id)?;

        // 2: from and to have the same parent -> difference in largest and smallest axes
        if from.satellite_of == to.satellite_of {
            let distance = ((from.axis.0 - to.axis.0).abs()).max((from.axis.1 - to.axis.1).abs())
                / 24.0
                * BASE_DISTANCES[from_height - 1] as f32;

            return Ok(distance as u128);
        }

        // 3: from is a satellite of to -> largest 'from' axis divided by 24
        if from.satellite_of == Some(to.id) {
            let distance =
                (from.axis.0).max(from.axis.1) / 24.0 * BASE_DISTANCES[from_height - 1] as f32;

            return Ok(distance as u128);
        }

        // 4: to is a satellite of from -> largest 'to' axis divided by 24
        if to.satellite_of == Some(from.id) {
            let distance = (to.axis.0).max(to.axis.1) / 24.0 * BASE_DISTANCES[to_height - 1] as f32;

            return Ok(distance as u128);
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

        let parent_id = bottom.satellite_of.unwrap(); // This is guaranteed to be some, otherwise we would have matched already

        let distance =
            (bottom.axis.0).max(bottom.axis.1) / 24.0 * BASE_DISTANCES[bottom_height - 1] as f32;

        Ok(self.distance_between_planets(parent_id, top.id)? + distance as u128)
    }

    pub fn to_store(&self) -> AppResult<World> {
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
            past_games: self.past_games.clone(),
            serialized_size: self.serialized_size,
            ..Default::default()
        };
        w.filter_peer_data(None)?;
        Ok(w)
    }
}

#[cfg(test)]
mod test {
    use super::{AppResult, World, QUICK_EXPLORATION_TIME};
    use crate::{
        app::App,
        ui::ui_callback::UiCallbackPreset,
        world::{
            planet::PlanetType,
            player::Trait,
            resources::Resource,
            role::CrewRole,
            skill::Rated,
            types::{TeamBonus, TeamLocation},
            utils::PLANET_DATA,
            world::{ASTEROID_DISCOVERY_PROBABILITY, AU, HOURS, LONG_EXPLORATION_TIME, MAX_NUM_ASTEROID_PER_TEAM},
        },
    };
    use rand::{seq::IteratorRandom, Rng, SeedableRng};
    use rand_chacha::ChaCha8Rng;

    #[test]
    fn test_deterministic_randomness() {
        let seed = rand::random::<u64>();
        let rng = &mut ChaCha8Rng::seed_from_u64(seed);
        let mut v1 = vec![];
        let mut v2 = vec![];
        for _ in 0..10 {
            v1.push(rng.gen::<u8>());
        }
        let rng = &mut ChaCha8Rng::seed_from_u64(seed);
        for _ in 0..10 {
            v2.push(rng.gen::<u8>());
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
        let rng = &mut ChaCha8Rng::from_entropy();
        let planet = PLANET_DATA.iter().choose(rng).unwrap();
        println!(
            "Around planet {} - Population {} - Asteroid probability {}",
            planet.name,
            planet.total_population(),
            planet.asteroid_probability
        );
        println!(
            "Planet resourced:
    Satoshi {}
    Gold {}
    Scraps {}
    Fuel {}
    Rum {}",
            planet
                .resources
                .get(&Resource::SATOSHI)
                .copied()
                .unwrap_or_default(),
            planet
                .resources
                .get(&Resource::GOLD)
                .copied()
                .unwrap_or_default(),
            planet
                .resources
                .get(&Resource::SCRAPS)
                .copied()
                .unwrap_or_default(),
            planet
                .resources
                .get(&Resource::FUEL)
                .copied()
                .unwrap_or_default(),
            planet
                .resources
                .get(&Resource::RUM)
                .copied()
                .unwrap_or_default()
        );

        let team_id =
            world.generate_random_team(rng, planet.id, "test".into(), "testship".into())?;

        println!(
            "Pilot bonus {}",
            TeamBonus::SpaceshipSpeed.current_team_bonus(&world, team_id)?
        );

        let team = world.get_team_or_err(team_id)?.clone();
        println!("\nQUICK EXPLORATION");

        let duration = QUICK_EXPLORATION_TIME;
        let duration_bonus = duration as f64 / (1.0 * HOURS as f64);

        let found_resources = world.resources_found_after_exploration(&team, &planet, duration)?;

        println!("Found resources:");
        for res in found_resources.iter() {
            println!("  {} {}", res.1, res.0);
        }

        let found_free_pirates = world.free_pirates_found_after_exploration(&planet, duration);
        for &player_id in found_free_pirates.iter() {
            let player = world.get_player_or_err(player_id)?;
            println!(
                "  {:<16} {}\n",
                player.info.shortened_name(),
                player.stars()
            );
        }
        
        let team_asteroid_modifier = (MAX_NUM_ASTEROID_PER_TEAM - team.asteroid_ids.len()) as f64 / MAX_NUM_ASTEROID_PER_TEAM as f64;
        if planet.planet_type != PlanetType::Asteroid
            && rng.gen_bool(
                (ASTEROID_DISCOVERY_PROBABILITY
                    * planet.asteroid_probability 
                    * duration_bonus*team_asteroid_modifier)
                    .min(1.0),
            )
        {
            println!("Found asteroid!!!");
        }

        println!("\nLONG EXPLORATION");
        let duration = LONG_EXPLORATION_TIME;
        let duration_bonus = duration as f64 / (1.0 * HOURS as f64);
        let found_resources = world.resources_found_after_exploration(&team, &planet, duration)?;
        println!("Found resources:");
        for res in found_resources.iter() {
            println!("  {} {}", res.1, res.0);
        }

        let found_free_pirates = world.free_pirates_found_after_exploration(&planet, duration);
        for &player_id in found_free_pirates.iter() {
            let player = world.get_player_or_err(player_id)?;
            println!(
                "  {:<16} {}\n",
                player.info.shortened_name(),
                player.stars()
            );
        }
        let team_asteroid_modifier = (MAX_NUM_ASTEROID_PER_TEAM - team.asteroid_ids.len()) as f64 / MAX_NUM_ASTEROID_PER_TEAM as f64;
        if planet.planet_type != PlanetType::Asteroid
            && rng.gen_bool(
                (ASTEROID_DISCOVERY_PROBABILITY
                    * planet.asteroid_probability 
                    * duration_bonus*team_asteroid_modifier)
                    .min(1.0),
            )
        {
            println!("Found asteroid!!!");
        }

        Ok(())
    }

    #[test]
    fn test_spugna_portal() -> AppResult<()> {
        // To actually test this, set PORTAL_DISCOVERY_PROBABILITY to 1.0
        let mut app = App::new(None, true, true, false, false, None, None, None);
        app.new_world();

        let world = &mut app.world;
        let rng = &mut ChaCha8Rng::from_entropy();
        let planet = PLANET_DATA[0].clone();
        let team_id =
            world.generate_random_team(rng, planet.id, "test".into(), "testship".into())?;

        let team = &mut world.get_team_or_err(team_id)?.clone();
        team.add_resource(Resource::RUM, 20);

        let mut spugna = world.get_player_or_err(team.player_ids[0])?.clone();
        let spugna_id = spugna.id.clone();
        spugna.special_trait = Some(Trait::Spugna);
        if spugna.info.crew_role != CrewRole::Pilot {
            world.set_team_crew_role(CrewRole::Pilot, spugna.id)?;
        }
        world.players.insert(spugna.id, spugna);

        let target = PLANET_DATA[1].clone();
        team.current_location = TeamLocation::Travelling {
            from: planet.id,
            to: target.id,
            started: 0,
            duration: world.travel_time_to_planet(team.id, target.id)?,
            distance: world.distance_between_planets(planet.id, target.id)?,
        };

        println!("Team resources {:?}", team.resources);
        println!("Team location {:?}", team.current_location);
        world.teams.insert(team.id, team.clone());

        UiCallbackPreset::Drink {
            player_id: spugna_id,
        }
        .call(&mut app)?;

        let team = app.world.get_team_or_err(team_id)?;
        println!("Team resources {:?}", team.resources);
        println!("Team location {:?}", team.current_location);

        Ok(())
    }

    #[test]
    fn test_tick_players_update() -> AppResult<()> {
        let mut app = App::new(None, true, true, true, false, None, None, None);
        app.new_world();

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
                "Age {} - Overall {} {} - Potential {} {}",
                player.info.age,
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
                "Age {} - Overall {} {} - Potential {} {}",
                player.info.age,
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
    fn test_tick_player_leaving_own_team_for_age() -> AppResult<()> {
        let mut app = App::new(None, true, true, true, false, None, None, None);
        app.new_world();

        let world = &mut app.world;
        let rng = &mut ChaCha8Rng::from_entropy();
        world.own_team_id = world.generate_random_team(
            rng,
            world.planets.keys().next().unwrap().clone(),
            "team_name".into(),
            "ship_name".into(),
        )?;

        let own_team = world.get_own_team()?;
        let player_id = own_team.player_ids[0];
        let mut player = world.get_player_or_err(player_id)?.clone();
        assert!(player.team.is_some());

        player.info.age = player.info.population.max_age();
        world.players.insert(player_id, player);
        world.tick_player_leaving_own_team()?;

        let player = world.get_player_or_err(player_id)?;
        assert!(player.team.is_none());

        Ok(())
    }

    #[test]
    fn test_tick_player_leaving_own_team_for_morale() -> AppResult<()> {
        let mut app = App::new(None, true, true, true, false, None, None, None);
        app.new_world();

        let world = &mut app.world;
        let rng = &mut ChaCha8Rng::from_entropy();
        world.own_team_id = world.generate_random_team(
            rng,
            world.planets.keys().next().unwrap().clone(),
            "team_name".into(),
            "ship_name".into(),
        )?;

        let own_team = world.get_own_team()?;
        let player_id = own_team.player_ids[0];
        let mut player = world.get_player_or_err(player_id)?.clone();
        assert!(player.team.is_some());

        player.morale = 0.0;
        world.players.insert(player_id, player);

        // Players with low morale quit a team randomly
        let mut idx = 0;
        loop {
            world.tick_player_leaving_own_team()?;
            let player: &crate::world::player::Player = world.get_player_or_err(player_id)?;
            if player.team.is_none() {
                break;
            }
            idx += 1;
        }
        println!("Player left team after {idx} iterations");
        Ok(())
    }
}
