use super::constants::*;
use super::jersey::{Jersey, JerseyStyle};
use super::planet::{Planet, PlanetType};
use super::player::Player;
use super::position::{Position, MAX_POSITION};
use super::resources::Resource;
use super::role::CrewRole;
use super::skill::{GameSkill, Rated};
use super::spaceship::Spaceship;
use super::team::Team;
use super::types::{PlayerLocation, TeamBonus, TeamLocation};
use super::utils::{PLANET_DATA, TEAM_DATA};
use crate::engine::constants::RECOVERING_TIREDNESS_PER_SHORT_TICK;
use crate::engine::game::{Game, GameSummary};
use crate::engine::types::{Possession, TeamInGame};
use crate::image::color_map::ColorMap;
use crate::network::types::{NetworkGame, NetworkTeam};
use crate::store::{
    load_from_json, save_to_json, PERSISTED_GAMES_PREFIX, PERSISTED_WORLD_FILENAME,
};
use crate::types::*;
use crate::ui::ui_callback::UiCallbackPreset;
use itertools::Itertools;
use libp2p::PeerId;
use rand::seq::{IteratorRandom, SliceRandom};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

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
    pub teams: TeamMap,
    pub players: PlayerMap,
    pub planets: PlanetMap,
    pub games: GameMap,
    pub past_games: GameSummaryMap,
}

impl World {
    pub fn new(seed: Option<u64>) -> Self {
        let mut planets = HashMap::new();
        let data_planets: Vec<Planet> = PLANET_DATA.iter().map(|p| p.clone()).collect();
        for planet in data_planets.iter() {
            planets.insert(planet.id, planet.clone());
        }
        let seed = if seed.is_none() {
            rand::random()
        } else {
            seed.unwrap()
        };

        Self {
            seed,
            last_tick_short_interval: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis(),
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
        // generate free agents per each planet
        let number_free_agents = planet.total_population() / 10;
        let mut position = 0 as Position;
        let base_level = rng.gen_range(0..5) as f32;
        for _ in 0..number_free_agents {
            self.generate_random_player(rng, None, Some(position), planet.id, base_level);
            position = (position + 1) % MAX_POSITION;
        }
    }

    pub fn load() -> AppResult<Self> {
        load_from_json(PERSISTED_WORLD_FILENAME)
    }

    pub fn generate_local_world(&mut self, rng: &mut ChaCha8Rng) -> AppResult<()> {
        for (team_name, ship_name) in TEAM_DATA.iter() {
            let home_planet = self
                .planets
                .values()
                .filter(|planet| planet.total_population() >= 10)
                .choose(rng)
                .unwrap();

            self.generate_random_team(rng, home_planet.id, team_name.clone(), ship_name.clone())?;
        }
        Ok(())
    }

    pub fn generate_random_team(
        &mut self,
        rng: &mut ChaCha8Rng,
        home_planet: PlanetId,
        team_name: String,
        ship_name: String,
    ) -> AppResult<TeamId> {
        let team_id = TeamId::new();
        let mut team = Team::random(team_id, home_planet, team_name);
        team.spaceship.name = ship_name;
        let home_planet = team.home_planet_id.clone();
        let team_base_level = rng.gen_range(0..=5) as f32;
        for position in 0..MAX_POSITION {
            self.generate_random_player(
                rng,
                Some(&mut team),
                Some(position),
                home_planet,
                team_base_level,
            );
        }
        while team.player_ids.len() < team.spaceship.crew_capacity() as usize {
            self.generate_random_player(rng, Some(&mut team), None, home_planet, team_base_level);
        }

        let players = team
            .player_ids
            .iter()
            .map(|id| self.players.get(id).unwrap())
            .collect::<Vec<&Player>>();
        team.player_ids = Team::best_position_assignment(players.clone());

        let mut planet = self.get_planet_or_err(home_planet)?.clone();
        planet.team_ids.push(team_id);
        self.planets.insert(planet.id, planet);

        self.teams.insert(team.id, team.clone());
        self.auto_set_team_roles(&mut team)?;
        self.dirty = true;
        self.dirty_ui = true;
        Ok(team_id)
    }

    pub fn generate_player_team(
        &mut self,
        name: String,
        home_planet_id: PlanetId,
        jersey_style: JerseyStyle,
        jersey_colors: ColorMap,
        players: Vec<PlayerId>,
        balance: u32,
        spaceship: Spaceship,
    ) -> AppResult<TeamId> {
        let team_id = TeamId::new();
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
        satellite_of: PlanetId,
    ) -> AppResult<PlanetId> {
        let asteroid = Planet::asteroid(PlanetId::new(), name, satellite_of);
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
        home_planet: PlanetId,
        base_level: f32,
    ) -> PlayerId {
        let player_id = PlayerId::new();
        let planet = self.get_planet_or_err(home_planet).unwrap();
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
        if player.team.is_none() {
            return Err(format!("Player {:?} is not in a team", player_id,).into());
        }

        let team_id = player.team.unwrap();
        let mut team = self.get_team_or_err(team_id)?.clone();
        team.can_set_crew_role(&player, role)?;

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

        // Demote previous crew role player to mozzo.
        if current_role_player.is_some() {
            let mut current_role_player = self
                .get_player_or_err(current_role_player.unwrap())?
                .clone();
            current_role_player.info.crew_role = CrewRole::Mozzo;
            // Demoted player is a bit demoralized :(
            if current_role_player.morale > MORALE_DEMOTION_MALUS {
                current_role_player.morale -= MORALE_DEMOTION_MALUS;
            } else {
                current_role_player.morale = 0.0;
            }
            team.crew_roles.mozzo.push(current_role_player.id);
            current_role_player.set_jersey(&jersey);
            self.players
                .insert(current_role_player.id, current_role_player);
        }

        let previous_spaceship_speed_bonus =
            TeamBonus::SpaceshipSpeed.current_team_bonus(&self, team.id)?;

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
        self.players.insert(player.id, player);

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
                let bonus = TeamBonus::SpaceshipSpeed.current_team_bonus(&self, team.id)?;

                let new_duration =
                    (duration - time_elapsed) as f32 * previous_spaceship_speed_bonus / bonus;

                team.current_location = TeamLocation::Travelling {
                    from,
                    to,
                    started: new_start,
                    duration: new_duration as Tick,
                    distance,
                };
            }
            _ => {}
        }

        self.teams.insert(team.id, team);
        self.dirty = true;
        self.dirty_ui = true;
        Ok(())
    }

    pub fn next_free_agents_refresh(&self) -> Tick {
        // Returns the time to the next FA refresh in milliseconds
        let next_refresh = self.last_tick_long_interval + TickInterval::LONG;
        if next_refresh > self.last_tick_short_interval {
            next_refresh - self.last_tick_short_interval
        } else {
            0
        }
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

    pub fn release_player_from_team(&mut self, player_id: PlayerId) -> AppResult<()> {
        let mut player = self
            .get_player(player_id)
            .ok_or(format!("Player {:?} not found", player_id))?
            .clone();

        let mut team = self.get_team_or_err(player.team.unwrap())?.clone();

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

    pub fn generate_game(
        &mut self,
        game_id: GameId,
        home_team_in_game: TeamInGame,
        away_team_in_game: TeamInGame,
        starting_at: Tick,
    ) -> AppResult<()> {
        let mut home_team = self.get_team_or_err(home_team_in_game.team_id)?.clone();
        let mut away_team = self.get_team_or_err(away_team_in_game.team_id)?.clone();

        // home_team.can_challenge_team(&away_team)?;

        let location = match home_team.current_location {
            TeamLocation::OnPlanet { planet_id } => planet_id,
            _ => {
                panic!("Should have failed in can_challenge_team")
            }
        };

        home_team.current_game = Some(game_id);
        away_team.current_game = Some(game_id);

        if home_team.id == self.own_team_id || away_team.id == self.own_team_id {
            // Update network that game has ended.
            self.dirty_network = true;
        }

        self.dirty = true;
        self.dirty_ui = true;

        if home_team.id == self.own_team_id || away_team.id == self.own_team_id {
            // Update network that game has started.
            self.dirty_network = true;
        }

        self.teams.insert(home_team.id, home_team);
        self.teams.insert(away_team.id, away_team);

        let planet = self.get_planet_or_err(location)?;

        let game = Game::new(
            game_id,
            home_team_in_game,
            away_team_in_game,
            starting_at,
            planet,
        );
        self.games.insert(game.id, game);

        Ok(())
    }

    pub fn add_network_game(&mut self, network_game: NetworkGame) -> AppResult<()> {
        // Check that the game does not involve the own team (otherwise we would have generated it).
        if network_game.home_team_in_game.team_id == self.own_team_id
            || network_game.away_team_in_game.team_id == self.own_team_id
        {
            return Err("Cannot receive game involving own team over the network.".into());
        }

        if network_game.timer.has_ended() {
            return Err("Cannot receive game that has ended over the network.".into());
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
            return Err("Cannot receive team without peer_id over the network.".into());
        }
        let db_team = self.get_team(team.id);
        // here the version can also be equal since we want to override the network team with the new peer_id in case of disconnections.
        if db_team.is_none() || db_team.unwrap().version <= team.version {
            // Remove team from previous planet
            if db_team.is_some() {
                match db_team.unwrap().current_location {
                    TeamLocation::OnPlanet { planet_id } => {
                        let mut planet = self.get_planet_or_err(planet_id)?.clone();
                        planet.team_ids.retain(|&id| id != team.id);
                        self.planets.insert(planet.id, planet);
                    }
                    _ => {}
                }
            }

            // When ading a network planet, we do not update the parent planet satellites on purpose,
            // to avoid complications when cleaning up to store the world.
            // This means that the network satellite will not appear in the galaxy.
            if let Some(planet) = home_planet {
                if planet.peer_id.is_none() {
                    return Err("Cannot receive planet without peer_id over the network.".into());
                }
                let db_planet = self.get_planet(planet.id);
                if db_planet.is_none() || db_planet.unwrap().version < planet.version {
                    self.planets.insert(planet.id, planet);
                }
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

            self.teams.insert(team.id, team);

            for player in players {
                if player.peer_id.is_none() {
                    return Err("Cannot receive player without peer_id over the network.".into());
                }
                let db_player = self.get_player(player.id);
                if db_player.is_none() || db_player.unwrap().version <= player.version {
                    self.players.insert(player.id, player);
                }
            }

            self.dirty_ui = true;
        }
        Ok(())
    }

    pub fn get_team(&self, id: TeamId) -> Option<&Team> {
        self.teams.get(&id)
    }

    pub fn get_team_or_err(&self, id: TeamId) -> AppResult<&Team> {
        self.get_team(id)
            .ok_or(format!("Team {:?} not found", id).into())
    }

    pub fn get_own_team(&self) -> AppResult<&Team> {
        self.get_team_or_err(self.own_team_id)
    }

    pub fn get_planet(&self, id: PlanetId) -> Option<&Planet> {
        self.planets.get(&id)
    }

    pub fn get_planet_or_err(&self, id: PlanetId) -> AppResult<&Planet> {
        self.get_planet(id)
            .ok_or(format!("Planet {:?} not found", id).into())
    }

    pub fn get_player(&self, id: PlayerId) -> Option<&Player> {
        self.players.get(&id)
    }

    pub fn get_player_or_err(&self, id: PlayerId) -> AppResult<&Player> {
        self.get_player(id)
            .ok_or(format!("Player {:?} not found", id).into())
    }

    pub fn get_players_by_team(&self, team: &Team) -> AppResult<Vec<Player>> {
        Ok(team
            .player_ids
            .iter()
            .map(|&id| {
                self.get_player(id)
                    .ok_or(format!("Player {:?} not found", id))
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
        self.get_game(id)
            .ok_or(format!("Game {:?} not found", id).into())
    }

    pub fn team_total_skills(&self, team_id: TeamId) -> u16 {
        self.get_team_or_err(team_id)
            .unwrap()
            .player_ids
            .iter()
            .filter(|&&id| self.get_player(id).is_some())
            .take(5)
            .map(|&id| self.get_player(id).unwrap().total_skills())
            .sum::<u16>()
    }

    pub fn team_rating(&self, team_id: TeamId) -> f32 {
        let team = self.get_team_or_err(team_id).unwrap();
        team.player_ids
            .iter()
            .filter(|&&id| self.get_player(id).is_some())
            .map(|&id| self.get_player(id).unwrap().rating())
            .sum::<u8>() as f32
            / team.player_ids.len() as f32
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
            let callback = self.tick_free_agents()?;
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
        let duration_bonus = (duration as f32 / (1 * HOURS) as f32).powf(1.3);

        let base_gold = 1;
        let base_scraps = 16;
        let base_rum = 5;

        let max_gold = (base_gold
            + planet
                .resources
                .get(&Resource::GOLD)
                .copied()
                .unwrap_or_default() as i32) as f32
            * bonus
            * duration_bonus;
        let max_scraps = (base_scraps
            + planet
                .resources
                .get(&Resource::SCRAPS)
                .copied()
                .unwrap_or_default() as i32) as f32
            * bonus
            * duration_bonus;
        let max_rum = (base_rum
            + planet
                .resources
                .get(&Resource::RUM)
                .copied()
                .unwrap_or_default() as i32) as f32
            * bonus
            * duration_bonus;

        resources.insert(
            Resource::GOLD,
            rng.gen_range((-50 + base_gold).min(0)..max_gold as i32)
                .max(0) as u32,
        );
        resources.insert(
            Resource::SCRAPS,
            rng.gen_range((-10 + base_scraps).min(0)..max_scraps as i32)
                .max(0) as u32,
        );
        resources.insert(
            Resource::RUM,
            rng.gen_range((-20 + base_rum).min(0)..max_rum as i32)
                .max(0) as u32,
        );

        Ok(resources)
    }

    fn free_agents_found_after_exploration(
        &self,
        _team: &Team,
        planet: &Planet,
        duration: u128,
    ) -> Vec<Player> {
        let mut rng = ChaCha8Rng::from_entropy();
        let mut free_agents = vec![];

        let duration_bonus = (duration as f32 / (1 * HOURS) as f32).powf(1.3);
        let population_bonus = planet.total_population() as f32 / 8.0;

        let amount = rng
            .gen_range((-32 + (population_bonus + duration_bonus) as i32).min(0)..3)
            .max(0);

        if amount > 0 {
            for _ in 0..amount {
                let base_level = rng.gen_range(0.0..5.0);
                let player_id = PlayerId::new();
                let player = Player::random(&mut rng, player_id, None, planet, base_level);
                free_agents.push(player);
            }
        }

        free_agents
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
            self.last_tick_short_interval += TickInterval::SHORT;
            // Round up to the TickInterval::SHORT
            self.last_tick_short_interval -= self.last_tick_short_interval % TickInterval::SHORT;
        }

        if current_timestamp >= self.last_tick_medium_interval + TickInterval::MEDIUM {
            self.tick_tiredness_recovery()?;

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
                let callback = self.tick_free_agents()?;
                callbacks.push(callback);
                self.tick_skill_improvements_reset()?;
            }
            self.tick_player_aging();
            self.tick_player_morale();
            self.modify_players_reputation();
            self.modify_teams_reputation()?;
            self.last_tick_long_interval += TickInterval::LONG;
        }

        Ok(callbacks)
    }

    fn cleanup_games(&mut self, current_timestamp: Tick) -> AppResult<()> {
        for (_, game) in self.games.iter() {
            if game.ended_at.is_some()
                && current_timestamp > game.ended_at.unwrap() + GAME_CLEANUP_TIME
            {
                log::info!(
                    "Game {} vs {}: started at {}, ended at {} and is being removed at {}",
                    game.home_team_in_game.name,
                    game.away_team_in_game.name,
                    game.starting_at.formatted_as_time(),
                    game.ended_at.unwrap().formatted_as_time(),
                    current_timestamp.formatted_as_time()
                );
                for team in [&game.home_team_in_game, &game.away_team_in_game] {
                    //we do not apply end of game logic to peer teams
                    if team.peer_id.is_some() && team.team_id != self.own_team_id {
                        continue;
                    }
                    for game_player in team.players.values() {
                        // Cloning sets the tiredness to the value in game as well
                        let mut player = game_player.clone();
                        let stats = team
                            .stats
                            .get(&player.id)
                            .ok_or(format!("Player {:?} not found in team stats", player.id))?;

                        let training_bonus =
                            TeamBonus::Training.current_team_bonus(&self, team.team_id)?;
                        let training_focus = team.training_focus;
                        player.apply_end_of_game_logic(
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
                    save_to_json(
                        format!("{}{}.json", PERSISTED_GAMES_PREFIX, game.id).as_str(),
                        &game,
                    )?;
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

                // Set playing teams current game to None and assign income and reputation.
                if let Ok(res) = self.get_team_or_err(game.home_team_in_game.team_id) {
                    let mut home_team = res.clone();
                    home_team.current_game = None;
                    home_team.add_resource(Resource::SATOSHI, home_team_income);
                    home_team.reputation = (home_team.reputation + home_team_reputation).bound();
                    self.teams.insert(home_team.id, home_team.clone());
                }

                if let Ok(res) = self.get_team_or_err(game.away_team_in_game.team_id) {
                    let mut away_team = res.clone();
                    away_team.current_game = None;
                    away_team.add_resource(Resource::SATOSHI, away_team_income);
                    away_team.reputation = (away_team.reputation + away_team_reputation).bound();
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

                    planet.team_ids.push(team.id);

                    for player in team.player_ids.iter() {
                        let mut player = self.get_player_or_err(*player)?.clone();
                        player.set_jersey(&team.jersey);
                        self.players.insert(player.id, player);
                    }

                    team.spaceship.total_travelled += distance;

                    self.teams.insert(team.id, team);
                    self.planets.insert(planet.id, planet);
                    self.dirty = true;
                    self.dirty_network = true;
                    self.dirty_ui = true;
                    return Ok(Some(UiCallbackPreset::PushUiPopup {
                        popup_message: crate::ui::popup_message::PopupMessage::Ok(
                            format!("{} has landed on planet {}", team_name, planet_name),
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
                    if home_planet.planet_type != PlanetType::Asteroid
                        && rng.gen_bool(
                            (ASTEROID_DISCOVERY_PROBABILITY
                                * around_planet.asteroid_probability as f64
                                * duration_bonus)
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
                            popup_message:
                                crate::ui::popup_message::PopupMessage::AsteroidNameDialog(
                                    Tick::now(),
                                ),
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

                    let found_free_agents =
                        self.free_agents_found_after_exploration(&team, &around_planet, duration);

                    for player in found_free_agents.iter() {
                        self.players.insert(player.id, player.clone());
                    }

                    let mut exploration_result_text = "".to_string();
                    for (resource, &amount) in found_resources.iter() {
                        if amount > 0 {
                            exploration_result_text.push_str(
                                format!("  {} {}\n", amount, resource.to_string().to_lowercase())
                                    .as_str(),
                            );
                        }
                    }

                    if found_free_agents.len() > 0 {
                        exploration_result_text.push_str(
                            format! {"\nFound {} stranded pirate{}:\n", found_free_agents.len(), if found_free_agents.len() > 1 {
                                "s"
                            }else{""}}.as_str(),
                        );
                        for player in found_free_agents.iter() {
                            let text = format!(
                                "  {:<16} {}\n",
                                format!(
                                    "{}.{}",
                                    player.info.first_name.chars().next().unwrap_or_default(),
                                    player.info.last_name
                                ),
                                player.stars()
                            );
                            exploration_result_text.push_str(text.as_str());
                        }
                    }
                    self.planets.insert(around_planet.id, around_planet);

                    self.teams.insert(team.id, team);
                    self.dirty = true;
                    self.dirty_network = true;
                    self.dirty_ui = true;

                    if exploration_result_text.len() == 0 {
                        exploration_result_text.push_str("Nothing found!")
                    }

                    return Ok(Some(UiCallbackPreset::PushUiPopup {
                        popup_message: crate::ui::popup_message::PopupMessage::Ok(
                            format!(
                                "Team has returned from exploration:\n\n{}",
                                exploration_result_text
                            ),
                            Tick::now(),
                        ),
                    }));
                }
            }
            _ => {}
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
                    .ok_or(format!("Player {:?} not found", player_id))?;
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

    fn tick_free_agents(&mut self) -> AppResult<UiCallbackPreset> {
        self.players.retain(|_, player| player.team.is_some());

        let rng = &mut ChaCha8Rng::seed_from_u64(rand::random());
        for planet in PLANET_DATA.iter() {
            self.populate_planet(rng, planet);
        }
        Ok(UiCallbackPreset::PushUiPopup {
            popup_message: crate::ui::popup_message::PopupMessage::Ok(
                "Free agents refreshed".into(),
                Tick::now(),
            ),
        })
    }

    fn tick_skill_improvements_reset(&mut self) -> AppResult<()> {
        for (_, player) in self.players.iter_mut() {
            player.previous_skills = player.current_skill_array();
        }
        Ok(())
    }

    fn tick_player_aging(&mut self) {
        for (_, player) in self.players.iter_mut() {
            if player.peer_id.is_some() {
                continue;
            }
            player.info.age = (player.info.age + AGE_INCREASE_PER_LONG_TICK).min(100.0);
        }
    }

    fn tick_player_morale(&mut self) {
        for (_, player) in self.players.iter_mut() {
            if player.peer_id.is_some() {
                continue;
            }
            if player.morale > MORALE_DECREASE_PER_LONG_TICK {
                player.morale = player.morale - MORALE_DECREASE_PER_LONG_TICK;
            } else {
                player.morale = 0.0;
            }
        }
    }

    fn modify_players_reputation(&mut self) {
        for (_, player) in self.players.iter_mut() {
            if player.peer_id.is_some() {
                continue;
            }
            if player.reputation > REPUTATION_DECREASE_PER_LONG_TICK {
                player.reputation -= REPUTATION_DECREASE_PER_LONG_TICK;
            } else {
                player.reputation = 0.0;
            }
        }
    }

    fn modify_teams_reputation(&mut self) -> AppResult<()> {
        let mut reputation_update: Vec<(TeamId, f32)> = vec![];
        for (_, team) in self.teams.iter() {
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
                    .ok_or(format!("Team {:?} not found in world", teams[0].id))?;

            let away_team_in_game =
                TeamInGame::from_team_id(teams[1].id, &self.teams, &self.players)
                    .ok_or(format!("Team {:?} not found in world", teams[1].id))?;

            let starting_at = Tick::now() + BASE_GAME_START_DELAY * rng.gen_range(1..=6);

            return self.generate_game(
                GameId::new(),
                home_team_in_game,
                away_team_in_game,
                starting_at,
            );
        }

        Ok(())
    }

    pub fn filter_peer_data(&mut self, peer_id: Option<PeerId>) {
        if peer_id.is_none() {
            // Filter all data that has a peer_id (i.e. keep only local data)
            self.teams.retain(|_, team| team.peer_id.is_none());
            self.players.retain(|_, player| player.peer_id.is_none());
            self.planets.retain(|_, planet| planet.peer_id.is_none());
        } else {
            // Filter all data that has a specific peer_id
            let peer_id = peer_id.unwrap();
            self.teams
                .retain(|_, team| team.peer_id.is_none() || team.peer_id.unwrap() != peer_id);
            self.players
                .retain(|_, player| player.peer_id.is_none() || player.peer_id.unwrap() != peer_id);
            self.planets
                .retain(|_, planet| planet.peer_id.is_none() || planet.peer_id.unwrap() != peer_id);
        }
        // Remove teams from planet teams vector.
        for (_, planet) in self.planets.iter_mut() {
            planet
                .team_ids
                .retain(|&team_id| self.teams.contains_key(&team_id));
        }
        // Remove games not involving own team
        // self.games.retain(|_, game| {
        //     game.home_team_in_game.team_id == self.own_team_id
        //         || game.away_team_in_game.team_id == self.own_team_id
        // });

        // Set current game to None for teams not in a game.
        for (_, team) in self.teams.iter_mut() {
            if team.current_game.is_some() && !self.games.contains_key(&team.current_game.unwrap())
            {
                team.current_game = None;
            }
        }
        self.dirty_ui = true;
    }

    pub fn travel_time_to_planet(&self, team_id: TeamId, to: PlanetId) -> AppResult<Tick> {
        let team = self.get_team_or_err(team_id)?;
        let from = match team.current_location {
            TeamLocation::OnPlanet { planet_id } => planet_id,
            TeamLocation::Travelling { .. } => {
                return Err(format!("Team {} is travelling", team.name).into())
            }
            TeamLocation::Exploring { .. } => {
                return Err(format!("Team {} is exploring", team.name).into())
            }
        };

        let distance = self.distance_between_planets(from, to)?;
        let bonus = TeamBonus::SpaceshipSpeed.current_team_bonus(&self, team.id)?;
        Ok(
            ((LANDING_TIME_OVERHEAD as f32 + (distance as f32 / team.spaceship.speed())) / bonus)
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

    pub fn to_store(&self) -> World {
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
        w.filter_peer_data(None);
        w
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
            types::TeamLocation,
            utils::PLANET_DATA,
            world::{ASTEROID_DISCOVERY_PROBABILITY, AU, HOURS, LONG_EXPLORATION_TIME},
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
            println!(
                "Earth to {:20} = {} Km = {:.4} AU",
                planet.name,
                distance,
                distance as f32 / AU as f32
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
        let team_id =
            world.generate_random_team(rng, planet.id, "test".into(), "testship".into())?;

        let team = world.get_team_or_err(team_id)?;
        println!("\nQUICK EXPLORATION");

        let duration = QUICK_EXPLORATION_TIME;
        let duration_bonus = duration as f64 / (1.0 * HOURS as f64);

        let found_resources = world.resources_found_after_exploration(&team, &planet, duration)?;

        println!("Found resources:");
        for res in found_resources.iter() {
            println!("  {} {}", res.1, res.0);
        }

        let found_free_agents = world.free_agents_found_after_exploration(&team, &planet, duration);
        let fa_duration_bonus = (duration as f32 / (1 * HOURS) as f32).powf(1.3);
        let population_bonus = planet.total_population() as f32 / 8.0;
        println!(
            "Found pirates (min {}):",
            (-35 + (population_bonus + fa_duration_bonus) as i32)
        );

        for player in found_free_agents.iter() {
            println!(
                "  {:<16} {}\n",
                format!(
                    "{}.{}",
                    player.info.first_name.chars().next().unwrap_or_default(),
                    player.info.last_name
                ),
                player.stars()
            );
        }

        if planet.planet_type != PlanetType::Asteroid
            && rng.gen_bool(
                (ASTEROID_DISCOVERY_PROBABILITY
                    * planet.asteroid_probability as f64
                    * duration_bonus)
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

        let found_free_agents = world.free_agents_found_after_exploration(&team, &planet, duration);
        let fa_duration_bonus = (duration as f32 / (1 * HOURS) as f32).powf(1.3);
        let population_bonus = planet.total_population() as f32 / 8.0;
        println!(
            "Found pirates (min {}):",
            (-35 + (population_bonus + fa_duration_bonus) as i32)
        );
        for player in found_free_agents.iter() {
            println!(
                "  {:<16} {}\n",
                format!(
                    "{}.{}",
                    player.info.first_name.chars().next().unwrap_or_default(),
                    player.info.last_name
                ),
                player.stars()
            );
        }

        if planet.planet_type != PlanetType::Asteroid
            && rng.gen_bool(
                (ASTEROID_DISCOVERY_PROBABILITY
                    * planet.asteroid_probability as f64
                    * duration_bonus)
                    .min(1.0),
            )
        {
            println!("Found asteroid!!!");
        }

        Ok(())
    }

    #[test]
    fn test_spugna_portal() -> AppResult<()> {
        let app = &mut App::new(None, true, true, false, false, None);
        app.new_world();

        let mut world = app.world.clone();
        let rng = &mut ChaCha8Rng::from_entropy();
        let planet = PLANET_DATA[0].clone();
        let team_id =
            world.generate_random_team(rng, planet.id, "test".into(), "testship".into())?;

        let mut team = world.get_team_or_err(team_id)?.clone();
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
        world.teams.insert(team.id, team);
        app.world = world;

        UiCallbackPreset::Drink {
            player_id: spugna_id,
        }
        .call(app)?;

        let team = app.world.get_team_or_err(team_id)?.clone();
        println!("Team resources {:?}", team.resources);
        println!("Team location {:?}", team.current_location);

        Ok(())
    }
}
