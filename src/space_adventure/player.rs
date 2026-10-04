use crate::{core::spaceship::Spaceship, types::ResourceMap};

#[derive(Debug, Clone, PartialEq)]
pub struct ShipLoadout {
    pub spaceship: Spaceship,
    pub resources: ResourceMap,
    pub speed_bonus: f32,
    pub weapons_bonus: f32,
    pub fuel: u32,
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
