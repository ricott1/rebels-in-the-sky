use super::{
    action::{ActionOutput, ActionSituation},
    game::Game,
    timer::{Period, Timer},
};
use rand_chacha::ChaCha8Rng;

#[derive(Debug, Default)]
pub struct EndOfQuarter;

impl EndOfQuarter {
    pub fn execute(
        &self,
        input: &ActionOutput,
        game: &Game,
        _rng: &mut ChaCha8Rng,
    ) -> Option<ActionOutput> {
        let end_of_quarter = input.start_at.period().end();
        let beginning_of_next_quarter = input.end_at.period().next().start();
        let description = match input.start_at.period() {
            Period::Q1 => format!("It's the end of the first quarter.",),
            Period::Q2 => format!("It's the end of the second quarter. Halftime!",),
            Period::Q3 => format!("It's the end of the third quarter.",),
            Period::Q4 => match input.home_score as  i16 - input.away_score as i16 {
                x if x > 0 => format!(
                    "It's the end of the game. {} won this nice game over {}. The final score is {} {}-{} {}.",
                    game.home_team_in_game.name,
                    game.away_team_in_game.name,
                    game.home_team_in_game.name,
                    input.home_score,
                    input.away_score,
                    game.away_team_in_game.name,
                ),
                x if x < 0 => format!(
                    "It's the end of the game. {} won this nice game over {}. The final score is {} {}-{} {}.",
                    game.away_team_in_game.name,
                    game.home_team_in_game.name,
                    game.home_team_in_game.name,
                    input.home_score,
                    input.away_score,
                    game.away_team_in_game.name,
                ),
                x if x == 0 => format!(
                    "It's a tie between {} and {}. The final score is {} {}-{} {}.",
                    game.home_team_in_game.name,
                    game.away_team_in_game.name,
                    game.home_team_in_game.name,
                    input.home_score,
                    input.away_score,
                    game.away_team_in_game.name,
                ),
                _ => panic!("Invalid score"),
            },
            _ => panic!("Invalid period {}", game.timer.period()),
        };

        let result = ActionOutput {
            possession: input.possession.clone(),
            situation: ActionSituation::EndOfQuarter,
            description,
            start_at: Timer::from(end_of_quarter),
            end_at: Timer::from(beginning_of_next_quarter),
            home_score: input.home_score,
            away_score: input.away_score,
            ..Default::default()
        };
        Some(result)
    }
}
