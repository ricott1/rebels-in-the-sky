use super::types::Population;
use crate::types::{PlanetId, TeamId};
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
    pub version: u64,
    pub name: String,
    pub populations: Vec<(Population, u32)>,
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
}
