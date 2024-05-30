use std::fmt::Display;

use crate::{
    image::{color_map::ColorMap, spaceship::SpaceshipImage, types::Gif},
    types::{AppResult, Tick},
};

use super::constants::{BASE_FUEL_CONSUMPTION, BASE_SPEED, MIN_PLAYERS_PER_TEAM};
use rand::{seq::IteratorRandom, Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use strum::IntoEnumIterator;
use strum_macros::{Display, EnumIter};

#[derive(Debug, Display, Clone, Copy, PartialEq, EnumIter)]
pub enum SpaceshipStyle {
    Shuttle,
    Pincher,
    Jester,
}

pub trait SpaceshipComponent {
    fn style(&self) -> SpaceshipStyle;
    fn crew_capacity(&self) -> u8;
    fn storage_capacity(&self) -> u32;
    fn fuel_capacity(&self) -> u32;
    fn fuel_consumption(&self) -> f32;
    fn speed(&self) -> f32;
    fn durability(&self) -> f32;
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
    JesterStandard,
}

impl Display for Hull {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ShuttleSmall => write!(f, "Small"),
            Self::ShuttleStandard => write!(f, "Standard"),
            Self::ShuttleLarge => write!(f, "Large"),
            Self::PincherStandard => write!(f, "Standard"),
            Self::PincherLarge => write!(f, "Large"),
            Self::JesterStandard => write!(f, "Standard"),
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
            Self::JesterStandard => Self::JesterStandard,
        }
    }

    pub fn previous(&self) -> Self {
        match self {
            Self::ShuttleSmall => Self::ShuttleLarge,
            Self::ShuttleStandard => Self::ShuttleSmall,
            Self::ShuttleLarge => Self::ShuttleStandard,
            Self::PincherStandard => Self::PincherLarge,
            Self::PincherLarge => Self::PincherStandard,
            Self::JesterStandard => Self::JesterStandard,
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
            Self::JesterStandard => SpaceshipStyle::Jester,
        }
    }
    fn crew_capacity(&self) -> u8 {
        match self {
            Self::ShuttleSmall => MIN_PLAYERS_PER_TEAM as u8 + 2,
            Self::ShuttleStandard => MIN_PLAYERS_PER_TEAM as u8 + 3,
            Self::ShuttleLarge => MIN_PLAYERS_PER_TEAM as u8 + 4,
            Self::PincherStandard => MIN_PLAYERS_PER_TEAM as u8 + 3,
            Self::PincherLarge => MIN_PLAYERS_PER_TEAM as u8 + 4,
            Self::JesterStandard => MIN_PLAYERS_PER_TEAM as u8 + 3,
        }
    }

    fn storage_capacity(&self) -> u32 {
        match self {
            Self::ShuttleSmall => 1800,
            Self::ShuttleStandard => 2700,
            Self::ShuttleLarge => 5000,
            Self::PincherStandard => 2000,
            Self::PincherLarge => 3800,
            Self::JesterStandard => 1000,
        }
    }

    fn fuel_capacity(&self) -> u32 {
        match self {
            Self::ShuttleSmall => 110,
            Self::ShuttleStandard => 200,
            Self::ShuttleLarge => 380,
            Self::PincherStandard => 300,
            Self::PincherLarge => 500,
            Self::JesterStandard => 450,
        }
    }

    fn fuel_consumption(&self) -> f32 {
        match self {
            Self::ShuttleSmall => 0.75,
            Self::ShuttleStandard => 0.95,
            Self::ShuttleLarge => 1.45,
            Self::PincherStandard => 1.25,
            Self::PincherLarge => 1.75,
            Self::JesterStandard => 1.15,
        }
    }

    fn speed(&self) -> f32 {
        match self {
            Self::ShuttleSmall => 1.45,
            Self::ShuttleStandard => 1.0,
            Self::ShuttleLarge => 0.6,
            Self::PincherStandard => 1.25,
            Self::PincherLarge => 0.75,
            Self::JesterStandard => 1.05,
        }
    }
    fn durability(&self) -> f32 {
        match self {
            Self::ShuttleSmall => 16.0,
            Self::ShuttleStandard => 18.0,
            Self::ShuttleLarge => 19.0,
            Self::PincherStandard => 18.0,
            Self::PincherLarge => 20.0,
            Self::JesterStandard => 16.0,
        }
    }
    fn cost(&self) -> u32 {
        match self {
            Self::ShuttleSmall => 15000,
            Self::ShuttleStandard => 25000,
            Self::ShuttleLarge => 32000,
            Self::PincherStandard => 33000,
            Self::PincherLarge => 45000,
            Self::JesterStandard => 52000,
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
    JesterDouble,
    JesterQuadruple,
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
            Engine::JesterDouble => write!(f, "Double"),
            Engine::JesterQuadruple => write!(f, "Quadruple"),
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
            Self::JesterDouble => Self::JesterQuadruple,
            Self::JesterQuadruple => Self::JesterDouble,
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
            Self::JesterDouble => Self::JesterQuadruple,
            Self::JesterQuadruple => Self::JesterDouble,
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
            Self::JesterDouble => SpaceshipStyle::Jester,
            Self::JesterQuadruple => SpaceshipStyle::Jester,
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
            Self::JesterDouble => 1.25,
            Self::JesterQuadruple => 2.2,
        }
    }

    fn speed(&self) -> f32 {
        match self {
            Self::ShuttleSingle => 1.2,
            Self::ShuttleDouble => 1.7,
            Self::ShuttleTriple => 2.2,
            Self::PincherSingle => 1.1,
            Self::PincherDouble => 1.65,
            Self::PincherTriple => 2.25,
            Self::JesterDouble => 1.7,
            Self::JesterQuadruple => 2.45,
        }
    }

    fn durability(&self) -> f32 {
        match self {
            Self::ShuttleSingle => 8.0,
            Self::ShuttleDouble => 7.0,
            Self::ShuttleTriple => 6.0,
            Self::PincherSingle => 8.0,
            Self::PincherDouble => 7.0,
            Self::PincherTriple => 6.0,
            Self::JesterDouble => 6.0,
            Self::JesterQuadruple => 5.0,
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
            Self::JesterDouble => 14000,
            Self::JesterQuadruple => 29000,
        }
    }
}

#[derive(
    Debug, Clone, Copy, Serialize_repr, Deserialize_repr, PartialEq, Default, EnumIter, Eq, Hash,
)]
#[repr(u8)]
pub enum Storage {
    #[default]
    ShuttleSingle,
    ShuttleDouble,
    PincherSingle,
}

impl Display for Storage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Storage::ShuttleSingle => write!(f, "Single"),
            Storage::ShuttleDouble => write!(f, "Double"),
            Storage::PincherSingle => write!(f, "Single"),
        }
    }
}

impl Storage {
    pub fn next(&self) -> Self {
        match self {
            Self::ShuttleSingle => Self::ShuttleDouble,
            Self::ShuttleDouble => Self::ShuttleSingle,
            Self::PincherSingle => Self::PincherSingle,
        }
    }

    pub fn previous(&self) -> Self {
        match self {
            Self::ShuttleSingle => Self::ShuttleDouble,
            Self::ShuttleDouble => Self::ShuttleSingle,
            Self::PincherSingle => Self::PincherSingle,
        }
    }
}

impl SpaceshipComponent for Storage {
    fn style(&self) -> SpaceshipStyle {
        match self {
            Self::ShuttleSingle => SpaceshipStyle::Shuttle,
            Self::ShuttleDouble => SpaceshipStyle::Shuttle,
            Self::PincherSingle => SpaceshipStyle::Pincher,
        }
    }
    fn crew_capacity(&self) -> u8 {
        match self {
            Self::ShuttleSingle => 0,
            Self::ShuttleDouble => 1,
            Self::PincherSingle => 0,
        }
    }

    fn storage_capacity(&self) -> u32 {
        match self {
            Self::ShuttleSingle => 1000,
            Self::ShuttleDouble => 3000,
            Self::PincherSingle => 4000,
        }
    }

    fn fuel_capacity(&self) -> u32 {
        match self {
            Self::ShuttleSingle => 30,
            Self::ShuttleDouble => 60,
            Self::PincherSingle => 20,
        }
    }

    fn fuel_consumption(&self) -> f32 {
        match self {
            Self::ShuttleSingle => 1.02,
            Self::ShuttleDouble => 1.03,
            Self::PincherSingle => 1.03,
        }
    }

    fn speed(&self) -> f32 {
        match self {
            Self::ShuttleSingle => 0.96,
            Self::ShuttleDouble => 0.93,
            Self::PincherSingle => 0.97,
        }
    }

    fn durability(&self) -> f32 {
        match self {
            Self::ShuttleSingle => 10.0,
            Self::ShuttleDouble => 11.0,
            Self::PincherSingle => 7.0,
        }
    }
    fn cost(&self) -> u32 {
        match self {
            Self::ShuttleSingle => 5000,
            Self::ShuttleDouble => 6000,
            Self::PincherSingle => 6000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Spaceship {
    pub name: String,
    pub hull: Hull,
    pub engine: Engine,
    pub storage: Option<Storage>,
    pub image: SpaceshipImage,
    pub total_travelled: u128,
}

impl Spaceship {
    pub fn new(
        name: String,
        hull: Hull,
        engine: Engine,
        storage: Option<Storage>,
        color_map: ColorMap,
    ) -> Self {
        Self {
            name,
            hull,
            engine,
            storage,
            image: SpaceshipImage::new(color_map),
            total_travelled: 0,
        }
    }

    pub fn size(&self) -> u8 {
        match self.hull {
            Hull::ShuttleSmall => 0,
            Hull::ShuttleStandard => 1,
            Hull::ShuttleLarge => 2,
            Hull::PincherStandard => 1,
            Hull::PincherLarge => 2,
            Hull::JesterStandard => 1,
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
        let storage = match rng.gen_bool(0.5) {
            true => Storage::iter().filter(|s| s.style() == style).choose(rng),
            false => None,
        };

        Storage::iter().filter(|s| s.style() == style).choose(rng);
        Self::new(name, hull, engine, storage, color_map)
    }

    pub fn set_color_map(&mut self, color_map: ColorMap) {
        self.image.set_color_map(color_map);
    }

    pub fn compose_image(&self) -> AppResult<Gif> {
        self.image
            .compose(self.size(), self.hull, self.engine, self.storage)
    }

    pub fn speed(&self) -> f32 {
        // Returns the speed in Km/ms (Kilometers per Tick)
        let storage_speed = match self.storage {
            Some(storage) => storage.speed(),
            None => 1.0,
        };
        BASE_SPEED * self.hull.speed() * self.engine.speed() * storage_speed
    }

    pub fn crew_capacity(&self) -> u8 {
        let storage_crew_capacity = match self.storage {
            Some(storage) => storage.crew_capacity(),
            None => 0,
        };
        self.hull.crew_capacity() + self.engine.crew_capacity() + storage_crew_capacity
    }

    pub fn storage_capacity(&self) -> u32 {
        let storage_capacity = match self.storage {
            Some(storage) => storage.storage_capacity(),
            None => 0,
        };
        self.hull.storage_capacity() + self.engine.storage_capacity() + storage_capacity
    }

    pub fn fuel_capacity(&self) -> u32 {
        let storage_fuel_capacity = match self.storage {
            Some(storage) => storage.fuel_capacity(),
            None => 0,
        };
        self.hull.fuel_capacity() + self.engine.fuel_capacity() + storage_fuel_capacity
    }

    pub fn fuel_consumption(&self) -> f32 {
        // Returns the fuel consumption in t/ms (tonnes per Tick)
        let storage_fuel_consumption = match self.storage {
            Some(storage) => storage.fuel_consumption(),
            None => 1.0,
        };
        BASE_FUEL_CONSUMPTION
            * self.hull.fuel_consumption()
            * self.engine.fuel_consumption()
            * storage_fuel_consumption
    }

    pub fn cost(&self) -> u32 {
        self.hull.cost()
            + self.engine.cost()
            + if let Some(storage) = self.storage {
                storage.cost()
            } else {
                0
            }
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
    Ibarruri,
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
            Self::Ragnarok => Self::Ibarruri,
            Self::Ibarruri => Self::Bresci,
        }
    }

    pub fn previous(&self) -> Self {
        match self {
            Self::Bresci => Self::Ibarruri,
            Self::Cafiero => Self::Bresci,
            Self::Yukawa => Self::Cafiero,
            Self::Milwaukee => Self::Yukawa,
            Self::Pincher => Self::Milwaukee,
            Self::Orwell => Self::Pincher,
            Self::Ragnarok => Self::Orwell,
            Self::Ibarruri => Self::Ragnarok,
        }
    }

    pub fn specs(&self, name: String, color_map: ColorMap) -> Spaceship {
        match self {
            Self::Yukawa => Spaceship::new(
                name,
                Hull::ShuttleStandard,
                Engine::ShuttleDouble,
                None,
                color_map,
            ),
            Self::Milwaukee => Spaceship::new(
                name,
                Hull::ShuttleLarge,
                Engine::ShuttleTriple,
                None,
                color_map,
            ),
            Self::Cafiero => Spaceship::new(
                name,
                Hull::ShuttleStandard,
                Engine::ShuttleSingle,
                Some(Storage::ShuttleSingle),
                color_map,
            ),
            Self::Bresci => Spaceship::new(
                name,
                Hull::ShuttleSmall,
                Engine::ShuttleTriple,
                None,
                color_map,
            ),
            Self::Pincher => Spaceship::new(
                name,
                Hull::PincherStandard,
                Engine::PincherSingle,
                None,
                color_map,
            ),
            Self::Orwell => Spaceship::new(
                name,
                Hull::PincherStandard,
                Engine::PincherTriple,
                Some(Storage::PincherSingle),
                color_map,
            ),
            Self::Ragnarok => Spaceship::new(
                name,
                Hull::PincherLarge,
                Engine::PincherDouble,
                None,
                color_map,
            ),
            Self::Ibarruri => Spaceship::new(
                name,
                Hull::JesterStandard,
                Engine::JesterQuadruple,
                None,
                color_map,
            ),
        }
    }

    pub fn cost(&self) -> u32 {
        self.specs("".to_string(), ColorMap::default()).cost()
    }
}

#[cfg(test)]

mod tests {
    use itertools::Itertools;

    use crate::{
        types::{IdSystem, SystemTimeTick, TeamId, AU, HOURS},
        world::{team::Team, types::TeamLocation, world::World},
    };

    use super::*;

    #[test]
    fn test_spaceship_prefab_data() {
        let color_map = ColorMap::random();
        let name = "test".to_string();
        let spaceship = SpaceshipPrefab::Yukawa.specs(name, color_map);
        let speed = spaceship.speed();
        let crew_capacity = spaceship.crew_capacity();
        let storage_capacity = spaceship.storage_capacity();
        let fuel_capacity = spaceship.fuel_capacity();
        let fuel_consumption = spaceship.fuel_consumption();
        let cost = spaceship.cost();
        let max_distance = spaceship.max_distance(fuel_capacity);
        let max_travel_time = spaceship.max_travel_time(fuel_capacity);

        //log the spaceship data
        println!(
            "Speed: {} Km/ms = {:.2} Km/h = {:.2} AU/h",
            speed,
            speed * HOURS as f32,
            speed * HOURS as f32 / AU as f32
        );
        println!("Crew Capacity: {}", crew_capacity);
        println!("Storage Capacity: {}", storage_capacity);
        println!("Fuel Capacity: {} t", fuel_capacity);
        println!(
            "Fuel Consumption: {} t/ms = {:.2} t/h",
            fuel_consumption,
            fuel_consumption * HOURS as f32
        );
        println!("Cost: {}", cost);
        println!(
            "Max Distance: {} Km = {:.2} AU",
            max_distance,
            max_distance / AU as f32
        );
        println!(
            "Max Travel Time: {} ms = {}",
            max_travel_time,
            max_travel_time.formatted()
        );
    }

    #[test]
    fn test_total_travelled_au() -> AppResult<()> {
        let color_map = ColorMap::random();
        let name = "test".to_string();
        let spaceship = SpaceshipPrefab::Yukawa.specs(name, color_map);

        let mut world = World::new(None);

        world.initialize(false)?;

        let planet_ids = world.planets.keys().collect_vec();
        let from = planet_ids[0].clone();
        let to = planet_ids[1].clone();
        let mut team = Team::random(TeamId::new(), from.clone(), "test".into());
        team.spaceship = spaceship;
        team.current_location = TeamLocation::Travelling {
            from,
            to,
            started: Tick::now(),
            duration: 100,
            distance: 1000,
        };
        println!("TOTAL AU: {}", team.spaceship.total_travelled);
        world.own_team_id = team.id;

        world.teams.insert(team.id, team);

        let mut current_timestamp = Tick::now();

        loop {
            let own_team = world.get_own_team()?;
            match own_team.current_location {
                TeamLocation::Travelling {
                    started, duration, ..
                } => println!(
                    "Team is travelling: {} < {} + {} = {}\r",
                    current_timestamp,
                    started,
                    duration,
                    started + duration
                ),
                _ => {
                    println!("Team landed");
                    println!("TOTALAU: {}", own_team.spaceship.total_travelled);
                    break;
                }
            };

            match world.tick_travel(current_timestamp, false) {
                Ok(message_option) => {
                    if let Some(messages) = message_option {
                        println!("{:#?}", messages)
                    }
                }
                Err(e) => {
                    eprintln!("Failed to tick event: {}", e);
                    break;
                }
            }
            current_timestamp += 1;
        }

        Ok(())
    }
}
