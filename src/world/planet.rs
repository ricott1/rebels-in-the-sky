use super::{resources::Resource, types::Population};
use crate::types::{PlanetId, TeamId};
use libp2p::PeerId;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use rand_distr::Distribution;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use strum_macros::{Display, EnumIter};

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
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
pub struct Planet {
    pub id: PlanetId,
    pub peer_id: Option<PeerId>,
    pub version: u64,
    pub name: String,
    pub populations: Vec<(Population, u32)>,
    pub base_resources: Vec<(Resource, u32)>,
    pub filename: String,
    pub rotation_period: usize,
    pub revolution_period: usize,
    pub gravity: usize,
    pub planet_type: PlanetType,
    pub satellites: Vec<PlanetId>,
    pub satellite_of: Option<PlanetId>,
    pub axis: (f32, f32),
    pub teams: Vec<TeamId>,
}

impl Planet {
    pub fn resource_price(&self, resource: Resource) -> u32 {
        // Resource price follows a hyperbolic tangent curve
        let base_amount = self
            .base_resources
            .iter()
            .find(|(r, _)| *r == resource)
            .map(|(_, p)| *p)
            .unwrap_or(0);

        let amount_modifier = 1.0 + (2.0 - base_amount as f32 / 20.0);

        let price = (resource.base_price()
            * amount_modifier
            * (resource.base_price() * amount_modifier).tanh()) as u32;

        price
    }

    pub fn resource_buy_price(&self, resource: Resource) -> u32 {
        let price = self.resource_price(resource);
        let delta = 10.0 + 100.0 / self.total_population() as f32;
        (price + (price as f32 / delta) as u32).max(1)
    }

    pub fn resource_sell_price(&self, resource: Resource) -> u32 {
        let price = self.resource_price(resource);
        let delta = 10.0 + 100.0 / self.total_population() as f32;
        price - (price as f32 / delta) as u32
    }

    pub fn total_population(&self) -> u32 {
        self.populations.iter().map(|(_, p)| p).sum()
    }

    pub fn random_population(&self, rng: &mut ChaCha8Rng) -> Option<Population> {
        let mut weights = self
            .populations
            .iter()
            .map(|(_, p)| *p as f32)
            .collect::<Vec<f32>>();
        let total = weights.iter().sum::<f32>();
        if total == 0.0 {
            return None;
        }
        weights.iter_mut().for_each(|w| *w /= total);
        let dist = rand_distr::WeightedIndex::new(&weights).ok()?;
        Some(self.populations[dist.sample(rng)].0)
    }

    pub fn asteroid(id: PlanetId, name: String, satellite_of: PlanetId) -> Self {
        let rng = &mut ChaCha8Rng::from_entropy();

        Self {
            id,
            peer_id: None,
            version: 0,
            name,
            populations: vec![],
            base_resources: vec![],
            filename: format!("asteroid{}", rng.gen_range(1..=12)),
            rotation_period: rng.gen_range(1..24),
            revolution_period: 365,
            gravity: rng.gen_range(1..4),
            planet_type: PlanetType::Rocky,
            satellites: vec![],
            satellite_of: Some(satellite_of),
            axis: (rng.gen_range(10.0..60.0), rng.gen_range(10.0..60.0)),
            teams: vec![],
        }
    }

    pub fn add_population(&mut self, population: Population, amount: u32) {
        if let Some((_, p)) = self
            .populations
            .iter_mut()
            .find(|(pop, _)| *pop == population)
        {
            *p += amount;
        } else {
            self.populations.push((population, amount));
        }
    }
}
