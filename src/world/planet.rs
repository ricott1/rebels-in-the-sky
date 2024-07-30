use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
    time::SystemTime,
};

use super::{resources::Resource, skill::MAX_SKILL, types::Population};
use crate::{
    types::{PlanetId, TeamId},
    world::skill::GameSkill,
};
use libp2p::PeerId;
use rand::{seq::SliceRandom, Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use rand_distr::Distribution;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use strum_macros::{Display, EnumIter};

const TRADE_DELTA_SCARCITY: f32 = 3.0;
const TRADE_DELTA_BUY_SELL: f32 = 0.12;
const RESOURCE_PRICE_REFRESH_RATE_SECS: u64 = 60 * 60 * 2;

#[derive(Debug, Display, Clone, Serialize_repr, Deserialize_repr, PartialEq, Default, EnumIter)]
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

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
pub struct Planet {
    pub id: PlanetId,
    pub peer_id: Option<PeerId>,
    pub version: u64,
    pub name: String,
    pub populations: HashMap<Population, u32>,
    pub resources: HashMap<Resource, u32>,
    pub filename: String,
    pub rotation_period: usize,
    pub revolution_period: usize,
    pub gravity: usize,
    pub asteroid_probability: f32,
    pub planet_type: PlanetType,
    pub satellites: Vec<PlanetId>,
    pub satellite_of: Option<PlanetId>,
    pub axis: (f32, f32),
    pub team_ids: Vec<TeamId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub custom_radio_stream: Option<String>,
}

impl Planet {
    fn price_delta(&self, merchant_bonus: f32) -> f32 {
        (TRADE_DELTA_BUY_SELL + 1.0 / (10.0 + self.total_population() as f32)) / merchant_bonus
    }
    fn resource_price(&self, resource: Resource) -> f32 {
        // Resource price follows a hyperbolic tangent curve
        let relative_amount =
            (self.resources.get(&resource).copied().unwrap_or_default() as f32).bound() / MAX_SKILL;
        let amount_modifier =
            relative_amount / TRADE_DELTA_SCARCITY + (1.0 - relative_amount) * TRADE_DELTA_SCARCITY;

        let unix_secs = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("Time flows bacwards")
            .as_secs();

        let random_fluctuation =
            0.2 * ((unix_secs / RESOURCE_PRICE_REFRESH_RATE_SECS) as f32).sin();

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
        self.populations.iter().map(|(_, p)| p).sum()
    }

    pub fn random_population(&self, rng: &mut ChaCha8Rng) -> Option<Population> {
        let weights = self
            .populations
            .iter()
            .map(|(pop, n)| (pop.clone(), n.clone()))
            .collect::<Vec<(Population, u32)>>();

        let dist = rand_distr::WeightedIndex::new(weights.iter().map(|(_, w)| w)).ok()?;
        Some(weights[dist.sample(rng)].0)
    }

    pub fn asteroid(id: PlanetId, name: String, satellite_of: PlanetId) -> Self {
        let rng = &mut ChaCha8Rng::from_entropy();
        let revolution_period = vec![120, 180, 360].choose(rng).unwrap().clone() as usize;

        Self {
            id,
            peer_id: None,
            version: 0,
            name,
            populations: HashMap::new(),
            resources: HashMap::new(),
            filename: format!("asteroid{}", rng.gen_range(0..=29)),
            rotation_period: rng.gen_range(1..24),
            revolution_period,
            gravity: rng.gen_range(1..4),
            asteroid_probability: 0.0,
            planet_type: PlanetType::Asteroid,
            satellites: vec![],
            satellite_of: Some(satellite_of),
            axis: (rng.gen_range(10.0..60.0), rng.gen_range(10.0..60.0)),
            team_ids: vec![],
            //FIXME: add option to customize asteroid radio stream
            custom_radio_stream: None,
        }
    }
}
