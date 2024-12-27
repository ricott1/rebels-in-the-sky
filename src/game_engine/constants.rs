use crate::world::constants::TirednessCost;

pub const ADV_ATTACK_LIMIT: i16 = 16;
pub const ADV_NEUTRAL_LIMIT: i16 = 0;
pub const ADV_DEFENSE_LIMIT: i16 = -20;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShotDifficulty {
    Close = 15,
    Medium = 22,
    Long = 28,
}

pub const RECOVERING_TIREDNESS_PER_SHORT_TICK: f32 = TirednessCost::LOW;
pub const MIN_TIREDNESS_FOR_SUB: f32 = 10.0;
pub const MIN_TIREDNESS_FOR_ROLL_DECLINE: f32 = 10.0;

pub const BASE_ATTENDANCE: u32 = 60;
pub const BRAWL_ACTION_PROBABILITY: f32 = 0.06;
