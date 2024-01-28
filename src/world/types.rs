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
    if bmi >= 27 {
        size += SIZE_LARGE_OFFSET;
    }
    size as Size
}

#[derive(
    Debug, Default, PartialEq, Clone, Copy, Display, EnumIter, Serialize_repr, Deserialize_repr,
)]
#[repr(u8)]
pub enum Population {
    #[default]
    Italy,
    Germany,
    Spain,
    Greece,
    Nigeria,
    India,
    Yardalaim,
    Polpett,
    Juppa,
    Galdari,
}

impl Population {
    pub fn apply_skill_modifiers(&self, player: &mut Player) {
        match self {
            Population::Yardalaim => {
                player.info.weight = (player.info.weight * 1.5).min(255.0);
                player.athleticism.strength = (player.athleticism.strength * 1.25).round().bound();
            }
            Population::Juppa => {
                player.info.height = (player.info.height * 1.085).min(225.0);
            }
            Population::Galdari => {
                player.info.height = (player.info.height * 1.005).min(225.0);
                player.athleticism.quickness =
                    (player.athleticism.quickness * 1.25).round().bound();
                player.mental.charisma = (player.mental.charisma * 1.25).round().bound();
                player.defense.steal = (player.defense.steal * 1.25).round().bound();
            }
            _ => {}
        }
    }

    // pub random_hair_map(&self, rng: &mut ChaCha8Rng) -> HairColorMap {};

    pub fn random_skin_map(&self, rng: &mut ChaCha8Rng) -> SkinColorMap {
        let weights = match self {
            Self::Italy => vec![
                (SkinColorMap::Pale, 0.1),
                (SkinColorMap::Light, 0.2),
                (SkinColorMap::Medium, 0.2),
                (SkinColorMap::Dark, 0.1),
            ],
            Self::Germany => vec![
                (SkinColorMap::Pale, 0.2),
                (SkinColorMap::Light, 0.2),
                (SkinColorMap::Medium, 0.1),
                (SkinColorMap::Dark, 0.05),
            ],
            Self::Spain => vec![
                (SkinColorMap::Pale, 0.15),
                (SkinColorMap::Light, 0.1),
                (SkinColorMap::Medium, 0.2),
                (SkinColorMap::Dark, 0.15),
            ],
            Self::Greece => vec![
                (SkinColorMap::Pale, 0.1),
                (SkinColorMap::Light, 0.2),
                (SkinColorMap::Medium, 0.2),
                (SkinColorMap::Dark, 0.1),
            ],
            Self::Nigeria => vec![
                (SkinColorMap::Pale, 0.025),
                (SkinColorMap::Light, 0.05),
                (SkinColorMap::Medium, 0.1),
                (SkinColorMap::Dark, 0.3),
            ],
            Self::India => vec![
                (SkinColorMap::Pale, 0.05),
                (SkinColorMap::Light, 0.1),
                (SkinColorMap::Medium, 0.3),
                (SkinColorMap::Dark, 0.2),
            ],
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
    },
    OnPlanet {
        planet_id: PlanetId,
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
        match rand::thread_rng().gen_range(0..=2) {
            0 => Self::He,
            1 => Self::She,
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
        };
        assert_ne!(team_location, team_location2);
        assert_ne!(team_location, team_location3);
    }
}
