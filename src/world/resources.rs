use image::Rgba;
use serde_repr::{Deserialize_repr, Serialize_repr};
use std::{fmt::Display, hash::Hash};
use strum::EnumIter;

#[derive(Debug, Serialize_repr, Deserialize_repr, Clone, Copy, PartialEq, Eq, Hash, EnumIter)]
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
    pub fn color(&self) -> Rgba<u8> {
        match self {
            Self::GOLD => Rgba([240, 230, 140, 255]),
            Self::SCRAPS => Rgba([192, 192, 192, 255]),
            Self::RUM => Rgba([114, 47, 55, 255]),
            Self::FUEL => Rgba([64, 224, 208, 255]),
            Self::SATOSHI => Rgba([255, 255, 255, 255]),
        }
    }
    pub fn base_price(&self) -> f32 {
        match self {
            Resource::SATOSHI => 1.0,
            Resource::GOLD => 1750.0,
            Resource::SCRAPS => 40.0,
            Resource::FUEL => 30.0,
            Resource::RUM => 125.0,
        }
    }

    pub fn to_storing_space(&self) -> u32 {
        match self {
            Resource::SATOSHI => 0,
            Resource::GOLD => 2,
            Resource::SCRAPS => 10,
            Resource::FUEL => 0, // Fuel is stored in the spaceship tank
            Resource::RUM => 1,
        }
    }
}
