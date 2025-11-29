use super::constants::HOURS;
use super::{resources::Resource, skill::MAX_SKILL, types::Population};
use crate::types::{SystemTimeTick, Tick};
use crate::world::skill::GameSkill;
use crate::world::utils::is_default;
use crate::{
    types::*,
    types::{PlanetId, TeamId},
};
use libp2p::PeerId;
use rand::prelude::Distribution;
use rand::seq::IndexedRandom;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use rand_distr::weighted::WeightedIndex;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
};
use strum_macros::{Display, EnumIter};

// Remve this imports once we remove the fallback logic for the asteroid upgrade serialization
use serde::de::{self, Visitor};
use serde::{Deserializer, Serializer};
use std::fmt;

const TRADE_DELTA_SCARCITY: f32 = 3.25;
const TRADE_DELTA_BUY_SELL: f32 = 0.05;
const RESOURCE_PRICE_REFRESH_RATE_MILLIS: Tick = 2 * HOURS;

#[derive(
    Debug, Display, Clone, Copy, Serialize_repr, Deserialize_repr, PartialEq, Default, EnumIter,
)]
#[repr(u8)]
pub enum PlanetType {
    BlackHole,
    Sol,
    #[default]
    Earth,
    Lava,
    Ice,
    Gas,
    Islands,
    Ring,
    Rocky,
    Wet,
    Asteroid,
}

#[derive(Debug, Clone, Copy, PartialEq, Hash, EnumIter)]
#[repr(u8)]
pub enum AsteroidUpgradeTarget {
    TeleportationPad,
    TortugaSpacePort,
}

// FIXME: remove in two releases
impl Serialize for AsteroidUpgradeTarget {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Always serialize as u8 (new format)
        serializer.serialize_u8(*self as u8)
    }
}

impl<'de> Deserialize<'de> for AsteroidUpgradeTarget {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct EnumVisitor;

        impl<'de> Visitor<'de> for EnumVisitor {
            type Value = AsteroidUpgradeTarget;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a u8 or string enum variant")
            }

            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                match v {
                    0 => Ok(AsteroidUpgradeTarget::TeleportationPad),
                    1 => Ok(AsteroidUpgradeTarget::TortugaSpacePort),
                    _ => Err(E::custom(format!("unknown enum value {}", v))),
                }
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                match v {
                    "TeleportationPad" => Ok(AsteroidUpgradeTarget::TeleportationPad),
                    "TortugaSpacePort" => Ok(AsteroidUpgradeTarget::TortugaSpacePort),
                    _ => Err(E::custom(format!("unknown enum variant {}", v))),
                }
            }
        }

        deserializer.deserialize_any(EnumVisitor)
    }
}

impl AsteroidUpgradeTarget {
    pub fn name(&self) -> &str {
        match self {
            Self::TeleportationPad => "Teleportation pad",
            Self::TortugaSpacePort => "Tortuga space port",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Hash)]
pub struct AsteroidUpgrade {
    pub target: AsteroidUpgradeTarget,
    pub started: Tick,
    pub duration: Tick,
}

impl AsteroidUpgrade {
    pub const BASE_DURATION: Tick = 8 * HOURS;

    pub fn new(target: AsteroidUpgradeTarget, bonus: f32) -> Self {
        let duration = (Self::BASE_DURATION as f32 / bonus) as Tick;
        Self {
            started: Tick::now(),
            duration,
            target,
        }
    }

    pub fn description(&self) -> String {
        match self.target {
            AsteroidUpgradeTarget::TeleportationPad => "Building teleportation pad".to_string(),
            AsteroidUpgradeTarget::TortugaSpacePort => {
                "Building the Tortuga space port".to_string()
            }
        }
    }
    pub fn cost(&self) -> Vec<(Resource, u32)> {
        match self.target {
            AsteroidUpgradeTarget::TeleportationPad => {
                vec![(Resource::SCRAPS, 125), (Resource::GOLD, 25)]
            }
            AsteroidUpgradeTarget::TortugaSpacePort => {
                vec![(Resource::SCRAPS, 250), (Resource::GOLD, 1000)]
            }
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
pub struct Planet {
    pub id: PlanetId,
    pub peer_id: Option<PeerId>,
    pub version: u64,
    pub name: String,
    pub populations: HashMap<Population, u32>,
    pub resources: ResourceMap,
    pub filename: String,
    pub rotation_period: usize,
    pub revolution_period: usize,
    pub gravity: usize,
    pub asteroid_probability: f64,
    pub planet_type: PlanetType,
    pub satellites: Vec<PlanetId>,
    pub satellite_of: Option<PlanetId>,
    pub axis: (f32, f32),
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub team_ids: Vec<TeamId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub custom_radio_stream: Option<String>,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub pending_upgrade: Option<AsteroidUpgrade>,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub upgrades: Vec<AsteroidUpgradeTarget>,
}

impl Planet {
    fn price_delta(&self, merchant_bonus: f32) -> f32 {
        (TRADE_DELTA_BUY_SELL + 1.0 / (10.0 + self.total_population() as f32)) / merchant_bonus
    }
    fn resource_price(&self, resource: Resource) -> f32 {
        // Resource price follows a hyperbolic tangent curve
        let relative_amount = (self.resources.value(&resource) as f32).bound() / MAX_SKILL;
        let amount_modifier =
            relative_amount / TRADE_DELTA_SCARCITY + (1.0 - relative_amount) * TRADE_DELTA_SCARCITY;

        let random_fluctuation =
            0.2 * ((Tick::now() / RESOURCE_PRICE_REFRESH_RATE_MILLIS) as f32).sin();

        let mut s = DefaultHasher::new();
        self.name.hash(&mut s);
        let planet_fluctation = 0.05 * (s.finish() as f32).sin();

        let price = resource.base_price()
            * amount_modifier
            * (1.0 + random_fluctuation + planet_fluctation);
        log::debug!(
            "Calculated price for {} (amount={}): {} * {} = {}",
            resource,
            relative_amount,
            resource.base_price(),
            amount_modifier,
            price
        );

        price
    }

    pub fn resource_buy_price(&self, resource: Resource, merchant_bonus: f32) -> u32 {
        let price = self.resource_price(resource);
        let delta = self.price_delta(merchant_bonus);
        let buy_price = price * (1.0 + delta);

        log::debug!(
            "Buy price: {} * {} = {}",
            price,
            delta,
            (buy_price as u32).max(1)
        );
        (buy_price as u32).max(1)
    }

    pub fn resource_sell_price(&self, resource: Resource, merchant_bonus: f32) -> u32 {
        let price = self.resource_price(resource);
        let delta = self.price_delta(merchant_bonus);
        let sell_price = price * (1.0 - delta);

        log::debug!(
            "Sell price: {} * {} = {}",
            price,
            delta,
            (sell_price as u32).max(1)
        );
        sell_price as u32
    }

    pub fn total_population(&self) -> u32 {
        self.populations.values().sum()
    }

    pub fn random_population(&self, rng: &mut ChaCha8Rng) -> Option<Population> {
        let weights = self
            .populations
            .iter()
            .map(|(pop, n)| (*pop, *n))
            .collect::<Vec<(Population, u32)>>();

        let dist = WeightedIndex::new(weights.iter().map(|(_, w)| w)).ok()?;
        Some(weights[dist.sample(rng)].0)
    }

    pub fn asteroid(name: String, filename: String, satellite_of: PlanetId) -> Self {
        let rng = &mut ChaCha8Rng::from_os_rng();
        let revolution_period: usize = [120, 180, 360]
            .choose(rng)
            .copied()
            .expect("Should select a random value");

        Self {
            id: PlanetId::new_v4(),
            peer_id: None,
            version: 0,
            name,
            populations: HashMap::new(),
            resources: HashMap::new(),
            filename,
            rotation_period: rng.random_range(1..24),
            revolution_period,
            gravity: rng.random_range(1..4),
            asteroid_probability: 0.0,
            planet_type: PlanetType::Asteroid,
            satellites: vec![],
            satellite_of: Some(satellite_of),
            axis: (rng.random_range(10.0..60.0), rng.random_range(10.0..60.0)),
            team_ids: vec![],
            //TODO: add option to customize asteroid radio stream
            custom_radio_stream: None,
            pending_upgrade: None,
            upgrades: vec![],
        }
    }
}
