use crate::types::PlanetId;
use serde::{Deserialize, Serialize};
use strum::Display;

#[derive(Debug, Display, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
pub enum SpaceCoveState {
    #[default]
    None,
    Pending {
        planet_id: PlanetId,
    },
    Ready {
        planet_id: PlanetId,
    },
}

#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq)]
pub struct SpaceCove {
    pub planet_id: PlanetId,
}
