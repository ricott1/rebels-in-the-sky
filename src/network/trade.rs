use super::types::NetworkRequestState;
use crate::types::PlayerId;
use libp2p::PeerId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Trade {
    pub state: NetworkRequestState,
    pub proposer_peer_id: PeerId,
    pub target_peer_id: PeerId,
    pub proposer_player_ids: Vec<PlayerId>,
    pub target_player_ids: Vec<PlayerId>,
    pub extra_cash: i64,
    pub error_message: Option<String>,
}

impl Trade {
    pub fn new(proposer_peer_id: PeerId, target_peer_id: PeerId) -> Self {
        Self {
            state: NetworkRequestState::Syn,
            proposer_peer_id,
            target_peer_id,
            proposer_player_ids: vec![],
            target_player_ids: vec![],
            extra_cash: 0,
            error_message: None,
        }
    }

    pub fn format(&self) -> String {
        format!(
            "Trade: {} - {} {:#?} <--> {} {:#?} {:+}",
            self.state,
            self.proposer_peer_id,
            self.proposer_player_ids,
            self.target_peer_id,
            self.target_player_ids,
            self.extra_cash
        )
    }
}
