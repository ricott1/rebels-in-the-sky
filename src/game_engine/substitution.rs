use super::{
    action::{ActionOutput, ActionSituation},
    constants::MIN_TIREDNESS_FOR_SUB,
    game::Game,
    types::{GameStats, GameStatsMap, Possession},
};
use crate::{
    types::SortablePlayerMap,
    world::{constants::MAX_TIREDNESS, player::Player, position::Position, team::Team},
};
use itertools::Itertools;
use rand_chacha::ChaCha8Rng;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct Substitution;

fn get_subs<'a>(players: Vec<&'a Player>, team_stats: &GameStatsMap) -> Vec<&'a Player> {
    if players.len() <= 5 {
        return vec![];
    }

    let bench: Vec<&Player> = players
        .iter()
        .skip(5)
        .filter(|&p| {
            let stats = team_stats.get(&p.id).unwrap();
            !stats.is_playing() && !p.is_knocked_out()
        })
        .map(|&p| p)
        .collect();

    if bench.len() == 0 {
        return vec![];
    }

    let playing: Vec<&Player> = players
        .iter()
        .take(5)
        .filter(|&p| {
            let stats = team_stats.get(&p.id).unwrap();
            return stats.is_playing() == true && p.tiredness > MIN_TIREDNESS_FOR_SUB;
        })
        //Sort from less to most skilled*tired
        .sorted_by(|&a, &b| {
            let a_stats = team_stats
                .get(&a.id)
                .expect("Playing player should have stats");
            let a_position = a_stats
                .position
                .expect("Playing player should have a position");
            let v1 = a.tiredness_weighted_rating_at_position(a_position) as u16;
            let b_stats = team_stats
                .get(&b.id)
                .expect("Playing player should have stats");
            let b_position = b_stats
                .position
                .expect("Playing player should have a position");
            let v2 = b.tiredness_weighted_rating_at_position(b_position) as u16;

            v1.cmp(&v2)
        })
        .map(|&p| p)
        .collect();

    if playing.len() == 0 {
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
            let v1 = a.tiredness_weighted_rating_at_position(out_position) as u16;
            let v2 = b.tiredness_weighted_rating_at_position(out_position) as u16;
            v1.cmp(&v2)
        })
        .expect("There should be a in candidate");

    return vec![in_candidate, out_candidate];
}

fn make_substitution(
    players: Vec<&Player>,
    stats: &GameStatsMap,
) -> Option<(String, GameStatsMap)> {
    let subs = get_subs(players.clone(), stats);
    if subs.len() == 0 {
        return None;
    }
    let player_in = subs[0];
    let player_out = subs[1];
    let tiredness = player_out.tiredness;
    let position = stats.get(&player_out.id)?.position?;

    let mut description = format!(
        "{} is substituted by {}. ",
        player_out.info.shortened_name(),
        player_in.info.shortened_name()
    );

    if tiredness > MIN_TIREDNESS_FOR_SUB {
        description.push_str(
            format!(
                "{} looked very tired. ",
                player_out.info.pronouns.as_subject()
            )
            .as_str(),
        );
    } else if tiredness > MAX_TIREDNESS / 4.0 {
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
    let mut player_in_update = GameStats::default();
    player_in_update.position = Some(position);
    let mut player_out_update = GameStats::default();
    player_out_update.position = None;
    stats_update.insert(player_in.id, player_in_update);
    stats_update.insert(player_out.id, player_out_update);

    let mut playing: Vec<&Player> = players
        .iter()
        .filter(|&p| stats.get(&p.id).unwrap().is_playing() && p.id != player_out.id)
        .map(|&p| p)
        .collect();
    playing.push(player_in);
    // assert!(playing.len() == 5);
    let assignement = Team::best_position_assignment(playing.clone());
    // assert!(assignement.len() == 5);
    for (idx, &id) in assignement.clone().iter().enumerate() {
        let mut player_update: GameStats;
        if stats_update.get(&id).is_none() {
            player_update = GameStats::default();
        } else {
            player_update = stats_update.get(&id).unwrap().clone();
        }
        player_update.position = Some(idx as Position);
        stats_update.insert(id.clone(), player_update.clone());
    }

    Some((description, stats_update))
}

impl Substitution {
    pub fn execute(
        input: &ActionOutput,
        game: &Game,
        _rng: &mut ChaCha8Rng,
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
                result.description.push_str(
                    format!("Substitution for {}. ", game.away_team_in_game.name).as_str(),
                );
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
}
