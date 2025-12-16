use super::constants::*;
use crate::{
    image::{
        color_map::ColorMap,
        spaceship::{SpaceshipImage, SpaceshipImageId},
        types::Gif,
    },
    types::{AppResult, Tick},
    world::{utils::is_default, SpaceshipUpgradeTarget, Upgrade, UpgradeableElement},
};
use rand::seq::IteratorRandom;
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

pub trait SpaceshipComponent: Sized + Clone + Copy + PartialEq + UpgradeableElement {
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
            Self::Small => 1.025,
            Self::Medium => 1.125,
            Self::Large => 1.25,
        }
    }

    fn speed(&self) -> f32 {
        1.0
    }

    fn durability(&self) -> u32 {
        match self {
            Self::Small => 0,
            Self::Medium => 1,
            Self::Large => 2,
        }
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
    fn max_durability(&self) -> f32 {
        match self {
            Self::None => 0.0,
            Self::Small => 8.0,
            Self::Medium => 14.0,
        }
    }

    fn damage_reduction(&self) -> f32 {
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
        0
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
            Self::JesterSingle => 1.02,
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
            Self::JesterSingle => 2,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Hash)]
pub struct Spaceship {
    pub name: String,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub charge_unit: ChargeUnit,
    pub hull: Hull,
    pub engine: Engine,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub shield: Shield,
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
    pub pending_upgrade: Option<Upgrade<SpaceshipUpgradeTarget>>,
}

impl Spaceship {
    pub fn new(
        charge_unit: ChargeUnit,
        hull: Hull,
        engine: Engine,
        storage: Storage,
        shield: Shield,
        shooter: Shooter,
        color_map: ColorMap,
    ) -> Self {
        let mut spaceship = Self {
            name: "".into(),
            charge_unit,
            hull,
            engine,
            shield,
            storage,
            shooter,
            current_durability: 0,
            image: SpaceshipImage::new(color_map),
            pending_upgrade: None,
        };
        spaceship.reset_durability();

        spaceship
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    pub fn reset_durability(&mut self) {
        self.current_durability = self.max_durability()
    }

    pub fn can_be_repaired(&self) -> bool {
        self.current_durability() < self.max_durability()
    }

    pub fn can_be_upgraded(&self, target: SpaceshipUpgradeTarget) -> bool {
        match target {
            SpaceshipUpgradeTarget::ChargeUnit { .. } => self.charge_unit.can_be_upgraded(),
            SpaceshipUpgradeTarget::Hull { .. } => self.hull.can_be_upgraded(),
            SpaceshipUpgradeTarget::Engine { .. } => self.engine.can_be_upgraded(),
            SpaceshipUpgradeTarget::Shield { .. } => self.shield.can_be_upgraded(),
            SpaceshipUpgradeTarget::Shooter { .. } => self.shooter.can_be_upgraded(),
            SpaceshipUpgradeTarget::Storage { .. } => self.storage.can_be_upgraded(),
            SpaceshipUpgradeTarget::Repairs { .. } => self.can_be_repaired(),
        }
    }

    pub fn random(rng: &mut ChaCha8Rng) -> Self {
        let style = SpaceshipStyle::iter()
            .choose(rng)
            .expect("There should be one available style.");

        let charge_unit = ChargeUnit::random_for(rng, style);
        let hull = Hull::random_for(rng, style);
        let engine = Engine::random_for(rng, style);
        let shield = Shield::random_for(rng, style);
        let storage = Storage::random_for(rng, style);
        let shooter = Shooter::random_for(rng, style);

        Self::new(
            charge_unit,
            hull,
            engine,
            storage,
            shield,
            shooter,
            ColorMap::random(rng),
        )
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
            Shield::None,
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
            Shield::None,
            false,
            true,
        )
    }

    pub fn compose_image_with_shield(&self) -> AppResult<Gif> {
        self.image.compose(
            self.hull,
            self.engine,
            self.storage,
            self.shooter,
            self.shield,
            false,
            false,
        )
    }

    pub fn compose_image_in_shipyard(&self) -> AppResult<Gif> {
        self.image.compose(
            self.hull,
            self.engine,
            self.storage,
            self.shooter,
            Shield::None,
            true,
            false,
        )
    }

    pub fn speed(&self, storage_units: u32) -> f32 {
        // Returns the speed in Km/ms (Kilometers per Tick)
        BASE_SPEED * self.hull.speed() * self.engine.speed() * self.storage.speed()
            / (1.0 + SPEED_PENALTY_PER_UNIT_STORAGE * storage_units as f32)
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
        self.current_durability = value.min(self.max_durability());
    }

    pub fn current_durability(&self) -> u32 {
        self.current_durability
    }

    pub fn max_durability(&self) -> u32 {
        self.hull.durability() + self.engine.durability() + self.storage.durability()
    }

    pub fn max_charge(&self) -> u32 {
        self.charge_unit.max_charge() as u32
    }

    pub fn shield_max_durability(&self) -> f32 {
        self.shield.max_durability()
    }

    pub fn shield_damage_reduction(&self) -> f32 {
        self.shield.damage_reduction()
    }

    pub fn crew_capacity(&self) -> u8 {
        let capacity =
            self.hull.crew_capacity() + self.engine.crew_capacity() + self.storage.crew_capacity();
        // Just to be sure :)
        assert!(capacity <= MAX_CREW_SIZE as u8);
        capacity
    }

    pub fn value(&self) -> u32 {
        let base_value = self.hull.value() + self.engine.value() + self.storage.value();
        (base_value as f32 * SPACESHIP_BASE_COST_MULTIPLIER) as u32
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
    pub fn spaceship(&self) -> Spaceship {
        match self {
            Self::Yukawa => Spaceship::new(
                ChargeUnit::Small,
                Hull::ShuttleSmall,
                Engine::ShuttleTriple,
                Storage::ShuttleNone,
                Shield::Small,
                Shooter::ShuttleNone,
                ColorMap::default(),
            ),
            Self::Milwaukee => Spaceship::new(
                ChargeUnit::Medium,
                Hull::ShuttleLarge,
                Engine::ShuttleTriple,
                Storage::ShuttleNone,
                Shield::Small,
                Shooter::ShuttleTriple,
                ColorMap::default(),
            ),
            Self::Cafiero => Spaceship::new(
                ChargeUnit::Medium,
                Hull::ShuttleStandard,
                Engine::ShuttleSingle,
                Storage::ShuttleSingle,
                Shield::Medium,
                Shooter::ShuttleTriple,
                ColorMap::default(),
            ),
            Self::Bresci => Spaceship::new(
                ChargeUnit::Small,
                Hull::ShuttleSmall,
                Engine::ShuttleSingle,
                Storage::ShuttleNone,
                Shield::None,
                Shooter::ShuttleNone,
                ColorMap::default(),
            ),
            Self::Pincher => Spaceship::new(
                ChargeUnit::Large,
                Hull::PincherStandard,
                Engine::PincherTriple,
                Storage::PincherSingle,
                Shield::Small,
                Shooter::PincherDouble,
                ColorMap::default(),
            ),
            Self::Orwell => Spaceship::new(
                ChargeUnit::Small,
                Hull::PincherStandard,
                Engine::PincherSingle,
                Storage::PincherNone,
                Shield::None,
                Shooter::PincherNone,
                ColorMap::default(),
            ),
            Self::Ragnarok => Spaceship::new(
                ChargeUnit::Large,
                Hull::PincherLarge,
                Engine::PincherDouble,
                Storage::PincherNone,
                Shield::Medium,
                Shooter::PincherQuadruple,
                ColorMap::default(),
            ),
            Self::Ibarruri => Spaceship::new(
                ChargeUnit::Small,
                Hull::JesterStandard,
                Engine::JesterDouble,
                Storage::JesterNone,
                Shield::None,
                Shooter::JesterNone,
                ColorMap::default(),
            ),
            Self::Rivolta => Spaceship::new(
                ChargeUnit::Large,
                Hull::JesterStandard,
                Engine::JesterQuadruple,
                Storage::JesterNone,
                Shield::Medium,
                Shooter::JesterQuadruple,
                ColorMap::default(),
            ),
        }
    }

    pub fn value(&self) -> u32 {
        self.spaceship().value()
    }
}

#[cfg(test)]

mod tests {
    use super::*;
    use crate::{
        types::SystemTimeTick,
        world::{team::Team, types::TeamLocation, world::World},
    };
    use itertools::Itertools;
    use rand::SeedableRng;

    #[test]
    fn test_spaceship_prefab_data() {
        let spaceship = SpaceshipPrefab::Yukawa.spaceship();
        let speed = spaceship.speed(0);
        let crew_capacity = spaceship.crew_capacity();
        let storage_capacity = spaceship.storage_capacity();
        let fuel_capacity = spaceship.fuel_capacity();
        let fuel_consumption = spaceship.fuel_consumption_per_tick(0);
        let value = spaceship.value();
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
        println!("Value: {}", value);
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
        let spaceship = SpaceshipPrefab::Yukawa.spaceship();

        let mut world = World::new(None);

        world.initialize(false)?;

        let planet_ids = world.planets.keys().collect_vec();
        let from = planet_ids[0].clone();
        let to = planet_ids[1].clone();
        let rng = &mut ChaCha8Rng::from_os_rng();
        let mut team = Team::random(rng).with_home_planet(from);
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
