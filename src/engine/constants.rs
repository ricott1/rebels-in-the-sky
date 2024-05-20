pub const ADV_ATTACK_LIMIT: i16 = 15;
pub const ADV_NEUTRAL_LIMIT: i16 = 0;
pub const ADV_DEFENSE_LIMIT: i16 = -20;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShotDifficulty {
    Close = 18,
    Medium = 27,
    Long = 36,
}

pub struct TirednessCost;
impl TirednessCost {
    pub const NONE: f32 = 0.0;
    pub const LOW: f32 = 0.005;
    pub const MEDIUM: f32 = 0.15;
    pub const HIGH: f32 = 0.45;
    pub const SEVERE: f32 = 2.0;
    pub const CRITICAL: f32 = 4.0;
    pub const MAX: f32 = 20.0;
}

pub const RECOVERING_TIREDNESS_PER_SHORT_TICK: f32 = 0.01;
pub const MIN_TIREDNESS_FOR_SUB: f32 = 10.0;
pub const MIN_TIREDNESS_FOR_ROLL_DECLINE: f32 = 4.0;

pub const BASE_ATTENDANCE: u32 = 60;

pub const BRAWL_ACTION_PROBABILITY: f32 = 0.06;
