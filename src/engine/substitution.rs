use super::{
    action::ActionOutput, constants::MIN_TIREDNESS_FOR_SUB, game::Game, types::GameStatsMap,
};
use crate::{
    engine::{
        constants::MAX_TIREDNESS,
        types::{GameStats, Possession},
    },
    types::SortablePlayerMap,
    world::{player::Player, position::Position, team::Team, types::Pronoun},
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
            !stats.is_playing() && !stats.is_knocked_out()
        })
        //Sort from most to less skilled*tired
        .sorted_by(|&a, &b| {
            let t1 = team_stats.get(&a.id).unwrap().tiredness;
            let v1: u16 = a.total_skills() * (MAX_TIREDNESS - t1 / 2.0) as u16;
            let t2 = team_stats.get(&b.id).unwrap().tiredness;
            let v2 = b.total_skills() * (MAX_TIREDNESS - t2 / 2.0) as u16;
            v2.cmp(&v1)
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
            return stats.is_playing() == true && stats.tiredness > MIN_TIREDNESS_FOR_SUB;
        })
        //Sort from less to most skilled*tired
        .sorted_by(|&a, &b| {
            let v1 = if team_stats.get(&a.id).unwrap().is_knocked_out() {
                0
            } else {
                let t1 = team_stats.get(&a.id).unwrap().tiredness;
                a.total_skills() * (MAX_TIREDNESS - t1 / 2.0) as u16
            };
            let v2 = if team_stats.get(&b.id).unwrap().is_knocked_out() {
                0
            } else {
                let t2 = team_stats.get(&b.id).unwrap().tiredness;
                b.total_skills() * (MAX_TIREDNESS - t2 / 2.0) as u16
            };
            v1.cmp(&v2)
        })
        .map(|&p| p)
        .collect();

    if playing.len() == 0 {
        return vec![];
    }

    return vec![bench[0], playing[0]];
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
    let tiredness = stats.get(&player_out.id)?.tiredness;
    let position = stats.get(&player_out.id)?.position?;

    let mut description = format!(
        "{} is substituted by {}. ",
        player_out.info.last_name, player_in.info.last_name
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
        let verb = if player_out.info.pronouns == Pronoun::They {
            "are"
        } else {
            "is"
        };
        description.push_str(
            format!(
                "{} {} a bit tired. ",
                player_out.info.pronouns.as_subject(),
                verb
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
            situation: input.situation.clone(),
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
