use super::{action::*, constants::*, game::Game, types::*};
use crate::core::{
    constants::{MoraleModifier, TirednessCost},
    player::Player,
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

    let post_idx = match input.attackers.len() {
        0 => {
            let weights = [1, 2, 15, 30, 40];
            if let Some(idx) = sample_player_index(action_rng, weights, attacking_players_array) {
                idx
            } else {
                return ActionOutput {
                    situation: ActionSituation::Turnover,
                    possession: !input.possession,
                    description: format!(
                        "Oh no! No player of {} is left standing, they just turned the ball over like that!",
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

    let poster = attacking_players_array[post_idx];
    let defender = defending_players_array[post_idx];

    let timer_increase = 5 + action_rng.random_range(0..=5);

    let mut attack_stats_update = HashMap::new();
    let mut post_update = GameStats {
        extra_tiredness: TirednessCost::MEDIUM,
        ..Default::default()
    };

    let mut defense_stats_update = HashMap::new();
    let mut defender_update = GameStats {
        extra_tiredness: TirednessCost::MEDIUM,
        ..Default::default()
    };

    let atk_result = poster.roll(action_rng)
        + poster.technical.post_moves.game_value()
        + poster.athletics.strength.game_value()
        + game
            .attacking_team()
            .tactic
            .attack_roll_bonus(&Action::Post);

    let def_result = defender.roll(action_rng)
        + defender.defense.interior_defense.game_value()
        + (0.75 * defender.athletics.strength + 0.25 * defender.defense.steal).game_value()
        + game
            .defending_team()
            .tactic
            .defense_roll_bonus(&Action::Post);

    let mut result = match atk_result - def_result {
        x if x >= ADV_ATTACK_LIMIT => ActionOutput {
            possession: input.possession,
            advantage: Advantage::Attack,
            attackers: vec![post_idx],
            defenders: vec![post_idx],
            situation: ActionSituation::CloseShot,
            description: [
                format!(
                    "{} worked {}'s perfectly and got to the basket.",
                    poster.info.short_name(),
                    defender.info.short_name()
                ),
                format!(
                    "{} beats {}'s defense to create space and drive to the hoop.",
                    poster.info.short_name(),
                    defender.info.short_name()
                ),
                format!(
                    "{} beats {} with a slick step and makes a strong move to the basket.",
                    poster.info.short_name(),
                    defender.info.short_name()
                ),
                format!(
                    "{} spun past {} and now has an open lane to the basket.",
                    poster.info.short_name(),
                    defender.info.short_name()
                ),
                format!(
                    "{} took advantage of {}'s mistake and easily attacked the basket.",
                    poster.info.short_name(),
                    defender.info.short_name()
                ),
            ]
            .choose(description_rng)
            .expect("There should be one option")
            .clone(),
            start_at: input.end_at,
            end_at: input.end_at.plus(timer_increase),
            home_score: input.home_score,
            away_score: input.away_score,
            ..Default::default()
        },
        x if x > ADV_NEUTRAL_LIMIT => ActionOutput {
            possession: input.possession,
            advantage: Advantage::Neutral,
            attackers: vec![post_idx],
            defenders: vec![post_idx],
            situation: ActionSituation::CloseShot,
            description: [
                format!(
                    "{} bumps on {} and gathers the ball to shoot.",
                    poster.info.short_name(),
                    defender.info.short_name(),
                ),
                format!(
                    "{} backs down {} and collects the ball, looking for a shot.",
                    poster.info.short_name(),
                    defender.info.short_name(),
                ),
                format!(
                    "{} establishes position against {} and prepares for the shot.",
                    poster.info.short_name(),
                    defender.info.short_name(),
                ),
                format!(
                    "{} powers through {}'s defense to secure the ball and get ready to shoot.",
                    poster.info.short_name(),
                    defender.info.short_name(),
                ),
                format!(
                    "{} muscles up against {} and pulls in the ball for a post move.",
                    poster.info.short_name(),
                    defender.info.short_name(),
                ),
            ]
            .choose(description_rng)
            .expect("There should be one option")
            .clone(),
            start_at: input.end_at,
            end_at: input.end_at.plus(timer_increase),
            home_score: input.home_score,
            away_score: input.away_score,
            ..Default::default()
        },
        x if x > ADV_DEFENSE_LIMIT => {
            let num_ok_players = game
                .attacking_players_array()
                .iter()
                .filter(|p| !p.is_knocked_out())
                .count();
            if num_ok_players > 1
                && (0.75 * poster.mental.vision + 0.25 * poster.technical.passing).game_value() + x
                    > ADV_NEUTRAL_LIMIT
            {
                let mut weights = [3, 3, 2, 2, 1];
                weights[post_idx] = 0;
                let target_idx = sample_player_index(action_rng, weights, attacking_players_array)
                    .expect("There should be another ok player");
                let target: &Player = attacking_players_array[target_idx];
                ActionOutput {
                        possession: input.possession,
                        advantage: Advantage::Neutral,
                        attackers: vec![target_idx],
                        defenders: vec![],
                        situation: ActionSituation::BallInMidcourt,
                        description: [
                            format!(
                                "{} is struggling from the post due to {}'s defense. The ball is passed to {} to reset.",
                                poster.info.short_name(), defender.info.short_name(), target.info.short_name()
                            ),
                            format!(
                                "{} can't shake off {}'s defense in the post, so the ball is passed to {} to reset the offense.",
                                poster.info.short_name(), defender.info.short_name(), target.info.short_name()
                            ),
                            format!(
                                "{} is bottled up by {} in the post. The play resets as {} gets the ball.",
                                poster.info.short_name(), defender.info.short_name(), target.info.short_name()
                            ),
                            format!(
                                "{} is having trouble in the post against {}'s tough defense. The ball is swung to {} for a reset.",
                                poster.info.short_name(), defender.info.short_name(), target.info.short_name()
                            ),
                            format!(
                                "{} can't find an opening against {}'s defense, so the ball is passed out to {} to reset.",
                                poster.info.short_name(), defender.info.short_name(), target.info.short_name()
                            ),
                        ].choose(description_rng)
                        .expect("There should be one option")
                        .clone(),
                        start_at: input.end_at,
                        end_at: input.end_at.plus(timer_increase/2),
                        home_score: input.home_score,
                        away_score: input.away_score,
                        ..Default::default()
                    }
            } else {
                ActionOutput {
                        possession: input.possession,
                        advantage: Advantage::Defense,
                        attackers: vec![post_idx],
                        defenders: vec![post_idx],
                        situation: ActionSituation::MediumShot,
                        description: [
                            format!(
                                "{} tries to make the post moves work against {} but {} is all over {}.",
                                poster.info.short_name(), defender.info.short_name(), defender.info.pronouns.as_subject().to_lowercase(),  poster.info.pronouns.as_object()
                            ),
                            format!(
                                "{} attempts a post move on {} but can't shake off the tight defense, resulting in a bad shot.",
                                poster.info.short_name(), defender.info.short_name()
                            ),
                            format!(
                                "{} tries to power through {}'s defense in the post, but {} smothers {}, forcing a difficult shot.",
                                poster.info.short_name(), defender.info.short_name(), defender.info.short_name(), poster.info.pronouns.as_object()
                            ),
                            format!(
                                "{} works the post against {} but the defense is too strong, leading to an off-balance shot.",
                                poster.info.short_name(), defender.info.short_name()
                            ),
                            format!(
                                "{} makes an attempt in the post against {} but is completely shut down, forcing a bad shot.",
                                poster.info.short_name(), defender.info.short_name()
                            ),
                        ].choose(description_rng)
                        .expect("There should be one option")
                        .clone(),
                        start_at: input.end_at,
                        end_at: input.end_at.plus(timer_increase),
                        home_score: input.home_score,
                        away_score: input.away_score,
                        ..Default::default()
                    }
            }
        }
        _ => {
            post_update.turnovers = 1;
            post_update.extra_morale += MoraleModifier::MEDIUM_MALUS;
            defender_update.extra_morale += MoraleModifier::SMALL_BONUS;

            // Equivalent to `- def_result - target_defender.defense.steal.game_value() <= STEAL_LIMIT`
            let with_steal = def_result + defender.defense.steal.game_value() >= -STEAL_LIMIT;

            if with_steal {
                defender_update.steals = 1;
                defender_update.extra_morale += MoraleModifier::MEDIUM_BONUS;
                post_update.extra_morale += MoraleModifier::MEDIUM_MALUS;
            }

            let situation = if with_steal
                && action_rng.random_bool(
                    FASTBREAK_ACTION_PROBABILITY
                        * game
                            .defending_team()
                            .tactic
                            .fastbreak_probability_modifier(),
                ) {
                ActionSituation::Fastbreak
            } else {
                ActionSituation::Turnover
            };

            let attackers = if with_steal { vec![post_idx] } else { vec![] };

            let end_at = if with_steal {
                input.end_at.plus(3 + action_rng.random_range(0..=3))
            } else {
                input.end_at.plus(5 + action_rng.random_range(0..=4))
            };

            let description = if with_steal {
                [
                    format!(
                        "{} steals the ball from {} on the post.",
                        defender.info.short_name(),
                        poster.info.short_name(),
                    ),
                    format!(
                        "{} stops {} on the post and snaps the ball from {} hands.",
                        defender.info.short_name(),
                        poster.info.short_name(),
                        poster.info.pronouns.as_possessive()
                    ),
                ]
                .choose(description_rng)
                .expect("There should be one option")
                .clone()
            } else {
                format!(
                    "{}'s good defense is too much for {}, who fumbles the ball.",
                    defender.info.short_name(),
                    poster.info.short_name(),
                )
            };

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
    attack_stats_update.insert(poster.id, post_update);
    defense_stats_update.insert(defender.id, defender_update);
    result.attack_stats_update = Some(attack_stats_update);
    result.defense_stats_update = Some(defense_stats_update);
    result
}
