use super::{
    action::{ActionOutput, ActionSituation},
    game::Game,
    types::*,
};
use crate::{
    core::{constants::TirednessCost, GamePosition, GameSkill},
    game_engine::{
        action::{Action, Advantage},
        constants::*,
        types::GameStatsMap,
    },
};
use rand::{seq::IndexedRandom, Rng};
use rand_chacha::ChaCha8Rng;
use std::collections::HashMap;

const MIN_REBOUND_VALUE: i16 = 40;
const REBOUND_POSITION_SCALING: f32 = 12.0;

fn position_rebound_bonus(idx: GamePosition) -> f32 {
    1.0 + idx as f32 / REBOUND_POSITION_SCALING
}

pub(crate) fn execute(
    input: &ActionOutput,
    game: &Game,
    action_rng: &mut ChaCha8Rng,
    description_rng: &mut ChaCha8Rng,
) -> ActionOutput {
    let attacking_players_array = game.attacking_players_array();
    let defending_players_array = game.defending_players_array();

    let mut attack_rebounds: Vec<i16> = attacking_players_array
        .iter()
        .map(|&p| {
            (0.5 * p.athletics.vertical + 0.5 * (0.25 * (p.info.height - 150.0))).game_value()
                + p.technical.rebounds.game_value()
                + game
                    .attacking_team()
                    .tactic
                    .attack_roll_bonus(&Action::Rebound)
        })
        .collect();
    let mut defense_rebounds: Vec<i16> = defending_players_array
        .iter()
        .map(|&p| {
            (0.5 * p.athletics.vertical + 0.5 * (0.25 * (p.info.height - 150.0))).game_value()
                + p.technical.rebounds.game_value()
                + game
                    .defending_team()
                    .tactic
                    .defense_roll_bonus(&Action::Rebound)
        })
        .collect();

    // apply reduction for shooter rebounds
    assert!(input.attackers.len() == 1);
    attack_rebounds[input.attackers[0]] = (attack_rebounds[input.attackers[0]] as f32 * 0.7) as i16;

    for idx in 0..attacking_players_array.len() {
        // apply bonus based on position
        attack_rebounds[idx] =
            (attack_rebounds[idx] as f32 * position_rebound_bonus(idx as GamePosition)) as i16;

        //add random roll
        match input.advantage {
            Advantage::Attack => {
                attack_rebounds[idx] += attacking_players_array[idx]
                    .roll(action_rng)
                    .max(attacking_players_array[idx].roll(action_rng));
            }
            Advantage::Neutral => {
                attack_rebounds[idx] += attacking_players_array[idx].roll(action_rng);
            }
            Advantage::Defense => {
                attack_rebounds[idx] += attacking_players_array[idx]
                    .roll(action_rng)
                    .min(attacking_players_array[idx].roll(action_rng));
            }
        }
    }

    for idx in 0..defending_players_array.len() {
        // apply reduction for defender rebounds.
        if input.defenders.contains(&idx) {
            defense_rebounds[idx] = (defense_rebounds[idx] as f32 * 0.8) as i16;
        }
        // apply bonus based on position
        defense_rebounds[idx] =
            (defense_rebounds[idx] as f32 * position_rebound_bonus(idx as GamePosition)) as i16;
        //add random roll
        defense_rebounds[idx] += defending_players_array[idx].roll(action_rng);
    }

    let attack_result = *attack_rebounds
        .iter()
        .max()
        .expect("Attack rebounds should be non-empty");
    let defence_result = *defense_rebounds
        .iter()
        .max()
        .expect("Defense rebounds should be non-empty");

    let attack_rebounder_idx = attack_rebounds
        .iter()
        .position(|&r| r == attack_result)
        .expect("There should be an index");
    let defence_rebounder_idx = defense_rebounds
        .iter()
        .position(|&r| r == defence_result)
        .expect("There should be an index");

    let attack_rebounder = attacking_players_array[attack_rebounder_idx];
    let defence_rebounder = defending_players_array[defence_rebounder_idx];

    let result = match attack_result - defence_result {
        // Here we use ADV_ATTACK_LIMIT not to give an advantage, but to get the offensive rebound.
        x if x >= ADV_ATTACK_LIMIT
            || (x > 0
                && attack_result >= MIN_REBOUND_VALUE
                && attack_rebounder_idx == input.attackers[0]) =>
        {
            let mut attack_stats_update: GameStatsMap = HashMap::new();
            let rebounder_update = GameStats {
                offensive_rebounds: 1,
                extra_tiredness: TirednessCost::LOW,
                ..Default::default()
            };
            attack_stats_update.insert(attack_rebounder.id, rebounder_update);
            let description = if attack_rebounder_idx == input.attackers[0] {
                [
                    format!(
                        "{} grabs {} own rebound with a quick reaction.",
                        attack_rebounder.info.short_name(),
                        attack_rebounder.info.pronouns.as_possessive()
                    ),
                    format!(
                        "{} snatches the ball after missing the shot, showing persistence.",
                        attack_rebounder.info.short_name(),
                    ),
                    format!(
                        "{} secures {} own miss for a second chance.",
                        attack_rebounder.info.short_name(),
                        attack_rebounder.info.pronouns.as_possessive()
                    ),
                    format!(
                        "{} quickly leaps and grabs {} missed shot, avoiding defenders.",
                        attack_rebounder.info.short_name(),
                        attack_rebounder.info.pronouns.as_possessive()
                    ),
                    format!(
                        "{} fights through the defenders to secure {} own rebound.",
                        attack_rebounder.info.short_name(),
                        attack_rebounder.info.pronouns.as_possessive()
                    ),
                ]
                .choose(description_rng)
                .expect("There should be an option")
                .clone()
            } else {
                [
                        format!(
                            "{} leaps above the defenders and snags the offensive rebound.",
                            attack_rebounder.info.short_name(),
                        ),
                        format!(
                            "{} outmuscles the competition to grab the offensive rebound.",
                            attack_rebounder.info.short_name(),
                        ),
                        format!(
                            "{} beats everyone to the ball, securing the offensive rebound.",
                            attack_rebounder.info.short_name(),
                        ),
                        format!(
                            "{} extends high and grabs the ball over the defenders for an offensive rebound.",
                            attack_rebounder.info.short_name(),
                        ),
                        format!(
                            "{} crashes the boards and comes down with the offensive rebound.",
                            attack_rebounder.info.short_name(),
                        ),
                    ].choose(description_rng)
                    .expect("There should be an option")
                    .clone()
            };
            ActionOutput {
                possession: input.possession,
                situation: ActionSituation::AfterOffensiveRebound,
                description,
                attackers: vec![attack_rebounder_idx],
                attack_stats_update: Some(attack_stats_update),
                start_at: input.end_at,
                end_at: input.end_at.plus(1 + action_rng.random_range(0..=1)),
                home_score: input.home_score,
                away_score: input.away_score,
                ..Default::default()
            }
        }

        x if x > ADV_NEUTRAL_LIMIT && attack_result >= MIN_REBOUND_VALUE => {
            let mut attack_stats_update: GameStatsMap = HashMap::new();
            let rebounder_update = GameStats {
                offensive_rebounds: 1,
                extra_tiredness: TirednessCost::LOW,
                ..Default::default()
            };
            attack_stats_update.insert(attack_rebounder.id, rebounder_update);
            let description = [
                format!(
                    "The ball got to {} who can restart the offensive action.",
                    attack_rebounder.info.short_name(),
                ),
                format!(
                    "{} secures the offensive rebound and looks to reset the play.",
                    attack_rebounder.info.short_name(),
                ),
                format!(
                    "{} snags the rebound and reset the offense.",
                    attack_rebounder.info.short_name(),
                ),
                format!(
                    "{} pulls down the board and surveys the floor for the next move.",
                    attack_rebounder.info.short_name(),
                ),
            ]
            .choose(description_rng)
            .expect("There should be an option")
            .clone();

            ActionOutput {
                possession: input.possession,
                situation: ActionSituation::AfterLongOffensiveRebound,
                description,
                attackers: vec![attack_rebounder_idx],
                attack_stats_update: Some(attack_stats_update),
                start_at: input.end_at,
                end_at: input.end_at.plus(4 + action_rng.random_range(0..=3)),
                home_score: input.home_score,
                away_score: input.away_score,
                ..Default::default()
            }
        }
        x if x < ADV_NEUTRAL_LIMIT && defence_result >= MIN_REBOUND_VALUE => {
            let mut defence_stats_update: GameStatsMap = HashMap::new();
            let rebounder_update = GameStats {
                defensive_rebounds: 1,
                extra_tiredness: TirednessCost::LOW,
                ..Default::default()
            };
            defence_stats_update.insert(defence_rebounder.id, rebounder_update);

            ActionOutput {
                possession: !input.possession,
                situation: ActionSituation::AfterDefensiveRebound,
                description: [
                    format!(
                        "{} jumps high and gets the defensive rebound.",
                        defence_rebounder.info.short_name(),
                    ),
                    format!(
                        "{} reaches up to snare the ball, grabbing the defensive rebound.",
                        defence_rebounder.info.short_name(),
                    ),
                    format!(
                        "{} outmuscles the offense and secures the defensive board.",
                        defence_rebounder.info.short_name(),
                    ),
                    format!(
                        "{} claims the rebound, boxing out the attacker and controlling the ball.",
                        defence_rebounder.info.short_name(),
                    ),
                    format!(
                        "{} uses great positioning to grab the defensive rebound and take control.",
                        defence_rebounder.info.short_name(),
                    ),
                ]
                .choose(description_rng)
                .expect("There should be an option")
                .clone(),
                defense_stats_update: Some(defence_stats_update),
                start_at: input.end_at,
                end_at: input.end_at.plus(5 + action_rng.random_range(0..=6)),
                home_score: input.home_score,
                away_score: input.away_score,
                ..Default::default()
            }
        }
        _ => ActionOutput {
            possession: !input.possession,
            situation: ActionSituation::AfterDefensiveRebound,
            description: [
                "Nobody got the rebound, ball goes to defence.",
                "Neither team secures the board, and the ball rolls to the defensive side.",
                "The rebound bounces loose, and the defense grabs it.",
                "The ball is up for grabs but nobody claims it, and it’s recovered by the defense.",
            ]
            .choose(description_rng)
            .expect("There should be an option")
            .to_string(),
            start_at: input.end_at,
            end_at: input.end_at.plus(5 + action_rng.random_range(0..=6)),
            home_score: input.home_score,
            away_score: input.away_score,
            ..Default::default()
        },
    };
    result
}
