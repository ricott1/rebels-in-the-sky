use crate::world::constants::TirednessCost;

pub(crate) const RECOVERING_TIREDNESS_PER_SHORT_TICK: f32 = TirednessCost::LOW;
pub(crate) const MIN_TIREDNESS_FOR_ROLL_DECLINE: f32 = 10.0;
pub(crate) const MIN_TIREDNESS_FOR_SUB: f32 = MIN_TIREDNESS_FOR_ROLL_DECLINE;

pub const BASE_ATTENDANCE: u32 = 60;
pub(crate) const BRAWL_ACTION_PROBABILITY: f32 = 0.06;

pub(crate) const NUMBER_OF_ROLLS: usize = 9;
pub(crate) const TACTIC_MODIFIER_MULTIPLIER: i16 = 7;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum ShotDifficulty {
    Close = 6 * (NUMBER_OF_ROLLS as isize - 1),
    Medium = 6 + 7 * (NUMBER_OF_ROLLS as isize - 1),
    Long = 15 + 8 * (NUMBER_OF_ROLLS as isize - 1),
}

// FIXME: these limits should diverge away from zero uniformly
pub(crate) const ADV_ATTACK_LIMIT: i16 = 6 * (NUMBER_OF_ROLLS as i16 - 1);
pub(crate) const ADV_NEUTRAL_LIMIT: i16 = 0;
pub(crate) const ADV_DEFENSE_LIMIT: i16 = -6 * (NUMBER_OF_ROLLS as i16 - 1);
