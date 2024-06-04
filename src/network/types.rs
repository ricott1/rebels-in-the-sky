use std::collections::HashMap;

use crate::engine::timer::Timer;
use crate::engine::types::GameStats;
use crate::types::{PlanetId, Tick};
use crate::world::planet::{Planet, PlanetType};
use crate::world::position::{Position, MAX_POSITION};
use crate::{
    engine::types::TeamInGame,
    types::{AppResult, GameId, TeamId},
    world::{player::Player, team::Team, world::World},
};
use libp2p::PeerId;
use serde::{Deserialize, Serialize};
use strum_macros::Display;

#[derive(Debug, Clone, Display, Default, Serialize, Deserialize, PartialEq, Hash)]
pub enum NetworkRequestState {
    #[default]
    Syn,
    SynAck,
    Ack,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Challenge {
    pub state: NetworkRequestState,
    pub home_peer_id: PeerId,
    pub away_peer_id: PeerId,
    pub home_team: Option<TeamInGame>,
    pub away_team: Option<TeamInGame>,
    pub game_id: Option<GameId>,
    pub starting_at: Option<Tick>,
    pub error_message: Option<String>,
}

impl Challenge {
    pub fn new(home_peer_id: PeerId, away_peer_id: PeerId) -> Self {
        Self {
            state: NetworkRequestState::Syn,
            home_peer_id,
            away_peer_id,
            home_team: None,
            away_team: None,
            game_id: None,
            starting_at: None,
            error_message: None,
        }
    }

    pub fn format(&self) -> String {
        format!(
            "Challenge: {} {} {} - {} vs {} ",
            self.state,
            self.home_peer_id,
            self.away_peer_id,
            self.home_team
                .as_ref()
                .map(|t| t.name.clone())
                .unwrap_or_else(|| "None".to_string()),
            self.away_team
                .as_ref()
                .map(|t| t.name.clone())
                .unwrap_or_else(|| "None".to_string()),
        )
    }

    pub fn generate_game(&self, world: &mut World) -> AppResult<GameId> {
        if self.starting_at.is_none() {
            return Err("Cannot generate game, starting_at not set".into());
        }
        world.generate_game(
            self.game_id.unwrap(),
            self.home_team
                .as_ref()
                .ok_or("Cannot generate game, home team not found in challenge".to_string())?
                .clone(),
            self.away_team
                .as_ref()
                .ok_or("Cannot generate game, away team not found in challenge".to_string())?
                .clone(),
            self.starting_at.unwrap(),
        )?;
        Ok(self.game_id.unwrap())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NetworkTeam {
    pub team: Team,
    pub players: Vec<Player>,
    pub home_planet: Option<Planet>,
}

impl NetworkTeam {
    pub fn new(team: Team, players: Vec<Player>, home_planet: Option<Planet>) -> Self {
        Self {
            team,
            players,
            home_planet,
        }
    }

    pub fn from_team_id(world: &World, team_id: &TeamId) -> AppResult<Self> {
        let team = world.get_team_or_err(*team_id)?.clone();
        let players = world.get_players_by_team(&team)?;
        let planet = world.get_planet_or_err(team.home_planet_id)?;
        let home_planet = if planet.planet_type == PlanetType::Asteroid {
            Some(planet)
        } else {
            None
        }
        .cloned();

        Ok(Self::new(team, players, home_planet))
    }

    pub fn set_peer_id(&mut self, peer_id: PeerId) {
        self.team.peer_id = Some(peer_id);
        for player in self.players.iter_mut() {
            player.peer_id = Some(peer_id.clone());
        }
        if self.home_planet.is_some() {
            self.home_planet.as_mut().unwrap().peer_id = Some(peer_id);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NetworkGame {
    pub id: GameId,
    pub home_team_in_game: TeamInGame,
    pub away_team_in_game: TeamInGame,
    pub location: PlanetId,
    pub attendance: u32,
    pub starting_at: Tick,
    pub timer: Timer,
}

impl NetworkGame {
    pub fn from_game_id(world: &World, game_id: GameId) -> AppResult<Self> {
        let game = world.get_game_or_err(game_id)?.clone();

        let mut home_team_in_game = game.home_team_in_game.clone();
        // Reset stats
        let mut stats = HashMap::new();
        for (idx, player_id) in home_team_in_game.initial_positions.iter().enumerate() {
            // Set position in stats to initial one
            let mut player_stats = GameStats::default();
            if (idx as Position) < MAX_POSITION {
                player_stats.position = Some(idx as Position);
            }
            stats.insert(player_id.clone(), player_stats.clone());

            // Reset tiredness to initial one
            let tiredness = home_team_in_game.initial_tiredness[idx];
            let player = home_team_in_game
                .players
                .get_mut(player_id)
                .ok_or("Cannot get player for home team in game".to_string())?;
            player.tiredness = tiredness;
        }
        home_team_in_game.stats = stats;

        let mut away_team_in_game = game.away_team_in_game.clone();
        let mut stats = HashMap::new();
        for (idx, player_id) in away_team_in_game.initial_positions.iter().enumerate() {
            let mut player_stats = GameStats::default();
            if (idx as Position) < MAX_POSITION {
                player_stats.position = Some(idx as Position);
            }
            stats.insert(player_id.clone(), player_stats.clone());

            let tiredness = away_team_in_game.initial_tiredness[idx];
            let player = away_team_in_game
                .players
                .get_mut(player_id)
                .ok_or("Cannot get player for away team in game".to_string())?;
            player.tiredness = tiredness;
        }
        away_team_in_game.stats = stats;

        Ok(Self {
            id: game.id,
            home_team_in_game,
            away_team_in_game,
            location: game.location,
            attendance: game.attendance,
            starting_at: game.starting_at,
            timer: game.timer,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SeedInfo {
    pub connected_peers_count: usize,
    pub version_major: usize,
    pub version_minor: usize,
    pub version_patch: usize,
    pub message: Option<String>,
}

impl SeedInfo {
    pub fn new(connected_peers_count: usize, message: Option<String>) -> AppResult<Self> {
        Ok(Self {
            connected_peers_count,
            version_major: env!("CARGO_PKG_VERSION_MAJOR").parse()?,
            version_minor: env!("CARGO_PKG_VERSION_MINOR").parse()?,
            version_patch: env!("CARGO_PKG_VERSION_PATCH").parse()?,
            message,
        })
    }
}
