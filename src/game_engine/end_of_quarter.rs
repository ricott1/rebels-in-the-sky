use super::{
    action::{ActionOutput, ActionSituation, EngineAction},
    game::Game,
    timer::{Period, Timer},
};
use rand_chacha::ChaCha8Rng;

#[derive(Debug, Default)]
pub struct EndOfQuarter;

impl EngineAction for EndOfQuarter {
    fn execute(input: &ActionOutput, game: &Game, _rng: &mut ChaCha8Rng) -> Option<ActionOutput> {
        // This is executed at the beginning of a break
        let mut description = match game.timer.period() {
            Period::B1 => "It's the end of the first quarter.".to_string(),
            Period::B2 => "It's the end of the second quarter. Halftime!".to_string(),
            Period::B3 => "It's the end of the third quarter.".to_string(),
            Period::B4 => "It's the end of the game.".to_string(),
            _ => panic!("Invalid period {}", game.timer.period()),
        };

        match input.situation {
            ActionSituation::CloseShot
            | ActionSituation::MediumShot
            | ActionSituation::LongShot => {
                let shooter = game.attacking_players()[input.attackers[0]];
                description.push_str(
                    format!(
                        " {} didn't get to shoot in time.",
                        shooter.info.short_name()
                    )
                    .as_str(),
                );
            }
            _ => {}
        }

        let result = ActionOutput {
            possession: input.possession,
            situation: ActionSituation::EndOfQuarter,
            description,
            start_at: Timer::from(game.timer.period().previous().end()),
            end_at: Timer::from(game.timer.period().end()),
            home_score: input.home_score,
            away_score: input.away_score,
            ..Default::default()
        };
        Some(result)
    }
}
