use super::resources::Resource;
use super::utils::is_default;
use crate::core::{Upgrade, UpgradeableElement, WEEKS};
use crate::types::{PlanetId, Tick};
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use std::collections::HashSet;
use std::fmt::{self, Display};
use strum::Display;
use strum_macros::EnumIter;

#[derive(Debug, Display, Clone, Copy, Serialize_repr, Deserialize_repr, PartialEq)]
#[repr(u8)]
pub enum SpaceCoveState {
    UnderConstruction,
    Ready,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct SpaceCove {
    pub state: SpaceCoveState,
    pub planet_id: PlanetId,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub pending_upgrade: Option<Upgrade<SpaceCoveUpgradeTarget>>,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub upgrades: HashSet<SpaceCoveUpgradeTarget>,
}

impl SpaceCove {
    pub fn under_construction(planet_id: PlanetId) -> Self {
        Self {
            state: SpaceCoveState::UnderConstruction,
            planet_id,
            pending_upgrade: None,
            upgrades: HashSet::default(),
        }
    }

    pub fn ready(planet_id: PlanetId) -> Self {
        Self {
            state: SpaceCoveState::Ready,
            planet_id,
            pending_upgrade: None,
            upgrades: HashSet::default(),
        }
    }

    pub fn finish_contruction(&mut self) {
        self.state = SpaceCoveState::Ready;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum SpaceCoveUpgradeTarget {
    QuantumComputer,
    Skull,
}

impl Display for SpaceCoveUpgradeTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::QuantumComputer => write!(f, "Quantum computer"),
            Self::Skull => write!(f, "Skull"),
        }
    }
}

impl UpgradeableElement for SpaceCoveUpgradeTarget {
    fn next(&self) -> Option<Self> {
        match self {
            Self::QuantumComputer => None,
            Self::Skull => None,
        }
    }

    fn previous(&self) -> Option<Self> {
        match self {
            Self::QuantumComputer => None,
            Self::Skull => None,
        }
    }

    fn can_be_upgraded(&self) -> bool {
        true
    }

    fn upgrade_cost(&self) -> Vec<(Resource, u32)> {
        match self {
            Self::QuantumComputer => {
                vec![
                    (Resource::SATOSHI, 480_000),
                    (Resource::SCRAPS, 90),
                    (Resource::GOLD, 500),
                ]
            }
            Self::Skull => {
                vec![
                    (Resource::SATOSHI, 80_000),
                    (Resource::SCRAPS, 190),
                    (Resource::GOLD, 25),
                ]
            }
        }
    }

    fn upgrade_duration(&self) -> Tick {
        match self {
            Self::QuantumComputer => 1 * WEEKS,
            Self::Skull => 1 * WEEKS,
        }
    }

    fn description(&self) -> &str {
        match self {
            Self::QuantumComputer => "Quantum computers unlock superpolynomial speedups and keep an army of questionable physicists gainfully employed.",
            Self::Skull => "This is only a fancy decoration, or is it...",
        }
    }
}
