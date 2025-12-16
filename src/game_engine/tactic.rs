use super::action::Action;
use crate::types::AppResult;
use rand::seq::IteratorRandom;
use rand_chacha::ChaCha8Rng;
use rand_distr::{weighted::WeightedIndex, Distribution};
use serde_repr::{Deserialize_repr, Serialize_repr};
use std::fmt::Display;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

#[derive(Debug, Clone, Copy, Default, Serialize_repr, Deserialize_repr, PartialEq, EnumIter)]
#[repr(u8)]
pub enum Tactic {
    #[default]
    Balanced,
    BigPirates,
    Arrembaggio,
    Shooters,
}

impl Display for Tactic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Balanced => write!(f, "Balanced"),
            Self::BigPirates => write!(f, "Big Pirates"),
            Self::Arrembaggio => write!(f, "Arrembaggio"),
            Self::Shooters => write!(f, "Shooters"),
        }
    }
}

impl Tactic {
    pub fn random(rng: &mut ChaCha8Rng) -> Self {
        Self::iter()
            .choose_stable(rng)
            .expect("There should be at least a tactic")
    }

    pub fn next(&self) -> Self {
        match self {
            Self::Balanced => Self::BigPirates,
            Self::BigPirates => Self::Arrembaggio,
            Self::Arrembaggio => Self::Shooters,
            Self::Shooters => Self::Balanced,
        }
    }

    pub fn description(&self) -> &str {
        match self {
            Self::Balanced => "A balanced tactic, trying to alternate several possible actions. Ideal for crews with low stamina.",
            Self::BigPirates => "Focus on big pirates posting, slightly higher chance of brawls, slightly more tiring.",
            Self::Arrembaggio => {
                "Aggressive tactic focusing on giving it all, very high chance of brawl, extremely tiring."
            }
            Self::Shooters => "Focus on shooting from a distance, smaller chance of brawl,  more tiring.",
        }
    }

    pub fn pick_action(&self, rng: &mut ChaCha8Rng) -> AppResult<Action> {
        let weights = match self {
            Self::Balanced => [2, 2, 2, 2],
            Self::BigPirates => [1, 1, 2, 4],
            Self::Arrembaggio => [3, 1, 3, 1],
            Self::Shooters => [1, 4, 2, 1],
        };
        let action = match WeightedIndex::new(weights)?.sample(rng) {
            0 => Action::Isolation,
            1 => Action::OffTheScreen,
            2 => Action::PickAndRoll,
            3 => Action::Post,
            _ => unreachable!(),
        };
        Ok(action)
    }

    pub fn brawl_probability_modifier(&self) -> f32 {
        match self {
            Self::Balanced => 0.55,
            Self::BigPirates => 1.15,
            Self::Arrembaggio => 2.0,
            Self::Shooters => 0.8,
        }
    }

    pub fn tiredness_modifier(&self) -> f32 {
        match self {
            Self::Balanced => 0.5,
            Self::BigPirates => 1.0,
            Self::Arrembaggio => 1.2,
            Self::Shooters => 1.15,
        }
    }

    pub fn attack_roll_bonus(&self, action: &Action) -> i16 {
        // How does the tactic affect the outcome of the action from the attackers perspective?
        match self {
            Self::Balanced => 0,
            Self::BigPirates => match action {
                Action::Isolation => -1,
                Action::PickAndRoll => -1,
                Action::OffTheScreen => -1,
                Action::Post => 2,
                Action::Brawl => 0,
                Action::Rebound => 1,
                _ => 0,
            },
            Self::Arrembaggio => match action {
                Action::Isolation => 2,
                Action::PickAndRoll => 1,
                Action::OffTheScreen => 0,
                Action::Post => -2,
                Action::Brawl => 2,
                Action::Rebound => 0,
                _ => 0,
            },
            Self::Shooters => match action {
                Action::Isolation => 0,
                Action::PickAndRoll => 1,
                Action::OffTheScreen => 2,
                Action::Post => -1,
                Action::Brawl => 0,
                Action::Rebound => -2,
                _ => 0,
            },
        }
    }

    pub fn defense_roll_bonus(&self, action: &Action) -> i16 {
        // How does the tactic affect the outcome of the action from the defenders perspective?

        match self {
            Self::Balanced => 0,
            Self::BigPirates => match action {
                Action::Isolation => -2,
                Action::PickAndRoll => -1,
                Action::OffTheScreen => -1,
                Action::Post => 2,
                Action::Brawl => 0,
                Action::Rebound => 1,
                _ => 0,
            },
            Self::Arrembaggio => match action {
                Action::Isolation => 2,
                Action::PickAndRoll => 0,
                Action::OffTheScreen => -1,
                Action::Post => 0,
                Action::Brawl => 1,
                Action::Rebound => 0,
                _ => 0,
            },
            Self::Shooters => match action {
                Action::Isolation => 0,
                Action::PickAndRoll => 1,
                Action::OffTheScreen => 1,
                Action::Post => -2,
                Action::Brawl => -1,
                Action::Rebound => -1,
                _ => 0,
            },
        }
    }
}
