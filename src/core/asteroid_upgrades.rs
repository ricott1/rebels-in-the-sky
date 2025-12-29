use super::constants::HOURS;
use super::resources::Resource;
use crate::backcompat_repr_u8_enum;
use crate::core::{UpgradeableElement, DAYS};
use crate::types::Tick;
use std::fmt::{self, Display};
use strum_macros::EnumIter;

// FIXME: migrate to repr
backcompat_repr_u8_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter)]
    pub enum AsteroidUpgradeTarget {
        TeleportationPad,
        SpaceCove,
    }
}

impl Display for AsteroidUpgradeTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TeleportationPad => write!(f, "Teleportation pad"),
            Self::SpaceCove => write!(f, "Space cove"),
        }
    }
}

impl UpgradeableElement for AsteroidUpgradeTarget {
    fn next(&self) -> Option<Self> {
        match self {
            Self::TeleportationPad => Some(Self::SpaceCove),
            Self::SpaceCove => None,
        }
    }

    fn previous(&self) -> Option<Self> {
        match self {
            Self::TeleportationPad => None,
            Self::SpaceCove => Some(Self::TeleportationPad),
        }
    }

    fn can_be_upgraded(&self) -> bool {
        true
    }

    fn upgrade_cost(&self) -> Vec<(Resource, u32)> {
        match self {
            AsteroidUpgradeTarget::TeleportationPad => {
                vec![
                    (Resource::SCRAPS, 125),
                    (Resource::GOLD, 25),
                    (Resource::RUM, 10),
                ]
            }
            AsteroidUpgradeTarget::SpaceCove => {
                vec![
                    (Resource::SATOSHI, 150_000),
                    (Resource::SCRAPS, 250),
                    (Resource::GOLD, 777),
                ]
            }
        }
    }

    fn upgrade_duration(&self) -> Tick {
        match self {
            Self::TeleportationPad => 16 * HOURS,
            Self::SpaceCove => 3 * DAYS,
        }
    }

    fn description(&self) -> &str {
        match self {
            Self::TeleportationPad => "The teleportation pad allows to travel to a planet instantaneously for 1 Rum per pirate.",
            Self::SpaceCove => "The space cove is pretty cool...",
        }
    }
}
