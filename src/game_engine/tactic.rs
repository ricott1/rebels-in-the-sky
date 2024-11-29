use super::action::Action;
use crate::types::AppResult;
use rand::{seq::IteratorRandom, SeedableRng};
use rand_chacha::ChaCha8Rng;
use rand_distr::{Distribution, WeightedIndex};
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
    pub fn random() -> Self {
        let mut rng = ChaCha8Rng::from_entropy();
        Self::iter().choose_stable(&mut rng).unwrap()
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
            Self::Balanced => "A balanced tactic, trying to alternate several possible actions.",
            Self::BigPirates => "Focus on big pirates posting, slightly higher chance of brawls.",
            Self::Arrembaggio => {
                "Aggressive tactic focusing on sharing the ball, very high chance of brawl."
            }
            Self::Shooters => "Focus on shooting from a distance, smaller chance of brawl.",
        }
    }

    pub fn pick_action(&self, rng: &mut ChaCha8Rng) -> AppResult<Action> {
        let weights = match self {
            Self::Balanced => [2, 2, 2, 2],
            Self::BigPirates => [1, 1, 2, 4],
            Self::Arrembaggio => [3, 1, 3, 1],
            Self::Shooters => [1, 4, 2, 1],
        };
        let action = match WeightedIndex::new(&weights)?.sample(rng) {
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
            Self::Balanced => 1.0,
            Self::BigPirates => 1.25,
            Self::Arrembaggio => 2.0,
            Self::Shooters => 0.75,
        }
    }

    pub fn attack_roll_bonus(&self, action: &Action) -> i16 {
        match self {
            Self::Balanced => 0,
            Self::BigPirates => match action {
                Action::Isolation => -2,
                Action::PickAndRoll => -2,
                Action::OffTheScreen => -2,
                Action::Post => 4,
                Action::Brawl => 0,
                Action::Rebound => 2,
                _ => 0,
            },
            Self::Arrembaggio => match action {
                Action::Isolation => 2,
                Action::PickAndRoll => -2,
                Action::OffTheScreen => 0,
                Action::Post => -2,
                Action::Brawl => 4,
                Action::Rebound => -2,
                _ => 0,
            },
            Self::Shooters => match action {
                Action::Isolation => 0,
                Action::PickAndRoll => 2,
                Action::OffTheScreen => 4,
                Action::Post => -2,
                Action::Brawl => 0,
                Action::Rebound => -4,
                _ => 0,
            },
        }
    }

    pub fn defense_roll_bonus(&self, action: &Action) -> i16 {
        match self {
            Self::Balanced => 0,
            Self::BigPirates => match action {
                Action::Isolation => -2,
                Action::PickAndRoll => -2,
                Action::OffTheScreen => -2,
                Action::Post => 4,
                Action::Brawl => 0,
                Action::Rebound => 2,
                _ => 0,
            },
            Self::Arrembaggio => match action {
                Action::Isolation => 0,
                Action::PickAndRoll => -2,
                Action::OffTheScreen => 2,
                Action::Post => -2,
                Action::Brawl => 2,
                Action::Rebound => -2,
                _ => 0,
            },
            Self::Shooters => match action {
                Action::Isolation => 2,
                Action::PickAndRoll => 0,
                Action::OffTheScreen => 2,
                Action::Post => -2,
                Action::Brawl => -2,
                Action::Rebound => 0,
                _ => 0,
            },
        }
    }
}
