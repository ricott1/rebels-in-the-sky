use super::{action::*, constants::*, game::Game, types::*};
use crate::core::{
    constants::{MoraleModifier, TirednessCost},
    skill::GameSkill,
};
use rand::{seq::IndexedRandom, Rng};
use rand_chacha::ChaCha8Rng;
use std::collections::HashMap;

pub(crate) fn execute(
    input: &ActionOutput,
    game: &Game,
    action_rng: &mut ChaCha8Rng,
    description_rng: &mut ChaCha8Rng,
) -> ActionOutput {
    let attacking_players_array = game.attacking_players_array();
    let defending_players_array = game.defending_players_array();

    let play_idx = match input.attackers.len() {
        0 => {
            let weights = [50, 20, 25, 2, 1];
            if let Some(idx) = sample_player_index(action_rng, weights, attacking_players_array) {
                idx
            } else {
                return ActionOutput {
                    situation: ActionSituation::Turnover,
                    possession: !input.possession,
                    description: format!(
                    "Wow! No player of {} is left standing, they just turned the ball over like that!",
                    game.attacking_team().name,
                ),
                    start_at: input.end_at,
                    end_at: input.end_at.plus(4 + action_rng.random_range(0..=3)),
                    home_score: input.home_score,
                    away_score: input.away_score,
                    ..Default::default()
                };
            }
        }
        _ => input.attackers[0],
    };

    let target_idx = match input.attackers.len() {
        2 => input.attackers[1],
        _ => {
            let mut weights = [1, 2, 3, 3, 2];
            weights[play_idx] = 0;
            if let Some(idx) = sample_player_index(action_rng, weights, attacking_players_array) {
                idx
            } else {
                return ActionOutput {
                    situation: ActionSituation::Turnover,
                    possession: !input.possession,
                    description: format!(
                    "Oh no! {}'s players can't coordinate together as there's only one player left standing, they just turned the ball over!",
                    game.attacking_team().name,
                ),
                    start_at: input.end_at,
                    end_at: input.end_at.plus(4 + action_rng.random_range(0..=3)),
                    home_score: input.home_score,
                    away_score: input.away_score,
                    ..Default::default()
                };
            }
        }
    };

    let screener_idx = match input.attackers.len() {
        3 => Some(input.attackers[2]),
        _ => None,
    };

    let playmaker = attacking_players_array[play_idx];
    let playmaker_defender = defending_players_array[play_idx];

    let target = attacking_players_array[target_idx];
    let target_defender = defending_players_array[target_idx];

    let screener = screener_idx.map(|idx| attacking_players_array[idx]);

    let mut attack_stats_update: GameStatsMap = HashMap::new();
    let mut playmaker_update = GameStats {
        extra_tiredness: TirednessCost::MEDIUM,
        ..Default::default()
    };
    let target_update = GameStats {
        extra_tiredness: TirednessCost::MEDIUM,
        ..Default::default()
    };

    let mut defense_stats_update: GameStatsMap = HashMap::new();
    let mut playmaker_defender_update = GameStats {
        extra_tiredness: TirednessCost::MEDIUM,
        ..Default::default()
    };
    let mut target_defender_update = GameStats {
        extra_tiredness: TirednessCost::MEDIUM,
        ..Default::default()
    };

    let timer_increase = 4 + action_rng.random_range(0..=2);

    let atk_result = playmaker.roll(action_rng)
        + playmaker.technical.passing.game_value()
        + (0.5 * playmaker.mental.vision + 0.5 * target.mental.intuition).game_value()
        + game
            .attacking_team()
            .tactic
            .attack_roll_bonus(&Action::OffTheScreen);

    let def_result = playmaker_defender.roll(action_rng)
        + playmaker_defender.defense.perimeter_defense.game_value()
        + (0.5 * target_defender.athletics.quickness
            + 0.25 * target_defender.mental.intuition
            + 0.25 * target_defender.defense.steal)
            .game_value()
        + game
            .defending_team()
            .tactic
            .defense_roll_bonus(&Action::OffTheScreen);

    let mut result = match atk_result  - def_result  {
            x if x >= ADV_ATTACK_LIMIT => ActionOutput {
                possession: input.possession,
                advantage: Advantage::Attack,
                attackers: vec![target_idx],
                defenders: vec![target_idx],
                situation: ActionSituation::LongShot,
                description: if let Some(s) = screener {[
                    format!(
                        "{} gets the pass from {} after {}'s screen and is now open for the shot.",
                        target.info.short_name(), playmaker.info.short_name(), s.info.short_name(),
                    ),
                    format!(
                        "{} uses {}'s screen and passes to {} who is wide open for a clean shot.",
                        playmaker.info.short_name(), s.info.short_name(),target.info.short_name(),
                    ),
                    format!(
                        "{} receives the pass from {} and has a clear look at the basket. They played the triangle with {} perfectly.",
                        target.info.short_name(), playmaker.info.short_name(),s.info.short_name()
                    ),
                    format!(
                        "{} gets the ball from {} after {}'s screen and steps into an open shot attempt.",
                        target.info.short_name(), playmaker.info.short_name(), s.info.short_name()
                    ),
                    format!(
                        "{}'s great vision opened up {} after {}'s scren.",
                        playmaker.info.short_name(), target.info.short_name(),s.info.short_name()
                    ),
                ]} else {[
                    format!(
                        "{} gets the pass from {} and is now open for the shot.",
                        target.info.short_name(), playmaker.info.short_name(),
                    ),
                    format!(
                        "{} catches the pass from {} and is wide open for a clean shot.",
                        target.info.short_name(), playmaker.info.short_name(),
                    ),
                    format!(
                        "{} receives the pass from {} and has a clear look at the basket.",
                        target.info.short_name(), playmaker.info.short_name(),
                    ),
                    format!(
                        "{} gets the ball from {} and steps into an open shot attempt.",
                        target.info.short_name(), playmaker.info.short_name(),
                    ),
                    format!(
                        "{} grabs the pass from {} and now has an easy opportunity for a shot.",
                        target.info.short_name(), playmaker.info.short_name(),
                    ),
                ]}.choose(description_rng).expect("There should be one option").clone(),
                assist_from: Some(play_idx),
                start_at: input.end_at,
                end_at: input.end_at.plus(timer_increase),
                home_score: input.home_score,
                    away_score: input.away_score,
                ..Default::default()
            },
            x if x >= ADV_NEUTRAL_LIMIT => ActionOutput {
                possession: input.possession,
                advantage: Advantage::Neutral,
                attackers: vec![target_idx],
                defenders: vec![target_idx],
                situation: ActionSituation::LongShot,
                description: if let Some(s) = screener {[
                    format!(
                        "{} passes to {} after the screen from {}.",
                        playmaker.info.short_name(), target.info.short_name(),s.info.short_name()
                    ),
                    format!(
                        "{} finds {} open after the {}'s screen and makes the pass for a shot.",
                        playmaker.info.short_name(), target.info.short_name(),s.info.short_name()
                    ),
                    format!(
                        "{} passes to {} following a screen by {}, setting up for a quick shot.",
                        playmaker.info.short_name(), target.info.short_name(),s.info.short_name()
                    ),
                    format!(
                        "{} uses {}'s screen to get free, then passes to {} for the shot.",
                        playmaker.info.short_name(),s.info.short_name(), target.info.short_name(),
                    ),
                    format!(
                        "{} passes to {} as {} come off {}'s screen for a look at the basket.",
                        playmaker.info.short_name(), target.info.short_name(), target.info.pronouns.as_subject().to_lowercase(),s.info.short_name(),
                    ),
                ]}else{[
                    format!(
                        "{} passes to {} after the screen.",
                        playmaker.info.short_name(), target.info.short_name(),
                    ),
                    format!(
                        "{} finds {} open after the screen and makes the pass for a shot.",
                        playmaker.info.short_name(), target.info.short_name(),
                    ),
                    format!(
                        "{} passes to {} following a screen, setting up for a quick shot.",
                        playmaker.info.short_name(), target.info.short_name(),
                    ),
                    format!(
                        "{} uses the screen to get free, then passes to {} for the shot.",
                        playmaker.info.short_name(), target.info.short_name(),
                    ),
                    format!(
                        "{} passes to {} as they come off the screen for a look at the basket.",
                        playmaker.info.short_name(), target.info.short_name(),
                    ),
                ]}.choose(description_rng).expect("There should be one option").clone(),
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
                        playmaker.info.short_name(), target.info.short_name(), target_defender.info.short_name(), target.info.pronouns.as_possessive()
                    ),
                    format!(
                        "{} attempts to shake off {} with the screen, but {} sticks to {} like glue.",
                        target.info.short_name(), target_defender.info.short_name(), target_defender.info.short_name(), target.info.pronouns.as_possessive()
                    ),
                    format!(
                        "{} tries to use the screen to get open for the shot, but {} is right there, forcing a bad attempt.",
                        target.info.short_name(), target_defender.info.short_name(),
                                        ),
                    format!(
                        "{} receives the pass from {} but can't escape {}'s tight defense, resulting in a rushed shot.",
                        target.info.short_name(), playmaker.info.short_name(), target_defender.info.short_name()
                    ),
                    format!(
                        "{} gets the pass after the screen, but {} doesn't give an inch, and the shot is off balance.",
                        target.info.short_name(), target_defender.info.short_name(),
                    ),
                ].choose(description_rng).expect("There should be one option").clone(),
                assist_from: Some(play_idx),
                start_at: input.end_at,
                end_at: input.end_at.plus(timer_increase),
                home_score: input.home_score,
                    away_score: input.away_score,
                ..Default::default()
            },
            _ => {
                playmaker_update.turnovers = 1;
                playmaker_update.extra_morale += MoraleModifier::SMALL_MALUS;
                playmaker_defender_update.extra_morale += MoraleModifier::SMALL_BONUS;

                // Equivalent to `- def_result - target_defender.defense.steal.game_value() <= STEAL_LIMIT`
                let with_steal = def_result + target_defender.defense.steal.game_value() >= -STEAL_LIMIT;

                if with_steal{
                    target_defender_update.steals = 1;
                    target_defender_update.extra_morale += MoraleModifier::MEDIUM_BONUS;
                    playmaker_update.extra_morale += MoraleModifier::MEDIUM_MALUS;
                }

                let situation = if with_steal && action_rng.random_bool(FASTBREAK_ACTION_PROBABILITY * game.defending_team().tactic.fastbreak_probability_modifier()){
                    ActionSituation::Fastbreak
                } else {
                    ActionSituation::Turnover
                };

                let attackers = if with_steal {
                    vec![target_idx]
                } else {vec![]};

                let end_at = if with_steal {
                    input.end_at.plus(1 +  action_rng.random_range(0..=2))
                } else {
                    input.end_at.plus(4 +  action_rng.random_range(0..=2))
                };

                let description = if with_steal {
                    vec![
                        format!(
                            "{} tries to pass to {} off-the-screen but {} blocks the pass.",
                            playmaker.info.short_name(), target.info.short_name(), target_defender.info.short_name()
                        ),
                        format!(
                            "{} attempts the pass to {} after the screen, but {} jumps in the way, blocking it.",
                            playmaker.info.short_name(), target.info.short_name(), target_defender.info.short_name()
                        ),
                        format!(
                            "{} looks for {} off the screen, but {} intercepts the pass with perfect timing.",
                            playmaker.info.short_name(), target.info.short_name(), target_defender.info.short_name()
                        ),
                        format!(
                            "{} tries to feed the ball to {} after the screen, but {} steals it away.",
                            playmaker.info.short_name(), target.info.short_name(), target_defender.info.short_name()
                        )
                    ]
                } else {
                    vec![
                        format!(
                            "{} passes to {} off the screen, but the pass is too high and goes out of bounds.",
                            playmaker.info.short_name(), target.info.short_name()
                        ),
                        format!(
                            "{} sends it to {}, but the pass is off the mark.",
                            playmaker.info.short_name(), target.info.short_name()
                        )
                    ]
                    }.choose(description_rng).expect("There should be one option").clone();

                ActionOutput {
                    situation,
                    possession: !input.possession,
                    description,
                    start_at: input.end_at,
                    end_at,
                    home_score: input.home_score,
                    away_score: input.away_score,
                    attackers,
                    ..Default::default()
                }
            }
        };

    attack_stats_update.insert(playmaker.id, playmaker_update);
    attack_stats_update.insert(target.id, target_update);
    defense_stats_update.insert(target_defender.id, target_defender_update);
    defense_stats_update.insert(playmaker_defender.id, playmaker_defender_update);
    result.attack_stats_update = Some(attack_stats_update);
    result.defense_stats_update = Some(defense_stats_update);
    result
}
