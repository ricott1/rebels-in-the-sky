use super::{
    action::{ActionOutput, ActionSituation},
    game::Game,
    types::*,
};
use crate::{
    core::{player::Player, skill::GameSkill},
    game_engine::constants::ADV_NEUTRAL_LIMIT,
};
use rand::Rng;
use rand_chacha::ChaCha8Rng;

pub(crate) fn execute(
    input: &ActionOutput,
    game: &Game,
    action_rng: &mut ChaCha8Rng,
    _description_rng: &mut ChaCha8Rng,
) -> ActionOutput {
    let attacking_players_array = game.attacking_players_array();
    let defending_players_array = game.defending_players_array();

    let jump_ball = |player: &Player| {
        player.athletics.vertical.game_value() + (0.25 * (player.info.height - 150.0)).game_value()
    };
    let home_jumper = attacking_players_array
        .iter()
        .max_by_key(|&p| jump_ball(p))
        .expect("There should be a max");
    let away_jumper = defending_players_array
        .iter()
        .max_by_key(|&p| jump_ball(p))
        .expect("There should be a max");

    let home_result = home_jumper.roll(action_rng) + jump_ball(home_jumper);
    let away_result = away_jumper.roll(action_rng) + jump_ball(away_jumper);

    let timer_increase = 6 + action_rng.random_range(0..=7);

    let result = match home_result  - away_result  {
            x if x > ADV_NEUTRAL_LIMIT => {
                ActionOutput {
                    possession: Possession::Home,
                    situation: ActionSituation::AfterDefensiveRebound,
                    description: format!(
                        "{} and {} prepare for the jump ball. {} wins the jump ball. {} will have the first possession.",
                        home_jumper.info.short_name(),
                        away_jumper.info.short_name(), home_jumper.info.short_name(), game.home_team_in_game.name
                    ),
                    start_at: input.end_at,
                end_at: input.end_at.plus(timer_increase),
                home_score: input.home_score,
                    away_score: input.away_score,
                    ..Default::default()
                }
            }
            x if x < ADV_NEUTRAL_LIMIT => ActionOutput {
                possession: Possession::Away,
                situation: ActionSituation::AfterDefensiveRebound,
                description: format!(
                    "{} and {} prepare for the jump ball. {} wins the jump ball. {} will have the first possession.",
                    home_jumper.info.short_name(),
                    away_jumper.info.short_name(),away_jumper.info.short_name(), game.away_team_in_game.name
                ),
                start_at: input.end_at,
                end_at: input.end_at.plus(timer_increase),
                home_score: input.home_score,
                    away_score: input.away_score,
                ..Default::default()
            },
            _ => {
                let r = action_rng.random_bool(0.5);
                let ball_team = if r {
                    &game.home_team_in_game.name
                } else {
                    &game.away_team_in_game.name
                };
                ActionOutput {
                    possession: if r {
                        Possession::Home
                    } else {
                        Possession::Away
                    },
                    situation: ActionSituation::AfterDefensiveRebound,
                    description: format!(
                        "{} and {} prepare for the jump ball.\nNobody wins the jump ball, but {} hustles for it.",
                        home_jumper.info.short_name(),
                        away_jumper.info.short_name(), ball_team
                    ),
                    start_at: input.end_at,
                end_at: input.end_at.plus(timer_increase),
                home_score: input.home_score,
                    away_score: input.away_score,
                    ..Default::default()
                }
            }
        };
    result
}
