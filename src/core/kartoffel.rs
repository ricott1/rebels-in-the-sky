use super::{planet::Planet, types::KartoffelLocation};
use crate::types::KartoffelId;
use libp2p::PeerId;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

#[derive(Debug, Default, Serialize_repr, Deserialize_repr, Clone, PartialEq)]
#[repr(u8)]
pub enum KartoffelRarity {
    #[default]
    COMMON,
    UNCOMMON,
    RARE,
    LEGENDARY,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Kartoffel {
    pub id: KartoffelId,
    pub peer_id: Option<PeerId>,
    pub rarity: KartoffelRarity,
    pub version: u64,
    pub name: String,
    pub filename: String,
    pub current_location: KartoffelLocation,
}

impl Kartoffel {
    pub fn random(_rng: &mut ChaCha8Rng, home_planet: &Planet) -> Self {
        Self {
            id: KartoffelId::new_v4(),
            peer_id: None,
            rarity: KartoffelRarity::default(),
            version: 0,
            name: "Kartoffle".to_string(),
            filename: "kartoffel1".to_string(),
            current_location: KartoffelLocation::OnPlanet {
                planet_id: home_planet.id,
            },
        }
    }
}
