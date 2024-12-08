use super::types::NetworkRequestState;
use crate::game_engine::types::TeamInGame;
use crate::types::Tick;
use libp2p::PeerId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Challenge {
    pub state: NetworkRequestState,
    pub proposer_peer_id: PeerId,
    pub target_peer_id: PeerId,
    pub home_team_in_game: TeamInGame,
    pub away_team_in_game: TeamInGame,
    pub starting_at: Option<Tick>,
}

impl Challenge {
    pub fn new(
        proposer_peer_id: PeerId,
        target_peer_id: PeerId,
        mut home_team_in_game: TeamInGame,
        mut away_team_in_game: TeamInGame,
    ) -> Self {
        home_team_in_game.peer_id = Some(proposer_peer_id);
        away_team_in_game.peer_id = Some(target_peer_id);
        Self {
            state: NetworkRequestState::Syn,
            proposer_peer_id,
            target_peer_id,
            home_team_in_game,
            away_team_in_game,
            starting_at: None,
        }
    }

    pub fn format(&self) -> String {
        format!(
            "Challenge: {} {} {} - {} vs {} ",
            self.state,
            self.proposer_peer_id,
            self.target_peer_id,
            self.home_team_in_game.name,
            self.away_team_in_game.name,
        )
    }
}
