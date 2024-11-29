use super::{constants::*, resources::Resource};
use crate::{
    image::{
        color_map::ColorMap,
        spaceship::{SpaceshipImage, SpaceshipImageId},
        types::Gif,
    },
    types::{AppResult, SystemTimeTick, Tick},
    world::utils::is_default,
};
use rand::{seq::IteratorRandom, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use std::{fmt::Display, hash::Hash};
use strum::IntoEnumIterator;
use strum_macros::{Display, EnumIter};

#[derive(Debug, Display, Clone, Copy, PartialEq, EnumIter)]
pub enum SpaceshipStyle {
    Shuttle,
    Pincher,
    Jester,
}

pub trait SpaceshipComponent: Sized + Clone + Copy + PartialEq {
    fn next(&self) -> Self;
    fn previous(&self) -> Self;
    fn style(&self) -> SpaceshipStyle;
    fn crew_capacity(&self) -> u8;
    fn storage_capacity(&self) -> u32;
    fn fuel_capacity(&self) -> u32;
    fn fuel_consumption_per_tick(&self) -> f32;
    fn speed(&self) -> f32;
    fn durability(&self) -> u32;
    fn cost(&self) -> u32;
    fn upgrade_cost(&self) -> Vec<(Resource, u32)>;
    fn can_be_upgraded(&self) -> bool {
        // FIXME: not very stable method to check upgrades. Should be rather explicitly set.
        let next_component = self.next();
        if next_component.cost() < self.cost() {
            return false;
        }
        if next_component == *self {
            return false;
        }

        true
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

impl SpaceshipComponent for Hull {
    fn next(&self) -> Self {
        match self {
            Self::ShuttleSmall => Self::ShuttleStandard,
            Self::ShuttleStandard => Self::ShuttleLarge,
            Self::ShuttleLarge => Self::ShuttleSmall,
            Self::PincherStandard => Self::PincherLarge,
            Self::PincherLarge => Self::PincherStandard,
            Self::JesterStandard => Self::JesterStandard,
        }
    }
    fn previous(&self) -> Self {
        match self {
            Self::ShuttleSmall => Self::ShuttleLarge,
            Self::ShuttleStandard => Self::ShuttleSmall,
            Self::ShuttleLarge => Self::ShuttleStandard,
            Self::PincherStandard => Self::PincherLarge,
            Self::PincherLarge => Self::PincherStandard,
            Self::JesterStandard => Self::JesterStandard,
        }
    }

    fn style(&self) -> SpaceshipStyle {
        match self {
            Self::ShuttleSmall => SpaceshipStyle::Shuttle,
            Self::ShuttleStandard => SpaceshipStyle::Shuttle,
            Self::ShuttleLarge => SpaceshipStyle::Shuttle,
            Self::PincherStandard => SpaceshipStyle::Pincher,
            Self::PincherLarge => SpaceshipStyle::Pincher,
            Self::JesterStandard => SpaceshipStyle::Jester,
        }
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
    fn cost(&self) -> u32 {
        match self {
            Self::ShuttleSmall => 15000,
            Self::ShuttleStandard => 25000,
            Self::ShuttleLarge => 32000,
            Self::PincherStandard => 27000,
            Self::PincherLarge => 45000,
            Self::JesterStandard => 35000,
        }
    }

    fn upgrade_cost(&self) -> Vec<(Resource, u32)> {
        if self.next().cost() < self.cost() {
            return vec![];
        }

        let scraps_cost = match self.style() {
            SpaceshipStyle::Shuttle => (self.next().cost() - self.cost()) / 36,
            SpaceshipStyle::Pincher => (self.next().cost() - self.cost()) / 40,
            SpaceshipStyle::Jester => (self.next().cost() - self.cost()) / 44,
        };

        let mut cost = vec![
            (Resource::SATOSHI, self.next().cost() - self.cost()),
            (Resource::SCRAPS, scraps_cost),
        ];
        // Final upgrade has a cost in gold
        if self.next().cost() > self.next().next().cost() {
            let gold_cost = match self.style() {
                SpaceshipStyle::Shuttle => (self.next().cost() - self.cost()) / 4000,
                SpaceshipStyle::Pincher => (self.next().cost() - self.cost()) / 3000,
                SpaceshipStyle::Jester => (self.next().cost() - self.cost()) / 2750,
            };
            cost.push((Resource::GOLD, gold_cost))
        }

        cost
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
    fn next(&self) -> Self {
        match self {
            Self::ShuttleSingle => Self::ShuttleDouble,
            Self::ShuttleDouble => Self::ShuttleTriple,
            Self::ShuttleTriple => Self::ShuttleSingle,
            Self::PincherSingle => Self::PincherDouble,
            Self::PincherDouble => Self::PincherTriple,
            Self::PincherTriple => Self::PincherSingle,
            Self::JesterDouble => Self::JesterQuadruple,
            Self::JesterQuadruple => Self::JesterDouble,
        }
    }
    fn previous(&self) -> Self {
        match self {
            Self::ShuttleSingle => Self::ShuttleTriple,
            Self::ShuttleDouble => Self::ShuttleSingle,
            Self::ShuttleTriple => Self::ShuttleDouble,
            Self::PincherSingle => Self::PincherTriple,
            Self::PincherDouble => Self::PincherSingle,
            Self::PincherTriple => Self::PincherDouble,
            Self::JesterDouble => Self::JesterQuadruple,
            Self::JesterQuadruple => Self::JesterDouble,
        }
    }

    fn style(&self) -> SpaceshipStyle {
        match self {
            Self::ShuttleSingle => SpaceshipStyle::Shuttle,
            Self::ShuttleDouble => SpaceshipStyle::Shuttle,
            Self::ShuttleTriple => SpaceshipStyle::Shuttle,
            Self::PincherSingle => SpaceshipStyle::Pincher,
            Self::PincherDouble => SpaceshipStyle::Pincher,
            Self::PincherTriple => SpaceshipStyle::Pincher,
            Self::JesterDouble => SpaceshipStyle::Jester,
            Self::JesterQuadruple => SpaceshipStyle::Jester,
        }
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
    fn cost(&self) -> u32 {
        match self {
            Self::ShuttleSingle => 5000,
            Self::ShuttleDouble => 8000,
            Self::ShuttleTriple => 15000,
            Self::PincherSingle => 5000,
            Self::PincherDouble => 11000,
            Self::PincherTriple => 18000,
            Self::JesterDouble => 10000,
            Self::JesterQuadruple => 25000,
        }
    }

    fn upgrade_cost(&self) -> Vec<(Resource, u32)> {
        if self.next().cost() < self.cost() {
            return vec![];
        }

        let scraps_cost = match self.style() {
            SpaceshipStyle::Shuttle => (self.next().cost() - self.cost()) / 75,
            SpaceshipStyle::Pincher => (self.next().cost() - self.cost()) / 70,
            SpaceshipStyle::Jester => (self.next().cost() - self.cost()) / 100,
        };

        let mut cost = vec![
            (Resource::SATOSHI, self.next().cost() - self.cost()),
            (Resource::SCRAPS, scraps_cost),
        ];
        // Final upgrade has a cost in rum
        if self.next().cost() > self.next().next().cost() {
            cost.push((Resource::RUM, 10))
        }

        cost
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
    fn next(&self) -> Self {
        match self {
            Self::ShuttleNone => Self::ShuttleSingle,
            Self::ShuttleSingle => Self::ShuttleTriple,
            Self::ShuttleTriple => Self::ShuttleNone,
            Self::PincherNone => Self::PincherDouble,
            Self::PincherDouble => Self::PincherQuadruple,
            Self::PincherQuadruple => Self::PincherNone,
            Self::JesterNone => Self::JesterDouble,
            Self::JesterDouble => Self::JesterQuadruple,
            Self::JesterQuadruple => Self::JesterNone,
        }
    }
    fn previous(&self) -> Self {
        match self {
            Self::ShuttleNone => Self::ShuttleTriple,
            Self::ShuttleSingle => Self::ShuttleNone,
            Self::ShuttleTriple => Self::ShuttleSingle,
            Self::PincherNone => Self::PincherQuadruple,
            Self::PincherDouble => Self::PincherNone,
            Self::PincherQuadruple => Self::PincherDouble,
            Self::JesterNone => Self::JesterQuadruple,
            Self::JesterDouble => Self::JesterNone,
            Self::JesterQuadruple => Self::JesterDouble,
        }
    }

    fn style(&self) -> SpaceshipStyle {
        match self {
            Self::ShuttleNone | Self::ShuttleSingle | Self::ShuttleTriple => {
                SpaceshipStyle::Shuttle
            }
            Self::PincherNone | Self::PincherDouble | Self::PincherQuadruple => {
                SpaceshipStyle::Pincher
            }
            Self::JesterNone | Self::JesterDouble | Self::JesterQuadruple => SpaceshipStyle::Jester,
        }
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
    fn cost(&self) -> u32 {
        match self {
            Self::ShuttleSingle => 5000,
            Self::ShuttleTriple => 15000,
            Self::PincherDouble => 11000,
            Self::PincherQuadruple => 24000,
            Self::JesterDouble => 10000,
            Self::JesterQuadruple => 25000,
            _ => 0,
        }
    }

    fn upgrade_cost(&self) -> Vec<(Resource, u32)> {
        if self.next().cost() < self.cost() {
            return vec![];
        }

        let scraps_cost = match self.style() {
            SpaceshipStyle::Shuttle => (self.next().cost() - self.cost()) / 75,
            SpaceshipStyle::Pincher => (self.next().cost() - self.cost()) / 70,
            SpaceshipStyle::Jester => (self.next().cost() - self.cost()) / 100,
        };

        let mut cost = vec![
            (Resource::SATOSHI, self.next().cost() - self.cost()),
            (Resource::SCRAPS, scraps_cost),
        ];
        // Final upgrade has a cost in rum
        if self.next().cost() > self.next().next().cost() {
            cost.push((Resource::RUM, 5))
        }

        cost
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
}

impl Display for Storage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Storage::ShuttleSingle => write!(f, "Single"),
            Storage::ShuttleDouble => write!(f, "Double"),
            Storage::PincherSingle => write!(f, "Single"),
            _ => write!(f, "None"),
        }
    }
}

impl SpaceshipComponent for Storage {
    fn next(&self) -> Self {
        match self {
            Self::ShuttleNone => Self::ShuttleSingle,
            Self::ShuttleSingle => Self::ShuttleDouble,
            Self::ShuttleDouble => Self::ShuttleNone,
            Self::PincherNone => Self::PincherSingle,
            Self::PincherSingle => Self::PincherNone,
            Self::JesterNone => Self::JesterNone,
        }
    }

    fn previous(&self) -> Self {
        match self {
            Self::ShuttleNone => Self::ShuttleDouble,
            Self::ShuttleSingle => Self::ShuttleNone,
            Self::ShuttleDouble => Self::ShuttleSingle,
            Self::PincherNone => Self::PincherSingle,
            Self::PincherSingle => Self::PincherNone,
            Self::JesterNone => Self::JesterNone,
        }
    }

    fn style(&self) -> SpaceshipStyle {
        match self {
            Self::ShuttleNone => SpaceshipStyle::Shuttle,
            Self::ShuttleSingle => SpaceshipStyle::Shuttle,
            Self::ShuttleDouble => SpaceshipStyle::Shuttle,
            Self::PincherNone => SpaceshipStyle::Pincher,
            Self::PincherSingle => SpaceshipStyle::Pincher,
            Self::JesterNone => SpaceshipStyle::Jester,
        }
    }
    fn crew_capacity(&self) -> u8 {
        match self {
            Self::ShuttleDouble => 1,
            _ => 0,
        }
    }

    fn storage_capacity(&self) -> u32 {
        match self {
            Self::ShuttleSingle => 2000,
            Self::ShuttleDouble => 4000,
            Self::PincherSingle => 3000,
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
            _ => 1.0,
        }
    }

    fn speed(&self) -> f32 {
        match self {
            Self::ShuttleSingle => 0.99,
            Self::ShuttleDouble => 0.98,
            Self::PincherSingle => 0.99,
            _ => 1.0,
        }
    }

    fn durability(&self) -> u32 {
        match self {
            Self::ShuttleSingle => 10,
            Self::ShuttleDouble => 11,
            Self::PincherSingle => 7,
            _ => 0,
        }
    }
    fn cost(&self) -> u32 {
        match self {
            Self::ShuttleSingle => 5000,
            Self::ShuttleDouble => 6000,
            Self::PincherSingle => 6000,
            _ => 0,
        }
    }

    fn upgrade_cost(&self) -> Vec<(Resource, u32)> {
        if self.next().cost() < self.cost() {
            return vec![];
        }

        let scraps_cost = (self.next().cost() - self.cost()) / 30;

        let cost = vec![
            (Resource::SATOSHI, self.next().cost() - self.cost()),
            (Resource::SCRAPS, scraps_cost),
        ];

        cost
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Hash, EnumIter)]
pub enum SpaceshipUpgradeTarget {
    Hull { component: Hull },
    Engine { component: Engine },
    Storage { component: Storage },
    Shooter { component: Shooter },
    Repairs { amount: u32 },
}

impl SpaceshipUpgradeTarget {
    pub const MAX_INDEX: usize = 5; // = SpaceshipUpgradeTarget::iter().count();
}

impl Default for SpaceshipUpgradeTarget {
    fn default() -> Self {
        Self::Repairs { amount: 0 }
    }
}

impl Display for SpaceshipUpgradeTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Hull { .. } => write!(f, "Hull"),
            Self::Engine { .. } => write!(f, "Engine"),
            Self::Storage { .. } => write!(f, "Storage"),
            Self::Shooter { .. } => write!(f, "Shooter"),
            Self::Repairs { .. } => write!(f, "Repairs"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Hash)]
pub struct SpaceshipUpgrade {
    pub target: SpaceshipUpgradeTarget,
    pub started: Tick,
    pub duration: Tick,
}

impl SpaceshipUpgrade {
    pub const REPAIR_BASE_COST: u32 = 120;
    pub const REPAIR_BASE_DURATION: Tick = 2 * MINUTES;
    pub const SPACESHIP_UPGRADE_BASE_DURATION: Tick = 8 * HOURS;

    pub fn new(target: SpaceshipUpgradeTarget) -> Self {
        let duration = match target {
            SpaceshipUpgradeTarget::Repairs { amount } => {
                amount as Tick * SpaceshipUpgrade::REPAIR_BASE_DURATION
            }
            _ => SpaceshipUpgrade::SPACESHIP_UPGRADE_BASE_DURATION,
        };
        SpaceshipUpgrade {
            started: Tick::now(),
            duration,
            target,
        }
    }

    pub fn description(&self) -> String {
        match self.target {
            SpaceshipUpgradeTarget::Repairs { .. } => "Repairing spaceship".to_string(),
            _ => format!("Upgrading {}", self.target),
        }
    }

    pub fn cost(&self) -> Vec<(Resource, u32)> {
        match self.target {
            SpaceshipUpgradeTarget::Hull { component } => component.previous().upgrade_cost(),
            SpaceshipUpgradeTarget::Engine { component } => component.previous().upgrade_cost(),
            SpaceshipUpgradeTarget::Storage { component } => component.previous().upgrade_cost(),
            SpaceshipUpgradeTarget::Shooter { component } => component.previous().upgrade_cost(),
            SpaceshipUpgradeTarget::Repairs { amount } => {
                vec![
                    (
                        Resource::SATOSHI,
                        amount * SpaceshipUpgrade::REPAIR_BASE_COST,
                    ),
                    (Resource::SCRAPS, amount),
                ]
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Hash)]
pub struct Spaceship {
    pub name: String,
    pub hull: Hull,
    pub engine: Engine,
    pub storage: Storage,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub shooter: Shooter,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    current_durability: u32,
    pub image: SpaceshipImage,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub pending_upgrade: Option<SpaceshipUpgrade>,
}

impl Spaceship {
    pub fn new(
        name: String,
        hull: Hull,
        engine: Engine,
        storage: Storage,
        shooter: Shooter,
        color_map: ColorMap,
    ) -> Self {
        let mut spaceship = Self {
            name,
            hull,
            engine,
            storage,
            shooter,
            current_durability: 0,
            image: SpaceshipImage::new(color_map),
            pending_upgrade: None,
        };
        spaceship.reset_durability();

        spaceship
    }

    pub fn reset_durability(&mut self) {
        self.current_durability = self.durability()
    }

    pub fn can_be_repaired(&self) -> bool {
        self.current_durability() < self.durability()
    }

    pub fn can_be_upgraded(&self, target: SpaceshipUpgradeTarget) -> bool {
        match target {
            SpaceshipUpgradeTarget::Hull { .. } => self.hull.can_be_upgraded(),
            SpaceshipUpgradeTarget::Engine { .. } => self.engine.can_be_upgraded(),
            SpaceshipUpgradeTarget::Storage { .. } => self.storage.can_be_upgraded(),
            SpaceshipUpgradeTarget::Shooter { .. } => self.shooter.can_be_upgraded(),
            SpaceshipUpgradeTarget::Repairs { .. } => self.can_be_repaired(),
        }
    }

    pub fn random(name: String) -> Self {
        let rng = &mut ChaCha8Rng::from_entropy();
        let style = SpaceshipStyle::iter().choose(rng).unwrap();
        let hull = Hull::iter()
            .filter(|h| h.style() == style)
            .choose(rng)
            .expect("Should choose a valid component");
        let engine = Engine::iter()
            .filter(|e| e.style() == style)
            .choose(rng)
            .expect("Should choose a valid component");
        let storage = Storage::iter()
            .filter(|s| s.style() == style)
            .choose(rng)
            .expect("Should choose a valid component");

        let shooter = Shooter::iter()
            .filter(|s| s.style() == style)
            .choose(rng)
            .expect("Should choose a valid component");

        Self::new(name, hull, engine, storage, shooter, ColorMap::random())
    }

    pub fn style(&self) -> SpaceshipStyle {
        self.hull.style()
    }

    pub fn with_color_map(mut self, color_map: ColorMap) -> Self {
        self.image.set_color_map(color_map);
        self
    }

    pub fn image_id(&self) -> SpaceshipImageId {
        self.image.id(self.hull, self.engine, self.storage)
    }

    pub fn compose_image(&self) -> AppResult<Gif> {
        self.image.compose(
            self.hull,
            self.engine,
            self.storage,
            self.shooter,
            false,
            false,
        )
    }

    pub fn compose_image_shooting(&self) -> AppResult<Gif> {
        self.image.compose(
            self.hull,
            self.engine,
            self.storage,
            self.shooter,
            false,
            true,
        )
    }

    pub fn compose_image_in_shipyard(&self) -> AppResult<Gif> {
        self.image.compose(
            self.hull,
            self.engine,
            self.storage,
            self.shooter,
            true,
            false,
        )
    }

    pub fn speed(&self, storage_units: u32) -> f32 {
        // Returns the speed in Km/ms (Kilometers per Tick)
        BASE_SPEED * self.hull.speed() * self.engine.speed() * self.storage.speed()
            / (1.0 + SPEED_PENALTY_PER_UNIT_STORAGE * storage_units as f32)
    }

    pub fn durability(&self) -> u32 {
        self.hull.durability() + self.engine.durability() + self.storage.durability()
    }

    pub fn damage(&self) -> f32 {
        self.shooter.damage()
    }

    pub fn fire_rate(&self) -> f32 {
        self.shooter.fire_rate()
    }

    pub fn shooting_points(&self) -> u8 {
        self.shooter.shooting_points()
    }

    pub fn storage_capacity(&self) -> u32 {
        self.hull.storage_capacity()
            + self.engine.storage_capacity()
            + self.storage.storage_capacity()
    }

    pub fn fuel_capacity(&self) -> u32 {
        self.hull.fuel_capacity() + self.engine.fuel_capacity() + self.storage.fuel_capacity()
    }

    pub fn fuel_consumption_per_tick(&self, storage_units: u32) -> f32 {
        // Returns the fuel consumption in t/ms (tonnes per Tick)
        BASE_FUEL_CONSUMPTION
            * self.hull.fuel_consumption_per_tick()
            * self.engine.fuel_consumption_per_tick()
            * self.storage.fuel_consumption_per_tick()
            * (1.0 + FUEL_CONSUMPTION_PER_UNIT_STORAGE * storage_units as f32)
    }

    pub fn fuel_consumption_per_kilometer(&self, storage_units: u32) -> f32 {
        // Returns the fuel consumption in t/Km (tonnes per Kilometer)
        self.fuel_consumption_per_tick(storage_units) / self.speed(storage_units)
    }

    pub fn set_current_durability(&mut self, value: u32) {
        self.current_durability = value.min(self.durability());
    }

    pub fn current_durability(&self) -> u32 {
        self.current_durability
    }

    pub fn crew_capacity(&self) -> u8 {
        self.hull.crew_capacity() + self.engine.crew_capacity() + self.storage.crew_capacity()
    }

    pub fn cost(&self) -> u32 {
        let base_cost = self.hull.cost() + self.engine.cost() + self.storage.cost();
        (base_cost as f32 * SPACESHIP_BASE_COST_MULTIPLIER) as u32
    }

    pub fn max_distance(&self, current_fuel: u32) -> f32 {
        // Return the max distance in kilometers.
        let storage_units = 0;
        self.speed(storage_units) / self.fuel_consumption_per_tick(storage_units)
            * current_fuel as f32
    }

    pub fn max_travel_time(&self, current_fuel: u32) -> Tick {
        // Return the max travel time in milliseconds (Ticks)
        let storage_units = 0;
        (current_fuel as f32 / self.fuel_consumption_per_tick(storage_units)) as Tick
    }
}

#[derive(Display, Debug, Clone, Copy, PartialEq, Hash, EnumIter)]
pub enum SpaceshipPrefab {
    Bresci,
    Cafiero,
    Yukawa,
    Milwaukee,
    Pincher,
    Orwell,
    Ragnarok,
    Ibarruri,
    Rivolta,
}

impl SpaceshipPrefab {
    pub fn spaceship(&self, name: impl Into<String>) -> Spaceship {
        match self {
            Self::Yukawa => Spaceship::new(
                name.into(),
                Hull::ShuttleSmall,
                Engine::ShuttleTriple,
                Storage::ShuttleNone,
                Shooter::ShuttleSingle,
                ColorMap::default(),
            ),
            Self::Milwaukee => Spaceship::new(
                name.into(),
                Hull::ShuttleLarge,
                Engine::ShuttleTriple,
                Storage::ShuttleNone,
                Shooter::ShuttleTriple,
                ColorMap::default(),
            ),
            Self::Cafiero => Spaceship::new(
                name.into(),
                Hull::ShuttleStandard,
                Engine::ShuttleSingle,
                Storage::ShuttleSingle,
                Shooter::ShuttleTriple,
                ColorMap::default(),
            ),
            Self::Bresci => Spaceship::new(
                name.into(),
                Hull::ShuttleSmall,
                Engine::ShuttleSingle,
                Storage::ShuttleNone,
                Shooter::ShuttleSingle,
                ColorMap::default(),
            ),
            Self::Pincher => Spaceship::new(
                name.into(),
                Hull::PincherStandard,
                Engine::PincherTriple,
                Storage::PincherSingle,
                Shooter::PincherDouble,
                ColorMap::default(),
            ),
            Self::Orwell => Spaceship::new(
                name.into(),
                Hull::PincherStandard,
                Engine::PincherSingle,
                Storage::PincherNone,
                Shooter::PincherDouble,
                ColorMap::default(),
            ),
            Self::Ragnarok => Spaceship::new(
                name.into(),
                Hull::PincherLarge,
                Engine::PincherDouble,
                Storage::PincherNone,
                Shooter::PincherQuadruple,
                ColorMap::default(),
            ),
            Self::Ibarruri => Spaceship::new(
                name.into(),
                Hull::JesterStandard,
                Engine::JesterDouble,
                Storage::JesterNone,
                Shooter::JesterDouble,
                ColorMap::default(),
            ),
            Self::Rivolta => Spaceship::new(
                name.into(),
                Hull::JesterStandard,
                Engine::JesterQuadruple,
                Storage::JesterNone,
                Shooter::JesterQuadruple,
                ColorMap::default(),
            ),
        }
    }

    pub fn cost(&self) -> u32 {
        self.spaceship("".to_string()).cost()
    }
}

#[cfg(test)]

mod tests {
    use itertools::Itertools;

    use crate::{
        types::{SystemTimeTick, TeamId},
        world::{constants::*, team::Team, types::TeamLocation, world::World},
    };

    use super::*;

    #[test]
    fn test_spaceship_prefab_data() {
        let name = "test".to_string();
        let spaceship = SpaceshipPrefab::Yukawa.spaceship(name);
        let speed = spaceship.speed(0);
        let crew_capacity = spaceship.crew_capacity();
        let storage_capacity = spaceship.storage_capacity();
        let fuel_capacity = spaceship.fuel_capacity();
        let fuel_consumption = spaceship.fuel_consumption_per_tick(0);
        let cost = spaceship.cost();
        let max_distance = spaceship.max_distance(fuel_capacity);
        let max_travel_time = spaceship.max_travel_time(fuel_capacity);

        //log the spaceship data
        println!(
            "Speed: {} Km/ms = {:.2} Km/h = {:.2} AU/h",
            speed,
            speed * HOURS as f32,
            speed * HOURS as f32 / AU as f32
        );
        println!("Crew Capacity: {}", crew_capacity);
        println!("Storage Capacity: {}", storage_capacity);
        println!("Fuel Capacity: {} t", fuel_capacity);
        println!(
            "Fuel Consumption: {} t/ms = {:.2} t/h",
            fuel_consumption,
            fuel_consumption * HOURS as f32
        );
        println!("Cost: {}", cost);
        println!(
            "Max Distance: {} Km = {:.2} AU",
            max_distance,
            max_distance / AU as f32
        );
        println!(
            "Max Travel Time: {} ms = {}",
            max_travel_time,
            max_travel_time.formatted()
        );
    }

    #[test]
    fn test_total_travelled_au() -> AppResult<()> {
        let name = "test".to_string();
        let spaceship = SpaceshipPrefab::Yukawa.spaceship(name);

        let mut world = World::new(None);

        world.initialize(false)?;

        let planet_ids = world.planets.keys().collect_vec();
        let from = planet_ids[0].clone();
        let to = planet_ids[1].clone();
        let mut team = Team::random(TeamId::new_v4(), from.clone(), "test", "test");
        team.spaceship = spaceship;
        team.current_location = TeamLocation::Travelling {
            from,
            to,
            started: Tick::now(),
            duration: 100,
            distance: 1000,
        };
        println!("TOTAL AU: {}", team.total_travelled);
        world.own_team_id = team.id;

        world.teams.insert(team.id, team);

        let mut current_tick = Tick::now();

        loop {
            let own_team = world.get_own_team()?;
            match own_team.current_location {
                TeamLocation::Travelling {
                    started, duration, ..
                } => println!(
                    "Team is travelling: {} < {} + {} = {}\r",
                    current_tick,
                    started,
                    duration,
                    started + duration
                ),
                _ => {
                    println!("Team landed");
                    println!("TOTALAU: {}", own_team.total_travelled);
                    break;
                }
            };

            match world.tick_travel(current_tick) {
                Ok(messages) => {
                    for message in messages {
                        println!("{:#?}", message)
                    }
                }
                Err(e) => {
                    eprintln!("Failed to tick event: {}", e);
                    break;
                }
            }
            current_tick += 1;
        }

        Ok(())
    }
}
