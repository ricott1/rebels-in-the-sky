use serde_repr::{Deserialize_repr, Serialize_repr};
use std::{collections::HashMap, fmt::Display, hash::Hash};

#[derive(Debug, Serialize_repr, Deserialize_repr, Clone, Copy, PartialEq, Hash, Eq)]
#[repr(u8)]
pub enum Resource {
    SATOSHI,
    GOLD,
    SCRAPS,
    FUEL,
    RUM,
}

impl Display for Resource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Resource::SATOSHI => write!(f, "Satoshi"),
            Resource::GOLD => write!(f, "Gold"),
            Resource::SCRAPS => write!(f, "Scraps"),
            Resource::FUEL => write!(f, "Fuel"),
            Resource::RUM => write!(f, "Rum"),
        }
    }
}

impl Resource {
    pub const SATOSHI_STORING_SPACE: u32 = 0;
    // Fuel is stored in the spaceship tank
    pub const FUEL_STORING_SPACE: u32 = 0;
    pub const GOLD_STORING_SPACE: u32 = 1;
    pub const RUM_STORING_SPACE: u32 = 1;
    pub const SCRAPS_STORING_SPACE: u32 = 10;

    pub fn base_price(&self) -> f32 {
        match self {
            Resource::SATOSHI => 1.0,
            Resource::GOLD => 2000.0,
            Resource::SCRAPS => 35.0,
            Resource::FUEL => 60.0,
            Resource::RUM => 125.0,
        }
    }

    pub fn to_storing_space(&self) -> u32 {
        match self {
            Resource::SATOSHI => Self::SATOSHI_STORING_SPACE,
            Resource::GOLD => Self::GOLD_STORING_SPACE,
            Resource::SCRAPS => Self::SCRAPS_STORING_SPACE,
            Resource::FUEL => Self::FUEL_STORING_SPACE,
            Resource::RUM => Self::RUM_STORING_SPACE,
        }
    }

    pub fn used_storage_capacity(resources: &HashMap<Resource, u32>) -> u32 {
        resources
            .iter()
            .map(|(k, v)| k.to_storing_space() * v)
            .sum()
    }
}
