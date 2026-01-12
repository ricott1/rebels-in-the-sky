use super::constants::*;
use crate::core::UpgradeableElement;
use rand::seq::IteratorRandom;
use rand_chacha::ChaCha8Rng;
use serde_repr::{Deserialize_repr, Serialize_repr};
use std::{
    fmt::{Debug, Display},
    hash::Hash,
};
use strum::IntoEnumIterator;
use strum_macros::{Display, EnumIter};

#[derive(Debug, Display, Clone, Copy, PartialEq, EnumIter)]
pub enum SpaceshipStyle {
    Shuttle,
    Pincher,
    Jester,
}

pub trait SpaceshipComponent: Clone + Copy + Debug + PartialEq + UpgradeableElement {
    fn random_for(rng: &mut ChaCha8Rng, style: SpaceshipStyle) -> Self;
    fn crew_capacity(&self) -> u8;
    fn storage_capacity(&self) -> u32;
    fn fuel_capacity(&self) -> u32;
    fn fuel_consumption_per_tick(&self) -> f32;
    fn speed(&self) -> f32;
    fn durability(&self) -> u32;
    fn value(&self) -> u32;
}

#[derive(
    Debug,
    Display,
    Clone,
    Copy,
    Serialize_repr,
    Deserialize_repr,
    PartialEq,
    Hash,
    Default,
    EnumIter,
)]
#[repr(u8)]
pub enum ChargeUnit {
    #[default]
    Small,
    Medium,
    Large,
}

impl ChargeUnit {
    pub fn max_charge(&self) -> f32 {
        match self {
            Self::Small => 80.0,
            Self::Medium => 100.0,
            Self::Large => 120.0,
        }
    }
}

impl SpaceshipComponent for ChargeUnit {
    fn random_for(rng: &mut ChaCha8Rng, _style: SpaceshipStyle) -> Self {
        Self::iter()
            .choose(rng)
            .expect("There should be one possible selection")
    }

    fn crew_capacity(&self) -> u8 {
        0
    }

    fn storage_capacity(&self) -> u32 {
        0
    }

    fn fuel_capacity(&self) -> u32 {
        0
    }

    fn fuel_consumption_per_tick(&self) -> f32 {
        match self {
            Self::Small => 1.0,
            Self::Medium => 0.975,
            Self::Large => 0.955,
        }
    }

    fn speed(&self) -> f32 {
        1.0
    }

    fn durability(&self) -> u32 {
        0
    }

    fn value(&self) -> u32 {
        match self {
            Self::Small => 1_000,
            Self::Medium => 7_000,
            Self::Large => 21_000,
        }
    }
}

#[derive(
    Debug, Clone, Copy, Serialize_repr, Deserialize_repr, PartialEq, Hash, Default, EnumIter,
)]
#[repr(u8)]
pub enum Hull {
    #[default]
    ShuttleSmall,
    ShuttleStandard,
    ShuttleLarge,
    PincherStandard,
    PincherLarge,
    JesterStandard,
}

impl Display for Hull {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ShuttleSmall => write!(f, "Small"),
            Self::ShuttleStandard => write!(f, "Standard"),
            Self::ShuttleLarge => write!(f, "Large"),
            Self::PincherStandard => write!(f, "Standard"),
            Self::PincherLarge => write!(f, "Large"),
            Self::JesterStandard => write!(f, "Standard"),
        }
    }
}

impl Hull {
    pub fn spaceship_style(&self) -> SpaceshipStyle {
        match self {
            Self::ShuttleSmall | Self::ShuttleStandard | Self::ShuttleLarge => {
                SpaceshipStyle::Shuttle
            }
            Self::PincherStandard | Self::PincherLarge => SpaceshipStyle::Pincher,
            Self::JesterStandard => SpaceshipStyle::Jester,
        }
    }
}

impl SpaceshipComponent for Hull {
    fn random_for(rng: &mut ChaCha8Rng, style: SpaceshipStyle) -> Self {
        match style {
            SpaceshipStyle::Jester => vec![Self::JesterStandard],
            SpaceshipStyle::Pincher => vec![Self::PincherStandard, Self::PincherLarge],
            SpaceshipStyle::Shuttle => vec![
                Self::ShuttleSmall,
                Self::ShuttleStandard,
                Self::ShuttleLarge,
            ],
        }
        .iter()
        .choose(rng)
        .copied()
        .expect("There should be one possible selection")
    }

    fn crew_capacity(&self) -> u8 {
        match self {
            Self::ShuttleSmall => MIN_PLAYERS_PER_GAME as u8 + 2,
            Self::ShuttleStandard => MIN_PLAYERS_PER_GAME as u8 + 3,
            Self::ShuttleLarge => MIN_PLAYERS_PER_GAME as u8 + 4,
            Self::PincherStandard => MIN_PLAYERS_PER_GAME as u8 + 3,
            Self::PincherLarge => MIN_PLAYERS_PER_GAME as u8 + 4,
            Self::JesterStandard => MIN_PLAYERS_PER_GAME as u8 + 3,
        }
    }

    fn storage_capacity(&self) -> u32 {
        match self {
            Self::ShuttleSmall => 2100,
            Self::ShuttleStandard => 3000,
            Self::ShuttleLarge => 5000,
            Self::PincherStandard => 2000,
            Self::PincherLarge => 3800,
            Self::JesterStandard => 1800,
        }
    }

    fn fuel_capacity(&self) -> u32 {
        match self {
            Self::ShuttleSmall => 110,
            Self::ShuttleStandard => 200,
            Self::ShuttleLarge => 380,
            Self::PincherStandard => 300,
            Self::PincherLarge => 500,
            Self::JesterStandard => 400,
        }
    }

    fn fuel_consumption_per_tick(&self) -> f32 {
        match self {
            Self::ShuttleSmall => 0.95,
            Self::ShuttleStandard => 1.0,
            Self::ShuttleLarge => 1.05,
            Self::PincherStandard => 1.05,
            Self::PincherLarge => 1.1,
            Self::JesterStandard => 1.06,
        }
    }

    fn speed(&self) -> f32 {
        match self {
            Self::ShuttleSmall => 1.25,
            Self::ShuttleStandard => 1.0,
            Self::ShuttleLarge => 0.92,
            Self::PincherStandard => 1.15,
            Self::PincherLarge => 0.95,
            Self::JesterStandard => 1.05,
        }
    }
    fn durability(&self) -> u32 {
        match self {
            Self::ShuttleSmall => 16,
            Self::ShuttleStandard => 18,
            Self::ShuttleLarge => 19,
            Self::PincherStandard => 18,
            Self::PincherLarge => 20,
            Self::JesterStandard => 16,
        }
    }

    fn value(&self) -> u32 {
        match self {
            Self::ShuttleSmall => 15_000,
            Self::ShuttleStandard => 25_000,
            Self::ShuttleLarge => 45_000,
            Self::PincherStandard => 22_150,
            Self::PincherLarge => 42_000,
            Self::JesterStandard => 30_000,
        }
    }
}

#[derive(
    Debug, Clone, Copy, Serialize_repr, Deserialize_repr, PartialEq, Default, EnumIter, Hash,
)]
#[repr(u8)]
pub enum Engine {
    #[default]
    ShuttleSingle,
    ShuttleDouble,
    ShuttleTriple,
    PincherSingle,
    PincherDouble,
    PincherTriple,
    JesterDouble,
    JesterQuadruple,
}

impl Display for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ShuttleSingle => write!(f, "Single"),
            Self::ShuttleDouble => write!(f, "Double"),
            Self::ShuttleTriple => write!(f, "Triple"),
            Self::PincherSingle => write!(f, "Single"),
            Self::PincherDouble => write!(f, "Double"),
            Self::PincherTriple => write!(f, "Triple"),
            Self::JesterDouble => write!(f, "Double"),
            Self::JesterQuadruple => write!(f, "Quadruple"),
        }
    }
}

impl SpaceshipComponent for Engine {
    fn random_for(rng: &mut ChaCha8Rng, style: SpaceshipStyle) -> Self {
        match style {
            SpaceshipStyle::Jester => vec![Self::JesterDouble, Self::JesterQuadruple],
            SpaceshipStyle::Pincher => vec![
                Self::PincherSingle,
                Self::PincherDouble,
                Self::PincherTriple,
            ],
            SpaceshipStyle::Shuttle => vec![
                Self::ShuttleSingle,
                Self::ShuttleDouble,
                Self::ShuttleTriple,
            ],
        }
        .iter()
        .choose(rng)
        .copied()
        .expect("There should be one possible selection")
    }

    fn crew_capacity(&self) -> u8 {
        0
    }

    fn storage_capacity(&self) -> u32 {
        0
    }

    fn fuel_capacity(&self) -> u32 {
        0
    }

    fn fuel_consumption_per_tick(&self) -> f32 {
        match self {
            Self::ShuttleSingle => 1.0,
            Self::ShuttleDouble => 1.5,
            Self::ShuttleTriple => 2.0,
            Self::PincherSingle => 1.0,
            Self::PincherDouble => 1.5,
            Self::PincherTriple => 2.0,
            Self::JesterDouble => 1.35,
            Self::JesterQuadruple => 2.1,
        }
    }

    fn speed(&self) -> f32 {
        match self {
            Self::ShuttleSingle => 1.2,
            Self::ShuttleDouble => 1.8,
            Self::ShuttleTriple => 2.4,
            Self::PincherSingle => 1.1,
            Self::PincherDouble => 1.75,
            Self::PincherTriple => 2.5,
            Self::JesterDouble => 1.6,
            Self::JesterQuadruple => 2.65,
        }
    }

    fn durability(&self) -> u32 {
        match self {
            Self::ShuttleSingle => 8,
            Self::ShuttleDouble => 7,
            Self::ShuttleTriple => 6,
            Self::PincherSingle => 8,
            Self::PincherDouble => 7,
            Self::PincherTriple => 6,
            Self::JesterDouble => 6,
            Self::JesterQuadruple => 5,
        }
    }

    fn value(&self) -> u32 {
        match self {
            Self::ShuttleSingle => 4_500,
            Self::ShuttleDouble => 9_500,
            Self::ShuttleTriple => 15_300,
            Self::PincherSingle => 6_250,
            Self::PincherDouble => 13_500,
            Self::PincherTriple => 19_750,
            Self::JesterDouble => 10_500,
            Self::JesterQuadruple => 23_000,
        }
    }
}

#[derive(
    Debug,
    Display,
    Clone,
    Copy,
    Serialize_repr,
    Deserialize_repr,
    PartialEq,
    Hash,
    Default,
    EnumIter,
)]
#[repr(u8)]
pub enum Shield {
    #[default]
    None,
    Small,
    Medium,
}

impl Shield {
    pub fn max_durability(&self) -> f32 {
        match self {
            Self::None => 0.0,
            Self::Small => 8.0,
            Self::Medium => 14.0,
        }
    }

    pub fn damage_reduction(&self) -> f32 {
        match self {
            Self::None => 1.0,
            Self::Small => 0.7,
            Self::Medium => 0.4,
        }
    }
}

impl SpaceshipComponent for Shield {
    fn random_for(rng: &mut ChaCha8Rng, _style: SpaceshipStyle) -> Self {
        Self::iter()
            .choose(rng)
            .expect("There should be one possible selection")
    }

    fn crew_capacity(&self) -> u8 {
        0
    }

    fn storage_capacity(&self) -> u32 {
        0
    }

    fn fuel_capacity(&self) -> u32 {
        0
    }

    fn fuel_consumption_per_tick(&self) -> f32 {
        1.0
    }

    fn speed(&self) -> f32 {
        1.0
    }

    fn durability(&self) -> u32 {
        match self {
            Self::None => 0,
            Self::Small => 1,
            Self::Medium => 2,
        }
    }

    fn value(&self) -> u32 {
        match self {
            Self::None => 0,
            Self::Small => 4_500,
            Self::Medium => 11_250,
        }
    }
}

#[derive(
    Debug, Clone, Copy, Serialize_repr, Deserialize_repr, PartialEq, Default, EnumIter, Hash,
)]
#[repr(u8)]
pub enum Shooter {
    #[default]
    ShuttleNone,
    ShuttleSingle,
    ShuttleTriple,
    PincherNone,
    PincherDouble,
    PincherQuadruple,
    JesterNone,
    JesterDouble,
    JesterQuadruple,
}

impl Display for Shooter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ShuttleSingle => write!(f, "Single"),
            Self::ShuttleTriple => write!(f, "Triple"),
            Self::PincherDouble => write!(f, "Double"),
            Self::PincherQuadruple => write!(f, "Quadruple"),
            Self::JesterDouble => write!(f, "Double"),
            Self::JesterQuadruple => write!(f, "Quadruple"),
            _ => write!(f, "None"),
        }
    }
}

impl SpaceshipComponent for Shooter {
    fn random_for(rng: &mut ChaCha8Rng, style: SpaceshipStyle) -> Self {
        match style {
            SpaceshipStyle::Jester => {
                vec![Self::JesterNone, Self::JesterDouble, Self::JesterQuadruple]
            }
            SpaceshipStyle::Pincher => vec![
                Self::PincherNone,
                Self::PincherDouble,
                Self::PincherQuadruple,
            ],
            SpaceshipStyle::Shuttle => {
                vec![Self::ShuttleNone, Self::ShuttleSingle, Self::ShuttleTriple]
            }
        }
        .iter()
        .choose(rng)
        .copied()
        .expect("There should be one possible selection")
    }

    fn crew_capacity(&self) -> u8 {
        0
    }

    fn storage_capacity(&self) -> u32 {
        0
    }

    fn fuel_capacity(&self) -> u32 {
        0
    }

    fn fuel_consumption_per_tick(&self) -> f32 {
        1.0
    }

    fn speed(&self) -> f32 {
        1.0
    }

    fn durability(&self) -> u32 {
        match self {
            Self::ShuttleSingle => 1,
            Self::ShuttleTriple => 3,
            Self::PincherDouble => 2,
            Self::PincherQuadruple => 4,
            Self::JesterDouble => 2,
            Self::JesterQuadruple => 4,
            _ => 0,
        }
    }

    fn value(&self) -> u32 {
        match self {
            Self::ShuttleSingle => 4_500,
            Self::ShuttleTriple => 15_000,
            Self::PincherDouble => 8_000,
            Self::PincherQuadruple => 16_500,
            Self::JesterDouble => 10_500,
            Self::JesterQuadruple => 22_000,
            _ => 0,
        }
    }
}

impl Shooter {
    pub fn damage(&self) -> f32 {
        match self {
            Self::ShuttleSingle => 1.5,
            Self::ShuttleTriple => 1.8,
            Self::PincherDouble => 2.0,
            Self::PincherQuadruple => 2.5,
            Self::JesterDouble => 2.25,
            Self::JesterQuadruple => 3.0,
            _ => 0.0,
        }
    }

    pub fn fire_rate(&self) -> f32 {
        match self {
            Self::ShuttleSingle => 6.0,
            Self::ShuttleTriple => 4.0,
            Self::PincherDouble => 6.0,
            Self::PincherQuadruple => 6.0,
            Self::JesterDouble => 9.0,
            Self::JesterQuadruple => 9.0,
            _ => 0.0,
        }
    }

    pub fn shooting_points(&self) -> u8 {
        match self {
            Self::ShuttleSingle => 1,
            Self::ShuttleTriple => 3,
            Self::PincherDouble => 2,
            Self::PincherQuadruple => 4,
            Self::JesterDouble => 2,
            Self::JesterQuadruple => 4,
            _ => 0,
        }
    }
}
#[derive(
    Debug, Clone, Copy, Serialize_repr, Deserialize_repr, PartialEq, Default, EnumIter, Hash,
)]
#[repr(u8)]
pub enum Storage {
    #[default]
    ShuttleNone,
    ShuttleSingle,
    ShuttleDouble,
    PincherNone,
    PincherSingle,
    JesterNone,
    JesterSingle,
}

impl Display for Storage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ShuttleSingle | Self::PincherSingle | Self::JesterSingle => write!(f, "Single"),
            Self::ShuttleDouble => write!(f, "Double"),
            _ => write!(f, "None"),
        }
    }
}

impl SpaceshipComponent for Storage {
    fn random_for(rng: &mut ChaCha8Rng, style: SpaceshipStyle) -> Self {
        match style {
            SpaceshipStyle::Jester => vec![Self::JesterNone, Self::JesterSingle],
            SpaceshipStyle::Pincher => vec![Self::PincherNone, Self::PincherSingle],
            SpaceshipStyle::Shuttle => {
                vec![Self::ShuttleNone, Self::ShuttleSingle, Self::ShuttleDouble]
            }
        }
        .iter()
        .choose(rng)
        .copied()
        .expect("There should be one possible selection")
    }

    fn crew_capacity(&self) -> u8 {
        match self {
            Self::ShuttleDouble => 1,
            Self::JesterSingle => 1,
            _ => 0,
        }
    }

    fn storage_capacity(&self) -> u32 {
        match self {
            Self::ShuttleSingle => 2000,
            Self::ShuttleDouble => 4000,
            Self::PincherSingle => 3000,
            Self::JesterSingle => 1600,
            _ => 0,
        }
    }

    fn fuel_capacity(&self) -> u32 {
        match self {
            Self::ShuttleSingle => 30,
            Self::ShuttleDouble => 60,
            Self::PincherSingle => 20,
            _ => 0,
        }
    }

    fn fuel_consumption_per_tick(&self) -> f32 {
        match self {
            Self::ShuttleSingle => 1.02,
            Self::ShuttleDouble => 1.03,
            Self::PincherSingle => 1.03,
            Self::JesterSingle => 1.04,
            _ => 1.0,
        }
    }

    fn speed(&self) -> f32 {
        match self {
            Self::ShuttleSingle => 0.99,
            Self::ShuttleDouble => 0.98,
            Self::PincherSingle => 0.99,
            Self::JesterSingle => 0.99,
            _ => 1.0,
        }
    }

    fn durability(&self) -> u32 {
        match self {
            Self::ShuttleSingle => 10,
            Self::ShuttleDouble => 11,
            Self::PincherSingle => 7,
            Self::JesterSingle => 4,
            _ => 0,
        }
    }

    fn value(&self) -> u32 {
        match self {
            Self::ShuttleSingle => 7_000,
            Self::ShuttleDouble => 14_500,
            Self::PincherSingle => 6_000,
            Self::JesterSingle => 5_500,
            _ => 0,
        }
    }
}
