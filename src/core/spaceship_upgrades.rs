use super::{constants::*, resources::Resource};
use crate::{
    core::{spaceship::*, spaceship_components::*, types::UpgradeableElement},
    types::Tick,
};
use serde::{Deserialize, Serialize};
use std::{fmt::Display, hash::Hash};
use strum_macros::EnumIter;

const REPAIR_BASE_COST: u32 = 425;
const REPAIR_BASE_DURATION: Tick = 12 * MINUTES;
const UPGRADE_BASE_DURATION: Tick = 3 * HOURS;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Hash, EnumIter)]
pub enum SpaceshipUpgradeTarget {
    Hull { component: Hull },
    ChargeUnit { component: ChargeUnit },
    Engine { component: Engine },
    Shooter { component: Shooter },
    Storage { component: Storage },
    Shield { component: Shield },
    Repairs { amount: u32 },
}

impl Default for SpaceshipUpgradeTarget {
    fn default() -> Self {
        Self::Repairs { amount: 0 }
    }
}

impl Display for SpaceshipUpgradeTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ChargeUnit { .. } => write!(f, "Charge unit"),
            Self::Hull { .. } => write!(f, "Hull"),
            Self::Engine { .. } => write!(f, "Engine"),
            Self::Shield { .. } => write!(f, "Shield"),
            Self::Storage { .. } => write!(f, "Storage"),
            Self::Shooter { .. } => write!(f, "Shooter"),
            Self::Repairs { .. } => write!(f, "Repairs"),
        }
    }
}

impl UpgradeableElement for ChargeUnit {
    fn next(&self) -> Option<Self> {
        match self {
            Self::Small => Some(Self::Medium),
            Self::Medium => Some(Self::Large),
            Self::Large => None,
        }
    }
    fn previous(&self) -> Option<Self> {
        match self {
            Self::Small => None,
            Self::Medium => Some(Self::Small),
            Self::Large => Some(Self::Medium),
        }
    }

    fn upgrade_cost(&self) -> Vec<(Resource, u32)> {
        let satoshi_cost = match self {
            Self::Small => 0,
            Self::Medium => 9_750,
            Self::Large => 19_400,
        };

        let scraps_cost = match self {
            Self::Small => 0,
            Self::Medium => 85,
            Self::Large => 115,
        };

        let gold_cost = match self {
            Self::Small => 0,
            Self::Medium => 1,
            Self::Large => 3,
        };

        vec![
            (Resource::SATOSHI, satoshi_cost),
            (Resource::SCRAPS, scraps_cost),
            (Resource::GOLD, gold_cost),
        ]
    }

    fn upgrade_duration(&self) -> Tick {
        4 * UPGRADE_BASE_DURATION
    }

    fn description(&self) -> &str {
        "The charge unit powers the spaceship shooters and shield during space adventures."
    }
}

impl UpgradeableElement for Hull {
    fn next(&self) -> Option<Self> {
        match self {
            Self::ShuttleSmall => Some(Self::ShuttleStandard),
            Self::ShuttleStandard => Some(Self::ShuttleLarge),
            Self::ShuttleLarge => None,
            Self::PincherStandard => Some(Self::PincherLarge),
            Self::PincherLarge => None,
            Self::JesterStandard => None,
        }
    }
    fn previous(&self) -> Option<Self> {
        match self {
            Self::ShuttleSmall => None,
            Self::ShuttleStandard => Some(Self::ShuttleSmall),
            Self::ShuttleLarge => Some(Self::ShuttleStandard),
            Self::PincherStandard => None,
            Self::PincherLarge => Some(Self::PincherStandard),
            Self::JesterStandard => None,
        }
    }

    fn upgrade_cost(&self) -> Vec<(Resource, u32)> {
        let satoshi_cost = match self {
            Self::ShuttleSmall => 0,
            Self::ShuttleStandard => 15000,
            Self::ShuttleLarge => 25000,
            Self::PincherStandard => 0,
            Self::PincherLarge => 27000,
            Self::JesterStandard => 0,
        };

        let scraps_cost = match self {
            Self::ShuttleSmall => 0,
            Self::ShuttleStandard => 280,
            Self::ShuttleLarge => 380,
            Self::PincherStandard => 0,
            Self::PincherLarge => 310,
            Self::JesterStandard => 0,
        };

        // Final upgrade has a cost in gold
        let gold_cost = match self {
            Self::ShuttleLarge => 5,
            Self::PincherLarge => 6,
            _ => 0,
        };

        vec![
            (Resource::SATOSHI, satoshi_cost),
            (Resource::SCRAPS, scraps_cost),
            (Resource::GOLD, gold_cost),
        ]
    }

    fn upgrade_duration(&self) -> Tick {
        8 * UPGRADE_BASE_DURATION
    }

    fn description(&self) -> &str {
        "The hull determines the spaceship durability and the maximum number of pirates in the crew."
    }
}

impl UpgradeableElement for Engine {
    fn next(&self) -> Option<Self> {
        match self {
            Self::ShuttleSingle => Some(Self::ShuttleDouble),
            Self::ShuttleDouble => Some(Self::ShuttleTriple),
            Self::ShuttleTriple => None,
            Self::PincherSingle => Some(Self::PincherDouble),
            Self::PincherDouble => Some(Self::PincherTriple),
            Self::PincherTriple => None,
            Self::JesterDouble => Some(Self::JesterQuadruple),
            Self::JesterQuadruple => None,
        }
    }
    fn previous(&self) -> Option<Self> {
        match self {
            Self::ShuttleSingle => None,
            Self::ShuttleDouble => Some(Self::ShuttleSingle),
            Self::ShuttleTriple => Some(Self::ShuttleDouble),
            Self::PincherSingle => None,
            Self::PincherDouble => Some(Self::PincherSingle),
            Self::PincherTriple => Some(Self::PincherDouble),
            Self::JesterDouble => None,
            Self::JesterQuadruple => Some(Self::JesterDouble),
        }
    }

    fn upgrade_cost(&self) -> Vec<(Resource, u32)> {
        let satoshi_cost = match self {
            Self::ShuttleSingle => 0,
            Self::ShuttleDouble => 5_000,
            Self::ShuttleTriple => 8_000,
            Self::PincherSingle => 0,
            Self::PincherDouble => 5_000,
            Self::PincherTriple => 11_000,
            Self::JesterDouble => 0,
            Self::JesterQuadruple => 10_000,
        };

        let scraps_cost = match self {
            Self::ShuttleSingle => 0,
            Self::ShuttleDouble => 120,
            Self::ShuttleTriple => 200,
            Self::PincherSingle => 0,
            Self::PincherDouble => 160,
            Self::PincherTriple => 210,
            Self::JesterDouble => 0,
            Self::JesterQuadruple => 220,
        };

        // Final upgrade has a cost in rum
        let rum_cost = match self {
            Self::ShuttleTriple | Self::PincherTriple | Self::JesterQuadruple => 100,
            _ => 0,
        };

        vec![
            (Resource::SATOSHI, satoshi_cost),
            (Resource::SCRAPS, scraps_cost),
            (Resource::RUM, rum_cost),
        ]
    }

    fn upgrade_duration(&self) -> Tick {
        6 * UPGRADE_BASE_DURATION
    }

    fn description(&self) -> &str {
        "The engine determines the spaceship speed and acceleration."
    }
}

impl UpgradeableElement for Shield {
    fn next(&self) -> Option<Self> {
        match self {
            Self::None => Some(Self::Small),
            Self::Small => Some(Self::Medium),
            Self::Medium => None,
        }
    }
    fn previous(&self) -> Option<Self> {
        match self {
            Self::None => None,
            Self::Small => Some(Self::None),
            Self::Medium => Some(Self::Small),
        }
    }

    fn upgrade_cost(&self) -> Vec<(Resource, u32)> {
        let satoshi_cost = match self {
            Self::None => 0,
            Self::Small => 8_800,
            Self::Medium => 16_000,
        };

        let scraps_cost = match self {
            Self::None => 0,
            Self::Small => 25,
            Self::Medium => 60,
        };

        let rum_cost = match self {
            Self::None => 0,
            Self::Small => 90,
            Self::Medium => 205,
        };

        vec![
            (Resource::SATOSHI, satoshi_cost),
            (Resource::SCRAPS, scraps_cost),
            (Resource::RUM, rum_cost),
        ]
    }

    fn upgrade_duration(&self) -> Tick {
        4 * UPGRADE_BASE_DURATION
    }

    fn description(&self) -> &str {
        "The shield protects the spaceship from asteroid, projectiles, and more during space adventures."
    }
}

impl UpgradeableElement for Shooter {
    fn next(&self) -> Option<Self> {
        match self {
            Self::ShuttleNone => Some(Self::ShuttleSingle),
            Self::ShuttleSingle => Some(Self::ShuttleTriple),
            Self::ShuttleTriple => None,
            Self::PincherNone => Some(Self::PincherDouble),
            Self::PincherDouble => Some(Self::PincherQuadruple),
            Self::PincherQuadruple => None,
            Self::JesterNone => Some(Self::JesterDouble),
            Self::JesterDouble => Some(Self::JesterQuadruple),
            Self::JesterQuadruple => None,
        }
    }
    fn previous(&self) -> Option<Self> {
        match self {
            Self::ShuttleNone => None,
            Self::ShuttleSingle => Some(Self::ShuttleNone),
            Self::ShuttleTriple => Some(Self::ShuttleSingle),
            Self::PincherNone => None,
            Self::PincherDouble => Some(Self::PincherNone),
            Self::PincherQuadruple => Some(Self::PincherDouble),
            Self::JesterNone => None,
            Self::JesterDouble => Some(Self::JesterNone),
            Self::JesterQuadruple => Some(Self::JesterDouble),
        }
    }

    fn upgrade_cost(&self) -> Vec<(Resource, u32)> {
        let satoshi_cost = match self {
            Self::ShuttleNone => 0,
            Self::ShuttleSingle => 5_000,
            Self::ShuttleTriple => 13_000,
            Self::PincherNone => 0,
            Self::PincherDouble => 11_000,
            Self::PincherQuadruple => 15_000,
            Self::JesterNone => 0,
            Self::JesterDouble => 12_000,
            Self::JesterQuadruple => 18_000,
        };

        let scraps_cost = match self {
            Self::ShuttleNone => 0,
            Self::ShuttleSingle => 120,
            Self::ShuttleTriple => 190,
            Self::PincherNone => 0,
            Self::PincherDouble => 150,
            Self::PincherQuadruple => 240,
            Self::JesterNone => 0,
            Self::JesterDouble => 155,
            Self::JesterQuadruple => 260,
        };

        let rum_cost = scraps_cost / 12;

        vec![
            (Resource::SATOSHI, satoshi_cost),
            (Resource::SCRAPS, scraps_cost),
            (Resource::RUM, rum_cost),
        ]
    }

    fn upgrade_duration(&self) -> Tick {
        4 * UPGRADE_BASE_DURATION
    }

    fn description(&self) -> &str {
        "The shooters, well, shoot..."
    }
}

impl UpgradeableElement for Storage {
    fn next(&self) -> Option<Self> {
        match self {
            Self::ShuttleNone => Some(Self::ShuttleSingle),
            Self::ShuttleSingle => Some(Self::ShuttleDouble),
            Self::ShuttleDouble => None,
            Self::PincherNone => Some(Self::PincherSingle),
            Self::PincherSingle => None,
            Self::JesterNone => Some(Self::JesterSingle),
            Self::JesterSingle => None,
        }
    }

    fn previous(&self) -> Option<Self> {
        match self {
            Self::ShuttleNone => None,
            Self::ShuttleSingle => Some(Self::ShuttleNone),
            Self::ShuttleDouble => Some(Self::ShuttleSingle),
            Self::PincherNone => None,
            Self::PincherSingle => Some(Self::PincherNone),
            Self::JesterNone => None,
            Self::JesterSingle => Some(Self::JesterNone),
        }
    }

    fn upgrade_cost(&self) -> Vec<(Resource, u32)> {
        let satoshi_cost = match self {
            Self::ShuttleNone => 0,
            Self::ShuttleSingle => 5_000,
            Self::ShuttleDouble => 11_000,
            Self::PincherNone => 0,
            Self::PincherSingle => 7_000,
            Self::JesterNone => 0,
            Self::JesterSingle => 18_000,
        };

        let scraps_cost = match self {
            Self::ShuttleNone => 0,
            Self::ShuttleSingle => 110,
            Self::ShuttleDouble => 210,
            Self::PincherNone => 0,
            Self::PincherSingle => 90,
            Self::JesterNone => 0,
            Self::JesterSingle => 80,
        };

        let cost = vec![
            (Resource::SATOSHI, satoshi_cost),
            (Resource::SCRAPS, scraps_cost),
        ];

        cost
    }

    fn upgrade_duration(&self) -> Tick {
        3 * UPGRADE_BASE_DURATION
    }

    fn description(&self) -> &str {
        "The storage unit increases the maximum amount of resources and fuel that the spaceship can carry."
    }
}

impl UpgradeableElement for SpaceshipUpgradeTarget {
    fn next(&self) -> Option<Self> {
        match self {
            Self::ChargeUnit { component } => component
                .next()
                .map(|comp| Self::ChargeUnit { component: comp }),
            Self::Hull { component } => component.next().map(|comp| Self::Hull { component: comp }),
            Self::Engine { component } => component
                .next()
                .map(|comp| Self::Engine { component: comp }),
            Self::Shield { component } => component
                .next()
                .map(|comp| Self::Shield { component: comp }),
            Self::Storage { component } => component
                .next()
                .map(|comp| Self::Storage { component: comp }),
            Self::Shooter { component } => component
                .next()
                .map(|comp| Self::Shooter { component: comp }),
            Self::Repairs { .. } => None,
        }
    }

    fn previous(&self) -> Option<Self> {
        match self {
            Self::ChargeUnit { component } => component
                .previous()
                .map(|comp| Self::ChargeUnit { component: comp }),
            Self::Hull { component } => component
                .previous()
                .map(|comp| Self::Hull { component: comp }),
            Self::Engine { component } => component
                .previous()
                .map(|comp| Self::Engine { component: comp }),
            Self::Shield { component } => component
                .previous()
                .map(|comp| Self::Shield { component: comp }),
            Self::Storage { component } => component
                .previous()
                .map(|comp| Self::Storage { component: comp }),
            Self::Shooter { component } => component
                .previous()
                .map(|comp| Self::Shooter { component: comp }),
            Self::Repairs { .. } => None,
        }
    }

    fn upgrade_cost(&self) -> Vec<(Resource, u32)> {
        match self {
            Self::ChargeUnit { component } => component.upgrade_cost(),
            Self::Hull { component } => component.upgrade_cost(),
            Self::Engine { component } => component.upgrade_cost(),
            Self::Shield { component } => component.upgrade_cost(),
            Self::Storage { component } => component.upgrade_cost(),
            Self::Shooter { component } => component.upgrade_cost(),
            Self::Repairs { amount } => {
                vec![
                    (Resource::SATOSHI, amount * REPAIR_BASE_COST),
                    (Resource::SCRAPS, *amount * 3),
                ]
            }
        }
    }

    fn upgrade_duration(&self) -> Tick {
        match self {
            Self::ChargeUnit { component } => component.upgrade_duration(),
            Self::Hull { component } => component.upgrade_duration(),
            Self::Engine { component } => component.upgrade_duration(),
            Self::Shield { component } => component.upgrade_duration(),
            Self::Storage { component } => component.upgrade_duration(),
            Self::Shooter { component } => component.upgrade_duration(),
            Self::Repairs { amount } => REPAIR_BASE_DURATION * *amount as u64,
        }
    }

    fn description(&self) -> &str {
        match self {
            Self::ChargeUnit { component } => component.description(),
            Self::Hull { component } => component.description(),
            Self::Engine { component } => component.description(),
            Self::Shield { component } => component.description(),
            Self::Storage { component } => component.description(),
            Self::Shooter { component } => component.description(),
            Self::Repairs { .. } => "Repairing the spaceship restores its full durability.",
        }
    }
}

pub fn available_upgrade_targets(spaceship: &Spaceship) -> [Option<SpaceshipUpgradeTarget>; 7] {
    [
        spaceship
            .hull
            .next()
            .map(|next| SpaceshipUpgradeTarget::Hull { component: next }),
        spaceship
            .charge_unit
            .next()
            .map(|next| SpaceshipUpgradeTarget::ChargeUnit { component: next }),
        spaceship
            .engine
            .next()
            .map(|next| SpaceshipUpgradeTarget::Engine { component: next }),
        spaceship
            .shooter
            .next()
            .map(|next| SpaceshipUpgradeTarget::Shooter { component: next }),
        spaceship
            .storage
            .next()
            .map(|next| SpaceshipUpgradeTarget::Storage { component: next }),
        spaceship
            .shield
            .next()
            .map(|next| SpaceshipUpgradeTarget::Shield { component: next }),
        if spaceship.can_be_repaired() {
            Some(SpaceshipUpgradeTarget::Repairs {
                amount: spaceship.max_durability() - spaceship.current_durability(),
            })
        } else {
            None
        },
    ]
}
