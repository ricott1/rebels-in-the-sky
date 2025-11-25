use super::{
    action::{ActionOutput, ActionSituation, EngineAction},
    game::Game,
    types::*,
};
use crate::world::{player::Player, skill::GameSkill};
use rand::Rng;
use rand_chacha::ChaCha8Rng;

#[derive(Debug, Default)]
pub struct JumpBall;

impl EngineAction for JumpBall {
    fn execute(
        input: &ActionOutput,
        game: &Game,
        action_rng: &mut ChaCha8Rng,
        _description_rng: &mut ChaCha8Rng,
    ) -> Option<ActionOutput> {
        let attacking_players = game.attacking_players();
        let defending_players = game.defending_players();

        let jump_ball = |player: &Player| {
            player.athletics.vertical.game_value()
                + ((player.info.height as u16).max(150) - 150) / 4
        };
        let home_jumper = attacking_players.iter().max_by_key(|&p| jump_ball(p));
        let away_jumper = defending_players.iter().max_by_key(|&p| jump_ball(p));

        let home_result = home_jumper?.roll(action_rng) + jump_ball(home_jumper?);
        let away_result = away_jumper?.roll(action_rng) + jump_ball(away_jumper?);

        let timer_increase = 6 + action_rng.random_range(0..=7);

        let result = match home_result as i16 - away_result as i16 {
            x if x > 0 => {
                ActionOutput {
                    //default possession is home team
                    possession: input.possession,
                    situation: ActionSituation::AfterDefensiveRebound,
                    description: format!(
                        "{} and {} prepare for the jump ball. {} wins the jump ball. {} will have the first possession.",
                        home_jumper?.info.short_name(),
                        away_jumper?.info.short_name(), home_jumper?.info.short_name(), game.home_team_in_game.name
                    ),
                    start_at: input.end_at,
                end_at: input.end_at.plus(timer_increase),
                home_score: input.home_score,
                    away_score: input.away_score,
                    ..Default::default()
                }
            }
            x if x < 0 => ActionOutput {
                possession: !input.possession,
                situation: ActionSituation::AfterDefensiveRebound,
                description: format!(
                    "{} and {} prepare for the jump ball. {} wins the jump ball. {} will have the first possession.",
                    home_jumper?.info.short_name(),
                    away_jumper?.info.short_name(),away_jumper?.info.short_name(), game.away_team_in_game.name
                ),
                start_at: input.end_at,
                end_at: input.end_at.plus(timer_increase),
                home_score: input.home_score,
                    away_score: input.away_score,
                ..Default::default()
            },
            _ => {
                let r = action_rng.random_range(0..=1);
                let ball_team = match r {
                    0 => game.home_team_in_game.name.clone(),
                    _ => game.away_team_in_game.name.clone(),
                };
                ActionOutput {
                    possession: match r {
                        0=>Possession::Home,
                        _=>Possession::Away
                    },
                    situation: ActionSituation::AfterDefensiveRebound,
                    description: format!(
                        "{} and {} prepare for the jump ball.\nNobody wins the jump ball, but {} hustles for it.",
                        home_jumper?.info.short_name(),
                        away_jumper?.info.short_name(), ball_team
                    ),
                    start_at: input.end_at,
                end_at: input.end_at.plus(timer_increase),
                home_score: input.home_score,
                    away_score: input.away_score,
                    ..Default::default()
                }
            }
        };
        Some(result)
    }
}
