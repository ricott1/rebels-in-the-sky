use super::skill::MAX_SKILL;
use crate::types::{PlanetId, Tick};
use once_cell::sync::Lazy;

// DEBUG_TIME_MULTIPLIER should be between 1 and 1000;
pub const DEBUG_TIME_MULTIPLIER: Tick = 1;
pub const MILLISECONDS: Tick = 1;
pub const SECONDS: Tick = 1000 * MILLISECONDS / DEBUG_TIME_MULTIPLIER;
pub const MINUTES: Tick = 60 * SECONDS;
pub const HOURS: Tick = 60 * MINUTES;
pub const DAYS: Tick = 24 * HOURS;
pub const WEEKS: Tick = 7 * DAYS;

// A Kilometer represents a unit of distance in the game world.
// It corresponds to a kilometer in the real world.
pub type KILOMETER = u64;
pub const KILOMETERS: KILOMETER = 1;
pub const AU: KILOMETER = 149_597_870_700 * KILOMETERS;
pub const LIGHT_YEAR: KILOMETER = 9_460_730_472_580_800 * KILOMETERS;

// The CALENDAR_OFFSET is the number of years to add to the current year
// to get the year in the game world.
pub const CALENDAR_OFFSET: i32 = 77;

pub const MIN_PLAYERS_PER_GAME: usize = 5;
pub const MAX_PLAYERS_PER_TEAM: usize = MIN_PLAYERS_PER_GAME + 5;

// Try to keep these constants balanced.
// By playing a game, a player can get at most 5*40*60 = 12000 (5 because of the role, times 40 minutes in seconds).
// Then, with potential, focus, and doctor bonus, per game a skill can improve at most
// by 12000 * 8 * EXPERIENCE_PER_SKILL_MULTIPLIER ~ 0.5.
// More realistically, one should take a factor 10 less.
pub const EXPERIENCE_PER_SKILL_MULTIPLIER: f32 = 0.0000125;
pub const MAX_SKILL_INCREASE_PER_LONG_TICK: f32 = 0.5;
pub const SKILL_DECREMENT_PER_LONG_TICK: f32 = -0.05;

pub const REPUTATION_PER_EXPERIENCE: f32 = 0.0001;
pub const REPUTATION_DECREASE_PER_LONG_TICK: f32 = 0.1;
pub const AGE_INCREASE_PER_LONG_TICK: f32 = 0.1; // 1 year every 10 LONG_TICK

pub const INCOME_PER_ATTENDEE_HOME: u32 = 36;
pub const INCOME_PER_ATTENDEE_AWAY: u32 = 36;

pub const INITIAL_TEAM_BALANCE: u32 = 120_000;
pub const COST_PER_VALUE: f32 = 120.0;
pub const SPECIAL_TRAIT_VALUE_BONUS: f32 = 1.35;

pub const AUTO_GENERATE_GAMES_NUMBER: usize = 3;
pub const MAX_AVG_TIREDNESS_PER_AUTO_GAME: f32 = 2.0;

pub const BASE_DISTANCES: [u64; 5] = [
    1 * LIGHT_YEAR,
    1 * AU,
    400_000 * KILOMETERS,
    80_000 * KILOMETERS,
    1_000 * KILOMETERS,
];
pub const BASE_TANK_CAPACITY: u32 = 60;
pub const SPACESHIP_BASE_COST_MULTIPLIER: f32 = 1.1;

pub const LIGHT_SPEED: f32 = 1_079_252_848.8 * KILOMETERS as f32 / HOURS as f32;
pub const BASE_SPEED: f32 = 0.55 * LIGHT_SPEED; // Very fast ;)
pub const BASE_FUEL_CONSUMPTION: f32 = 2.5 / HOURS as f32; // TONNES per HOURS
pub const FUEL_CONSUMPTION_PER_UNIT_STORAGE: f32 = 1.0 / 3_000.0; // 3_000 storage units double the fuel consumption
pub const SPEED_PENALTY_PER_UNIT_STORAGE: f32 = 1.0 / 6_000.0; // 6_000 storage units halves the speed

pub const LANDING_TIME_OVERHEAD: Tick = 10 * MINUTES;

pub const QUICK_EXPLORATION_TIME: Tick = 1 * HOURS;
pub const LONG_EXPLORATION_TIME: Tick = 8 * QUICK_EXPLORATION_TIME;
pub const ASTEROID_DISCOVERY_PROBABILITY: f64 = 0.15;
pub const PORTAL_DISCOVERY_PROBABILITY: f64 = 0.05;

pub const MAX_NUM_ASTEROID_PER_TEAM: usize = 5;

pub struct TickInterval;
impl TickInterval {
    pub const SHORT: Tick = 1 * SECONDS;
    pub const MEDIUM: Tick = 1 * MINUTES;
    pub const LONG: Tick = 24 * HOURS;
}

pub const GAME_START_DELAY: Tick = 20 * SECONDS;
pub const NETWORK_GAME_START_DELAY: Tick = 30 * SECONDS;

static GALAXY_ROOT_STR: &str = "71a43700-0000-0000-0000-000000000000";
static DEFAULT_PLANET_STR: &str = "71a43700-0000-0000-0002-000000000000";
static SOL_STR: &str = "71a43700-0000-0000-0001-000000000000";
pub const GALAXY_ROOT_ID: Lazy<PlanetId> =
    Lazy::new(|| PlanetId::try_parse(GALAXY_ROOT_STR).unwrap());
pub const DEFAULT_PLANET_ID: Lazy<PlanetId> =
    Lazy::new(|| PlanetId::try_parse(DEFAULT_PLANET_STR).unwrap());
pub const SOL_ID: Lazy<PlanetId> = Lazy::new(|| PlanetId::try_parse(SOL_STR).unwrap());

pub struct TirednessCost;
impl TirednessCost {
    pub const NONE: f32 = 0.0;
    pub const LOW: f32 = 0.005;
    pub const MEDIUM: f32 = 0.15;
    pub const HIGH: f32 = 0.5;
    pub const SEVERE: f32 = 2.5;
    pub const CRITICAL: f32 = 5.0;
    pub const MAX: f32 = 20.0;
}

pub struct MoraleModifier;
impl MoraleModifier {
    pub const SEVERE_MALUS: f32 = -5.0;
    pub const HIGH_MALUS: f32 = -2.5;
    pub const MEDIUM_MALUS: f32 = -1.0;
    pub const SMALL_MALUS: f32 = -0.5;
    pub const NONE: f32 = 0.0;
    pub const SMALL_BONUS: f32 = 0.5;
    pub const MEDIUM_BONUS: f32 = 1.0;
    pub const HIGH_BONUS: f32 = 2.5;
    pub const SEVERE_BONUS: f32 = 5.0;
}

pub struct ReputationModifier;
impl ReputationModifier {
    pub const SEVERE_MALUS: f32 = -1.5;
    pub const HIGH_MALUS: f32 = -0.5;
    pub const MEDIUM_MALUS: f32 = -0.2;
    pub const SMALL_MALUS: f32 = -0.1;
    pub const NONE: f32 = 0.0;
    pub const SMALL_BONUS: f32 = 0.1;
    pub const MEDIUM_BONUS: f32 = 0.2;
    pub const HIGH_BONUS: f32 = 0.5;
    pub const SEVERE_BONUS: f32 = 1.5;
    pub const BONUS_PER_DISTANCE: f32 = 0.000002;
}

pub const MAX_TIREDNESS: f32 = MAX_SKILL;
pub const MAX_MORALE: f32 = MAX_SKILL;
pub const MORALE_DECREASE_PER_LONG_TICK: f32 = MoraleModifier::MEDIUM_MALUS;
pub const MORALE_INCREASE_PER_GAME: f32 = MoraleModifier::SEVERE_BONUS;
pub const MORALE_RELEASE_MALUS: f32 = MoraleModifier::MEDIUM_MALUS;
pub const MORALE_THRESHOLD_FOR_LEAVING: f32 = 2.0;
pub const LEAVING_PROBABILITY_MORALE_MODIFIER: f64 =
    0.025 * (1.0 / MORALE_THRESHOLD_FOR_LEAVING) as f64;
pub const MORALE_DEMOTION_MALUS: f32 = MoraleModifier::MEDIUM_MALUS;
pub const MORALE_GAME_POPULATION_MODIFIER: f32 = 0.5;
pub const MORALE_DRINK_BONUS: f32 = MoraleModifier::HIGH_BONUS;
pub const TIREDNESS_DRINK_MALUS: f32 = TirednessCost::SEVERE;
pub const TIREDNESS_DRINK_MALUS_SPUGNA: f32 = TirednessCost::HIGH;
pub const TRAIT_PROBABILITY: f64 = 0.25;

pub const MIN_RELATIVE_RETIREMENT_AGE: f32 = 0.96;
pub const PEAK_PERFORMANCE_RELATIVE_AGE: f32 = 0.65;
