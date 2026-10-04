use crate::types::{PlayerId, Tick};
use serde::{Deserialize, Serialize};

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
