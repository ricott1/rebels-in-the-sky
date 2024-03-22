pub const ADV_ATTACK_LIMIT: i16 = 15;
pub const ADV_NEUTRAL_LIMIT: i16 = 0;
pub const ADV_DEFENSE_LIMIT: i16 = -20;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShotDifficulty {
    Close = 20,
    Medium = 27,
    Long = 35,
}

pub struct TirednessCost;
impl TirednessCost {
    pub const NONE: f32 = 0.0;
    pub const LOW: f32 = 0.025;
    pub const MEDIUM: f32 = 1.0;
    pub const HIGH: f32 = 2.5;
    pub const SEVERE: f32 = 10.0;
    pub const CRITICAL: f32 = 20.0;
    pub const MAX: f32 = 100.0;
}

pub const MAX_TIREDNESS: f32 = 100.0;
pub const RECOVERING_TIREDNESS_PER_SHORT_TICK: f32 = 0.05;
pub const MIN_TIREDNESS_FOR_SUB: f32 = 50.0;

pub const BASE_ATTENDANCE: u32 = 60;

pub const BRAWL_ACTION_PROBABILITY: f32 = 4.0;
