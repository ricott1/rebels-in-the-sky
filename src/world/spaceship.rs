use std::fmt::Display;

use crate::{
    image::{color_map::ColorMap, spaceship::SpaceshipImage, types::Gif},
    types::{AppResult, Tick},
};

use super::constants::{BASE_FUEL_CONSUMPTION, BASE_SPEED, MIN_PLAYERS_PER_TEAM};
use rand::{seq::IteratorRandom, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use strum::IntoEnumIterator;
use strum_macros::{Display, EnumIter};

#[derive(Debug, Display, Clone, Copy, PartialEq, EnumIter)]
pub enum SpaceshipStyle {
    Shuttle,
    Pincher,
}

pub trait SpaceshipComponent {
    fn style(&self) -> SpaceshipStyle;
    fn crew_capacity(&self) -> u8;
    fn storage_capacity(&self) -> u32;
    fn fuel_capacity(&self) -> u32;
    fn fuel_consumption(&self) -> f32;
    fn speed(&self) -> f32;
    fn cost(&self) -> u32;
}

#[derive(
    Debug, Clone, Copy, Serialize_repr, Deserialize_repr, PartialEq, Eq, Hash, Default, EnumIter,
)]
#[repr(u8)]
pub enum Hull {
    #[default]
    ShuttleSmall,
    ShuttleStandard,
    ShuttleLarge,
    PincherStandard,
    PincherLarge,
}

impl Display for Hull {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Hull::ShuttleSmall => write!(f, "Small"),
            Hull::ShuttleStandard => write!(f, "Standard"),
            Hull::ShuttleLarge => write!(f, "Large"),
            Hull::PincherStandard => write!(f, "Standard"),
            Hull::PincherLarge => write!(f, "Large"),
        }
    }
}

impl Hull {
    pub fn next(&self) -> Self {
        match self {
            Self::ShuttleSmall => Self::ShuttleStandard,
            Self::ShuttleStandard => Self::ShuttleLarge,
            Self::ShuttleLarge => Self::ShuttleSmall,
            Self::PincherStandard => Self::PincherLarge,
            Self::PincherLarge => Self::PincherStandard,
        }
    }

    pub fn previous(&self) -> Self {
        match self {
            Self::ShuttleSmall => Self::ShuttleLarge,
            Self::ShuttleStandard => Self::ShuttleSmall,
            Self::ShuttleLarge => Self::ShuttleStandard,
            Self::PincherStandard => Self::PincherLarge,
            Self::PincherLarge => Self::PincherStandard,
        }
    }
}

impl SpaceshipComponent for Hull {
    fn style(&self) -> SpaceshipStyle {
        match self {
            Self::ShuttleSmall => SpaceshipStyle::Shuttle,
            Self::ShuttleStandard => SpaceshipStyle::Shuttle,
            Self::ShuttleLarge => SpaceshipStyle::Shuttle,
            Self::PincherStandard => SpaceshipStyle::Pincher,
            Self::PincherLarge => SpaceshipStyle::Pincher,
        }
    }
    fn crew_capacity(&self) -> u8 {
        match self {
            Self::ShuttleSmall => MIN_PLAYERS_PER_TEAM as u8 + 1,
            Self::ShuttleStandard => MIN_PLAYERS_PER_TEAM as u8 + 2,
            Self::ShuttleLarge => MIN_PLAYERS_PER_TEAM as u8 + 3,
            Self::PincherStandard => MIN_PLAYERS_PER_TEAM as u8 + 2,
            Self::PincherLarge => MIN_PLAYERS_PER_TEAM as u8 + 4,
        }
    }

    fn storage_capacity(&self) -> u32 {
        match self {
            Self::ShuttleSmall => 1000,
            Self::ShuttleStandard => 2000,
            Self::ShuttleLarge => 4000,
            Self::PincherStandard => 3000,
            Self::PincherLarge => 5000,
        }
    }

    fn fuel_capacity(&self) -> u32 {
        match self {
            Self::ShuttleSmall => 100,
            Self::ShuttleStandard => 200,
            Self::ShuttleLarge => 400,
            Self::PincherStandard => 300,
            Self::PincherLarge => 500,
        }
    }

    fn fuel_consumption(&self) -> f32 {
        match self {
            Self::ShuttleSmall => 0.75,
            Self::ShuttleStandard => 1.0,
            Self::ShuttleLarge => 1.5,
            Self::PincherStandard => 1.25,
            Self::PincherLarge => 1.75,
        }
    }

    fn speed(&self) -> f32 {
        match self {
            Self::ShuttleSmall => 1.5,
            Self::ShuttleStandard => 1.0,
            Self::ShuttleLarge => 0.5,
            Self::PincherStandard => 1.25,
            Self::PincherLarge => 0.75,
        }
    }

    fn cost(&self) -> u32 {
        match self {
            Self::ShuttleSmall => 15000,
            Self::ShuttleStandard => 18000,
            Self::ShuttleLarge => 25000,
            Self::PincherStandard => 25000,
            Self::PincherLarge => 35000,
        }
    }
}

#[derive(
    Debug, Clone, Copy, Serialize_repr, Deserialize_repr, PartialEq, Default, EnumIter, Eq, Hash,
)]
#[repr(u8)]
pub enum Engine {
    #[default]
    ShuttleSingle,
    ShuttleDouble,
    ShuttleTriple,
    PincherSingle,
    PincherDouble,
    PincherTriple,
}

impl Display for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Engine::ShuttleSingle => write!(f, "Single"),
            Engine::ShuttleDouble => write!(f, "Double"),
            Engine::ShuttleTriple => write!(f, "Triple"),
            Engine::PincherSingle => write!(f, "Single"),
            Engine::PincherDouble => write!(f, "Double"),
            Engine::PincherTriple => write!(f, "Triple"),
        }
    }
}

impl Engine {
    pub fn next(&self) -> Self {
        match self {
            Self::ShuttleSingle => Self::ShuttleDouble,
            Self::ShuttleDouble => Self::ShuttleTriple,
            Self::ShuttleTriple => Self::ShuttleSingle,
            Self::PincherSingle => Self::PincherDouble,
            Self::PincherDouble => Self::PincherTriple,
            Self::PincherTriple => Self::PincherSingle,
        }
    }

    pub fn previous(&self) -> Self {
        match self {
            Self::ShuttleSingle => Self::ShuttleTriple,
            Self::ShuttleDouble => Self::ShuttleSingle,
            Self::ShuttleTriple => Self::ShuttleDouble,
            Self::PincherSingle => Self::PincherTriple,
            Self::PincherDouble => Self::PincherSingle,
            Self::PincherTriple => Self::PincherDouble,
        }
    }
}

impl SpaceshipComponent for Engine {
    fn style(&self) -> SpaceshipStyle {
        match self {
            Self::ShuttleSingle => SpaceshipStyle::Shuttle,
            Self::ShuttleDouble => SpaceshipStyle::Shuttle,
            Self::ShuttleTriple => SpaceshipStyle::Shuttle,
            Self::PincherSingle => SpaceshipStyle::Pincher,
            Self::PincherDouble => SpaceshipStyle::Pincher,
            Self::PincherTriple => SpaceshipStyle::Pincher,
        }
    }
    fn crew_capacity(&self) -> u8 {
        0
    }

    fn storage_capacity(&self) -> u32 {
        0
    }

    fn fuel_capacity(&self) -> u32 {
        0
    }

    fn fuel_consumption(&self) -> f32 {
        match self {
            Self::ShuttleSingle => 1.0,
            Self::ShuttleDouble => 1.5,
            Self::ShuttleTriple => 2.0,
            Self::PincherSingle => 1.0,
            Self::PincherDouble => 1.5,
            Self::PincherTriple => 2.0,
        }
    }

    fn speed(&self) -> f32 {
        match self {
            Self::ShuttleSingle => 1.0,
            Self::ShuttleDouble => 1.5,
            Self::ShuttleTriple => 2.0,
            Self::PincherSingle => 1.0,
            Self::PincherDouble => 1.55,
            Self::PincherTriple => 2.05,
        }
    }
    fn cost(&self) -> u32 {
        match self {
            Self::ShuttleSingle => 5000,
            Self::ShuttleDouble => 8000,
            Self::ShuttleTriple => 15000,
            Self::PincherSingle => 8000,
            Self::PincherDouble => 12000,
            Self::PincherTriple => 19000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct Spaceship {
    pub name: String,
    pub hull: Hull,
    pub engine: Engine,
    pub image: SpaceshipImage,
}

impl Spaceship {
    pub fn new(name: String, hull: Hull, engine: Engine, color_map: ColorMap) -> Self {
        Self {
            name,
            hull,
            engine,
            image: SpaceshipImage::new(color_map),
        }
    }

    pub fn random(name: String, color_map: ColorMap) -> Self {
        let rng = &mut ChaCha8Rng::from_entropy();
        let style = SpaceshipStyle::iter().choose(rng).unwrap();
        let hull = Hull::iter()
            .filter(|h| h.style() == style)
            .choose(rng)
            .unwrap();
        let engine = Engine::iter()
            .filter(|e| e.style() == style)
            .choose(rng)
            .unwrap();
        Self::new(name, hull, engine, color_map)
    }

    pub fn set_color_map(&mut self, color_map: ColorMap) {
        self.image.set_color_map(color_map);
    }

    pub fn compose_image(&self) -> AppResult<Gif> {
        self.image.compose(self.hull, self.engine)
    }

    pub fn speed(&self) -> f32 {
        // Returns the speed in Km/ms (Kilometers per Tick)
        BASE_SPEED * self.hull.speed() * self.engine.speed()
    }

    pub fn crew_capacity(&self) -> u8 {
        self.hull.crew_capacity() + self.engine.crew_capacity()
    }

    pub fn storage_capacity(&self) -> u32 {
        self.hull.storage_capacity() + self.engine.storage_capacity()
    }

    pub fn fuel_capacity(&self) -> u32 {
        self.hull.fuel_capacity() + self.engine.fuel_capacity()
    }

    pub fn fuel_consumption(&self) -> f32 {
        // Returns the fuel consumption in t/ms (tonnes per Tick)
        BASE_FUEL_CONSUMPTION * self.hull.fuel_consumption() * self.engine.fuel_consumption()
    }

    pub fn cost(&self) -> u32 {
        self.hull.cost() + self.engine.cost()
    }

    pub fn max_distance(&self, current_fuel: u32) -> f32 {
        // Return the max distance in kilometers.
        self.speed() / self.fuel_consumption() * current_fuel as f32
    }

    pub fn max_travel_time(&self, current_fuel: u32) -> Tick {
        // Return the max travel time in milliseconds (Ticks)
        (current_fuel as f32 / self.fuel_consumption()) as Tick
    }
}

#[derive(Display, Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter)]
pub enum SpaceshipPrefab {
    Bresci,
    Cafiero,
    Yukawa,
    Milwaukee,
    Pincher,
    Orwell,
    Ragnarok,
}

impl SpaceshipPrefab {
    pub fn next(&self) -> Self {
        match self {
            Self::Bresci => Self::Cafiero,
            Self::Cafiero => Self::Yukawa,
            Self::Yukawa => Self::Milwaukee,
            Self::Milwaukee => Self::Pincher,
            Self::Pincher => Self::Orwell,
            Self::Orwell => Self::Ragnarok,
            Self::Ragnarok => Self::Bresci,
        }
    }

    pub fn previous(&self) -> Self {
        match self {
            Self::Bresci => Self::Ragnarok,
            Self::Cafiero => Self::Bresci,
            Self::Yukawa => Self::Cafiero,
            Self::Milwaukee => Self::Yukawa,
            Self::Pincher => Self::Milwaukee,
            Self::Orwell => Self::Pincher,
            Self::Ragnarok => Self::Orwell,
        }
    }

    pub fn specs(&self, name: String, color_map: ColorMap) -> Spaceship {
        match self {
            Self::Yukawa => Spaceship::new(
                name,
                Hull::ShuttleStandard,
                Engine::ShuttleDouble,
                color_map,
            ),
            Self::Milwaukee => {
                Spaceship::new(name, Hull::ShuttleLarge, Engine::ShuttleTriple, color_map)
            }
            Self::Cafiero => Spaceship::new(
                name,
                Hull::ShuttleStandard,
                Engine::ShuttleSingle,
                color_map,
            ),
            Self::Bresci => {
                Spaceship::new(name, Hull::ShuttleSmall, Engine::ShuttleTriple, color_map)
            }
            Self::Pincher => Spaceship::new(
                name,
                Hull::PincherStandard,
                Engine::PincherSingle,
                color_map,
            ),
            Self::Ragnarok => {
                Spaceship::new(name, Hull::PincherLarge, Engine::PincherDouble, color_map)
            }
            Self::Orwell => Spaceship::new(
                name,
                Hull::PincherStandard,
                Engine::PincherTriple,
                color_map,
            ),
        }
    }

    pub fn cost(&self) -> u32 {
        self.specs("".to_string(), ColorMap::default()).cost()
    }
}
