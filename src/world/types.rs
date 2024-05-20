use std::fmt::Display;

use super::{
    constants::DEFAULT_PLANET_ID,
    player::{InfoStats, Player},
    skill::GameSkill,
};
use crate::{
    image::color_map::SkinColorMap,
    types::{PlanetId, Tick},
};
use rand::Rng;
use rand_chacha::ChaCha8Rng;
use rand_distr::{Distribution, WeightedIndex};
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use strum::Display;
use strum_macros::EnumIter;

pub type Size = u8;
pub const SIZE_LARGE_OFFSET: Size = 7;

pub fn size_from_info(info: &InfoStats) -> Size {
    let mut size = match info.height {
        x if x <= 184.0 => 0,
        x if x <= 190.0 => 1,
        x if x <= 196.0 => 2,
        x if x <= 202.0 => 3,
        x if x <= 208.0 => 4,
        x if x <= 214.0 => 5,
        _ => 6,
    };
    let bmi = info.weight as u32 * 10_000 / (info.height as u32 * info.height as u32);
    if bmi >= 27 || info.population == Population::Pupparoll {
        size += SIZE_LARGE_OFFSET;
    }
    size as Size
}

#[derive(
    Debug, Default, PartialEq, Eq, Clone, Copy, EnumIter, Serialize_repr, Deserialize_repr, Hash,
)]
#[repr(u8)]
pub enum Region {
    #[default]
    Italy,
    Germany,
    Spain,
    Greece,
    Nigeria,
    India,
    Euskadi,
    Kurdistan,
    Palestine,
    Japan,
}

impl From<u8> for Region {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Italy,
            1 => Self::Germany,
            2 => Self::Spain,
            3 => Self::Greece,
            4 => Self::Nigeria,
            5 => Self::India,
            6 => Self::Euskadi,
            7 => Self::Kurdistan,
            8 => Self::Palestine,
            9 => Self::Japan,
            _ => Self::Italy,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, EnumIter, Hash)]
pub enum Population {
    Human { region: Region },
    Yardalaim,
    Polpett,
    Juppa,
    Galdari,
    Pupparoll,
}

impl Serialize for Population {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // Serialize the enum as a u8.
        // The Human option is serialized as 1000.(region as u8)
        let value = match self {
            Self::Human { region } => 100 + *region as u8,
            Self::Yardalaim => 1,
            Self::Polpett => 2,
            Self::Juppa => 3,
            Self::Galdari => 4,
            Self::Pupparoll => 5,
        };
        value.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Population {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Deserialize the enum from a u8.
        // The Human option is deserialized as 1000.(region as u8)
        let value = u8::deserialize(deserializer)?;
        match value {
            1 => Ok(Self::Yardalaim),
            2 => Ok(Self::Polpett),
            3 => Ok(Self::Juppa),
            4 => Ok(Self::Galdari),
            5 => Ok(Self::Pupparoll),
            100..=109 => Ok(Self::Human {
                region: (value - 100).into(),
            }),
            _ => Err(serde::de::Error::custom("Invalid value for Population")),
        }
    }
}

impl Default for Population {
    fn default() -> Self {
        Self::Human {
            region: Region::Italy,
        }
    }
}

impl Display for Population {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Human { .. } => write!(f, "Human"),
            Self::Yardalaim => write!(f, "Yardalaim"),
            Self::Polpett => write!(f, "Polpett"),
            Self::Juppa => write!(f, "Juppa"),
            Self::Galdari => write!(f, "Galdari"),
            Self::Pupparoll => write!(f, "Pupparoll"),
        }
    }
}

impl Population {
    pub fn apply_skill_modifiers(&self, player: &mut Player) {
        match self {
            Population::Human { region } => match region {
                Region::Italy => player.info.height = (player.info.height * 1.02).min(225.0),
                Region::Germany => player.info.height = (player.info.height * 1.05).min(225.0),
                Region::Spain => player.info.height = (player.info.height * 1.00).min(225.0),
                Region::Greece => player.info.height = (player.info.height * 0.96).min(225.0),
                Region::Nigeria => player.info.height = (player.info.height * 1.05).min(225.0),
                Region::India => player.info.height = (player.info.height * 0.95).min(225.0),
                Region::Euskadi => player.info.height = (player.info.height * 0.98).min(225.0),
                Region::Kurdistan => player.info.height = (player.info.height * 0.96).min(225.0),
                Region::Palestine => player.info.height = (player.info.height * 0.96).min(225.0),
                Region::Japan => player.info.height = (player.info.height * 0.94).min(225.0),
            },
            Population::Yardalaim => {
                player.info.weight = (player.info.weight * 1.5).min(255.0);
                player.athleticism.strength = (player.athleticism.strength * 1.35).round().bound();
                player.info.age -= 5.1;
            }
            Population::Juppa => {
                player.info.height = (player.info.height * 1.09).min(225.0);
                player.info.age += 15.5;
            }
            Population::Galdari => {
                player.info.height = (player.info.height * 1.02).min(225.0);
                player.mental.charisma = (player.mental.charisma * 1.15).round().bound();
                player.mental.vision = (player.mental.vision * 1.5).round().bound();
                player.defense.steal = (player.defense.steal * 1.2).round().bound();
                player.info.age += 85.2;
            }
            Population::Pupparoll => {
                player.athleticism.quickness =
                    (player.athleticism.quickness * 1.05).round().bound();
                player.technical.rebounds = (player.technical.rebounds * 1.25).round().bound();
                player.mental.aggression = (player.mental.aggression * 0.85).round().bound();
                player.offense.dunk = (player.offense.dunk * 1.25).round().bound();
            }
            _ => {}
        }
    }

    // pub random_hair_map(&self, rng: &mut ChaCha8Rng) -> HairColorMap {};

    pub fn random_skin_map(&self, rng: &mut ChaCha8Rng) -> SkinColorMap {
        let weights = match self {
            Self::Human { region } => match region {
                &Region::Italy => vec![
                    (SkinColorMap::Pale, 0.1),
                    (SkinColorMap::Light, 0.2),
                    (SkinColorMap::Medium, 0.2),
                    (SkinColorMap::Dark, 0.1),
                ],
                &Region::Germany => vec![
                    (SkinColorMap::Pale, 0.2),
                    (SkinColorMap::Light, 0.2),
                    (SkinColorMap::Medium, 0.1),
                    (SkinColorMap::Dark, 0.05),
                ],
                &Region::Spain => vec![
                    (SkinColorMap::Pale, 0.15),
                    (SkinColorMap::Light, 0.1),
                    (SkinColorMap::Medium, 0.2),
                    (SkinColorMap::Dark, 0.15),
                ],
                &Region::Greece => vec![
                    (SkinColorMap::Pale, 0.1),
                    (SkinColorMap::Light, 0.2),
                    (SkinColorMap::Medium, 0.2),
                    (SkinColorMap::Dark, 0.1),
                ],
                &Region::Nigeria => vec![
                    (SkinColorMap::Pale, 0.025),
                    (SkinColorMap::Light, 0.05),
                    (SkinColorMap::Medium, 0.1),
                    (SkinColorMap::Dark, 0.3),
                ],
                &Region::India => vec![
                    (SkinColorMap::Pale, 0.05),
                    (SkinColorMap::Light, 0.1),
                    (SkinColorMap::Medium, 0.3),
                    (SkinColorMap::Dark, 0.2),
                ],
                &Region::Euskadi => vec![
                    (SkinColorMap::Pale, 0.2),
                    (SkinColorMap::Light, 0.2),
                    (SkinColorMap::Medium, 0.15),
                    (SkinColorMap::Dark, 0.05),
                ],
                &Region::Kurdistan => vec![
                    (SkinColorMap::Pale, 0.01),
                    (SkinColorMap::Light, 0.1),
                    (SkinColorMap::Medium, 0.5),
                    (SkinColorMap::Dark, 0.1),
                ],
                &Region::Palestine => vec![
                    (SkinColorMap::Light, 0.05),
                    (SkinColorMap::Medium, 0.5),
                    (SkinColorMap::Dark, 0.2),
                ],
                &Region::Japan => vec![
                    (SkinColorMap::Pale, 0.2),
                    (SkinColorMap::Light, 0.25),
                    (SkinColorMap::Medium, 0.1),
                    (SkinColorMap::Dark, 0.025),
                ],
            },
            Self::Yardalaim => vec![(SkinColorMap::LightGreen, 0.5), (SkinColorMap::Green, 0.5)],
            Self::Polpett => vec![(SkinColorMap::LightRed, 0.75), (SkinColorMap::Red, 0.25)],
            Self::Juppa => vec![
                (SkinColorMap::LightBlue, 0.45),
                (SkinColorMap::Blue, 0.45),
                (SkinColorMap::Purple, 0.1),
            ],
            Self::Galdari => vec![
                (SkinColorMap::LightYellow, 0.55),
                (SkinColorMap::Yellow, 0.43),
                (SkinColorMap::Orange, 0.02),
            ],
            Self::Pupparoll => vec![
                (SkinColorMap::LightGreen, 0.1),
                (SkinColorMap::Green, 0.1),
                (SkinColorMap::LightBlue, 0.1),
                (SkinColorMap::Blue, 0.1),
                (SkinColorMap::LightRed, 0.1),
                (SkinColorMap::Red, 0.1),
                (SkinColorMap::Orange, 0.2),
                (SkinColorMap::LightYellow, 0.1),
                (SkinColorMap::Yellow, 0.1),
                (SkinColorMap::Rainbow, 0.3),
                (SkinColorMap::Dark, 0.05),
                (SkinColorMap::Purple, 0.2),
            ],
        };

        let dist = WeightedIndex::new(weights.iter().map(|(_, w)| w)).unwrap();
        weights[dist.sample(rng)].0
    }
}

#[derive(Debug, Clone, Copy, Display, Serialize, Deserialize)]
pub enum PlayerLocation {
    WithTeam,
    OnPlanet { planet_id: PlanetId },
}

impl PartialEq for PlayerLocation {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::OnPlanet { planet_id: p1 }, Self::OnPlanet { planet_id: p2 }) => p1 == p2,
            _ => false,
        }
    }
}

impl Default for PlayerLocation {
    fn default() -> Self {
        Self::OnPlanet {
            planet_id: DEFAULT_PLANET_ID.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, Display, Serialize, Deserialize)]
pub enum TeamLocation {
    Travelling {
        from: PlanetId,
        to: PlanetId,
        started: Tick,
        duration: Tick,
        distance: u128,
    },
    OnPlanet {
        planet_id: PlanetId,
    },
    Exploring {
        around: PlanetId,
        started: Tick,
        duration: Tick,
    },
}

impl PartialEq for TeamLocation {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::OnPlanet { planet_id: p1 }, Self::OnPlanet { planet_id: p2 }) => p1 == p2,
            _ => false,
        }
    }
}

impl Default for TeamLocation {
    fn default() -> Self {
        Self::OnPlanet {
            planet_id: DEFAULT_PLANET_ID.clone(),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, Serialize_repr, Deserialize_repr, PartialEq)]
#[repr(u8)]
pub enum Pronoun {
    He,
    She,
    #[default]
    They,
}

impl Pronoun {
    pub fn random() -> Self {
        match rand::thread_rng().gen_range(0..=4) {
            0 | 1 => Self::He,
            2 | 3 => Self::She,
            _ => Self::They,
        }
    }

    pub fn as_subject(&self) -> &'static str {
        match self {
            Self::He => "He",
            Self::She => "She",
            Self::They => "They",
        }
    }

    pub fn as_object(&self) -> &'static str {
        match self {
            Self::He => "him",
            Self::She => "her",
            Self::They => "them",
        }
    }

    pub fn as_possessive(&self) -> &'static str {
        match self {
            Self::He => "his",
            Self::She => "her",
            Self::They => "their",
        }
    }
}

#[derive(
    Debug, Clone, Copy, Display, Serialize_repr, Deserialize_repr, PartialEq, EnumIter, Default,
)]
#[repr(u8)]
pub enum TrainingFocus {
    #[default]
    Athleticism,
    Offense,
    Defense,
    Technical,
    Mental,
}

impl TrainingFocus {
    pub fn is_focus(&self, skill_index: usize) -> bool {
        match self {
            Self::Athleticism => skill_index < 4,
            Self::Offense => skill_index >= 4 && skill_index < 8,
            Self::Defense => skill_index >= 8 && skill_index < 12,
            Self::Technical => skill_index >= 12 && skill_index < 16,
            Self::Mental => skill_index >= 16,
        }
    }

    pub fn next(&self) -> Option<Self> {
        match self {
            Self::Athleticism => Some(Self::Offense),
            Self::Offense => Some(Self::Defense),
            Self::Defense => Some(Self::Technical),
            Self::Technical => Some(Self::Mental),
            Self::Mental => None,
        }
    }
}

// tests
#[cfg(test)]

mod tests {
    use crate::types::IdSystem;

    #[test]
    fn test_team_location_eq() {
        use super::TeamLocation;
        use crate::types::PlanetId;
        let planet_id = PlanetId::new();
        let team_location = TeamLocation::OnPlanet { planet_id };
        let team_location2 = TeamLocation::OnPlanet { planet_id };
        assert_eq!(team_location, team_location2);
    }

    #[test]
    fn test_team_location_ne() {
        use super::TeamLocation;
        use crate::types::PlanetId;
        let planet_id = PlanetId::new();
        let planet_id2 = PlanetId::new();
        let team_location = TeamLocation::OnPlanet { planet_id };
        let team_location2 = TeamLocation::OnPlanet {
            planet_id: planet_id2,
        };
        let team_location3 = TeamLocation::Travelling {
            from: planet_id,
            to: planet_id2,
            started: 0,
            duration: 0,
            distance: 0,
        };
        assert_ne!(team_location, team_location2);
        assert_ne!(team_location, team_location3);
    }
}
