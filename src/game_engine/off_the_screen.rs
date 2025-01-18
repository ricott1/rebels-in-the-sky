use super::{
    action::{Action, ActionOutput, ActionSituation, Advantage, EngineAction},
    constants::*,
    game::Game,
    types::*,
};
use crate::world::{
    constants::{MoraleModifier, TirednessCost},
    skill::GameSkill,
};
use rand::{seq::SliceRandom, Rng};
use rand_chacha::ChaCha8Rng;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct OffTheScreen;

impl EngineAction for OffTheScreen {
    fn execute(input: &ActionOutput, game: &Game, rng: &mut ChaCha8Rng) -> Option<ActionOutput> {
        let attacking_players = game.attacking_players();
        let defending_players = game.defending_players();

        let play_idx = match input.attackers.len() {
            0 => Self::sample(rng, [6, 1, 2, 0, 0])?,
            _ => input.attackers[0],
        };
        let target_idx = match input.attackers.len() {
            2 => input.attackers[1],
            _ => {
                let mut weights = [1, 2, 3, 3, 2];
                weights[play_idx] = 0;
                Self::sample(rng, weights)?
            }
        };

        let playmaker = attacking_players[play_idx];
        let playmaker_defender = defending_players[play_idx];

        let target = attacking_players[target_idx];
        let target_defender = defending_players[target_idx];

        let mut attack_stats_update: GameStatsMap = HashMap::new();
        let mut playmaker_update = GameStats::default();
        playmaker_update.extra_tiredness = TirednessCost::MEDIUM;

        let mut defense_stats_update: GameStatsMap = HashMap::new();
        let mut target_defender_update = GameStats::default();
        target_defender_update.extra_tiredness = TirednessCost::MEDIUM;

        let timer_increase = 3 + rng.gen_range(0..=1);

        let atk_result = playmaker.roll(rng)
            + playmaker.mental.vision.value()
            + playmaker.technical.passing.value()
            + target.mental.intuition.value();

        let def_result = playmaker_defender.roll(rng)
            + target_defender.defense.perimeter_defense.value()
            + target_defender.athletics.quickness.value();

        let mut result = match atk_result as i16 - def_result as i16 + Self::tactic_modifier(game, &Action::OffTheScreen) {
            x if x > ADV_ATTACK_LIMIT => ActionOutput {
                possession: input.possession,
                advantage: Advantage::Attack,
                attackers: vec![target_idx],
                defenders: vec![target_idx],
                situation: ActionSituation::LongShot,
                description: [
                    format!(
                        "{} gets the pass from {} and is now open for the shot.",
                        target.info.shortened_name(), playmaker.info.shortened_name(),
                    ),
                    format!(
                        "{} catches the pass from {} and is wide open for a clean shot.",
                        target.info.shortened_name(), playmaker.info.shortened_name(),
                    ),
                    format!(
                        "{} receives the pass from {} and has a clear look at the basket.",
                        target.info.shortened_name(), playmaker.info.shortened_name(),
                    ),
                    format!(
                        "{} gets the ball from {} and steps into an open shot attempt.",
                        target.info.shortened_name(), playmaker.info.shortened_name(),
                    ),
                    format!(
                        "{} grabs the pass from {} and now has an easy opportunity for a shot.",
                        target.info.shortened_name(), playmaker.info.shortened_name(),
                    ),
                ].choose(rng).expect("There should be one option").clone(),
                assist_from: Some(play_idx),
                start_at: input.end_at,
                end_at: input.end_at.plus(timer_increase),
                home_score: input.home_score,
                    away_score: input.away_score,
                ..Default::default()
            },
            x if x > 0 => ActionOutput {
                possession: input.possession,
                advantage: Advantage::Neutral,
                attackers: vec![target_idx],
                defenders: vec![target_idx],
                situation: ActionSituation::LongShot,
                description: [
                    format!(
                        "{} passes to {} after the screen.",
                        playmaker.info.shortened_name(), target.info.shortened_name(),
                    ),
                    format!(
                        "{} finds {} open after the screen and makes the pass for a shot.",
                        playmaker.info.shortened_name(), target.info.shortened_name(),
                    ),
                    format!(
                        "{} passes to {} following a screen, setting up for a quick shot.",
                        playmaker.info.shortened_name(), target.info.shortened_name(),
                    ),
                    format!(
                        "{} uses the screen to get free, then passes to {} for the shot.",
                        playmaker.info.shortened_name(), target.info.shortened_name(),
                    ),
                    format!(
                        "{} passes to {} as they come off the screen for a look at the basket.",
                        playmaker.info.shortened_name(), target.info.shortened_name(),
                    ),
                ].choose(rng).expect("There should be one option").clone(),
                assist_from: Some(play_idx),
                start_at: input.end_at,
                end_at: input.end_at.plus(timer_increase),
                home_score: input.home_score,
                    away_score: input.away_score,
                ..Default::default()
            },
            x if x > ADV_DEFENSE_LIMIT => ActionOutput {
                possession: input.possession,
                advantage: Advantage::Defense,
                attackers: vec![target_idx],
                defenders: vec![target_idx],
                situation: ActionSituation::MediumShot,
                description: [
                    format!(
                        "{} passes to {} who tried to get free using the screen, but {} is all over {}.",
                        playmaker.info.shortened_name(), target.info.shortened_name(), target_defender.info.shortened_name(), target.info.pronouns.as_possessive()
                    ),
                    format!(
                        "{} attempts to shake off {} with the screen, but {} sticks to {} like glue.",
                        playmaker.info.shortened_name(), target.info.shortened_name(), target_defender.info.shortened_name(), target.info.pronouns.as_possessive()
                    ),
                    format!(
                        "{} tries to use the screen to get open for the shot, but {} is right there, forcing a bad attempt.",
                        playmaker.info.shortened_name(), target.info.shortened_name(),
                                        ),
                    format!(
                        "{} receives the pass from {} but can't escape {}'s tight defense, resulting in a rushed shot.",
                        playmaker.info.shortened_name(), target.info.shortened_name(), target_defender.info.shortened_name()
                    ),
                    format!(
                        "{} gets the pass after the screen, but {} doesn't give an inch, and the shot is off balance.",
                        playmaker.info.shortened_name(), target.info.shortened_name(),
                    ),
                ].choose(rng).expect("There should be one option").clone(),
                assist_from: Some(play_idx),
                start_at: input.end_at,
                end_at: input.end_at.plus(timer_increase),
                home_score: input.home_score,
                    away_score: input.away_score,
                ..Default::default()
            },
            _ => {
                playmaker_update.turnovers = 1;
                target_defender_update.steals = 1;
                playmaker_update.extra_morale += MoraleModifier::SMALL_MALUS;
                target_defender_update.extra_morale += MoraleModifier::MEDIUM_BONUS;


                ActionOutput {
                    situation: ActionSituation::Turnover,
                    possession: !input.possession,
                    description:[
                        format!(
                            "{} tries to pass to {} off-the-screen but {} blocks the pass.",
                            playmaker.info.shortened_name(), target.info.shortened_name(), target_defender.info.shortened_name()
                        ),
                        format!(
                            "{} attempts the pass to {} after the screen, but {} jumps in the way, blocking it.",
                            playmaker.info.shortened_name(), target.info.shortened_name(), target_defender.info.shortened_name()
                        ),
                        format!(
                            "{} looks for {} off the screen, but {} intercepts the pass with perfect timing.",
                            playmaker.info.shortened_name(), target.info.shortened_name(), target_defender.info.shortened_name()
                        ),
                        format!(
                            "{} tries to feed the ball to {} after the screen, but {} steals it away.",
                            playmaker.info.shortened_name(), target.info.shortened_name(), target_defender.info.shortened_name()
                        ),
                        format!(
                            "{} passes to {} off the screen, but the pass is too high and goes out of bounds.",
                            playmaker.info.shortened_name(), target.info.shortened_name()
                        ),
                    ].choose(rng).expect("There should be one option").clone(),
                    start_at: input.end_at,
                end_at: input.end_at.plus(3),
                home_score: input.home_score,
                    away_score: input.away_score,
                    ..Default::default()
                }
            }
        };

        attack_stats_update.insert(playmaker.id, playmaker_update);
        defense_stats_update.insert(target_defender.id, target_defender_update);
        result.attack_stats_update = Some(attack_stats_update);
        result.defense_stats_update = Some(defense_stats_update);
        Some(result)
    }
}
