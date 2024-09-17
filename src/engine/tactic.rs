use std::fmt::Display;

use crate::types::AppResult;

use super::action::Action;
use anyhow::anyhow;
use rand::seq::IteratorRandom;
use rand_chacha::ChaCha8Rng;
use rand_distr::{Distribution, WeightedIndex};
use serde_repr::{Deserialize_repr, Serialize_repr};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

#[derive(Debug, Clone, Copy, Default, Serialize_repr, Deserialize_repr, PartialEq, EnumIter)]
#[repr(u8)]
pub enum Tactic {
    #[default]
    Balanced,
    BigPirates,
    Arrembaggio,
}

impl Display for Tactic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Tactic::Balanced => write!(f, "Balanced"),
            Tactic::BigPirates => write!(f, "Big Pirates"),
            Tactic::Arrembaggio => write!(f, "Arrembaggio"),
        }
    }
}

impl Tactic {
    pub fn random() -> Self {
        let mut rng = rand::thread_rng();
        Self::iter().choose(&mut rng).unwrap()
    }

    pub fn next(&self) -> Self {
        match self {
            Self::Balanced => Self::BigPirates,
            Self::BigPirates => Self::Arrembaggio,
            Self::Arrembaggio => Self::Balanced,
        }
    }

    pub fn description(&self) -> &str {
        match self {
            Self::Balanced => "A balanced tactic, trying to alternate several possible actions.",
            Self::BigPirates => "Focus on big pirates posting, slightly higher chance of brawls.",
            Self::Arrembaggio => {
                "Aggressive tactic focusing on sharing the ball, very high chance of brawl."
            }
        }
    }

    pub fn pick_action(&self, rng: &mut ChaCha8Rng) -> AppResult<Action> {
        let weights = match self {
            Self::Balanced => [2, 2, 2, 2],
            Self::BigPirates => [1, 1, 1, 3],
            Self::Arrembaggio => [2, 4, 4, 1],
        };
        let action = match WeightedIndex::new(&weights)?.sample(rng) {
            0 => Action::Isolation,
            1 => Action::OffTheScreen,
            2 => Action::PickAndRoll,
            3 => Action::Post,
            _ => return Err(anyhow!("Invalid index in pick_action.")),
        };
        Ok(action)
    }

    pub fn brawl_probability_modifier(&self) -> f32 {
        match self {
            Self::Balanced => 1.0,
            Self::BigPirates => 1.25,
            Self::Arrembaggio => 2.0,
        }
    }

    pub fn drink_probability_modifier(&self) -> f32 {
        match self {
            Self::Balanced => 1.0,
            Self::BigPirates => 0.25,
            Self::Arrembaggio => 2.0,
        }
    }
}
