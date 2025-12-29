use super::{
    action::{ActionOutput, ActionSituation},
    constants::MIN_TIREDNESS_FOR_SUB,
    game::Game,
    types::{GameStats, GameStatsMap, Possession},
};
use crate::{
    core::{
        player::Player,
        position::{GamePosition, MAX_GAME_POSITION},
        skill::MAX_SKILL,
        team::Team,
        GameSkill,
    },
    game_engine::{constants::SUBSTITUTION_ACTION_PROBABILITY, types::EnginePlayer},
    types::SortablePlayerMap,
};
use itertools::Itertools;
use rand::{seq::IndexedRandom, Rng};
use rand_chacha::ChaCha8Rng;
use std::collections::HashMap;

fn get_subs<'a>(
    players: &[&'a Player],
    team_stats: &GameStatsMap,
    action_rng: &mut ChaCha8Rng,
) -> Vec<&'a Player> {
    let bench: Vec<&Player> = players
        .iter()
        .skip(MAX_GAME_POSITION as usize)
        .filter(|&p| {
            let stats = team_stats.get(&p.id).unwrap();
            !stats.is_playing() && !p.is_knocked_out()
        })
        .copied()
        .collect();

    if bench.is_empty() {
        return vec![];
    }

    let playing: Vec<&Player> = players
        .iter()
        .take(MAX_GAME_POSITION as usize)
        .filter(|&p| {
            let stats = team_stats.get(&p.id).unwrap();
            stats.is_playing() && p.tiredness > MIN_TIREDNESS_FOR_SUB
        })
        //Sort from less to most skilled*tired
        .sorted_by(|&a, &b| {
            let a_stats = team_stats
                .get(&a.id)
                .expect("Playing player should have stats");
            let a_position = a_stats
                .position
                .expect("Playing player should have a position");
            let v1 = a.in_game_rating_at_position(a_position) as u16;
            let b_stats = team_stats
                .get(&b.id)
                .expect("Playing player should have stats");
            let b_position = b_stats
                .position
                .expect("Playing player should have a position");
            let v2 = b.in_game_rating_at_position(b_position) as u16;

            v1.cmp(&v2)
        })
        .copied()
        .collect();

    if playing.is_empty() {
        return vec![];
    }

    let out_candidate = playing[0];
    let out_stats = team_stats
        .get(&out_candidate.id)
        .expect("Player should have stats");
    let out_position = out_stats
        .position
        .expect("Out candidate should have a position");

    let in_candidate = bench
        .iter()
        //Sort from most to less skilled*tired
        .max_by(|&a, &b| {
            let v1 = a.in_game_rating_at_position(out_position) as u16;
            let v2 = b.in_game_rating_at_position(out_position) as u16;
            v1.cmp(&v2)
        })
        .expect("There should be a in candidate");

    // If in candidate is worse than out candidate, there is still a 25% chance of subbing.
    // This probability increases linearly up to 100% when the in candidate skills
    // are 15 points moreis than the out candidate's.
    let sub_probability_modifier = (0.25
        + (in_candidate.in_game_rating_at_position(out_position)
            - out_candidate.in_game_rating_at_position(out_position))
        .bound()
            / MAX_SKILL) as f64;

    let sub_probability = SUBSTITUTION_ACTION_PROBABILITY * sub_probability_modifier;

    if action_rng.random_bool(sub_probability.clamp(0.0, 1.0)) {
        vec![in_candidate, out_candidate]
    } else {
        vec![]
    }
}

fn make_substitution(
    players: Vec<&Player>,
    stats: &GameStatsMap,
    action_rng: &mut ChaCha8Rng,
    description_rng: &mut ChaCha8Rng,
) -> Option<(String, GameStatsMap)> {
    let subs = get_subs(&players, stats, action_rng);
    if subs.is_empty() {
        return None;
    }
    let player_in = subs[0];
    let player_out = subs[1];
    let tiredness = player_out.tiredness;
    let position = stats.get(&player_out.id)?.position?;

    let mut description = [
        format!(
            "{} is substituted by {}. ",
            player_out.info.short_name(),
            player_in.info.short_name()
        ),
        format!(
            "{} gets in for {}. ",
            player_in.info.short_name(),
            player_out.info.short_name()
        ),
    ]
    .choose(description_rng)
    .cloned()
    .expect("There should be one option");

    if tiredness == MAX_SKILL {
        description.push_str(
            format!(
                "{} {} completely done. ",
                player_out.info.pronouns.as_subject(),
                player_out.info.pronouns.to_be(),
            )
            .as_str(),
        );
    } else if tiredness > MIN_TIREDNESS_FOR_SUB + 5.0 {
        description.push_str(
            format!(
                "{} looked exhausted. ",
                player_out.info.pronouns.as_subject()
            )
            .as_str(),
        );
    } else if tiredness > MIN_TIREDNESS_FOR_SUB + 2.5 {
        description.push_str(
            format!(
                "{} looked very tired. ",
                player_out.info.pronouns.as_subject()
            )
            .as_str(),
        );
    } else if tiredness > MIN_TIREDNESS_FOR_SUB {
        description.push_str(
            format!(
                "{} {} a bit tired. ",
                player_out.info.pronouns.as_subject(),
                player_out.info.pronouns.to_be(),
            )
            .as_str(),
        );
    } else {
        description.push_str(
            format!(
                "{} did not look tired. ",
                player_out.info.pronouns.as_subject()
            )
            .as_str(),
        );
    }

    let mut stats_update: GameStatsMap = HashMap::new();
    let player_in_update = GameStats {
        position: Some(position),
        ..Default::default()
    };
    let player_out_update = GameStats {
        position: None,
        ..Default::default()
    };
    stats_update.insert(player_in.id, player_in_update);
    stats_update.insert(player_out.id, player_out_update);

    let mut playing: Vec<&Player> = players
        .iter()
        .filter(|&p| stats.get(&p.id).unwrap().is_playing() && p.id != player_out.id)
        .copied()
        .collect();
    playing.push(player_in);
    let assignement = Team::best_position_assignment(playing);
    for (idx, &id) in assignement.clone().iter().enumerate() {
        let mut player_update = if let Some(update) = stats_update.get(&id) {
            update.clone()
        } else {
            GameStats::default()
        };

        player_update.position = Some(idx as GamePosition);
        stats_update.insert(id, player_update.clone());
    }

    Some((description, stats_update))
}

pub(crate) fn should_execute(
    input: &ActionOutput,
    game: &Game,
    action_rng: &mut ChaCha8Rng,
    description_rng: &mut ChaCha8Rng,
) -> Option<ActionOutput> {
    let home_players = &game.home_team_in_game.players;
    let away_players = &game.away_team_in_game.players;
    let mut result = ActionOutput {
        advantage: input.advantage,
        possession: input.possession,
        attackers: input.attackers.clone(),
        defenders: input.defenders.clone(),
        situation: ActionSituation::AfterSubstitution,
        assist_from: input.assist_from,
        start_at: input.start_at,
        end_at: input.end_at,
        home_score: input.home_score,
        away_score: input.away_score,
        ..Default::default()
    };

    let mut home_sub = false;
    let mut away_sub = false;
    if let Some((description, stats_update)) = make_substitution(
        home_players.by_position(&game.home_team_in_game.stats),
        &game.home_team_in_game.stats,
        action_rng,
        description_rng,
    ) {
        result
            .description
            .push_str(format!("Substitution for {}. ", game.home_team_in_game.name).as_str());
        result.description.push_str(description.as_str());

        match game.possession {
            Possession::Home => {
                result.attack_stats_update = Some(stats_update);
            }
            Possession::Away => {
                result.defense_stats_update = Some(stats_update);
            }
        }
        home_sub = true;
    }

    if let Some((description, stats_update)) = make_substitution(
        away_players.by_position(&game.away_team_in_game.stats),
        &game.away_team_in_game.stats,
        action_rng,
        description_rng,
    ) {
        if home_sub {
            result.description.push_str(
                format!(
                    "Also {} will make a substitution. ",
                    game.away_team_in_game.name
                )
                .as_str(),
            );
        } else {
            result
                .description
                .push_str(format!("Substitution for {}. ", game.away_team_in_game.name).as_str());
        }
        result.description.push_str(description.as_str());

        match game.possession {
            Possession::Home => {
                result.defense_stats_update = Some(stats_update);
            }
            Possession::Away => {
                result.attack_stats_update = Some(stats_update);
            }
        }
        away_sub = true;
    }
    if home_sub || away_sub {
        return Some(result);
    }
    None
}
