use super::constants::*;
use crate::{
    core::{
        spaceship_components::*, utils::is_default, SpaceshipUpgradeTarget, Upgrade,
        UpgradeableElement,
    },
    image::{
        color_map::ColorMap,
        spaceship::{SpaceshipImage, SpaceshipImageId},
        utils::{Gif, LightMaskStyle},
    },
    types::{AppResult, Tick},
};
use rand::seq::IteratorRandom;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use std::hash::Hash;
use strum::IntoEnumIterator;
use strum_macros::{Display, EnumIter};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Hash)]
pub struct Spaceship {
    pub name: String,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub charge_unit: ChargeUnit,
    pub hull: Hull,
    pub engine: Engine,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub shield: Shield,
    pub storage: Storage,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub shooter: Shooter,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    current_durability: u32,
    pub image: SpaceshipImage,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub pending_upgrade: Option<Upgrade<SpaceshipUpgradeTarget>>,
}

impl Spaceship {
    pub fn new(
        charge_unit: ChargeUnit,
        hull: Hull,
        engine: Engine,
        storage: Storage,
        shield: Shield,
        shooter: Shooter,
        color_map: ColorMap,
    ) -> Self {
        let mut spaceship = Self {
            name: "".into(),
            charge_unit,
            hull,
            engine,
            shield,
            storage,
            shooter,
            current_durability: 0,
            image: SpaceshipImage::new(color_map),
            pending_upgrade: None,
        };
        spaceship.reset_durability();

        spaceship
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    pub fn reset_durability(&mut self) {
        self.current_durability = self.max_durability()
    }

    pub fn can_be_repaired(&self) -> bool {
        self.current_durability() < self.max_durability()
    }

    pub fn can_be_upgraded(&self, target: SpaceshipUpgradeTarget) -> bool {
        match target {
            SpaceshipUpgradeTarget::ChargeUnit { .. } => self.charge_unit.can_be_upgraded(),
            SpaceshipUpgradeTarget::Hull { .. } => self.hull.can_be_upgraded(),
            SpaceshipUpgradeTarget::Engine { .. } => self.engine.can_be_upgraded(),
            SpaceshipUpgradeTarget::Shield { .. } => self.shield.can_be_upgraded(),
            SpaceshipUpgradeTarget::Shooter { .. } => self.shooter.can_be_upgraded(),
            SpaceshipUpgradeTarget::Storage { .. } => self.storage.can_be_upgraded(),
            SpaceshipUpgradeTarget::Repairs { .. } => self.can_be_repaired(),
        }
    }

    pub fn random(rng: &mut ChaCha8Rng) -> Self {
        let style = SpaceshipStyle::iter()
            .choose(rng)
            .expect("There should be one available style.");

        let charge_unit = ChargeUnit::random_for(rng, style);
        let hull = Hull::random_for(rng, style);
        let engine = Engine::random_for(rng, style);
        let shield = Shield::random_for(rng, style);
        let storage = Storage::random_for(rng, style);
        let shooter = Shooter::random_for(rng, style);

        Self::new(
            charge_unit,
            hull,
            engine,
            storage,
            shield,
            shooter,
            ColorMap::random(rng),
        )
    }

    pub fn with_color_map(mut self, color_map: ColorMap) -> Self {
        self.image.set_color_map(color_map);
        self
    }

    pub fn image_id(&self) -> SpaceshipImageId {
        self.image.id(self.hull, self.engine, self.storage)
    }

    pub fn compose_image(&self, light_mask: Option<LightMaskStyle>) -> AppResult<Gif> {
        self.image.compose(
            self.hull,
            self.engine,
            self.storage,
            self.shooter,
            Shield::None,
            false,
            false,
            light_mask,
        )
    }

    pub fn compose_image_shooting(&self) -> AppResult<Gif> {
        self.image.compose(
            self.hull,
            self.engine,
            self.storage,
            self.shooter,
            Shield::None,
            false,
            true,
            Some(LightMaskStyle::radial()),
        )
    }

    pub fn compose_image_with_shield(&self) -> AppResult<Gif> {
        self.image.compose(
            self.hull,
            self.engine,
            self.storage,
            self.shooter,
            self.shield,
            false,
            false,
            Some(LightMaskStyle::radial()),
        )
    }

    pub fn compose_image_in_shipyard(&self) -> AppResult<Gif> {
        self.image.compose(
            self.hull,
            self.engine,
            self.storage,
            self.shooter,
            Shield::None,
            true,
            false,
            Some(LightMaskStyle::radial()),
        )
    }

    pub fn speed(&self, storage_units: u32) -> f32 {
        // Returns the speed in Km/ms (Kilometers per Tick)
        BASE_SPEED * self.hull.speed() * self.engine.speed() * self.storage.speed()
            / (1.0 + SPEED_PENALTY_PER_UNIT_STORAGE * storage_units as f32)
    }

    pub fn damage(&self) -> f32 {
        self.shooter.damage()
    }

    pub fn fire_rate(&self) -> f32 {
        self.shooter.fire_rate()
    }

    pub fn shooting_points(&self) -> u8 {
        self.shooter.shooting_points()
    }

    pub fn has_shooters(&self) -> bool {
        self.shooter.shooting_points() > 0
    }

    pub fn storage_capacity(&self) -> u32 {
        self.hull.storage_capacity()
            + self.charge_unit.storage_capacity()
            + self.engine.storage_capacity()
            + self.shooter.storage_capacity()
            + self.shield.storage_capacity()
            + self.storage.storage_capacity()
    }

    pub fn fuel_capacity(&self) -> u32 {
        self.hull.fuel_capacity()
            + self.charge_unit.fuel_capacity()
            + self.engine.fuel_capacity()
            + self.shooter.fuel_capacity()
            + self.shield.fuel_capacity()
            + self.storage.fuel_capacity()
    }

    pub fn fuel_consumption_per_tick(&self, storage_units: u32) -> f32 {
        // Returns the fuel consumption in t/ms (tonnes per Tick)
        BASE_FUEL_CONSUMPTION
            * self.hull.fuel_consumption_per_tick()
            * self.charge_unit.fuel_consumption_per_tick()
            * self.engine.fuel_consumption_per_tick()
            * self.shield.fuel_consumption_per_tick()
            * self.shooter.fuel_consumption_per_tick()
            * self.storage.fuel_consumption_per_tick()
            * (1.0 + FUEL_CONSUMPTION_PER_UNIT_STORAGE * storage_units as f32)
    }

    pub fn fuel_consumption_per_kilometer(&self, storage_units: u32) -> f32 {
        // Returns the fuel consumption in t/Km (tonnes per Kilometer)
        self.fuel_consumption_per_tick(storage_units) / self.speed(storage_units)
    }

    pub fn set_current_durability(&mut self, value: u32) {
        self.current_durability = value.min(self.max_durability());
    }

    pub fn current_durability(&self) -> u32 {
        self.current_durability
    }

    pub fn max_durability(&self) -> u32 {
        self.hull.durability()
            + self.charge_unit.durability()
            + self.engine.durability()
            + self.shooter.durability()
            + self.shield.durability()
            + self.storage.durability()
    }

    pub fn max_charge(&self) -> u32 {
        self.charge_unit.max_charge() as u32
    }

    pub fn shield_max_durability(&self) -> f32 {
        self.shield.max_durability()
    }

    pub fn shield_damage_reduction(&self) -> f32 {
        self.shield.damage_reduction()
    }

    pub fn crew_capacity(&self) -> u8 {
        let capacity = self.hull.crew_capacity()
            + self.charge_unit.crew_capacity()
            + self.engine.crew_capacity()
            + self.shooter.crew_capacity()
            + self.shield.crew_capacity()
            + self.storage.crew_capacity();
        // Just to be sure :)
        assert!(capacity <= MAX_CREW_SIZE as u8);
        capacity
    }

    pub fn value(&self) -> u32 {
        let base_value = self.hull.value() + self.engine.value() + self.storage.value();
        (base_value as f32 * SPACESHIP_BASE_COST_MULTIPLIER) as u32
    }

    pub fn max_distance(&self, current_fuel: u32) -> f32 {
        // Return the max distance in kilometers.
        let storage_units = 0;
        self.speed(storage_units) / self.fuel_consumption_per_tick(storage_units)
            * current_fuel as f32
    }

    pub fn max_travel_time(&self, current_fuel: u32) -> Tick {
        // Return the max travel time in milliseconds (Ticks)
        let storage_units = 0;
        (current_fuel as f32 / self.fuel_consumption_per_tick(storage_units)) as Tick
    }
}

#[derive(Display, Debug, Clone, Copy, PartialEq, Hash, EnumIter)]
pub enum SpaceshipPrefab {
    Bresci,
    Cafiero,
    Yukawa,
    Milwaukee,
    Pincher,
    Orwell,
    Ragnarok,
    Ibarruri,
    Rivolta,
}

impl SpaceshipPrefab {
    pub fn spaceship(&self) -> Spaceship {
        match self {
            Self::Yukawa => Spaceship::new(
                ChargeUnit::Small,
                Hull::ShuttleSmall,
                Engine::ShuttleTriple,
                Storage::ShuttleNone,
                Shield::Small,
                Shooter::ShuttleNone,
                ColorMap::default(),
            ),
            Self::Milwaukee => Spaceship::new(
                ChargeUnit::Medium,
                Hull::ShuttleLarge,
                Engine::ShuttleTriple,
                Storage::ShuttleNone,
                Shield::Small,
                Shooter::ShuttleTriple,
                ColorMap::default(),
            ),
            Self::Cafiero => Spaceship::new(
                ChargeUnit::Medium,
                Hull::ShuttleStandard,
                Engine::ShuttleSingle,
                Storage::ShuttleSingle,
                Shield::Medium,
                Shooter::ShuttleTriple,
                ColorMap::default(),
            ),
            Self::Bresci => Spaceship::new(
                ChargeUnit::Small,
                Hull::ShuttleSmall,
                Engine::ShuttleSingle,
                Storage::ShuttleNone,
                Shield::None,
                Shooter::ShuttleNone,
                ColorMap::default(),
            ),
            Self::Pincher => Spaceship::new(
                ChargeUnit::Large,
                Hull::PincherStandard,
                Engine::PincherTriple,
                Storage::PincherSingle,
                Shield::Small,
                Shooter::PincherDouble,
                ColorMap::default(),
            ),
            Self::Orwell => Spaceship::new(
                ChargeUnit::Small,
                Hull::PincherStandard,
                Engine::PincherSingle,
                Storage::PincherNone,
                Shield::None,
                Shooter::PincherNone,
                ColorMap::default(),
            ),
            Self::Ragnarok => Spaceship::new(
                ChargeUnit::Large,
                Hull::PincherLarge,
                Engine::PincherDouble,
                Storage::PincherNone,
                Shield::Medium,
                Shooter::PincherQuadruple,
                ColorMap::default(),
            ),
            Self::Ibarruri => Spaceship::new(
                ChargeUnit::Small,
                Hull::JesterStandard,
                Engine::JesterDouble,
                Storage::JesterNone,
                Shield::None,
                Shooter::JesterNone,
                ColorMap::default(),
            ),
            Self::Rivolta => Spaceship::new(
                ChargeUnit::Large,
                Hull::JesterStandard,
                Engine::JesterQuadruple,
                Storage::JesterNone,
                Shield::Medium,
                Shooter::JesterQuadruple,
                ColorMap::default(),
            ),
        }
    }

    pub fn value(&self) -> u32 {
        self.spaceship().value()
    }
}

#[cfg(test)]

mod tests {
    use super::*;
    use crate::{
        core::{team::Team, types::TeamLocation, world::World},
        types::SystemTimeTick,
    };
    use itertools::Itertools;

    #[test]
    fn test_spaceship_prefab_data() {
        let spaceship = SpaceshipPrefab::Yukawa.spaceship();
        let speed = spaceship.speed(0);
        let crew_capacity = spaceship.crew_capacity();
        let storage_capacity = spaceship.storage_capacity();
        let fuel_capacity = spaceship.fuel_capacity();
        let fuel_consumption = spaceship.fuel_consumption_per_tick(0);
        let value = spaceship.value();
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
        println!("Value: {}", value);
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
        let spaceship = SpaceshipPrefab::Yukawa.spaceship();

        let mut world = World::new(None);

        world.initialize(false)?;

        let planet_ids = world.planets.keys().collect_vec();
        let from = planet_ids[0].clone();
        let to = planet_ids[1].clone();
        let mut team = Team::random(None).with_home_planet(from);
        team.spaceship = spaceship;
        team.current_location = TeamLocation::Travelling {
            from,
            to,
            started: Tick::now(),
            duration: 100,
            distance: 1000,
        };
        println!("TOTAL AU: {}", team.total_travelled);
        world.own_team_id = team.id;

        world.teams.insert(team.id, team);

        let mut current_tick = Tick::now();

        loop {
            let own_team = world.get_own_team()?;
            match own_team.current_location {
                TeamLocation::Travelling {
                    started, duration, ..
                } => println!(
                    "Team is travelling: {} < {} + {} = {}\r",
                    current_tick,
                    started,
                    duration,
                    started + duration
                ),
                _ => {
                    println!("Team landed");
                    println!("TOTALAU: {}", own_team.total_travelled);
                    break;
                }
            };

            match world.tick_travel(current_tick) {
                Ok(messages) => {
                    for message in messages {
                        println!("{:#?}", message)
                    }
                }
                Err(e) => {
                    eprintln!("Failed to tick event: {}", e);
                    break;
                }
            }
            current_tick += 1;
        }

        Ok(())
    }
}
