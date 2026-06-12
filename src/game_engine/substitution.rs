use super::{
    action::{ActionOutput, ActionSituation},
    constants::MIN_TIREDNESS_FOR_SUB,
    game::Game,
    types::{GameStats, GameStatsMap, Possession},
};
use crate::{
    core::{
        player::Player,
        position::{GamePosition, NUM_GAME_POSITIONS},
        skill::MAX_SKILL,
        team::Team,
        GameSkill, TickInterval,
    },
    game_engine::types::{EnginePlayer, GamePositionFluidity, SubstitutionTendency},
    types::{SortablePlayerMap, Tick},
};
use itertools::Itertools;
use rand::{seq::IndexedRandom, RngExt};
use rand_chacha::ChaCha8Rng;
use std::collections::HashMap;

fn get_subs<'a>(
    players: &[&'a Player],
    team_stats: &GameStatsMap,
    substitution_tendency: SubstitutionTendency,
    game_position_fluidity: GamePositionFluidity,
    ticks_since_last_substitution: Tick,
    action_rng: &mut ChaCha8Rng,
) -> Vec<&'a Player> {
    let bench: Vec<&Player> = players
        .iter()
        .skip(NUM_GAME_POSITIONS as usize)
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
        .take(NUM_GAME_POSITIONS as usize)
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
            let v1 = a.in_game_rating_at_position(a_position, game_position_fluidity) as u16;
            let b_stats = team_stats
                .get(&b.id)
                .expect("Playing player should have stats");
            let b_position = b_stats
                .position
                .expect("Playing player should have a position");
            let v2 = b.in_game_rating_at_position(b_position, game_position_fluidity) as u16;

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
            let v1 = a.in_game_rating_at_position(out_position, game_position_fluidity) as u16;
            let v2 = b.in_game_rating_at_position(out_position, game_position_fluidity) as u16;
            v1.cmp(&v2)
        })
        .expect("There should be a in candidate");

    let sub_probability = if out_candidate.is_knocked_out() && !in_candidate.is_knocked_out() {
        1.0
    } else {
        // If in candidate is worse than out candidate, there is still a finite chance of subbing.
        // This probability increases when the in candidate skills are better than the out candidate's.
        let rating_modifier = (in_candidate
            .in_game_rating_at_position(out_position, game_position_fluidity)
            - out_candidate.in_game_rating_at_position(out_position, game_position_fluidity))
        .bound()
            / MAX_SKILL;

        // A small negative modifier if the last substitution was recent, a boost if it was long ago.
        let recency_modifier = if ticks_since_last_substitution < TickInterval::SHORT * 30 {
            0.05
        } else if ticks_since_last_substitution < TickInterval::SHORT * 60 {
            0.5
        } else if ticks_since_last_substitution > TickInterval::SHORT * 60 * 10 {
            1.5
        } else {
            1.0
        };
        ((substitution_tendency.substitution_probability() + rating_modifier as f64)
            * recency_modifier)
            .clamp(0.0, 1.0)
    };

    if action_rng.random_bool(sub_probability) {
        vec![in_candidate, out_candidate]
    } else {
        vec![]
    }
}

fn make_substitution(
    players: Vec<&Player>,
    stats: &GameStatsMap,
    substitution_tendency: SubstitutionTendency,
    game_position_fluidity: GamePositionFluidity,
    ticks_since_last_substitution: Tick,
    action_rng: &mut ChaCha8Rng,
    description_rng: &mut ChaCha8Rng,
) -> Option<(String, GameStatsMap)> {
    let subs = get_subs(
        &players,
        stats,
        substitution_tendency,
        game_position_fluidity,
        ticks_since_last_substitution,
        action_rng,
    );
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
    let assignement = Team::best_position_assignment(playing, game_position_fluidity);
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
    let home_ticks_since_last_substitution = game.timer.as_tick() - game.last_substitution_tick[0];
    if let Some((description, stats_update)) = make_substitution(
        home_players.by_position(&game.home_team_in_game.stats),
        &game.home_team_in_game.stats,
        game.home_team_in_game.substitution_tendency,
        game.home_team_in_game.game_position_fluidity,
        home_ticks_since_last_substitution,
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

    let away_ticks_since_last_substitution = game.timer.as_tick() - game.last_substitution_tick[1];
    if let Some((description, stats_update)) = make_substitution(
        away_players.by_position(&game.away_team_in_game.stats),
        &game.away_team_in_game.stats,
        game.away_team_in_game.substitution_tendency,
        game.away_team_in_game.game_position_fluidity,
        away_ticks_since_last_substitution,
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
