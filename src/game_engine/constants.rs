use crate::core::constants::TirednessCost;

pub const RECOVERING_TIREDNESS_PER_SHORT_TICK: f32 = TirednessCost::LOW;
pub const MIN_TIREDNESS_FOR_ROLL_DECLINE: f32 = 10.0;
pub(crate) const MIN_TIREDNESS_FOR_SUB: f32 = 0.95 * MIN_TIREDNESS_FOR_ROLL_DECLINE;

pub const BASE_ATTENDANCE: u32 = 60;
pub(crate) const BRAWL_ACTION_PROBABILITY: f64 = 0.045;
pub(crate) const FASTBREAK_ACTION_PROBABILITY: f64 = 0.4;
pub(crate) const SUBSTITUTION_ACTION_PROBABILITY: f64 = 1.1;

pub(crate) const DUNK_PROBABILITY: f64 = 0.45;

// Action checks compare attacker and defender as
// NUMBER_OF_ROLLS + 2 player skill + 1 tactic skill
// The higher the number of rolls, the less relevant skills and tactics are.
pub(crate) const NUMBER_OF_ROLLS: usize = 9;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum ShotDifficulty {
    Close = NUMBER_OF_ROLLS as isize,
    Medium = 4 + 2 * NUMBER_OF_ROLLS as isize,
    Long = 16 + 2 * NUMBER_OF_ROLLS as isize,
}

// result:  <= STEAL_LIMIT/   <=ADV_DEFENSE_LIMIT/      <=ADV_NEUTRAL_LIMIT/       <=ADV_ATTACK_LIMIT/ ------------>
//               steal   /       turnover       / shot Advantage::Defense / shot Advantage::Neutral / shot Advantage::Attack
pub(crate) const ADV_ATTACK_LIMIT: i16 = 5 * NUMBER_OF_ROLLS as i16;
pub(crate) const ADV_NEUTRAL_LIMIT: i16 = 0;
pub(crate) const ADV_DEFENSE_LIMIT: i16 = -6 * NUMBER_OF_ROLLS as i16;
// Here we sum 4 cause in the steal check we also add the defender steal skill.
pub(crate) const STEAL_LIMIT: i16 = -13 * (NUMBER_OF_ROLLS as i16 + 4);
