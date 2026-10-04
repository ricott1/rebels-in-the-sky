use crate::{core::spaceship::Spaceship, types::ResourceMap};

#[derive(Debug, Clone, PartialEq)]
pub struct ShipLoadout {
    pub spaceship: Spaceship,
    pub resources: ResourceMap,
    pub speed_bonus: f32,
    pub weapons_bonus: f32,
    pub fuel: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlayerOutcome {
    pub resources: ResourceMap,
    pub durability: u32,
}

#[derive(Debug, Clone, PartialEq, Default)]
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
