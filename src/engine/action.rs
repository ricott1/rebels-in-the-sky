use super::{
    end_of_quarter::EndOfQuarter,
    game::Game,
    isolation::Isolation,
    jump_ball::JumpBall,
    off_the_screen::OffTheScreen,
    pick_and_roll::PickAndRoll,
    post::Post,
    rebound::Rebound,
    shot::{CloseShot, LongShot, MediumShot},
    start_of_quarter::StartOfQuarter,
    substitution::Substitution,
    timer::Timer,
    types::{GameStatsMap, Possession},
};
use core::fmt::Debug;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

#[derive(Debug, Default, PartialEq, Clone, Copy, Serialize, Deserialize)]
pub enum Advantage {
    Attack,
    #[default]
    Neutral,
    Defense,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq)]
pub enum ActionSituation {
    #[default]
    JumpBall,
    EndOfQuarter,
    BallInBackcourt,
    BallInMidcourt,
    BallInFrontcourt,
    AfterOffensiveRebound,
    AfterDefensiveRebound,
    MissedShot,
    Turnover,
    FreeThrow,
    CloseShot,
    MediumShot,
    LongShot,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct ActionOutput {
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
    pub score_change: u8,
    pub possession: Possession,
}

#[derive(Debug, Serialize_repr, Deserialize_repr, Clone, Default)]
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
    Rebound,
    CloseShot,
    MediumShot,
    LongShot,
    Substitution,
}

impl Action {
    fn _name(&self) -> String {
        match self {
            Action::JumpBall => "Jump Ball".into(),
            Action::StartOfQuarter => "Start of Quarter".into(),
            Action::EndOfQuarter => "End of Quarter".into(),
            Action::Isolation => "Isolation".into(),
            Action::PickAndRoll => "Pick and Roll".into(),
            Action::OffTheScreen => "Off the Screen".into(),
            Action::Post => "Post".into(),
            Action::Rebound => "Rebound".into(),
            Action::CloseShot => "Close Shot".into(),
            Action::MediumShot => "Medium Shot".into(),
            Action::LongShot => "Long Shot".into(),
            Action::Substitution => "Substitution".into(),
        }
    }
    pub fn execute(
        &self,
        input: &ActionOutput,
        game: &Game,
        rng: &mut ChaCha8Rng,
    ) -> Option<ActionOutput> {
        match self {
            Action::JumpBall => JumpBall.execute(input, game, rng),
            Action::StartOfQuarter => StartOfQuarter.execute(input, game, rng),
            Action::EndOfQuarter => EndOfQuarter.execute(input, game, rng),
            Action::Isolation => Isolation.execute(input, game, rng),
            Action::PickAndRoll => PickAndRoll.execute(input, game, rng),
            Action::OffTheScreen => OffTheScreen.execute(input, game, rng),
            Action::Post => Post.execute(input, game, rng),
            Action::Rebound => Rebound.execute(input, game, rng),
            Action::CloseShot => CloseShot.execute(input, game, rng),
            Action::MediumShot => MediumShot.execute(input, game, rng),
            Action::LongShot => LongShot.execute(input, game, rng),
            Action::Substitution => Substitution.execute(input, game, rng),
        }
    }
}
