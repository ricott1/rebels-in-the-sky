use super::challenge::Challenge;
use super::trade::Trade;
use crate::core::planet::Planet;
use crate::core::position::{GamePosition, MAX_GAME_POSITION};
use crate::core::skill::Skill;
use crate::core::TournamentRegistrationState;
use crate::game_engine::timer::Timer;
use crate::game_engine::types::GameStats;
use crate::game_engine::{Tournament, TournamentId};
use crate::types::{PlanetId, PlayerId, Tick};
use crate::{
    core::{player::Player, team::Team, world::World},
    game_engine::types::TeamInGame,
    types::{AppResult, GameId, TeamId},
};
use anyhow::anyhow;
use itertools::Itertools;
use libp2p::PeerId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;
use strum_macros::Display;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum NetworkData {
    Team {
        timestamp: Tick,
        team: NetworkTeam,
    },
    Challenge {
        timestamp: Tick,
        challenge: Challenge,
    },
    Trade {
        timestamp: Tick,
        trade: Trade,
    },
    Message {
        timestamp: Tick,
        message: String,
    },
    Game {
        timestamp: Tick,
        game: NetworkGame,
    },
    SeedInfo {
        timestamp: Tick,
        seed_info: SeedInfo,
    },
    RelayerMessageToTeam {
        timestamp: Tick,
        message: String,
        team_id: TeamId,
    },
    TournamentRegistrationRequest {
        timestamp: Tick,
        tournament_id: TournamentId,
        team_id: TeamId,
        state: TournamentRegistrationState,
    },
    Tournament {
        timestamp: Tick,
        tournament: Tournament,
    },
}

#[derive(Debug, Clone, Display, Default, Serialize, Deserialize, PartialEq, Hash)]
pub enum NetworkRequestState {
    #[default]
    Syn,
    SynAck,
    Ack,
    Failed {
        error_message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NetworkTeam {
    pub team: Team,
    pub players: Vec<Player>,
    pub asteroids: Vec<Planet>,
}

impl NetworkTeam {
    pub fn new(team: Team, players: Vec<Player>, asteroids: Vec<Planet>) -> Self {
        Self {
            team,
            players,
            asteroids,
        }
    }

    pub fn from_team_id(world: &World, team_id: &TeamId, peer_id: PeerId) -> AppResult<Self> {
        let mut team = world.get_team_or_err(team_id)?.clone();
        let mut players = world.get_players_by_team(&team)?;
        let asteroids = team
            .asteroid_ids
            .iter()
            .map(|asteroid_id| {
                let mut asteroid = world
                    .get_planet_or_err(asteroid_id)
                    .expect("Asteroid should be part of world")
                    .clone();
                asteroid.peer_id = Some(peer_id);
                asteroid
            })
            .collect_vec();

        // Set the peer_id for team we are sending out
        // This means that the team can be challenged online and it will not be stored.
        team.peer_id = Some(peer_id);
        for player in players.iter_mut() {
            player.peer_id = Some(peer_id);
        }

        Ok(Self::new(team, players, asteroids))
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
    pub part_of_tournament: Option<TournamentId>,
}

impl NetworkGame {
    pub fn from_game_id(world: &World, game_id: &GameId) -> AppResult<Self> {
        let game = world.get_game_or_err(game_id)?.clone();

        let mut home_team_in_game = game.home_team_in_game.clone();
        // Reset stats
        let mut stats = HashMap::new();
        for (idx, player_id) in home_team_in_game.initial_positions.iter().enumerate() {
            // Set position in stats to initial one
            let mut player_stats = GameStats::default();
            if (idx as GamePosition) < MAX_GAME_POSITION {
                player_stats.position = Some(idx as GamePosition);
            }
            stats.insert(*player_id, player_stats.clone());

            let player = home_team_in_game.players.get_mut(player_id).ok_or(anyhow!(
                "Cannot get player for home team in game".to_string()
            ))?;
            // Reset tiredness to initial one
            let tiredness = home_team_in_game.initial_tiredness[idx];
            player.tiredness = tiredness;
            // Reset morale to initial one
            let morale = home_team_in_game.initial_morale[idx];
            player.morale = morale;
        }
        home_team_in_game.stats = stats;

        let mut away_team_in_game = game.away_team_in_game.clone();
        let mut stats = HashMap::new();
        for (idx, player_id) in away_team_in_game.initial_positions.iter().enumerate() {
            let mut player_stats = GameStats::default();
            if (idx as GamePosition) < MAX_GAME_POSITION {
                player_stats.position = Some(idx as GamePosition);
            }
            stats.insert(*player_id, player_stats.clone());

            let player = away_team_in_game.players.get_mut(player_id).ok_or(anyhow!(
                "Cannot get player for away team in game".to_string()
            ))?;
            // Reset tiredness to initial one
            let tiredness = away_team_in_game.initial_tiredness[idx];
            player.tiredness = tiredness;
            // Reset morale to initial one
            let morale = away_team_in_game.initial_morale[idx];
            player.morale = morale;
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
            part_of_tournament: game.part_of_tournament,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TeamRanking {
    pub team: Team,
    pub timestamp: Tick,
    pub player_ratings: Vec<Skill>,
}

impl TeamRanking {
    pub fn from_network_team(timestamp: Tick, network_team: &NetworkTeam) -> Self {
        Self {
            team: network_team.team.clone(),
            timestamp,
            player_ratings: network_team
                .players
                .iter()
                .map(|p| p.average_skill())
                .collect_vec(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlayerRanking {
    pub player: Player,
    pub team_name: String,
    pub timestamp: Tick,
}

impl PlayerRanking {
    pub fn new(timestamp: Tick, player: Player, team_name: String) -> Self {
        Self {
            player,
            team_name,
            timestamp,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SeedInfo {
    pub connected_peers_count: usize,
    pub version: [usize; 3],
    pub message: Option<String>,
    pub team_ranking: Vec<(TeamId, TeamRanking)>,
    pub player_ranking: Vec<(PlayerId, PlayerRanking)>,
}

impl SeedInfo {
    pub fn new(
        connected_peers_count: usize,
        message: Option<String>,
        team_ranking: Vec<(TeamId, TeamRanking)>,
        player_ranking: Vec<(PlayerId, PlayerRanking)>,
    ) -> AppResult<Self> {
        Ok(Self {
            connected_peers_count,
            version: [
                env!("CARGO_PKG_VERSION_MAJOR").parse()?,
                env!("CARGO_PKG_VERSION_MINOR").parse()?,
                env!("CARGO_PKG_VERSION_PATCH").parse()?,
            ],
            message,
            team_ranking,
            player_ranking,
        })
    }
}
