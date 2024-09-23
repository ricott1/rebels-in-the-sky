use super::challenge::Challenge;
use super::trade::Trade;
use crate::engine::timer::Timer;
use crate::engine::types::GameStats;
use crate::types::{KartoffelId, PlanetId, Tick};
use crate::world::planet::{Planet, PlanetType};
use crate::world::position::{Position, MAX_POSITION};
use crate::world::skill::Skill;
use crate::{
    engine::types::TeamInGame,
    types::{AppResult, GameId, TeamId},
    world::{player::Player, team::Team, world::World},
};
use anyhow::anyhow;
use itertools::Itertools;
use libp2p::PeerId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;
use strum_macros::Display;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[repr(u8)]
pub(crate) enum NetworkData {
    Team(Tick, NetworkTeam),
    Challenge(Tick, Challenge),
    Trade(Tick, Trade),
    Message(Tick, String),
    Game(Tick, NetworkGame),
    SeedInfo(Tick, SeedInfo),
}

impl TryFrom<Vec<u8>> for NetworkData {
    type Error = anyhow::Error;
    fn try_from(item: Vec<u8>) -> AppResult<Self> {
        let network_data = serde_json::from_slice::<NetworkData>(item.as_slice())?;
        Ok(network_data)
    }
}

impl TryInto<Vec<u8>> for NetworkData {
    type Error = anyhow::Error;
    fn try_into(self) -> AppResult<Vec<u8>> {
        let data = serde_json::to_vec(&self)?;
        Ok(data)
    }
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
            if (idx as Position) < MAX_POSITION {
                player_stats.position = Some(idx as Position);
            }
            stats.insert(player_id.clone(), player_stats.clone());

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
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TeamRanking {
    pub timestamp: Tick,
    pub name: String,
    pub reputation: Skill,
    pub player_ratings: Vec<Skill>,
    pub record: [u32; 3],
    pub kartoffel_ids: Vec<KartoffelId>,
}

impl TeamRanking {
    pub fn from_network_team(timestamp: Tick, network_team: &NetworkTeam) -> Self {
        Self {
            timestamp,
            name: network_team.team.name.clone(),
            reputation: network_team.team.reputation,
            player_ratings: network_team
                .players
                .iter()
                .map(|p| p.average_skill())
                .collect_vec(),
            record: network_team.team.network_game_record,
            kartoffel_ids: network_team.team.kartoffel_ids.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SeedInfo {
    pub connected_peers_count: usize,
    pub version_major: usize,
    pub version_minor: usize,
    pub version_patch: usize,
    pub message: Option<String>,
    pub team_ranking: HashMap<TeamId, TeamRanking>,
}

impl SeedInfo {
    pub fn new(
        connected_peers_count: usize,
        message: Option<String>,
        team_ranking: HashMap<TeamId, TeamRanking>,
    ) -> AppResult<Self> {
        Ok(Self {
            connected_peers_count,
            version_major: env!("CARGO_PKG_VERSION_MAJOR").parse()?,
            version_minor: env!("CARGO_PKG_VERSION_MINOR").parse()?,
            version_patch: env!("CARGO_PKG_VERSION_PATCH").parse()?,
            message,
            team_ranking,
        })
    }
}
