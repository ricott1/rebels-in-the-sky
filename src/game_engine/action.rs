use super::{
    game::Game,
    timer::Timer,
    types::{GameStatsMap, Possession},
};
use crate::{
    backcompat_repr_u8_enum,
    core::{GamePosition, Player, MAX_GAME_POSITION},
    game_engine::{
        brawl, end_of_quarter, fastbreak, isolation, jump_ball, off_the_screen, pick_and_roll,
        post, rebound, shot, start_of_quarter,
    },
};
use core::fmt::Debug;
use rand_chacha::ChaCha8Rng;
use rand_distr::{weighted::WeightedIndex, Distribution};
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use strum::EnumIter;

// FIXME: migrate to repr
// #[derive(Debug, Default, PartialEq, Clone, Copy, Serialize, Deserialize)]
// pub enum Advantage {
//     Attack,
//     #[default]
//     Neutral,
//     Defense,
// }

backcompat_repr_u8_enum! {
    #[derive(Debug, EnumIter, PartialEq, Clone, Copy)]
    pub enum Advantage {
        Attack,
        Neutral,
        Defense,
    }
}

impl Default for Advantage {
    fn default() -> Self {
        Self::Neutral
    }
}

#[derive(Debug, Default, Serialize_repr, Deserialize_repr, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum ActionSituation {
    #[default]
    JumpBall,
    EndOfQuarter,
    BallInBackcourt,
    BallInMidcourt,
    AfterOffensiveRebound,
    AfterLongOffensiveRebound,
    AfterDefensiveRebound,
    AfterSubstitution,
    MissedShot,
    Turnover,
    CloseShot,
    MediumShot,
    LongShot,
    Fastbreak,
    ForcedOffTheScreenAction, // FIXME: would be better to use an interal enum property action: Action
}

#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq)]
pub struct ActionOutput {
    pub random_seed: [u8; 32],
    pub advantage: Advantage,
    pub attackers: Vec<usize>,
    pub defenders: Vec<usize>,
    pub assist_from: Option<usize>,
    pub situation: ActionSituation,
    pub description: String,
    pub start_at: Timer,
    pub end_at: Timer,
    pub attack_stats_update: Option<GameStatsMap>,
    pub defense_stats_update: Option<GameStatsMap>,
    pub foul_from: Option<usize>,
    pub foul_on: Option<usize>,
    pub home_score: u16,
    pub away_score: u16,
    pub score_change: u16,
    pub possession: Possession,
}

#[derive(Debug, Serialize_repr, Deserialize_repr, Clone, Default, PartialEq)]
#[repr(u8)]
pub enum Action {
    #[default]
    JumpBall,
    StartOfQuarter,
    EndOfQuarter,
    Isolation,
    PickAndRoll,
    OffTheScreen,
    Post,
    Brawl,
    Rebound,
    CloseShot,
    MediumShot,
    LongShot,
    Fastbreak,
}

impl Action {
    pub(crate) fn execute(
        &self,
        input: &ActionOutput,
        game: &Game,
        action_rng: &mut ChaCha8Rng,
        description_rng: &mut ChaCha8Rng,
    ) -> ActionOutput {
        let mut output = match self {
            Action::JumpBall => jump_ball::execute(input, game, action_rng, description_rng),
            Action::StartOfQuarter => {
                start_of_quarter::execute(input, game, action_rng, description_rng)
            }
            Action::EndOfQuarter => {
                end_of_quarter::execute(input, game, action_rng, description_rng)
            }
            Action::Isolation => isolation::execute(input, game, action_rng, description_rng),
            Action::PickAndRoll => pick_and_roll::execute(input, game, action_rng, description_rng),
            Action::OffTheScreen => {
                off_the_screen::execute(input, game, action_rng, description_rng)
            }
            Action::Post => post::execute(input, game, action_rng, description_rng),
            Action::Rebound => rebound::execute(input, game, action_rng, description_rng),
            Action::CloseShot => shot::execute_close_shot(input, game, action_rng, description_rng),
            Action::MediumShot => {
                shot::execute_medium_shot(input, game, action_rng, description_rng)
            }
            Action::LongShot => shot::execute_long_shot(input, game, action_rng, description_rng),
            Action::Brawl => brawl::execute(input, game, action_rng, description_rng),
            Action::Fastbreak => fastbreak::execute(input, game, action_rng, description_rng),
        };
        output.random_seed = action_rng.get_seed();
        output
    }
}

pub fn sample_player_index(
    rng: &mut ChaCha8Rng,
    mut weights: [GamePosition; MAX_GAME_POSITION as usize],
    players: [&Player; MAX_GAME_POSITION as usize],
) -> Option<usize> {
    for (idx, player) in players.iter().enumerate() {
        if player.is_knocked_out() {
            weights[idx] = 0;
        }
    }
    Some(WeightedIndex::new(weights).ok()?.sample(rng))
}
