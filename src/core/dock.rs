use crate::network::trade::satoshi_amount;
use crate::types::{PlayerId, TeamId, Tick, TradeId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum OfferKind {
    Direct,
    Dock,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq)]
pub struct DockListing {
    pub player_id: PlayerId,
    pub listed_on: Tick,
}

impl DockListing {
    pub const fn new(player_id: PlayerId, listed_on: Tick) -> Self {
        Self {
            player_id,
            listed_on,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Offer {
    pub trade_id: TradeId,
    pub kind: OfferKind,
    pub target_team_id: TeamId,
    pub target_player_id: PlayerId,
    /// Positive: held from us. Negative: asked from the target.
    pub satoshis: i64,
    pub pirate: Option<PlayerId>,
    pub placed_on: Tick,
}

impl Offer {
    pub fn held_satoshis(&self) -> u32 {
        satoshi_amount(self.satoshis)
    }

    pub const fn is_dock_offer(&self) -> bool {
        matches!(self.kind, OfferKind::Dock)
    }
}
