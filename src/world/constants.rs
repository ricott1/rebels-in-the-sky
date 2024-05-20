use crate::types::{PlanetId, Tick, AU, HOURS, KILOMETERS, LIGHT_YEAR, MINUTES, SECONDS};
use once_cell::sync::Lazy;

use super::skill::MAX_SKILL;

pub const MIN_PLAYERS_PER_TEAM: usize = 5;
pub const MAX_PLAYERS_PER_TEAM: usize = MIN_PLAYERS_PER_TEAM + 5;

pub const EXPERIENCE_PER_SKILL_MULTIPLIER: f32 = 0.00001;
pub const REPUTATION_PER_EXPERIENCE: f32 = 0.00005;
pub const REPUTATION_DECREASE_PER_LONG_TICK: f32 = 0.1;
pub const AGE_INCREASE_PER_LONG_TICK: f32 = 0.025;

pub const INCOME_PER_ATTENDEE_HOME: u32 = 12;
pub const INCOME_PER_ATTENDEE_AWAY: u32 = 12;

pub const INITIAL_TEAM_BALANCE: u32 = 180_000;
pub const CURRENCY_SYMBOL: &str = "sat";
pub const COST_PER_VALUE: u32 = 22;

pub const AUTO_GENERATE_GAMES_NUMBER: usize = 3;
pub const MAX_AVG_TIREDNESS_PER_AUTO_GAME: f32 = 5.0;

const DEBUG_TIME_MULTIPLIER: Tick = 1;
pub const BASE_DISTANCES: [u128; 4] = [
    1 * LIGHT_YEAR,
    1 * AU,
    400_000 * KILOMETERS,
    80_000 * KILOMETERS,
];
pub const BASE_TANK_CAPACITY: u32 = 60;
pub const BASE_SPEED: f32 =
    2_750_000_000.0 * KILOMETERS as f32 / HOURS as f32 * DEBUG_TIME_MULTIPLIER as f32; // Very fast ;)
pub const BASE_FUEL_CONSUMPTION: f32 = 6.0 / HOURS as f32 * DEBUG_TIME_MULTIPLIER as f32; // TONNES per HOURS
pub const LANDING_TIME_OVERHEAD: Tick = 5 * MINUTES / DEBUG_TIME_MULTIPLIER;

pub const BASE_BONUS: f32 = 1.0;
pub const BONUS_PER_SKILL: f32 = 1.0 / MAX_SKILL;

pub const REPUTATION_BONUS_WINNER: f32 = 0.5;
pub const REPUTATION_BONUS_LOSER: f32 = -0.2;
pub const REPUTATION_BONUS_DRAW: f32 = 0.25;

pub const BASE_EXPLORATION_TIME: Tick = 1 * SECONDS / DEBUG_TIME_MULTIPLIER;
pub const ASTEROID_DISCOVERY_PROBABILITY: f64 = 0.8;

pub struct TickInterval;
impl TickInterval {
    pub const SHORT: Tick = 1 * SECONDS / DEBUG_TIME_MULTIPLIER;
    pub const MEDIUM: Tick = 1 * MINUTES / DEBUG_TIME_MULTIPLIER;
    pub const LONG: Tick = 24 * HOURS / DEBUG_TIME_MULTIPLIER;
}

pub const BASE_GAME_START_DELAY: Tick = 10 * SECONDS;
pub const GAME_CLEANUP_TIME: Tick = 10 * SECONDS;

static GALAXY_ROOT_STR: &str = "71a43700-0000-0000-0000-000000000000";
static DEFAULT_PLANET_STR: &str = "71a43700-0000-0000-0002-000000000000";
static SOL_STR: &str = "71a43700-0000-0000-0001-000000000000";
pub const GALAXY_ROOT_ID: Lazy<PlanetId> =
    Lazy::new(|| PlanetId::try_parse(GALAXY_ROOT_STR).unwrap());
pub const DEFAULT_PLANET_ID: Lazy<PlanetId> =
    Lazy::new(|| PlanetId::try_parse(DEFAULT_PLANET_STR).unwrap());
pub const SOL_ID: Lazy<PlanetId> = Lazy::new(|| PlanetId::try_parse(SOL_STR).unwrap());

pub const MAX_TIREDNESS: f32 = MAX_SKILL;
pub const MAX_MORALE: f32 = MAX_SKILL;
pub const MORALE_DECREASE_PER_LONG_TICK: f32 = 0.5;
pub const MORALE_INCREASE_PER_GAME_PLAYER: f32 = 1.5;
pub const MORALE_BONUS_ON_TEAM_ADD: f32 = 10.0;
pub const MORALE_THRESHOLD_FOR_LEAVING: f32 = 8.0;
pub const MORALE_DEMOTION_MALUS: f32 = 1.0;
