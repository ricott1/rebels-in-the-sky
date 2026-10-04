use crate::{
    core::{resources::Resource, spaceship::Spaceship},
    types::{ResourceMap, StorableResourceMap},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShipLoadout {
    #[serde(with = "super::wire::json_bytes")]
    pub spaceship: Spaceship,
    pub resources: ResourceMap,
    pub speed_bonus: f32,
    pub weapons_bonus: f32,
    pub fuel: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerOutcome {
    pub resources: ResourceMap,
    pub durability: u32,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct LocalPlayerView {
    pub durability: u32,
    pub max_durability: u32,
    pub shield_durability: u32,
    pub shield_max_durability: u32,
    pub charge: u32,
    pub max_charge: u32,
    pub is_recharging: bool,
    pub fuel: u32,
    pub fuel_capacity: u32,
    pub resources: ResourceMap,
    pub storage_capacity: u32,
}

impl LocalPlayerView {
    pub fn outcome(&self) -> PlayerOutcome {
        PlayerOutcome {
            resources: self.resources.clone(),
            durability: self.durability,
        }
    }
}

const MIN_BONUS: f32 = 1.0;
const MAX_BONUS: f32 = 2.0;

fn plausible_bonus(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(MIN_BONUS, MAX_BONUS)
    } else {
        MIN_BONUS
    }
}

impl ShipLoadout {
    pub fn sanitized(mut self) -> Self {
        self.speed_bonus = plausible_bonus(self.speed_bonus);
        self.weapons_bonus = plausible_bonus(self.weapons_bonus);
        self.fuel = self.fuel.min(self.spaceship.fuel_capacity());
        self
    }

    pub fn outcome(&self) -> PlayerOutcome {
        let mut resources = self.resources.clone();
        resources.insert(Resource::FUEL, self.fuel);
        PlayerOutcome {
            resources,
            durability: self.spaceship.current_durability(),
        }
    }
}

impl PlayerOutcome {
    pub fn capped(mut self, loadout: &ShipLoadout) -> Self {
        let fuel = self.resources.value(&Resource::FUEL).min(loadout.fuel);
        if self.resources.used_storage_capacity() > loadout.spaceship.storage_capacity() {
            self.resources = loadout.resources.clone();
        }
        self.resources.insert(Resource::FUEL, fuel);
        self.resources.insert(
            Resource::SATOSHI,
            loadout.resources.value(&Resource::SATOSHI),
        );
        self.durability = self.durability.min(loadout.spaceship.current_durability());
        self
    }
}

#[cfg(test)]
impl ShipLoadout {
    pub fn test_default() -> Self {
        Self {
            spaceship: crate::core::spaceship::SpaceshipPrefab::Ibarruri.spaceship(),
            resources: ResourceMap::new(),
            speed_bonus: 1.0,
            weapons_bonus: 1.0,
            fuel: 100,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitized_loadout_keeps_bonuses_and_fuel_plausible() {
        let mut loadout = ShipLoadout::test_default();
        loadout.speed_bonus = f32::NAN;
        loadout.weapons_bonus = 1e9;
        loadout.fuel = u32::MAX;
        let sanitized = loadout.clone().sanitized();
        assert_eq!(sanitized.speed_bonus, 1.0);
        assert_eq!(sanitized.weapons_bonus, 2.0);
        assert_eq!(sanitized.fuel, loadout.spaceship.fuel_capacity());

        let mut honest = ShipLoadout::test_default();
        honest.speed_bonus = 1.4;
        honest.fuel = honest.spaceship.fuel_capacity() / 2;
        assert_eq!(honest.clone().sanitized(), honest);
    }
}
