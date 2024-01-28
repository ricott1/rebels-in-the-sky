use crate::types::Tick;
use crate::{
    engine::types::TeamInGame,
    types::{AppResult, GameId, TeamId},
    world::{player::Player, team::Team, world::World},
};
use libp2p::PeerId;
use serde::{Deserialize, Serialize};
use strum_macros::Display;

#[derive(Debug, Clone, Display, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ChallengeState {
    #[default]
    Syn,
    SynAck,
    Ack,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Challenge {
    pub state: ChallengeState,
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
            state: ChallengeState::Syn,
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
}

impl NetworkTeam {
    pub fn new(team: Team, players: Vec<Player>) -> Self {
        Self { team, players }
    }

    pub fn from_team_id(world: &World, team_id: &TeamId) -> AppResult<Self> {
        let team = world.get_team_or_err(*team_id)?.clone();
        let players = world.get_players_by_team(&team)?;
        Ok(Self::new(team, players))
    }

    pub fn set_peer_id(&mut self, peer_id: PeerId) {
        self.team.peer_id = Some(peer_id);
        for player in self.players.iter_mut() {
            player.peer_id = Some(peer_id.clone());
        }
    }
}
