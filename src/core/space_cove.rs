use super::resources::Resource;
use super::utils::is_default;
use crate::core::{Population, Upgrade, UpgradeableElement, DAYS, MAX_TAVERN_POPULATION, WEEKS};
use crate::types::{PlanetId, PlayerId, ResourceMap, StorableResourceMap, Tick};
use rand::prelude::Distribution;
use rand::RngExt;
use rand_chacha::ChaCha8Rng;
use rand_distr::weighted::WeightedIndex;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use std::collections::{HashMap, HashSet};
use std::fmt::{self, Display};
use strum::Display;
use strum_macros::EnumIter;

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct DrinkingCompetition {
    participants: [PlayerId; 2],
}
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct Tavern {
    // The tavern increases the cove asteroid population,
    // which in turns means that tick_free_pirates populate the asteroid with free pirates.
    pub upkeep_cost: ResourceMap,
    pub drinking_competition: Option<DrinkingCompetition>,
}

impl Tavern {
    pub fn refresh_populations(
        &self,
        parent_planet_populations: &HashMap<Population, u32>,
        available_rum: u32,
        rng: &mut ChaCha8Rng,
    ) -> HashMap<Population, u32> {
        const POPULATION_WEIGHT_CAP: u32 = 1;
        // Rum/day at which the tavern fills all MAX slots ~50% of the time.
        const RUM_FOR_HALF_MAX: f64 = 5.0;

        // Per-slot fill chance, tuned so that available_rum == RUM_FOR_HALF_MAX gives
        // a full house (MAX pirates) 50% of the time, and double that guarantees MAX.
        // Only the rum actually consumed is considered.
        let fill_chance = (0.5_f64.powf(1.0 / MAX_TAVERN_POPULATION as f64) * available_rum as f64
            / RUM_FOR_HALF_MAX)
            .clamp(0.0, 1.0);

        let weights: Vec<(Population, u32)> = parent_planet_populations
            .iter()
            .map(|(&pop, &value)| (pop, value.min(POPULATION_WEIGHT_CAP)))
            .filter(|(_, weight)| *weight > 0)
            .collect();

        let mut populations = HashMap::default();
        if let Ok(distribution) = WeightedIndex::new(weights.iter().map(|(_, weight)| weight)) {
            for _ in 0..MAX_TAVERN_POPULATION {
                if rng.random_range(0.0..1.0) < fill_chance {
                    let population = weights[distribution.sample(rng)].0;
                    *populations.entry(population).or_insert(0) += 1;
                }
            }
        }

        populations
    }
}

#[derive(Debug, Display, Clone, Copy, Serialize_repr, Deserialize_repr, PartialEq)]
#[repr(u8)]
pub enum SpaceCoveState {
    UnderConstruction,
    Ready,
}

fn default_upgrades() -> HashSet<SpaceCoveUpgradeTarget> {
    HashSet::from([SpaceCoveUpgradeTarget::TeleportationPad])
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct SpaceCove {
    state: SpaceCoveState,
    pub planet_id: PlanetId,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub pending_upgrade: Option<Upgrade<SpaceCoveUpgradeTarget>>,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default = "default_upgrades")]
    pub upgrades: HashSet<SpaceCoveUpgradeTarget>,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub tavern: Option<Tavern>,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub resources: ResourceMap,
}

impl SpaceCove {
    pub fn under_construction(planet_id: PlanetId) -> Self {
        Self {
            state: SpaceCoveState::UnderConstruction,
            planet_id,
            pending_upgrade: None,
            upgrades: HashSet::default(),
            tavern: None,
            resources: ResourceMap::default(),
        }
    }

    pub fn finish_contruction(&mut self) {
        self.state = SpaceCoveState::Ready;
        self.upgrades
            .insert(SpaceCoveUpgradeTarget::TeleportationPad);
    }

    pub fn is_ready(&self) -> bool {
        self.state == SpaceCoveState::Ready
    }

    pub fn has_stadium(&self) -> bool {
        self.upgrades.contains(&SpaceCoveUpgradeTarget::Stadium)
    }

    pub fn can_pay_tavern_upkeep(&self) -> bool {
        let rum_per_day = self
            .tavern
            .as_ref()
            .and_then(|tavern| tavern.upkeep_cost.get(&Resource::RUM).copied())
            .unwrap_or(0);
        self.resources.value(&Resource::RUM) >= rum_per_day
    }

    /// Draws the tavern's daily rum from the cove store, returning how much was
    /// actually available (less than rum-per-day when the store runs short).
    pub fn consume_daily_rum(&mut self) -> u32 {
        let rum_per_day = self
            .tavern
            .as_ref()
            .and_then(|tavern| tavern.upkeep_cost.get(&Resource::RUM).copied())
            .unwrap_or(0);
        let effective_rum = rum_per_day.min(self.resources.value(&Resource::RUM));
        self.resources.saturating_sub(Resource::RUM, effective_rum);

        effective_rum
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum SpaceCoveUpgradeTarget {
    Market,
    Stadium,
    Tavern,
    TeleportationPad, // NOTE: this exists also on the PlanetUpgradeTarget. We repeat it here for convenience
}

impl Display for SpaceCoveUpgradeTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TeleportationPad => write!(f, "Teleportation Pad"),
            Self::Market => write!(f, "Market"),
            Self::Stadium => write!(f, "Stadium"),
            Self::Tavern => write!(f, "Tavern"),
        }
    }
}

impl UpgradeableElement for SpaceCoveUpgradeTarget {
    fn next(&self) -> Option<Self> {
        None
    }

    fn previous(&self) -> Option<Self> {
        None
    }

    fn can_be_upgraded(&self) -> bool {
        true
    }

    fn upgrade_cost(&self) -> Vec<(Resource, u32)> {
        match self {
            Self::TeleportationPad => {
                vec![]
            }
            Self::Market => {
                vec![
                    (Resource::SATOSHI, 80_000),
                    (Resource::SCRAPS, 60),
                    (Resource::GOLD, 5),
                    (Resource::RUM, 25),
                ]
            }
            Self::Stadium => vec![
                (Resource::SATOSHI, 70_000),
                (Resource::SCRAPS, 220),
                (Resource::GOLD, 80),
            ],
            Self::Tavern => vec![
                (Resource::SATOSHI, 60_000),
                (Resource::SCRAPS, 100),
                (Resource::RUM, 150),
            ],
        }
    }

    fn upgrade_duration(&self) -> Tick {
        match self {
            Self::TeleportationPad => 0,
            Self::Market => 2 * DAYS,
            Self::Stadium => WEEKS,
            Self::Tavern => 3 * DAYS,
        }
    }

    fn description(&self) -> &str {
        match self {
            Self::TeleportationPad => "The teleportation pad allows to travel to the cove instantaneously for 1 Rum per pirate.",
            Self::Market => "A nice opportunity to trade your nice little goodies.",
            Self::Stadium => "Allows to organize tournaments in the space cove",
            Self::Tavern => "The best way to attract talented pirates to the cove",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_legacy_ready_cove_defaults_to_teleportation_pad() {
        let json = r#"{"state":1,"planet_id":"00000000-0000-0000-0000-000000000000"}"#;
        let cove: SpaceCove = serde_json::from_str(json).unwrap();
        assert!(cove.is_ready());
        assert!(cove
            .upgrades
            .contains(&SpaceCoveUpgradeTarget::TeleportationPad));
    }
}
