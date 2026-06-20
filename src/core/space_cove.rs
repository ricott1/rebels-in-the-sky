use super::resources::Resource;
use super::utils::is_default;
use crate::core::{Population, Upgrade, UpgradeableElement, DAYS, WEEKS};
use crate::types::{PlanetId, PlayerId, ResourceMap, StorableResourceMap, Tick};
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
    pub populations: HashMap<Population, u32>,
    pub upkeep_cost: ResourceMap,
    pub stored_rum_amount: u32,
    pub drinking_competition: Option<DrinkingCompetition>,
}

impl Tavern {
    pub fn refresh_populations(&mut self, parent_planet_populations: &HashMap<Population, u32>) {
        const TAVERN_POPULATION_REDUCTION_FACTOR: f32 = 0.75;
        let max_total_population = (TAVERN_POPULATION_REDUCTION_FACTOR
            * parent_planet_populations.values().copied().sum::<u32>() as f32)
            as u32;
        let max_rum_amount = self
            .upkeep_cost
            .value(&Resource::RUM)
            .min(max_total_population);

        let populations = if max_rum_amount == 0 {
            HashMap::default()
        } else {
            parent_planet_populations
                .iter()
                .map(|(&pop, &value)| {
                    (
                        pop,
                        (TAVERN_POPULATION_REDUCTION_FACTOR * value as f32) as u32 * max_rum_amount
                            / max_total_population,
                    )
                })
                .collect()
        };

        self.populations = populations;
    }
}

#[derive(Debug, Display, Clone, Copy, Serialize_repr, Deserialize_repr, PartialEq)]
#[repr(u8)]
pub enum SpaceCoveState {
    UnderConstruction,
    Ready,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct SpaceCove {
    state: SpaceCoveState,
    pub planet_id: PlanetId,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub pending_upgrade: Option<Upgrade<SpaceCoveUpgradeTarget>>,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub upgrades: HashSet<SpaceCoveUpgradeTarget>,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub tavern: Option<Tavern>,
}

impl SpaceCove {
    pub fn under_construction(planet_id: PlanetId) -> Self {
        Self {
            state: SpaceCoveState::UnderConstruction,
            planet_id,
            pending_upgrade: None,
            upgrades: HashSet::default(),
            tavern: None,
        }
    }

    pub fn ready(planet_id: PlanetId) -> Self {
        Self {
            state: SpaceCoveState::Ready,
            planet_id,
            pending_upgrade: None,
            upgrades: HashSet::default(),
            tavern: None,
        }
    }

    pub fn finish_contruction(&mut self) {
        self.state = SpaceCoveState::Ready;
    }

    pub fn is_ready(&self) -> bool {
        self.state == SpaceCoveState::Ready
    }

    pub fn has_stadium(&self) -> bool {
        self.upgrades.contains(&SpaceCoveUpgradeTarget::Stadium)
    }

    pub fn upkeep(&mut self, team_resources: &mut ResourceMap) {
        if let Some(tavern_costs) = self.tavern.as_ref().map(|t| &t.upkeep_cost) {
            for (&resource, &amount) in tavern_costs.iter() {
                // FIXME: if there are not enough resources, we should fail the upkeep somehow
                team_resources.saturating_sub(resource, amount);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum SpaceCoveUpgradeTarget {
    Market,
    Stadium,
    Tavern,
}

impl Display for SpaceCoveUpgradeTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
            Self::Market => {
                vec![
                    (Resource::SATOSHI, 80_000),
                    (Resource::SCRAPS, 60),
                    (Resource::GOLD, 5),
                    (Resource::RUM, 100),
                ]
            }
            Self::Stadium => vec![(Resource::SCRAPS, 220), (Resource::GOLD, 80)],
            Self::Tavern => vec![
                (Resource::SATOSHI, 180_000),
                (Resource::SCRAPS, 120),
                (Resource::RUM, 50),
            ],
        }
    }

    fn upgrade_duration(&self) -> Tick {
        match self {
            Self::Market => 2 * DAYS,
            Self::Stadium => WEEKS,
            Self::Tavern => 3 * DAYS,
        }
    }

    fn description(&self) -> &str {
        match self {
            Self::Market => "A nice opportunity to trade your nice little goodies.",
            Self::Stadium => "Allows to organize tournaments in the space cove",
            Self::Tavern => "The best way to attract talented pirates to the cove",
        }
    }
}
