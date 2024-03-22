use crate::types::{PlanetId, Tick, AU, HOURS, KILOMETERS, LIGHT_YEAR, MINUTES, SECONDS};
use once_cell::sync::Lazy;

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

const DEBUG_TIME_MULTIPLIER: Tick = 1;
pub const BASE_DISTANCES: [u128; 3] = [1 * LIGHT_YEAR, 1 * AU, 400_000 * KILOMETERS];
pub const BASE_TANK_CAPACITY: u32 = 50;
pub const BASE_SPEED: f32 =
    2_500_000_000.0 * KILOMETERS as f32 / HOURS as f32 * DEBUG_TIME_MULTIPLIER as f32; // Very fast ;)
pub const BASE_FUEL_CONSUMPTION: f32 = 25.0 / HOURS as f32 * DEBUG_TIME_MULTIPLIER as f32; // 1 TONNES per HOURS
pub const LANDING_TIME_OVERHEAD: Tick = 5 * MINUTES / DEBUG_TIME_MULTIPLIER;

pub const BASE_BONUS: f32 = 0.5;
pub const BONUS_PER_SKILL: f32 = 0.1;

pub const REPUTATION_BONUS_WINNER: f32 = 0.5;
pub const REPUTATION_BONUS_LOSER: f32 = -0.2;
pub const REPUTATION_BONUS_DRAW: f32 = 0.25;

pub const BASE_EXPLORATION_TIME: Tick = 10 * SECONDS;

pub struct TickInterval;
impl TickInterval {
    pub const SHORT: Tick = 1 * SECONDS / DEBUG_TIME_MULTIPLIER;
    pub const MEDIUM: Tick = 1 * MINUTES / DEBUG_TIME_MULTIPLIER;
    pub const LONG: Tick = 24 * HOURS / DEBUG_TIME_MULTIPLIER;
}

pub const BASE_GAME_START_DELAY: Tick = 10 * SECONDS;

static GALAXY_ROOT_STR: &str = "71a43700-0000-0000-0000-000000000000";
static DEFAULT_PLANET_STR: &str = "71a43700-0000-0000-0002-000000000000";
static SOL_STR: &str = "71a43700-0000-0000-0001-000000000000";
pub const GALAXY_ROOT_ID: Lazy<PlanetId> =
    Lazy::new(|| PlanetId::try_parse(GALAXY_ROOT_STR).unwrap());
pub const DEFAULT_PLANET_ID: Lazy<PlanetId> =
    Lazy::new(|| PlanetId::try_parse(DEFAULT_PLANET_STR).unwrap());
pub const SOL_ID: Lazy<PlanetId> = Lazy::new(|| PlanetId::try_parse(SOL_STR).unwrap());
