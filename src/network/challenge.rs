use super::types::NetworkRequestState;
use crate::types::Tick;
use crate::{
    engine::types::TeamInGame,
    types::{AppResult, GameId},
    world::world::World,
};
use anyhow::anyhow;
use libp2p::PeerId;
use serde::{Deserialize, Serialize};

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
            return Err(anyhow!("Cannot generate game, starting_at not set"));
        }
        world.generate_game(
            self.game_id.unwrap(),
            self.home_team
                .as_ref()
                .ok_or(anyhow!(
                    "Cannot generate game, home team not found in challenge".to_string()
                ))?
                .clone(),
            self.away_team
                .as_ref()
                .ok_or(anyhow!(
                    "Cannot generate game, away team not found in challenge".to_string()
                ))?
                .clone(),
            self.starting_at.unwrap(),
        )?;
        Ok(self.game_id.unwrap())
    }
}
