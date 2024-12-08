use std::fmt::Display;

use super::{
    constants::{DEFAULT_PLANET_ID, KILOMETER},
    player::Player,
    skill::MAX_SKILL,
    world::World,
};
use crate::{
    image::color_map::SkinColorMap,
    types::{AppResult, PlanetId, TeamId, Tick},
};
use rand_chacha::ChaCha8Rng;
use rand_distr::{Distribution, WeightedIndex};
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use strum::{Display, FromRepr};
use strum_macros::EnumIter;

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
    Octopulp,
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
            Self::Octopulp => 6,
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
            6 => Ok(Self::Octopulp),
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
            Self::Octopulp => write!(f, "Octopulp"),
        }
    }
}

impl Population {
    pub fn relative_age(&self, age: f32) -> f32 {
        (age - self.min_age()) / (self.max_age() - self.min_age())
    }

    pub fn min_age(&self) -> f32 {
        match self {
            Self::Human { .. } => 16.0,
            Self::Yardalaim => 35.0,
            Self::Polpett => 14.0,
            Self::Juppa => 50.0,
            Self::Galdari => 80.0,
            Self::Pupparoll => 6.0,
            Self::Octopulp => 3.0,
        }
    }

    pub fn max_age(&self) -> f32 {
        match self {
            Self::Human { .. } => 65.0,
            Self::Yardalaim => 120.0,
            Self::Polpett => 41.0,
            Self::Juppa => 110.0,
            Self::Galdari => 270.0,
            Self::Pupparoll => 45.0,
            Self::Octopulp => 18.0,
        }
    }

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
            Self::Octopulp => vec![
                (SkinColorMap::LightPurple, 0.45),
                (SkinColorMap::Dark, 0.05),
                (SkinColorMap::LightBlue, 0.5),
                (SkinColorMap::Yellow, 0.02),
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

#[derive(Debug, Clone, Copy, Display, Serialize, Deserialize, PartialEq)]
pub enum KartoffelLocation {
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
        distance: KILOMETER,
    },
    OnPlanet {
        planet_id: PlanetId,
    },
    Exploring {
        around: PlanetId,
        started: Tick,
        duration: Tick,
    },
    OnSpaceAdventure {
        around: PlanetId,
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

#[derive(Debug, Default, Clone, Copy, Serialize_repr, Deserialize_repr, PartialEq, FromRepr)]
#[repr(u8)]
pub enum Pronoun {
    He,
    She,
    #[default]
    They,
}

impl Pronoun {
    pub fn random(rng: &mut ChaCha8Rng) -> Self {
        if let Some(dist) = WeightedIndex::new(&[8, 8, 1]).ok() {
            return Self::from_repr(dist.sample(rng) as u8).unwrap_or_default();
        }

        Self::default()
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

    pub fn to_be(&self) -> &'static str {
        match self {
            Self::He | Self::She => "is",
            Self::They => "are",
        }
    }

    pub fn to_have(&self) -> &'static str {
        match self {
            Self::He | Self::She => "has",
            Self::They => "have",
        }
    }
}

#[derive(
    Debug, Clone, Copy, Display, Serialize_repr, Deserialize_repr, PartialEq, EnumIter, Default,
)]
#[repr(u8)]
pub enum TrainingFocus {
    #[default]
    Athletics,
    Offense,
    Defense,
    Technical,
    Mental,
}

impl TrainingFocus {
    pub fn is_focus(&self, skill_index: usize) -> bool {
        match self {
            Self::Athletics => skill_index < 4,
            Self::Offense => skill_index >= 4 && skill_index < 8,
            Self::Defense => skill_index >= 8 && skill_index < 12,
            Self::Technical => skill_index >= 12 && skill_index < 16,
            Self::Mental => skill_index >= 16,
        }
    }

    pub fn next(&self) -> Option<Self> {
        match self {
            Self::Athletics => Some(Self::Offense),
            Self::Offense => Some(Self::Defense),
            Self::Defense => Some(Self::Technical),
            Self::Technical => Some(Self::Mental),
            Self::Mental => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum TeamBonus {
    Exploration,       //pilot
    Reputation,        //captain
    SpaceshipSpeed,    //pilot
    TirednessRecovery, //doctor
    TradePrice,        //captain
    Training,          //doctor
}

impl Display for TeamBonus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TeamBonus::Exploration => write!(f, "Exploration"),
            TeamBonus::Reputation => write!(f, "Reputation"),
            TeamBonus::SpaceshipSpeed => write!(f, "Ship speed"),
            TeamBonus::TirednessRecovery => write!(f, "Recovery"),
            TeamBonus::TradePrice => write!(f, "Trading"),
            TeamBonus::Training => write!(f, "Training"),
        }
    }
}

impl TeamBonus {
    pub const BASE_BONUS: f32 = 1.0;
    const BONUS_PER_SKILL: f32 = 1.0 / MAX_SKILL;
    pub fn current_team_bonus(&self, world: &World, team_id: &TeamId) -> AppResult<f32> {
        let team = world.get_team_or_err(team_id)?;
        let player_id = match self {
            TeamBonus::Exploration => team.crew_roles.pilot,
            TeamBonus::Reputation => team.crew_roles.captain,
            TeamBonus::SpaceshipSpeed => team.crew_roles.pilot,
            TeamBonus::TirednessRecovery => team.crew_roles.doctor,
            TeamBonus::TradePrice => team.crew_roles.captain,
            TeamBonus::Training => team.crew_roles.doctor,
        };

        let skill = if let Some(id) = player_id {
            let player = world.get_player_or_err(&id)?;
            self.as_skill(player).unwrap_or_default()
        } else {
            0.0
        };

        Ok(Self::BASE_BONUS + Self::BONUS_PER_SKILL * skill)
    }

    pub fn current_player_bonus(&self, player: &Player) -> AppResult<f32> {
        let skill = self.as_skill(player).unwrap_or_default();
        Ok(Self::BASE_BONUS + Self::BONUS_PER_SKILL * skill)
    }

    pub fn as_skill(&self, player: &Player) -> AppResult<f32> {
        match self {
            TeamBonus::Exploration => {
                Ok(0.35 * player.athletics.stamina + 0.65 * player.mental.vision)
            }
            TeamBonus::Reputation => Ok(0.8 * player.mental.charisma
                + 0.1 * player.mental.aggression
                + 0.1 * player.athletics.strength),
            TeamBonus::SpaceshipSpeed => {
                Ok(0.75 * player.athletics.quickness + 0.25 * player.mental.vision)
            }
            TeamBonus::TirednessRecovery => {
                Ok(0.8 * player.athletics.stamina + 0.2 * player.mental.intuition)
            }
            TeamBonus::TradePrice => Ok(0.5 * player.mental.charisma
                + 0.25 * player.mental.aggression
                + 0.25 * player.mental.intuition),
            TeamBonus::Training => Ok(0.25 * player.athletics.strength
                + 0.25 * player.athletics.vertical
                + 0.5 * player.mental.intuition),
        }
    }
}

// tests
#[cfg(test)]

mod tests {

    #[test]
    fn test_team_location_eq() {
        use super::TeamLocation;
        use crate::types::PlanetId;
        let planet_id = PlanetId::new_v4();
        let team_location = TeamLocation::OnPlanet { planet_id };
        let team_location2 = TeamLocation::OnPlanet { planet_id };
        assert_eq!(team_location, team_location2);
    }

    #[test]
    fn test_team_location_ne() {
        use super::TeamLocation;
        use crate::types::PlanetId;
        let planet_id = PlanetId::new_v4();
        let planet_id2 = PlanetId::new_v4();
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
