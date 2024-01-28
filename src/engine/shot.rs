use crate::{
    engine::{constants::TirednessCost, types::*},
    world::{player::Player, skill::GameSkill},
};

use super::{
    action::{ActionOutput, ActionSituation, Advantage},
    constants::ShotDifficulty,
    game::Game,
    types::GameStats,
    utils::roll,
};
use rand::{seq::SliceRandom, Rng};
use rand_chacha::ChaCha8Rng;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct CloseShot;

#[derive(Debug, Default)]
pub struct MediumShot;

#[derive(Debug, Default)]
pub struct LongShot;

impl CloseShot {
    pub fn execute(
        &self,
        input: &ActionOutput,
        game: &Game,
        rng: &mut ChaCha8Rng,
    ) -> Option<ActionOutput> {
        return execute_shot(input, game, rng, ShotDifficulty::Close);
    }
}

impl MediumShot {
    pub fn execute(
        &self,
        input: &ActionOutput,
        game: &Game,
        rng: &mut ChaCha8Rng,
    ) -> Option<ActionOutput> {
        return execute_shot(input, game, rng, ShotDifficulty::Medium);
    }
}

impl LongShot {
    pub fn execute(
        &self,
        input: &ActionOutput,
        game: &Game,
        rng: &mut ChaCha8Rng,
    ) -> Option<ActionOutput> {
        return execute_shot(input, game, rng, ShotDifficulty::Long);
    }
}

fn description(
    rng: &mut ChaCha8Rng,
    shooter: &Player,
    assist: Option<&Player>,
    defenders: Vec<&Player>,
    shot: ShotDifficulty,
    advantage: Advantage,
    success: bool,
) -> Option<String> {
    let text = match (shot, advantage, success) {
        (ShotDifficulty::Close, Advantage::Attack, true) => vec![
            format!("{} scores an easy layup.", shooter.info.last_name),
            format!(
                "{} would never miss in this situation.",
                shooter.info.last_name
            ),
            format!("{} scores with ease.", shooter.info.last_name),
            format!("{} scores the easy layup.", shooter.info.last_name),
        ],

        (ShotDifficulty::Close, Advantage::Neutral, true) => vec![
            format!("{} scores.", shooter.info.last_name),
            format!("{} scores the layup.", shooter.info.last_name),
        ],
        (ShotDifficulty::Close, Advantage::Defense, true) => vec![
            format!("{} scores with a miracle!", shooter.info.last_name),
            format!(
                "{} scores the layup over {}.",
                shooter.info.last_name, defenders[0].info.last_name
            ),
        ],
        (ShotDifficulty::Close, Advantage::Attack, false) => vec![
            format!(
                "{} manages to miss the open layup! The coach is furious...",
                shooter.info.last_name
            ),
            format!("{} misses the layup, what a shame!", shooter.info.last_name),
        ],
        (ShotDifficulty::Close, Advantage::Neutral, false) => {
            vec![format!("{} misses the shot.", shooter.info.last_name)]
        }
        (ShotDifficulty::Close, Advantage::Defense, false) => vec![
            format!(
                "{} misses the layup, blocked by {}.",
                shooter.info.last_name, defenders[0].info.last_name
            ),
            format!(
                "{} misses the layup, {} got a piece of it.",
                shooter.info.last_name, defenders[0].info.last_name
            ),
        ],

        (ShotDifficulty::Medium, Advantage::Attack, true) => vec![format!(
            "{} converts all alone from the mid range.",
            shooter.info.last_name
        )],
        (ShotDifficulty::Medium, Advantage::Neutral, true) => {
            vec![format!("{} scores the jumper.", shooter.info.last_name)]
        }
        (ShotDifficulty::Medium, Advantage::Defense, true) => vec![
            format!("{} scores a contested mid ranger.", shooter.info.last_name),
            format!(
                "{} scores a mid ranger over {}.",
                shooter.info.last_name, defenders[0].info.last_name
            ),
        ],
        (ShotDifficulty::Medium, Advantage::Attack, false) => {
            vec![format!("{} misses an open shot!", shooter.info.last_name)]
        }
        (ShotDifficulty::Medium, Advantage::Neutral, false) => {
            vec![format!("{} misses the shot.", shooter.info.last_name)]
        }
        (ShotDifficulty::Medium, Advantage::Defense, false) => vec![format!(
            "{} misses the jumper, blocked by {}.",
            shooter.info.last_name, defenders[0].info.last_name
        )],

        (ShotDifficulty::Long, Advantage::Attack, true) => {
            vec![format!("{} scores the open three!", shooter.info.last_name)]
        }
        (ShotDifficulty::Long, Advantage::Neutral, true) => vec![format!(
            "{} scores the contested jumper!",
            shooter.info.last_name
        )],
        (ShotDifficulty::Long, Advantage::Defense, true) => vec![format!(
            "{} scores a bomb in the face of {}!",
            shooter.info.last_name, defenders[0].info.last_name
        )],
        (ShotDifficulty::Long, Advantage::Attack, false) => {
            vec![format!("{} misses the open three!", shooter.info.last_name)]
        }
        (ShotDifficulty::Long, Advantage::Neutral, false) => vec![format!(
            "{} misses from the long range.",
            shooter.info.last_name
        )],
        (ShotDifficulty::Long, Advantage::Defense, false) => vec![format!(
            "{} misses the three, blocked by {}.",
            shooter.info.last_name, defenders[0].info.last_name
        )],
    };

    let mut description = text.choose(rng)?.to_string();
    if let Some(passer) = assist {
        description.push_str(format!(" Assist from {}.", passer.info.last_name).as_str());
    };
    Some(description)
}

fn execute_shot(
    input: &ActionOutput,
    game: &Game,
    rng: &mut ChaCha8Rng,
    shot: ShotDifficulty,
) -> Option<ActionOutput> {
    let attacking_players = game.attacking_players();
    let defending_players = game.defending_players();
    let attacking_stats = game.attacking_stats();
    let defending_stats = game.defending_stats();

    assert!(input.attackers.len() == 1);
    let shooter_idx = input.attackers[0];
    let shooter = attacking_players[shooter_idx];
    let shooter_stats = attacking_stats.get(&shooter.id)?;

    if input.advantage == Advantage::Defense {
        assert!(input.defenders.len() > 0);
    }
    let defenders = input
        .defenders
        .iter()
        .map(|&idx| defending_players[idx])
        .collect::<Vec<&Player>>();

    let atk_skill = match shot.clone() {
        ShotDifficulty::Close => shooter.offense.close_range.value(),
        ShotDifficulty::Medium => shooter.offense.medium_range.value(),
        ShotDifficulty::Long => shooter.offense.long_range.value(),
    };
    let def_skill = defenders
        .iter()
        .map(|&p| {
            let defender_stats = defending_stats.get(&p.id).unwrap();
            roll(rng, defender_stats.tiredness) / defenders.len() as u8 + p.defense.block.value()
        })
        .sum::<u8>();

    let roll = match input.advantage {
        Advantage::Attack => {
            (roll(rng, shooter_stats.tiredness) + atk_skill) as i16 - (shot as u8) as i16
        }
        Advantage::Neutral => {
            (roll(rng, shooter_stats.tiredness) + atk_skill) as i16
                - (shot as u8 + def_skill / 2) as i16
        }
        Advantage::Defense => {
            (roll(rng, shooter_stats.tiredness) + atk_skill) as i16
                - (shot as u8 + def_skill) as i16
        }
    };

    let success = roll > 0;
    let mut result = match success {
        false => {
            // Attackers and defenders will get a malus in the rebound action.
            let advantage = match input.advantage {
                Advantage::Attack => Advantage::Neutral,
                Advantage::Neutral | Advantage::Defense => Advantage::Defense,
            };
            ActionOutput {
                advantage,
                possession: input.possession.clone(),
                attackers: vec![shooter_idx],
                defenders: input.defenders.clone(),
                situation: ActionSituation::MissedShot,
                description: description(
                    rng,
                    shooter,
                    None,
                    defenders.clone(),
                    shot,
                    input.advantage,
                    success,
                )?,
                start_at: input.end_at,
                end_at: input.end_at.plus(rng.gen_range(1..=2)),
                home_score: input.home_score,
                away_score: input.away_score,
                ..Default::default()
            }
        }
        true => {
            let assist = if input.assist_from.is_some() {
                Some(attacking_players[input.assist_from?])
            } else {
                None
            };
            let score_change = match shot.clone() {
                ShotDifficulty::Close => 2,
                ShotDifficulty::Medium => 2,
                ShotDifficulty::Long => 3,
            };
            ActionOutput {
                score_change,
                home_score: match input.possession {
                    Possession::Home => input.home_score + score_change as u16,
                    Possession::Away => input.home_score,
                },
                away_score: match input.possession {
                    Possession::Home => input.away_score,
                    Possession::Away => input.away_score + score_change as u16,
                },
                possession: !input.possession.clone(),
                situation: ActionSituation::BallInBackcourt,
                description: description(
                    rng,
                    shooter,
                    assist,
                    defenders.clone(),
                    shot,
                    input.advantage,
                    success,
                )?,
                start_at: input.end_at,
                end_at: input.end_at.plus(10 + rng.gen_range(0..=8)),
                ..Default::default()
            }
        }
    };

    // Update stats
    let mut attack_stats_update = HashMap::new();
    let mut shooter_update = GameStats::default();
    let mut defense_stats_update = HashMap::new();

    match shot {
        ShotDifficulty::Close => {
            shooter_update.attempted_2pt = 1;
            shooter_update.add_tiredness(TirednessCost::MEDIUM, shooter.athleticism.stamina);
            shooter_update.shot_positions = match game.possession {
                Possession::Home => {
                    let (x, y) = HOME_CLOSE_SHOT_POSITIONS.choose(rng)?.clone();
                    vec![(x, y, result.score_change > 0)]
                }
                Possession::Away => {
                    let (x, y) = AWAY_CLOSE_SHOT_POSITIONS.choose(rng)?.clone();
                    vec![(x, y, result.score_change > 0)]
                }
            }
        }
        ShotDifficulty::Medium => {
            shooter_update.attempted_2pt = 1;
            shooter_update.add_tiredness(TirednessCost::MEDIUM, shooter.athleticism.stamina);
            shooter_update.shot_positions = match game.possession {
                Possession::Home => {
                    let (x, y) = HOME_MEDIUM_SHOT_POSITIONS.choose(rng)?.clone();
                    vec![(x, y, result.score_change > 0)]
                }
                Possession::Away => {
                    let (x, y) = AWAY_MEDIUM_SHOT_POSITIONS.choose(rng)?.clone();
                    vec![(x, y, result.score_change > 0)]
                }
            }
        }
        ShotDifficulty::Long => {
            shooter_update.attempted_3pt = 1;
            shooter_update.add_tiredness(TirednessCost::MEDIUM, shooter.athleticism.stamina);
            shooter_update.shot_positions = match input.advantage {
                Advantage::Defense => match game.possession {
                    Possession::Home => {
                        let (x, y) = HOME_IMPOSSIBLE_SHOT_POSITIONS.choose(rng)?.clone();
                        vec![(x, y, result.score_change > 0)]
                    }
                    Possession::Away => {
                        let (x, y) = AWAY_IMPOSSIBLE_SHOT_POSITIONS.choose(rng)?.clone();
                        vec![(x, y, result.score_change > 0)]
                    }
                },
                _ => match game.possession {
                    Possession::Home => {
                        let (x, y) = HOME_LONG_SHOT_POSITIONS.choose(rng)?.clone();
                        vec![(x, y, result.score_change > 0)]
                    }
                    Possession::Away => {
                        let (x, y) = AWAY_LONG_SHOT_POSITIONS.choose(rng)?.clone();
                        vec![(x, y, result.score_change > 0)]
                    }
                },
            }
        }
    };

    if result.score_change > 0 {
        shooter_update.points = result.score_change;
        match shot {
            ShotDifficulty::Close => shooter_update.made_2pt = 1,

            ShotDifficulty::Medium => shooter_update.made_2pt = 1,

            ShotDifficulty::Long => shooter_update.made_3pt = 1,
        };
        if input.assist_from.is_some() {
            let mut passer_update = GameStats::default();
            passer_update.assists = 1;
            let passer_id = attacking_players[input.assist_from?].id;
            attack_stats_update.insert(passer_id, passer_update);
        }
    }

    attack_stats_update.insert(shooter.id, shooter_update);

    for (idx, defender) in defenders.iter().enumerate() {
        let mut defender_update = GameStats::default();
        match input.advantage {
            Advantage::Defense => {
                defender_update.add_tiredness(TirednessCost::MEDIUM, defender.athleticism.stamina);
                // Only the first defender gets the block
                if !success && idx == 0 {
                    defender_update.blocks = 1;
                }
            }
            Advantage::Neutral => {
                defender_update.add_tiredness(TirednessCost::MEDIUM, defender.athleticism.stamina);
            }
            _ => {}
        }
        defense_stats_update.insert(defender.id, defender_update);
    }
    result.attack_stats_update = Some(attack_stats_update);
    result.defense_stats_update = Some(defense_stats_update);
    return Some(result);
}
