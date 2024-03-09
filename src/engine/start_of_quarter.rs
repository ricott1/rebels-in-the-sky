use super::{
    action::{ActionOutput, ActionSituation},
    game::Game,
    timer::Period,
};
use rand::Rng;
use rand_chacha::ChaCha8Rng;

#[derive(Debug, Default)]
pub struct StartOfQuarter;

impl StartOfQuarter {
    pub fn execute(
        input: &ActionOutput,
        game: &Game,
        rng: &mut ChaCha8Rng,
    ) -> Option<ActionOutput> {
        let timer_increase = 6 + rng.gen_range(0..=6);
        let description = match input.end_at.period() {
            Period::Q2 => format!("It's the start of the second quarter.",),
            Period::Q3 => format!("It's the start of the third quarter.",),
            Period::Q4 => format!("It's the start of the last period.",),
            _ => panic!("Invalid period {}", input.end_at.period()),
        };
        let possession = match input.end_at.period() {
            // Q2: Assign possession to team that did not win the jump ball
            Period::Q2 => !game.won_jump_ball.clone(),
            // Q3: Assign possession to team that did not win the jump ball
            Period::Q3 => !game.won_jump_ball.clone(),
            // Q4: Assign possession to team that won the jump ball
            Period::Q4 => game.won_jump_ball.clone(),
            // OT: FIXME: for the moment we just switch, but in reality OT are not handled atm
            _ => !game.won_jump_ball.clone(),
        };

        let result = ActionOutput {
            situation: ActionSituation::BallInBackcourt,
            description,
            start_at: input.end_at,
            end_at: input.end_at.plus(timer_increase),
            possession,
            home_score: input.home_score,
            away_score: input.away_score,
            ..Default::default()
        };
        Some(result)
    }
}
