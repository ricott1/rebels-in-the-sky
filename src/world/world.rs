use super::constants::*;
use super::jersey::{Jersey, JerseyStyle};
use super::planet::Planet;
use super::player::Player;
use super::position::Position;
use super::role::CrewRole;
use super::skill::{GameSkill, Rated};
use super::spaceship::Spaceship;
use super::team::Team;
use super::types::TeamLocation;
use super::utils::{PLANET_DATA, TEAM_DATA};
use crate::engine::constants::RECOVERING_TIREDNESS_PER_SHORT_TICK;
use crate::engine::game::{Game, GameSummary};
use crate::engine::types::TeamInGame;
use crate::image::color_map::ColorMap;
use crate::network::types::{NetworkGame, NetworkTeam};
use crate::store::{
    load_from_json, save_to_json, PERSISTED_GAMES_PREFIX, PERSISTED_WORLD_FILENAME,
};
use crate::types::*;
use crate::world::position::MAX_POSITION;
use crate::world::types::PlayerLocation;
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
        let data_planets: Vec<Planet> = PLANET_DATA
            .as_ref()
            .unwrap()
            .iter()
            .map(|p| p.clone())
            .collect();

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
        let data_planets = PLANET_DATA.as_ref().unwrap();
        for planet in data_planets.iter() {
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

    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        load_from_json(PERSISTED_WORLD_FILENAME)
    }

    pub fn generate_local_world(&mut self, rng: &mut ChaCha8Rng) -> AppResult<()> {
        let t_data = TEAM_DATA.as_ref().unwrap();
        for (team_name, ship_name) in t_data.names.iter() {
            let home_planet = self.planets.values().choose(rng).unwrap();
            if home_planet.total_population() < 10 {
                continue;
            }
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
        let home_planet = team.home_planet.clone();
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
        while team.player_ids.len() < team.spaceship.capacity() as usize {
            self.generate_random_player(rng, Some(&mut team), None, home_planet, team_base_level);
        }

        let players = team
            .player_ids
            .iter()
            .map(|id| self.players.get(id).unwrap())
            .collect::<Vec<&Player>>();
        team.player_ids = Team::best_position_assignment(players.clone());

        let mut planet = self.get_planet_or_err(home_planet)?.clone();
        planet.teams.push(team_id);
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
        home_planet: PlanetId,
        jersey_style: JerseyStyle,
        jersey_colors: ColorMap,
        players: Vec<PlayerId>,
        balance: u32,
        spaceship: Spaceship,
    ) -> AppResult<TeamId> {
        let team_id = TeamId::new();
        let current_location = TeamLocation::OnPlanet {
            planet_id: home_planet,
        };
        let mut team = Team {
            id: team_id,
            name,
            jersey: Jersey {
                style: jersey_style,
                color: jersey_colors,
            },
            home_planet,
            current_location,
            spaceship,
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

        team.balance = balance;
        self.own_team_id = team.id;
        self.teams.insert(team.id, team.clone());
        self.auto_set_team_roles(&mut team)?;

        let mut planet = self.get_planet_or_err(home_planet)?.clone();
        planet.teams.push(team_id);
        self.planets.insert(planet.id, planet);

        self.dirty = true;
        self.dirty_network = true;
        self.dirty_ui = true;
        Ok(team_id)
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
            team.crew_roles.mozzo.push(current_role_player.id);
            current_role_player.set_jersey(&jersey);
            self.players
                .insert(current_role_player.id, current_role_player);
        }

        let previous_spaceship_speed_bonus = self.spaceship_speed_bonus(&team)?;

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
            } => {
                let new_start = Tick::now();
                let time_elapsed = new_start - started;

                let new_duration = (duration - time_elapsed) as f32
                    * previous_spaceship_speed_bonus
                    / self.spaceship_speed_bonus(&team)?;

                team.current_location = TeamLocation::Travelling {
                    from,
                    to,
                    started: new_start,
                    duration: new_duration as Tick,
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

        team.balance -= player.hire_cost(team.reputation);
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

        team.can_release_player(&player)?;

        team.remove_player(&mut player)?;
        team.balance += player.release_cost();
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

        // if team.crew_roles.captain == Some(player.id) {
        //     self.auto_set_team_captain(&mut team)?;
        // }

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
            TeamLocation::Travelling { .. } => {
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

            while game.timer < network_game.timer {
                game.tick();
            }

            self.games.insert(game.id, game);
            self.dirty_ui = true;
        }

        Ok(())
    }

    pub fn add_network_team(&mut self, network_team: NetworkTeam) -> AppResult<()> {
        let NetworkTeam { team, players } = network_team;
        if team.peer_id.is_none() {
            return Err("Cannot receive team without peer_id over the network.".into());
        }
        let db_team = self.get_team(team.id);
        if db_team.is_none() || db_team.unwrap().version < team.version {
            // Remove team from previous planet
            if db_team.is_some() {
                match db_team.unwrap().current_location {
                    TeamLocation::OnPlanet { planet_id } => {
                        let mut planet = self.get_planet_or_err(planet_id)?.clone();
                        planet.teams.retain(|&id| id != team.id);
                        self.planets.insert(planet.id, planet);
                    }
                    _ => {}
                }
            }

            // Add team to new planet
            match team.current_location {
                TeamLocation::OnPlanet { planet_id } => {
                    let mut planet = self.get_planet_or_err(planet_id)?.clone();
                    planet.teams.push(team.id);
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
                if db_player.is_none() || db_player.unwrap().version < player.version {
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

    pub fn simulate_until_now(&mut self) -> AppResult<Vec<String>> {
        if !self.has_own_team() {
            return Ok(vec![]);
        }

        let mut messages: Vec<String> = vec![];
        let now = Tick::now();
        let last_tick_long = self.last_tick_long_interval;

        while self.last_tick_short_interval + TickInterval::SHORT < now {
            let mut new_messages =
                self.handle_tick_events(self.last_tick_short_interval + TickInterval::SHORT, true)?;
            messages.append(&mut new_messages);
        }

        // Workaround to ensure we only generate FAs at most once.
        if now - last_tick_long >= TickInterval::LONG {
            messages.push(self.tick_free_agents()?);
        }

        Ok(messages)
    }

    pub fn handle_tick_events(
        &mut self,
        current_timestamp: Tick,
        is_simulating: bool,
    ) -> AppResult<Vec<String>> {
        let mut messages = vec![];

        if current_timestamp >= self.last_tick_short_interval + TickInterval::SHORT {
            if self.games.len() > 0 {
                self.tick_games(current_timestamp)?;
                self.cleanup_games()?;
            }

            if !is_simulating && self.games.len() < AUTO_GENERATE_GAMES_NUMBER {
                self.generate_random_game()?;
            }

            self.tick_travel(current_timestamp)?;
            self.last_tick_short_interval += TickInterval::SHORT;
            // Round up to the TickInterval::SHORT
            self.last_tick_short_interval -= self.last_tick_short_interval % TickInterval::SHORT;
        }

        if current_timestamp >= self.last_tick_medium_interval + TickInterval::MEDIUM {
            self.tick_tiredness_recovery()?;

            // Once every MEDIUM interval, set dirty_network flag,
            // so that we send our team to the network.
            if !is_simulating {
                self.dirty_network = true;
            }

            self.last_tick_medium_interval += TickInterval::MEDIUM;
        }

        if current_timestamp >= self.last_tick_long_interval + TickInterval::LONG {
            if !is_simulating {
                messages.push(self.tick_free_agents()?);
                self.tick_skill_improvements_reset()?;
            }
            self.tick_player_aging();
            self.modify_players_reputation();
            self.modify_teams_reputation()?;
            self.last_tick_long_interval += TickInterval::LONG;
        }

        Ok(messages)
    }

    fn cleanup_games(&mut self) -> AppResult<()> {
        for (_, game) in self.games.iter() {
            if game.timer.has_ended() {
                for team in [&game.home_team_in_game, &game.away_team_in_game] {
                    //we do not apply end of game logic to peer teams
                    if team.peer_id.is_some() && team.team_id != self.own_team_id {
                        continue;
                    }
                    for player in team.players.values() {
                        let mut player = player.clone();
                        let stats = team
                            .stats
                            .get(&player.id)
                            .ok_or(format!("Player {:?} not found in team stats", player.id))?;
                        player.apply_end_of_game_logic(
                            &stats.experience_at_position,
                            stats.tiredness,
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
                let home_team_income = 100 + game.attendance * INCOME_PER_ATTENDEE_HOME;
                let away_team_income = 100 + game.attendance * INCOME_PER_ATTENDEE_AWAY;
                // Winner team gets reputation bonus
                let score = game.get_score();
                let home_team_reputation = if score.0 > score.1 {
                    0.5
                } else if score.0 < score.1 {
                    -0.25
                } else {
                    0.2
                };

                let away_team_reputation = if score.0 < score.1 {
                    0.5
                } else if score.0 > score.1 {
                    -0.25
                } else {
                    0.2
                };

                // Set playing teams current game to None
                if let Ok(res) = self.get_team_or_err(game.home_team_in_game.team_id) {
                    let mut home_team = res.clone();
                    home_team.current_game = None;
                    home_team.balance += home_team_income;
                    home_team.reputation = (home_team.reputation + home_team_reputation).bound();
                    self.teams.insert(home_team.id, home_team.clone());
                }

                if let Ok(res) = self.get_team_or_err(game.away_team_in_game.team_id) {
                    let mut away_team = res.clone();
                    away_team.current_game = None;
                    away_team.balance += away_team_income;
                    away_team.reputation = (away_team.reputation + away_team_reputation).bound();
                    self.teams.insert(away_team.id, away_team.clone());
                }

                self.dirty = true;
                self.dirty_ui = true;
            }
        }
        self.games.retain(|_, game| !game.timer.has_ended());
        Ok(())
    }

    fn tick_games(&mut self, current_timestamp: Tick) -> AppResult<()> {
        // NOTE!!: we do not set the world to dirty so we don't save on every tick.
        //         the idea is that the game is completely determined at the beginning,
        //         so we can similuate it through.
        for (_, game) in self.games.iter_mut() {
            if current_timestamp >= game.starting_at && !game.timer.has_ended() {
                game.tick();
            }
        }
        Ok(())
    }

    fn tick_travel(&mut self, current_timestamp: Tick) -> AppResult<()> {
        let own_team = self.get_own_team()?;

        match own_team.current_location {
            TeamLocation::Travelling {
                from: _,
                to,
                started,
                duration,
            } => {
                if current_timestamp >= started + duration {
                    let mut team = own_team.clone();
                    team.current_location = TeamLocation::OnPlanet { planet_id: to };
                    let mut planet = self.get_planet_or_err(to)?.clone();
                    planet.teams.push(team.id);

                    for player in team.player_ids.iter() {
                        let mut player = self.get_player_or_err(*player)?.clone();
                        player.set_jersey(&team.jersey);
                        self.players.insert(player.id, player);
                    }

                    self.teams.insert(team.id, team);
                    self.planets.insert(planet.id, planet);
                    self.dirty = true;
                    self.dirty_network = true;
                    self.dirty_ui = true;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn tick_tiredness_recovery(&mut self) -> AppResult<()> {
        let teams = self
            .teams
            .values()
            .filter(|team| team.current_game.is_none() && team.peer_id.is_none())
            .collect::<Vec<&Team>>();

        for team in teams {
            let bonus = self.tiredness_recovery_bonus(team)?;
            for player_id in team.player_ids.iter() {
                let db_player = self
                    .get_player(*player_id)
                    .ok_or(format!("Player {:?} not found", player_id))?;
                if db_player.tiredness > 0.0 {
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

    fn tick_free_agents(&mut self) -> AppResult<String> {
        self.players.retain(|_, player| player.team.is_some());

        let rng = &mut ChaCha8Rng::seed_from_u64(rand::random());
        let data_planets = PLANET_DATA.as_ref().unwrap();
        for planet in data_planets.iter() {
            self.populate_planet(rng, planet);
        }
        Ok("Free agents refreshed".to_string())
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

    pub fn spaceship_speed_bonus(&self, team: &Team) -> AppResult<f32> {
        let role_fitness = if let Some(pilot_id) = team.crew_roles.pilot {
            let pilot = self.get_player_or_err(pilot_id)?;
            pilot.athleticism.quickness
        } else {
            0.0
        };
        Ok(BASE_BONUS + BONUS_PER_SKILL * role_fitness)
    }

    pub fn team_reputation_bonus(&self, team: &Team) -> AppResult<f32> {
        let role_fitness = if let Some(captain_id) = team.crew_roles.captain {
            let captain = self.get_player_or_err(captain_id)?;
            captain.mental.charisma
        } else {
            0.0
        };
        Ok(BASE_BONUS + BONUS_PER_SKILL * role_fitness)
    }

    pub fn tiredness_recovery_bonus(&self, team: &Team) -> AppResult<f32> {
        let role_fitness = if let Some(doctor_id) = team.crew_roles.doctor {
            let doctor = self.get_player_or_err(doctor_id)?;
            doctor.athleticism.stamina
        } else {
            0.0
        };
        Ok(BASE_BONUS + BONUS_PER_SKILL * role_fitness)
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
            let players_reputation = team
                .player_ids
                .iter()
                .map(|id| self.players.get(id).unwrap().reputation)
                .sum::<f32>()
                / team.player_ids.len() as f32;

            let bonus = self.team_reputation_bonus(team)?;
            let new_reputation = (team.reputation + bonus * players_reputation / 100.0)
                .min(players_reputation)
                .bound();
            reputation_update.push((team.id, new_reputation));
        }

        for (team_id, new_reputation) in reputation_update {
            let mut team = self.get_team_or_err(team_id)?.clone();
            team.reputation = new_reputation;
            self.teams.insert(team.id, team);
        }
        Ok(())
    }

    fn generate_random_game(&mut self) -> AppResult<()> {
        let rng = &mut rand::thread_rng();
        let candidate_teams = self
            .teams
            .values()
            .into_iter()
            .filter(|team| {
                team.current_game.is_none() && team.id != self.own_team_id && team.peer_id.is_none()
            })
            .collect::<Vec<&Team>>();

        if candidate_teams.len() < 2 {
            return Ok(());
        }

        let teams = candidate_teams.iter().choose_multiple(rng, 2);

        if teams[0].current_location != teams[1].current_location {
            return Ok(());
        }

        let home_team_in_game = TeamInGame::from_team_id(teams[0].id, &self.teams, &self.players)
            .ok_or(format!("Team {:?} not found in world", teams[0].id))?;

        let away_team_in_game = TeamInGame::from_team_id(teams[1].id, &self.teams, &self.players)
            .ok_or(format!("Team {:?} not found in world", teams[1].id))?;

        let starting_at = Tick::now() + BASE_GAME_START_DELAY * rng.gen_range(1..=6);

        self.generate_game(
            GameId::new(),
            home_team_in_game,
            away_team_in_game,
            starting_at,
        )
    }

    pub fn filter_peer_data(&mut self, peer_id: Option<PeerId>) {
        if peer_id.is_none() {
            self.teams.retain(|_, team| team.peer_id.is_none());
            self.players.retain(|_, player| player.peer_id.is_none());
        } else {
            let peer_id = peer_id.unwrap();
            self.teams
                .retain(|_, team| team.peer_id.is_none() || team.peer_id.unwrap() != peer_id);
            self.players
                .retain(|_, player| player.peer_id.is_none() || player.peer_id.unwrap() != peer_id);
        }
        // Remove teams from planet teams vector.
        for (_, planet) in self.planets.iter_mut() {
            planet
                .teams
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
            _ => return Err(format!("Team {} is travelling", team.name).into()),
        };

        let distance = self.distance_between_planets(from, to)?;
        let bonus = self.spaceship_speed_bonus(team)?;
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

    fn distance_between_planets(&self, from_id: PlanetId, to_id: PlanetId) -> AppResult<u128> {
        // We calculate the distance. 5 cases:

        // 1: from and to are the same planet -> 0
        if from_id == to_id {
            return Ok(0);
        }
        let from = self.get_planet_or_err(from_id)?;
        let to = self.get_planet_or_err(to_id)?;

        let from_height: usize = self.planet_height(from_id)?;
        let to_height: usize = self.planet_height(to_id)?;

        // 2: from and to have the same parent -> difference in position in parent.satellites array
        if from.satellite_of == to.satellite_of {
            // we guarantee that these are some, since only one planet has no parent
            let parent = self.get_planet_or_err(from.satellite_of.unwrap())?;
            let from_index = parent
                .satellites
                .iter()
                .position(|&id| id == from.id)
                .unwrap()
                + 1;
            let to_index = parent
                .satellites
                .iter()
                .position(|&id| id == to.id)
                .unwrap()
                + 1;

            let distance = (from_index as i32 - to_index as i32).abs() as u128
                * BASE_DISTANCES[from_height - 1];

            return Ok(distance);
        }

        // 3: from is a satellite of to -> position in parent satellites array + 1
        if from.satellite_of == Some(to.id) {
            let from_index = to.satellites.iter().position(|&id| id == from.id).unwrap() + 1;
            let distance = from_index as u128 * BASE_DISTANCES[from_height - 1];

            return Ok(distance);
        }

        // 4: to is a satellite of from -> idx position
        if to.satellite_of == Some(from.id) {
            let to_index = from.satellites.iter().position(|&id| id == to.id).unwrap() + 1;
            let distance = to_index as u128 * BASE_DISTANCES[to_height - 1];

            return Ok(distance);
        }

        // 5: from and to are not related -> find degrees of separation and multiply by 10
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
        let parent = self.get_planet_or_err(parent_id)?;
        let bottom_index = parent
            .satellites
            .iter()
            .position(|&id| id == bottom.id)
            .unwrap()
            + 1;

        Ok(self.distance_between_planets(parent_id, top.id)?
            + bottom_index as u128 * BASE_DISTANCES[bottom_height - 1])
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
            ..Default::default()
        };
        w.filter_peer_data(None);
        w
    }
}

#[cfg(test)]
mod test {
    use super::World;
    use crate::world::constants::BASE_DISTANCES;
    use rand::{Rng, SeedableRng};
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
    fn test_distance_between_planets() {
        let world = World::new(None);
        let earth = world
            .planets
            .values()
            .find(|p| p.name == "Earth")
            .unwrap()
            .id;

        let moon = world
            .planets
            .values()
            .find(|p| p.name == "Moon")
            .unwrap()
            .id;

        let mars = world
            .planets
            .values()
            .find(|p| p.name == "Mars")
            .unwrap()
            .id;

        let uranus = world
            .planets
            .values()
            .find(|p| p.name == "Uranus")
            .unwrap()
            .id;

        let sagittarius = world
            .planets
            .values()
            .find(|p| p.name == "Sagittarius")
            .unwrap()
            .id;

        let proxima = world
            .planets
            .values()
            .find(|p| p.name == "Proxima Centauri")
            .unwrap()
            .id;

        assert_eq!(world.distance_between_planets(earth, earth).unwrap(), 0);

        assert_eq!(
            world.distance_between_planets(earth, moon).unwrap(),
            1 * BASE_DISTANCES[2]
        );

        assert_eq!(
            world.distance_between_planets(earth, moon).unwrap(),
            world.distance_between_planets(moon, earth).unwrap()
        );

        assert_eq!(
            world.distance_between_planets(earth, mars).unwrap(),
            1 * BASE_DISTANCES[1]
        );
        assert_eq!(
            world.distance_between_planets(mars, earth).unwrap(),
            world.distance_between_planets(earth, mars).unwrap(),
        );

        assert_eq!(
            world.distance_between_planets(moon, mars).unwrap(),
            1 * BASE_DISTANCES[1] + 1 * BASE_DISTANCES[2]
        );
        assert_eq!(
            world.distance_between_planets(mars, moon).unwrap(),
            world.distance_between_planets(moon, mars).unwrap(),
        );

        assert_eq!(
            world.distance_between_planets(moon, uranus).unwrap(),
            4 * BASE_DISTANCES[1] + 1 * BASE_DISTANCES[2]
        );

        assert_eq!(
            world.distance_between_planets(uranus, moon).unwrap(),
            world.distance_between_planets(moon, uranus).unwrap(),
        );

        assert_eq!(
            world.distance_between_planets(earth, sagittarius).unwrap(),
            1 * BASE_DISTANCES[0] + 1 * BASE_DISTANCES[1]
        );

        assert_eq!(
            world.distance_between_planets(earth, sagittarius).unwrap(),
            world.distance_between_planets(sagittarius, earth).unwrap(),
        );

        assert_eq!(
            world.distance_between_planets(uranus, proxima).unwrap(),
            1 * BASE_DISTANCES[0] + 5 * BASE_DISTANCES[1]
        );
        assert_eq!(
            world.distance_between_planets(uranus, proxima).unwrap(),
            world.distance_between_planets(proxima, uranus).unwrap(),
        );

        assert_eq!(
            world.distance_between_planets(moon, proxima).unwrap(),
            1 * BASE_DISTANCES[0] + 1 * BASE_DISTANCES[1] + 1 * BASE_DISTANCES[2]
        );
        assert_eq!(
            world.distance_between_planets(proxima, moon).unwrap(),
            world.distance_between_planets(moon, proxima).unwrap(),
        );
    }
}
