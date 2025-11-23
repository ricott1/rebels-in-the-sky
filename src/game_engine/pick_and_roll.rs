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
use rand::{seq::IndexedRandom, Rng};
use rand_chacha::ChaCha8Rng;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct PickAndRoll;

impl EngineAction for PickAndRoll {
    fn execute(input: &ActionOutput, game: &Game, rng: &mut ChaCha8Rng) -> Option<ActionOutput> {
        let attacking_players = game.attacking_players();
        let defending_players = game.defending_players();

        let play_idx = match input.attackers.len() {
            0 => Self::sample(rng, [6, 1, 2, 0, 0])?,
            _ => input.attackers[0],
        };

        let target_idx = match input.attackers.len() {
            0 | 1 => Self::sample(rng, [1, 2, 3, 3, 2])?,
            _ => input.attackers[1],
        };

        let playmaker = attacking_players[play_idx];
        let playmaker_defender = defending_players[play_idx];

        let target = attacking_players[target_idx];
        let target_defender = defending_players[target_idx];

        let mut attack_stats_update: GameStatsMap = HashMap::new();
        let mut playmaker_update = GameStats::default();
        playmaker_update.extra_tiredness = TirednessCost::MEDIUM;

        let mut defense_stats_update: GameStatsMap = HashMap::new();
        let mut playmaker_defender_update = GameStats::default();
        playmaker_defender_update.extra_tiredness = TirednessCost::MEDIUM;

        let mut target_defender_update = GameStats::default();
        target_defender_update.extra_tiredness = TirednessCost::MEDIUM;

        let timer_increase = 3 + rng.random_range(0..=3);
        let mut result: ActionOutput;

        if play_idx == target_idx {
            let atk_result = playmaker.roll(rng)
                + playmaker.technical.ball_handling.game_value()
                + playmaker.athletics.quickness.game_value()
                + target.mental.vision.game_value();

            let def_result = playmaker_defender.roll(rng)
                + playmaker_defender.defense.perimeter_defense.game_value()
                + playmaker_defender.mental.vision.game_value();

            result = match atk_result as i16 - def_result as i16 + Self::tactic_modifier(game, &Action::PickAndRoll) {
                x if x >= ADV_ATTACK_LIMIT => ActionOutput {
                    possession: input.possession,
                    advantage: Advantage::Attack,
                    attackers: vec![play_idx],
                    defenders: vec![play_idx],
                    situation: ActionSituation::LongShot,
                    description: [
                        format!(
                            "{} uses the screen perfectly and is now open for the shot.",
                            playmaker.info.short_name()
                        ),
                        format!(
                            "{} navigates {}'s screen flawlessly and gets wide open for the shot.",
                            playmaker.info.short_name(), target.info.short_name()
                        ),
                        format!(
                            "{} reads the defense well and uses the screen to get free for an open shot.",
                            playmaker.info.short_name()
                        ),
                        format!(
                            "{} uses the pick to perfection, getting a clean look at the basket.",
                            playmaker.info.short_name()
                        ),
                        format!(
                            "{} takes full advantage of {}'s screen and has an easy opportunity for a shot.",
                            playmaker.info.short_name(), target.info.short_name()
                        ),
                    ].choose(rng).expect("There should be one option").clone(),
                    start_at: input.end_at,
                        end_at: input.end_at.plus(timer_increase),
                        home_score: input.home_score,
                    away_score: input.away_score,
                    ..Default::default()
                },
                x if x > ADV_NEUTRAL_LIMIT => ActionOutput {
                    possession: input.possession,
                    advantage: Advantage::Neutral,
                    attackers: vec![play_idx],
                    defenders: vec![play_idx],
                    situation: ActionSituation::LongShot,
                    description: [
                        format!(
                            "They go for the pick'n'roll. {} goes through {}'s screen and manages to get a bit of space to shot.",
                            playmaker.info.short_name(), target.info.short_name()
                        ),
                        format!(
                            "The pick'n'roll is set up. {} uses {}'s screen to create just enough separation for a shot.",
                            playmaker.info.short_name(), target.info.short_name()
                        ),
                        format!(
                            "The pick'n'roll play is in motion. {} fights through {}'s screen and gets a little space to shoot.",
                            playmaker.info.short_name(), target.info.short_name()
                        ),
                        format!(
                            "They run the pick'n'roll. {} navigates through {}'s screen and manages a slight opening for the shot.",
                            playmaker.info.short_name(), target.info.short_name()
                        ),
                        format!(
                            "In the pick'n'roll, {} uses {}'s screen and finds just enough room to take a shot.",
                            playmaker.info.short_name(), target.info.short_name()
                        ),
                    ].choose(rng).expect("There should be one option").clone(),
                    start_at: input.end_at,
                        end_at: input.end_at.plus(timer_increase),
                        home_score: input.home_score,
                    away_score: input.away_score,
                    ..Default::default()
                },
                x if x > ADV_DEFENSE_LIMIT => {
                    match rng.random_bool(0.5) {
                        false => ActionOutput {
                            possession: input.possession,
                            advantage: Advantage::Defense,
                            attackers: vec![play_idx],
                            defenders: vec![target_idx],
                            situation: ActionSituation::LongShot,
                            description:[
                                format!(
                                    "{} tries to use the screen but {} slides nicely to cover.",
                                    playmaker.info.short_name(), target_defender.info.short_name()
                                ),
                                format!(
                                    "{} eludes {}'s screen and slides to cover {}.",
                                    target_defender.info.short_name(), target.info.short_name(),playmaker.info.short_name()
                                ),
                                format!(
                                    "{} tries to move past {} using the screen but {} swaps cover and is all over {}.",
                                    playmaker.info.short_name(), playmaker_defender.info.short_name(),target_defender.info.short_name(),playmaker.info.pronouns.as_object()
                                ),
                            ] .choose(rng).expect("There should be one option").clone(),
                            start_at: input.end_at,
                                end_at: input.end_at.plus(timer_increase),
                                home_score: input.home_score,
                            away_score: input.away_score,
                            ..Default::default()
                        },
                        true => ActionOutput {
                            possession: input.possession,
                            advantage: Advantage::Defense,
                            attackers: vec![play_idx],
                            defenders: vec![play_idx],
                            situation: ActionSituation::LongShot,
                            description:[
                                format!(
                                    "{} attempts to navigate the screen, but {} stays right with {}, denying the space.",
                                    playmaker.info.short_name(), playmaker_defender.info.short_name(), playmaker.info.pronouns.as_object()
                                ),
                                format!(
                                    "{} goes for the screen, but {} expertly fights through, staying tight on defense.",
                                    playmaker.info.short_name(), playmaker_defender.info.short_name()
                                ),
                                format!(
                                    "{} tries to use the pick, but {} anticipates the move and stays in front.",
                                    playmaker.info.short_name(), playmaker_defender.info.short_name()
                                ),
                                format!(
                                    "{} tries to get open off the screen, but {} moves with him step for step, preventing any separation.",
                                    playmaker.info.short_name(), playmaker_defender.info.short_name()
                                ),
                            ] .choose(rng).expect("There should be one option").clone(),
                            start_at: input.end_at,
                                end_at: input.end_at.plus(timer_increase),
                                home_score: input.home_score,
                            away_score: input.away_score,
                            ..Default::default()
                        }
                    }
            },
                _ => {
                    playmaker_update.turnovers = 1;
                    target_defender_update.steals = 1;
                    playmaker_update.extra_morale += MoraleModifier::SMALL_MALUS;
                    target_defender_update.extra_morale += MoraleModifier::MEDIUM_BONUS;

                    ActionOutput {
                        situation: ActionSituation::Turnover,
                        possession: !input.possession,
                        description: [
                            format!(
                                "{} tries to use the screen but {} snatches the ball from {} hands.",
                                playmaker.info.short_name(), playmaker_defender.info.short_name(), playmaker.info.pronouns.as_possessive()
                            ),
                            format!(
                                "{} attempts to use the screen, but {} swipes the ball right out of {} hands.",
                                playmaker.info.short_name(), playmaker_defender.info.short_name(), playmaker.info.pronouns.as_possessive()
                            ),
                            format!(
                                "{} tries to get open with the screen, but {} anticipates the play and steals the ball.",
                                playmaker.info.short_name(), playmaker_defender.info.short_name()
                            ),
                            format!(
                                "{} attempts the screen play, but {} jumps the passing lane and takes the ball.",
                                playmaker.info.short_name(), target_defender.info.short_name()
                            ),
                            format!(
                                "{} goes for the screen, but {} is quick to jump in, stealing the ball away from {}.",
                                playmaker.info.short_name(), target_defender.info.short_name(), playmaker.info.pronouns.as_possessive()
                            ),
                        ].choose(rng).expect("There should be one option").clone(),
                        start_at: input.end_at,
                end_at: input.end_at.plus(3 + rng.random_range(0..=1)),
                home_score: input.home_score,
                    away_score: input.away_score,
                        ..Default::default()
                    }
                }
            };
        } else {
            let atk_result = playmaker.roll(rng)
                + playmaker.technical.ball_handling.game_value()
                + playmaker.technical.passing.game_value()
                + target.mental.intuition.game_value();

            let def_result = playmaker_defender.roll(rng)
                + playmaker_defender.defense.perimeter_defense.game_value()
                + target_defender.athletics.quickness.game_value();

            result = match atk_result as i16 - def_result as i16 + Self::tactic_modifier(game, &Action::PickAndRoll){
            x if x >= ADV_ATTACK_LIMIT => ActionOutput {
                possession: input.possession,
                advantage: Advantage::Attack,
                attackers: vec![target_idx],
                defenders: vec![play_idx],
                situation: ActionSituation::CloseShot,
                description: [
                    format!(
                        "{} and {} execute the pick'n'roll perfectly! {} is now open for the shot.",
                        playmaker.info.short_name(), target.info.short_name(), target.info.short_name()
                    ),
                    format!(
                        "{} and {} run the pick'n'roll to perfection, and now {} has a wide-open shot.",
                        playmaker.info.short_name(), target.info.short_name(), target.info.short_name()
                    ),
                    format!(
                        "{} and {} work the pick'n'roll flawlessly! {} is left with a clean look at the basket.",
                        playmaker.info.short_name(), target.info.short_name(), target.info.short_name()
                    ),
                    format!(
                        "{} and {} execute the pick'n'roll to perfection, freeing {} for an easy shot attempt.",
                        playmaker.info.short_name(), target.info.short_name(), target.info.short_name()
                    ),
                    format!(
                        "{} and {} perform a flawless pick'n'roll, and now {} is in prime position for the shot.",
                        playmaker.info.short_name(), target.info.short_name(), target.info.short_name()
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
                defenders: vec![play_idx],
                situation: ActionSituation::CloseShot,
                description:[
                    format!(
                        "They go for the pick'n'roll. {} passes to {} and is now ready to shoot.",
                        playmaker.info.short_name(), target.info.short_name(),
                    ),
                    format!(
                        "The pick'n'roll is executed smoothly. {} passes to {} who now has an opportunity to shoot.",
                        playmaker.info.short_name(), target.info.short_name(),
                    ),
                    format!(
                        "They run the pick'n'roll, and after a solid pass from {} to {}, the shot is ready.",
                        playmaker.info.short_name(), target.info.short_name(),
                    ),
                    format!(
                        "Nice pick'n'roll execution. {} delivers a pass to {} who's now in position to take the shot.",
                        playmaker.info.short_name(), target.info.short_name(),
                    ),
                    format!(
                        "The pick'n'roll is set up well, and {} passes to {} who prepares for the shot.",
                        playmaker.info.short_name(), target.info.short_name(),
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
                description:[
                    format!(
                        "They go for the pick'n'roll. {} passes to {} but {} is all over {}.",
                        playmaker.info.short_name(), target.info.short_name(), target_defender.info.short_name(), target.info.pronouns.as_object()
                    ),
                    format!(
                        "The pick'n'roll is executed, but {} is quick to cover as {} passes to {}.",
                       target_defender.info.short_name(), playmaker.info.short_name(), target.info.short_name(),
                    ),
                    format!(
                        "They try the pick'n'roll. {} passes to {} but {} sticks to {} like glue.",
                        playmaker.info.short_name(), target.info.short_name(), target_defender.info.short_name(), target.info.pronouns.as_object()
                    ),
                    format!(
                        "They run the pick'n'roll, but {} anticipates it perfectly, covering {} as soon as the pass is made.",
                        target_defender.info.short_name(), target.info.short_name()
                    ),
                    format!(
                        "On the pick'n'roll, {} passes to {} but {} is right there, denying any space for a shot.",
                        playmaker.info.short_name(), target.info.short_name(), target_defender.info.short_name()
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
                playmaker_defender_update.steals = 1;
                playmaker_update.extra_morale += MoraleModifier::SMALL_MALUS;
                playmaker_defender_update.extra_morale += MoraleModifier::MEDIUM_BONUS;


                ActionOutput {
                    situation: ActionSituation::Turnover,
                    possession: !input.possession,
                    description:[
                        format!(
                            "They go for the pick'n'roll but the defender reads that perfectly. {} tries to pass to {} but {} blocks the pass.",
                            playmaker.info.short_name(), target.info.short_name(), playmaker_defender.info.short_name()
                        ),
                        format!(
                            "On the pick'n'roll, the defender anticipates the play perfectly. {} tries to pass to {} but {} deflects the ball.",
                            playmaker.info.short_name(), target.info.short_name(), playmaker_defender.info.short_name()
                        ),
                        format!(
                            "They attempt the pick'n'roll, but the defender reads it like a book. {} passes to {} but {} blocks the pass attempt.",
                            playmaker.info.short_name(), target.info.short_name(), playmaker_defender.info.short_name()
                        ),
                        format!(
                            "The pick'n'roll is set, but {} sees it coming and blocks the pass to {}.",
                            playmaker_defender.info.short_name(), target.info.short_name()
                        ),
                        format!(
                            "They try the pick'n'roll, but {} reads the move perfectly, blocking {}'s pass to {}.",
                            playmaker_defender.info.short_name(), playmaker.info.short_name(), target.info.short_name()
                        ),
                    ].choose(rng).expect("There should be one option").clone(),
                    start_at: input.end_at,
                end_at: input.end_at.plus(2+ rng.random_range(0..=1)),
                home_score: input.home_score,
                    away_score: input.away_score,
                    ..Default::default()
                }
            }
        };
        }
        attack_stats_update.insert(playmaker.id, playmaker_update);
        defense_stats_update.insert(playmaker_defender.id, playmaker_defender_update);
        defense_stats_update.insert(target_defender.id, target_defender_update);
        result.attack_stats_update = Some(attack_stats_update);
        result.defense_stats_update = Some(defense_stats_update);
        Some(result)
    }
}
