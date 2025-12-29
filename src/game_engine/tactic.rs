use super::action::Action;
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

    pub fn pick_action(&self, rng: &mut ChaCha8Rng, num_active_players: usize) -> Option<Action> {
        if num_active_players < 1 {
            return None;
        }

        let mut weights = match self {
            Self::Balanced => [2, 2, 2, 2],
            Self::BigPirates => [1, 1, 2, 4],
            Self::Arrembaggio => [3, 1, 3, 1],
            Self::Shooters => [1, 4, 2, 1],
        };

        if num_active_players < 2 {
            weights[1] = 0;
            weights[2] = 0;
        }

        let action = match WeightedIndex::new(weights).ok()?.sample(rng) {
            0 => Action::Isolation,
            1 => Action::OffTheScreen,
            2 => Action::PickAndRoll,
            3 => Action::Post,
            _ => unreachable!(),
        };
        Some(action)
    }

    pub fn brawl_probability_modifier(&self) -> f64 {
        match self {
            Self::Balanced => 0.75,
            Self::BigPirates => 1.15,
            Self::Arrembaggio => 2.0,
            Self::Shooters => 0.8,
        }
    }

    pub fn playing_tiredness_modifier(&self) -> f32 {
        match self {
            Self::Balanced => 0.85,
            Self::BigPirates => 1.0,
            Self::Arrembaggio => 1.15,
            Self::Shooters => 1.1,
        }
    }

    pub fn fastbreak_probability_modifier(&self) -> f64 {
        match self {
            Self::Balanced => 1.0,
            Self::BigPirates => 0.7,
            Self::Arrembaggio => 2.1,
            Self::Shooters => 1.0,
        }
    }

    pub fn attack_roll_bonus(&self, action: &Action) -> i16 {
        // How does the tactic affect the outcome of the action from the attackers perspective?
        match self {
            Self::Balanced => match action {
                Action::Isolation => 10,
                Action::PickAndRoll => 10,
                Action::OffTheScreen => 10,
                Action::Post => 10,
                Action::Brawl => 10,
                Action::Rebound => 10,
                Action::Fastbreak => 16,
                _ => unreachable!(),
            },
            Self::BigPirates => match action {
                Action::Isolation => 5,
                Action::PickAndRoll => 8,
                Action::OffTheScreen => 8,
                Action::Post => 18,
                Action::Brawl => 10,
                Action::Rebound => 14,
                Action::Fastbreak => 14,
                _ => unreachable!(),
            },
            Self::Arrembaggio => match action {
                Action::Isolation => 18,
                Action::PickAndRoll => 12,
                Action::OffTheScreen => 10,
                Action::Post => 6,
                Action::Brawl => 18,
                Action::Rebound => 10,
                Action::Fastbreak => 20,
                _ => unreachable!(),
            },
            Self::Shooters => match action {
                Action::Isolation => 9,
                Action::PickAndRoll => 16,
                Action::OffTheScreen => 20,
                Action::Post => 6,
                Action::Brawl => 9,
                Action::Rebound => 5,
                Action::Fastbreak => 16,
                _ => unreachable!(),
            },
        }
    }

    pub fn defense_roll_bonus(&self, action: &Action) -> i16 {
        // How does the tactic affect the outcome of the action from the defenders perspective?
        match self {
            Self::Balanced => match action {
                Action::Isolation => 10,
                Action::PickAndRoll => 10,
                Action::OffTheScreen => 10,
                Action::Post => 10,
                Action::Brawl => 10,
                Action::Rebound => 10,
                Action::Fastbreak => 2,
                _ => unreachable!(),
            },
            Self::BigPirates => match action {
                Action::Isolation => 3,
                Action::PickAndRoll => 8,
                Action::OffTheScreen => 6,
                Action::Post => 16,
                Action::Brawl => 13,
                Action::Rebound => 14,
                Action::Fastbreak => 0,
                _ => unreachable!(),
            },
            Self::Arrembaggio => match action {
                Action::Isolation => 17,
                Action::PickAndRoll => 8,
                Action::OffTheScreen => 6,
                Action::Post => 10,
                Action::Brawl => 15,
                Action::Rebound => 10,
                Action::Fastbreak => 4,
                _ => unreachable!(),
            },
            Self::Shooters => match action {
                Action::Isolation => 10,
                Action::PickAndRoll => 12,
                Action::OffTheScreen => 14,
                Action::Post => 4,
                Action::Brawl => 8,
                Action::Rebound => 6,
                Action::Fastbreak => 2,
                _ => unreachable!(),
            },
        }
    }
}
