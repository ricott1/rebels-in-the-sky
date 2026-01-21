use crate::game_engine::types::Possession;

use super::{
    action::{ActionOutput, ActionSituation},
    game::Game,
    timer::Period,
};
use rand::Rng;
use rand_chacha::ChaCha8Rng;

pub(crate) fn execute(
    input: &ActionOutput,
    game: &Game,
    action_rng: &mut ChaCha8Rng,
    _description_rng: &mut ChaCha8Rng,
) -> ActionOutput {
    let timer_increase = 6 + action_rng.random_range(0..=6);

    let possession = match input.end_at.period() {
        // Q2: Assign possession to team that did not win the jump ball
        Period::B1 => !game.won_jump_ball,
        // Q3: Assign possession to team that did not win the jump ball
        Period::B2 => !game.won_jump_ball,
        // Q4: Assign possession to team that won the jump ball
        Period::B3 => game.won_jump_ball,
        // OT: FIXME: OT are not handled atm
        _ => unreachable!(),
    };

    let description = match input.end_at.period() {
        Period::B1 => format!(
            "It's the start of the second quarter. {} got the possesion.",
            if possession == Possession::Home {
                &game.home_team_in_game.name
            } else {
                &game.away_team_in_game.name
            }
        ),
        Period::B2 => format!(
            "It's the start of the third quarter. {} will play the first ball.",
            if possession == Possession::Home {
                &game.home_team_in_game.name
            } else {
                &game.away_team_in_game.name
            }
        ),
        Period::B3 => format!(
            "It's the start of the last period. {} will get the first possession.",
            if possession == Possession::Home {
                &game.home_team_in_game.name
            } else {
                &game.away_team_in_game.name
            }
        ),
        _ => unreachable!("Invalid period {}", input.end_at.period()),
    };

    ActionOutput {
        situation: ActionSituation::BallInBackcourt,
        description,
        start_at: input.end_at,
        end_at: input.end_at.plus(timer_increase),
        possession,
        home_score: input.home_score,
        away_score: input.away_score,
        ..Default::default()
    }
}
