use super::{
    action::{ActionOutput, ActionSituation, Advantage, EngineAction},
    constants::ShotDifficulty,
    game::Game,
    types::GameStats,
};
use crate::{
    game_engine::{constants::*, types::*},
    world::{
        constants::{MoraleModifier, TirednessCost},
        player::Player,
        skill::GameSkill,
    },
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

impl EngineAction for CloseShot {
    fn execute(input: &ActionOutput, game: &Game, rng: &mut ChaCha8Rng) -> Option<ActionOutput> {
        return execute_shot(input, game, rng, ShotDifficulty::Close);
    }
}

impl EngineAction for MediumShot {
    fn execute(input: &ActionOutput, game: &Game, rng: &mut ChaCha8Rng) -> Option<ActionOutput> {
        return execute_shot(input, game, rng, ShotDifficulty::Medium);
    }
}

impl EngineAction for LongShot {
    fn execute(input: &ActionOutput, game: &Game, rng: &mut ChaCha8Rng) -> Option<ActionOutput> {
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
            format!("{} scores an easy layup.", shooter.info.shortened_name()),
            format!(
                "{} would never miss in this situation.",
                shooter.info.shortened_name()
            ),
            format!("{} scores with ease.", shooter.info.shortened_name()),
            format!("{} scores the easy layup.", shooter.info.shortened_name()),
            format!(
                "{} glides to the rim for an effortless finish.",
                shooter.info.shortened_name()
            ),
        ],

        (ShotDifficulty::Close, Advantage::Neutral, true) => vec![
            format!("{} scores.", shooter.info.shortened_name()),
            format!("{} scores the layup.", shooter.info.shortened_name()),
            format!(
                "{} makes the shot in traffic.",
                shooter.info.shortened_name()
            ),
            format!(
                "{} finishes strong at the rim.",
                shooter.info.shortened_name()
            ),
        ],
        (ShotDifficulty::Close, Advantage::Defense, true) => vec![
            format!("{} scores with a miracle!", shooter.info.shortened_name()),
            format!(
                "{} scores the layup over {}.",
                shooter.info.shortened_name(),
                defenders[0].info.shortened_name()
            ),
            format!(
                "{} somehow gets the layup to fall over {}.",
                shooter.info.shortened_name(),
                defenders[0].info.shortened_name()
            ),
            format!(
                "{} banks it in against heavy defense from {}.",
                shooter.info.shortened_name(),
                defenders[0].info.shortened_name()
            ),
            format!(
                "{} fights through contact and scores over {}.",
                shooter.info.shortened_name(),
                defenders[0].info.shortened_name()
            ),
        ],
        (ShotDifficulty::Close, Advantage::Attack, false) => vec![
            format!(
                "{} manages to miss the open layup! The coach is furious...",
                shooter.info.shortened_name()
            ),
            format!(
                "{} misses the layup, what a shame!",
                shooter.info.shortened_name()
            ),
            format!(
                "{} blows an easy layup, what a shame!",
                shooter.info.shortened_name()
            ),
            format!(
                "{} can't believe {} missed that! Wide open!",
                shooter.info.shortened_name(),
                shooter.info.pronouns.as_possessive()
            ),
            format!(
                "{} fumbles the layup despite having no one near {}.",
                shooter.info.shortened_name(),
                shooter.info.pronouns.as_object()
            ),
        ],
        (ShotDifficulty::Close, Advantage::Neutral, false) => {
            vec![
                format!("{} misses the shot.", shooter.info.shortened_name()),
                format!(
                    "{} can't get the layup to fall.",
                    shooter.info.shortened_name()
                ),
                format!(
                    "{} tries but misses at the rim.",
                    shooter.info.shortened_name()
                ),
            ]
        }
        (ShotDifficulty::Close, Advantage::Defense, false) => vec![
            format!(
                "{} misses the layup, blocked by {}.",
                shooter.info.shortened_name(),
                defenders[0].info.shortened_name()
            ),
            format!(
                "{} misses the layup, {} got a piece of it.",
                shooter.info.shortened_name(),
                defenders[0].info.shortened_name()
            ),
            format!(
                "{} has {} layup denied by {} at the rim.",
                shooter.info.shortened_name(),
                shooter.info.pronouns.as_possessive(),
                defenders[0].info.shortened_name()
            ),
            format!(
                "{} misses as {} swats the ball away.",
                shooter.info.shortened_name(),
                defenders[0].info.shortened_name()
            ),
        ],

        (ShotDifficulty::Medium, Advantage::Attack, true) => vec![
            format!(
                "{} converts all alone from mid range.",
                shooter.info.shortened_name()
            ),
            format!("{} nails the open jumper.", shooter.info.shortened_name()),
            format!(
                "{} hits a smooth mid-range shot.",
                shooter.info.shortened_name()
            ),
        ],
        (ShotDifficulty::Medium, Advantage::Neutral, true) => {
            vec![
                format!("{} scores the jumper.", shooter.info.shortened_name()),
                format!(
                    "{} drains the mid-range shot.",
                    shooter.info.shortened_name()
                ),
                format!(
                    "{} makes a clean jumper from the elbow.",
                    shooter.info.shortened_name()
                ),
            ]
        }
        (ShotDifficulty::Medium, Advantage::Defense, true) => vec![
            format!(
                "{} scores a contested mid ranger.",
                shooter.info.shortened_name()
            ),
            format!(
                "{} scores a mid ranger over {}.",
                shooter.info.shortened_name(),
                defenders[0].info.shortened_name()
            ),
            format!(
                "{} drains a tough shot over {}.",
                shooter.info.shortened_name(),
                defenders[0].info.shortened_name()
            ),
            format!(
                "{} hits a difficult jumper in {}'s face.",
                shooter.info.shortened_name(),
                defenders[0].info.shortened_name()
            ),
        ],
        (ShotDifficulty::Medium, Advantage::Attack, false) => {
            vec![
                format!("{} misses an open shot!", shooter.info.shortened_name()),
                format!(
                    "{} can't connect from mid-range despite being wide open.",
                    shooter.info.shortened_name()
                ),
                format!(
                    "{} bricks an uncontested jumper.",
                    shooter.info.shortened_name()
                ),
            ]
        }
        (ShotDifficulty::Medium, Advantage::Neutral, false) => {
            vec![
                format!("{} misses the shot.", shooter.info.shortened_name()),
                format!(
                    "{} can't get the jumper to fall.",
                    shooter.info.shortened_name()
                ),
            ]
        }
        (ShotDifficulty::Medium, Advantage::Defense, false) => vec![
            format!(
                "{} misses the jumper, blocked by {}.",
                shooter.info.shortened_name(),
                defenders[0].info.shortened_name()
            ),
            format!(
                "{} is denied by {} on the mid-range attempt.",
                shooter.info.shortened_name(),
                defenders[0].info.shortened_name()
            ),
        ],

        (ShotDifficulty::Long, Advantage::Attack, true) => {
            vec![
                format!("{} scores the open three!", shooter.info.shortened_name()),
                format!(
                    "{} sinks the wide-open three-pointer.",
                    shooter.info.shortened_name()
                ),
                format!(
                    "{} nails the triple with no one around.",
                    shooter.info.shortened_name()
                ),
            ]
        }
        (ShotDifficulty::Long, Advantage::Neutral, true) => vec![
            format!(
                "{} scores the contested jumper!",
                shooter.info.shortened_name()
            ),
            format!(
                "{} drills the long-range shot.",
                shooter.info.shortened_name()
            ),
            format!("{} makes the three-pointer.", shooter.info.shortened_name()),
        ],
        (ShotDifficulty::Long, Advantage::Defense, true) => vec![
            format!(
                "{} makes the three-pointer under pressure.",
                shooter.info.shortened_name()
            ),
            format!(
                "{} scores a bomb in the face of {}!",
                shooter.info.shortened_name(),
                defenders[0].info.shortened_name()
            ),
            format!(
                "{} drills an incredible three over {}.",
                shooter.info.shortened_name(),
                defenders[0].info.shortened_name()
            ),
            format!(
                "{} hits a dagger with {} right on {} face.",
                shooter.info.shortened_name(),
                defenders[0].info.shortened_name(),
                shooter.info.pronouns.as_possessive()
            ),
        ],
        (ShotDifficulty::Long, Advantage::Attack, false) => vec![
            format!("{} misses the open three!", shooter.info.shortened_name()),
            format!(
                "{} can't capitalize on the wide-open three.",
                shooter.info.shortened_name()
            ),
            format!(
                "{} bricks the uncontested three-pointer.",
                shooter.info.shortened_name()
            ),
        ],
        (ShotDifficulty::Long, Advantage::Neutral, false) => vec![
            format!("{} misses from long range.", shooter.info.shortened_name()),
            format!(
                "{} can't connect on the deep shot.",
                shooter.info.shortened_name()
            ),
        ],
        (ShotDifficulty::Long, Advantage::Defense, false) => vec![
            format!(
                "{} misses the three, blocked by {}.",
                shooter.info.shortened_name(),
                defenders[0].info.shortened_name()
            ),
            format!(
                "{} is rejected by {} on the long-range attempt.",
                shooter.info.shortened_name(),
                defenders[0].info.shortened_name()
            ),
        ],
    };

    let mut description = text.choose(rng)?.to_string();
    if let Some(passer) = assist {
        description.push_str(format!(" Assist from {}.", passer.info.shortened_name()).as_str());
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

    assert!(input.attackers.len() == 1);
    let shooter_idx = input.attackers[0];
    let shooter = attacking_players[shooter_idx];

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
            p.roll(rng) / defenders.len() as u8
                + if p.is_knocked_out() {
                    0
                } else {
                    p.defense.block.value()
                }
        })
        .sum::<u8>();

    let roll = match input.advantage {
        Advantage::Attack => (shooter.roll(rng) + atk_skill) as i16 - (shot as u8) as i16,
        Advantage::Neutral => {
            (shooter.roll(rng) + atk_skill) as i16 - (shot as u8 + def_skill / 2) as i16
        }
        Advantage::Defense => {
            (shooter.roll(rng) + atk_skill) as i16 - (shot as u8 + def_skill) as i16
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
            shooter_update.extra_tiredness = TirednessCost::MEDIUM;
            shooter_update.last_action_shot = match game.possession {
                Possession::Home => {
                    let (x, y) = HOME_CLOSE_SHOT_POSITIONS.choose(rng)?.clone();
                    Some((x, y, result.score_change > 0))
                }
                Possession::Away => {
                    let (x, y) = AWAY_CLOSE_SHOT_POSITIONS.choose(rng)?.clone();
                    Some((x, y, result.score_change > 0))
                }
            }
        }
        ShotDifficulty::Medium => {
            shooter_update.attempted_2pt = 1;
            shooter_update.extra_tiredness = TirednessCost::MEDIUM;
            shooter_update.last_action_shot = match game.possession {
                Possession::Home => {
                    let (x, y) = HOME_MEDIUM_SHOT_POSITIONS.choose(rng)?.clone();
                    Some((x, y, result.score_change > 0))
                }
                Possession::Away => {
                    let (x, y) = AWAY_MEDIUM_SHOT_POSITIONS.choose(rng)?.clone();
                    Some((x, y, result.score_change > 0))
                }
            }
        }
        ShotDifficulty::Long => {
            shooter_update.attempted_3pt = 1;
            shooter_update.extra_tiredness = TirednessCost::MEDIUM;
            shooter_update.last_action_shot = match input.advantage {
                Advantage::Defense => match game.possession {
                    Possession::Home => {
                        let (x, y) = HOME_IMPOSSIBLE_SHOT_POSITIONS.choose(rng)?.clone();
                        Some((x, y, result.score_change > 0))
                    }
                    Possession::Away => {
                        let (x, y) = AWAY_IMPOSSIBLE_SHOT_POSITIONS.choose(rng)?.clone();
                        Some((x, y, result.score_change > 0))
                    }
                },
                _ => match game.possession {
                    Possession::Home => {
                        let (x, y) = HOME_LONG_SHOT_POSITIONS.choose(rng)?.clone();
                        Some((x, y, result.score_change > 0))
                    }
                    Possession::Away => {
                        let (x, y) = AWAY_LONG_SHOT_POSITIONS.choose(rng)?.clone();
                        Some((x, y, result.score_change > 0))
                    }
                },
            }
        }
    };

    if result.score_change > 0 {
        shooter_update.points = result.score_change;
        shooter_update.extra_morale += MoraleModifier::HIGH_BONUS;

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
    } else {
        shooter_update.extra_morale += MoraleModifier::MEDIUM_MALUS;
    }

    attack_stats_update.insert(shooter.id, shooter_update);

    for (idx, defender) in defenders.iter().enumerate() {
        let mut defender_update = GameStats::default();
        match input.advantage {
            Advantage::Defense => {
                defender_update.extra_tiredness = TirednessCost::MEDIUM;
                // Only the first defender gets the block
                if !success && idx == 0 && roll <= ADV_DEFENSE_LIMIT {
                    defender_update.blocks = 1;
                }
            }
            Advantage::Neutral => {
                defender_update.extra_tiredness = TirednessCost::MEDIUM;
            }
            _ => {}
        }
        defense_stats_update.insert(defender.id, defender_update);
    }
    result.attack_stats_update = Some(attack_stats_update);
    result.defense_stats_update = Some(defense_stats_update);
    return Some(result);
}
