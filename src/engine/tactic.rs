use std::fmt::Display;

use super::action::Action;
use rand::seq::IteratorRandom;
use rand_chacha::ChaCha8Rng;
use rand_distr::{Distribution, WeightedIndex};
use serde_repr::{Deserialize_repr, Serialize_repr};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

#[derive(Debug, Clone, Copy, Default, Serialize_repr, Deserialize_repr, PartialEq, EnumIter)]
#[repr(u8)]
pub enum DefenseTactic {
    #[default]
    PirateToPirate,
    Zone,
}

impl Display for DefenseTactic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DefenseTactic::PirateToPirate => write!(f, "Pirate-to-pirate"),
            DefenseTactic::Zone => write!(f, "Zone"),
        }
    }
}

impl DefenseTactic {
    pub fn random() -> Self {
        let mut rng = rand::thread_rng();
        Self::iter().choose(&mut rng).unwrap()
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize_repr, Deserialize_repr, PartialEq, EnumIter)]
#[repr(u8)]
pub enum OffenseTactic {
    #[default]
    Balanced,
    BigPirates,
    SmallBall,
}

impl Display for OffenseTactic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OffenseTactic::Balanced => write!(f, "Balanced"),
            OffenseTactic::BigPirates => write!(f, "Big Pirates"),
            OffenseTactic::SmallBall => write!(f, "Small Ball"),
        }
    }
}

impl OffenseTactic {
    pub fn random() -> Self {
        let mut rng = rand::thread_rng();
        Self::iter().choose(&mut rng).unwrap()
    }

    pub fn next(&self) -> Self {
        match self {
            Self::Balanced => Self::BigPirates,
            Self::BigPirates => Self::SmallBall,
            Self::SmallBall => Self::Balanced,
        }
    }

    pub fn pick_action(&self, rng: &mut ChaCha8Rng) -> Option<Action> {
        let weights = match self {
            Self::Balanced => [2, 2, 3, 2],
            Self::BigPirates => [2, 1, 1, 3],
            Self::SmallBall => [2, 3, 3, 1],
        };
        let idx = WeightedIndex::new(&weights).ok()?.sample(rng);
        match idx {
            0 => Some(Action::Isolation),
            1 => Some(Action::OffTheScreen),
            2 => Some(Action::PickAndRoll),
            3 => Some(Action::Post),
            _ => None,
        }
    }
}
