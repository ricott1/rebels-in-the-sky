use super::{
    action::{ActionOutput, ActionSituation, Advantage},
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
use rand::{seq::IndexedRandom, Rng};
use rand_chacha::ChaCha8Rng;
use std::collections::HashMap;

pub(crate) fn execute_close_shot(
    input: &ActionOutput,
    game: &Game,
    action_rng: &mut ChaCha8Rng,
    description_rng: &mut ChaCha8Rng,
) -> Option<ActionOutput> {
    execute_shot(
        input,
        game,
        action_rng,
        description_rng,
        ShotDifficulty::Close,
    )
}

pub(crate) fn execute_medium_shot(
    input: &ActionOutput,
    game: &Game,
    action_rng: &mut ChaCha8Rng,
    description_rng: &mut ChaCha8Rng,
) -> Option<ActionOutput> {
    execute_shot(
        input,
        game,
        action_rng,
        description_rng,
        ShotDifficulty::Medium,
    )
}

pub(crate) fn execute_long_shot(
    input: &ActionOutput,
    game: &Game,
    action_rng: &mut ChaCha8Rng,
    description_rng: &mut ChaCha8Rng,
) -> Option<ActionOutput> {
    execute_shot(
        input,
        game,
        action_rng,
        description_rng,
        ShotDifficulty::Long,
    )
}

fn description(
    description_rng: &mut ChaCha8Rng,
    shooter: &Player,
    assist_by: Option<&Player>,
    blocked_by: Option<&Player>,
    defenders: Vec<&Player>,
    shot: ShotDifficulty,
    advantage: Advantage,
    success: bool,
) -> Option<String> {
    let text = match (shot, advantage, success) {
        (ShotDifficulty::Close, Advantage::Attack, true) => vec![
            format!("{} scores an easy layup.", shooter.info.short_name()),
            format!(
                "{} would never miss in this situation.",
                shooter.info.short_name()
            ),
            format!("{} scores with ease.", shooter.info.short_name()),
            format!("{} scores the easy layup.", shooter.info.short_name()),
            format!(
                "{} glides to the rim for an effortless finish.",
                shooter.info.short_name()
            ),
        ],

        (ShotDifficulty::Close, Advantage::Neutral, true) => vec![
            format!("{} scores.", shooter.info.short_name()),
            format!("{} scores the layup.", shooter.info.short_name()),
            format!("{} makes the shot in traffic.", shooter.info.short_name()),
            format!("{} finishes strong at the rim.", shooter.info.short_name()),
        ],
        (ShotDifficulty::Close, Advantage::Defense, true) => vec![
            format!("{} scores with a miracle!", shooter.info.short_name()),
            format!(
                "{} scores the layup over {}.",
                shooter.info.short_name(),
                defenders[0].info.short_name()
            ),
            format!(
                "{} somehow gets the layup to fall over {}.",
                shooter.info.short_name(),
                defenders[0].info.short_name()
            ),
            format!(
                "{} banks it in against heavy defense from {}.",
                shooter.info.short_name(),
                defenders[0].info.short_name()
            ),
            format!(
                "{} fights through contact and scores over {}.",
                shooter.info.short_name(),
                defenders[0].info.short_name()
            ),
        ],
        (ShotDifficulty::Close, Advantage::Attack, false) => vec![
            format!(
                "{} manages to miss the open layup! The coach is furious...",
                shooter.info.short_name()
            ),
            format!(
                "{} misses the layup, what a shame!",
                shooter.info.short_name()
            ),
            format!(
                "{} blows an easy layup, what a shame!",
                shooter.info.short_name()
            ),
            format!(
                "{} can't believe {} missed that! Wide open!",
                shooter.info.short_name(),
                shooter.info.pronouns.as_possessive()
            ),
            format!(
                "{} fumbles the layup despite having no one near {}.",
                shooter.info.short_name(),
                shooter.info.pronouns.as_object()
            ),
        ],
        (ShotDifficulty::Close, Advantage::Neutral, false) => {
            vec![
                format!("{} misses the shot.", shooter.info.short_name()),
                format!("{} can't get the layup to fall.", shooter.info.short_name()),
                format!("{} tries but misses at the rim.", shooter.info.short_name()),
            ]
        }
        (ShotDifficulty::Close, Advantage::Defense, false) => {
            if let Some(p) = blocked_by {
                vec![
                    format!(
                        "{} misses the layup, blocked by {}.",
                        shooter.info.short_name(),
                        p.info.short_name()
                    ),
                    format!(
                        "{} misses the layup, {} got a piece of it.",
                        shooter.info.short_name(),
                        p.info.short_name()
                    ),
                    format!(
                        "{} has {} layup denied by {} at the rim.",
                        shooter.info.short_name(),
                        shooter.info.pronouns.as_possessive(),
                        p.info.short_name()
                    ),
                    format!(
                        "{} misses as {} swats the ball away.",
                        shooter.info.short_name(),
                        p.info.short_name()
                    ),
                ]
            } else {
                vec![
                    format!("{} misses the contested layup.", shooter.info.short_name(),),
                    format!(
                        "{} misses the layup, {} did a good job contesting it.",
                        shooter.info.short_name(),
                        defenders[0].info.short_name()
                    ),
                    format!(
                        "{} misses as {} keeps good watch.",
                        shooter.info.short_name(),
                        defenders[0].info.short_name()
                    ),
                ]
            }
        }

        (ShotDifficulty::Medium, Advantage::Attack, true) => vec![
            format!(
                "{} converts all alone from mid range.",
                shooter.info.short_name()
            ),
            format!("{} nails the open jumper.", shooter.info.short_name()),
            format!(
                "{} hits a smooth mid-range shot.",
                shooter.info.short_name()
            ),
        ],
        (ShotDifficulty::Medium, Advantage::Neutral, true) => {
            vec![
                format!("{} scores the jumper.", shooter.info.short_name()),
                format!("{} drains the mid-range shot.", shooter.info.short_name()),
                format!(
                    "{} makes a clean jumper from the elbow.",
                    shooter.info.short_name()
                ),
            ]
        }
        (ShotDifficulty::Medium, Advantage::Defense, true) => vec![
            format!(
                "{} scores a contested mid ranger.",
                shooter.info.short_name()
            ),
            format!(
                "{} scores a mid ranger over {}.",
                shooter.info.short_name(),
                defenders[0].info.short_name()
            ),
            format!(
                "{} drains a tough shot over {}.",
                shooter.info.short_name(),
                defenders[0].info.short_name()
            ),
            format!(
                "{} hits a difficult jumper in {}'s face.",
                shooter.info.short_name(),
                defenders[0].info.short_name()
            ),
        ],
        (ShotDifficulty::Medium, Advantage::Attack, false) => {
            vec![
                format!("{} misses an open shot!", shooter.info.short_name()),
                format!(
                    "{} can't connect from mid-range despite being wide open.",
                    shooter.info.short_name()
                ),
                format!(
                    "{} bricks an uncontested jumper.",
                    shooter.info.short_name()
                ),
            ]
        }
        (ShotDifficulty::Medium, Advantage::Neutral, false) => {
            vec![
                format!("{} misses the shot.", shooter.info.short_name()),
                format!(
                    "{} can't get the jumper to fall.",
                    shooter.info.short_name()
                ),
            ]
        }
        (ShotDifficulty::Medium, Advantage::Defense, false) => {
            if let Some(p) = blocked_by {
                vec![
                    format!(
                        "{} misses the jumper, blocked by {}.",
                        shooter.info.short_name(),
                        p.info.short_name()
                    ),
                    format!(
                        "{} is denied by {} on the mid-range attempt.",
                        shooter.info.short_name(),
                        p.info.short_name()
                    ),
                ]
            } else {
                vec![
                    format!("{} misses a tough jumper.", shooter.info.short_name(),),
                    format!(
                        "{} misses, good defense by {} to contest the mid-range attempt.",
                        shooter.info.short_name(),
                        defenders[0].info.short_name()
                    ),
                ]
            }
        }

        (ShotDifficulty::Long, Advantage::Attack, true) => {
            vec![
                format!("{} scores the open three!", shooter.info.short_name()),
                format!(
                    "{} sinks the wide-open three-pointer.",
                    shooter.info.short_name()
                ),
                format!(
                    "{} nails the triple with no one around.",
                    shooter.info.short_name()
                ),
            ]
        }
        (ShotDifficulty::Long, Advantage::Neutral, true) => vec![
            format!("{} scores the contested jumper!", shooter.info.short_name()),
            format!("{} drills the long-range shot.", shooter.info.short_name()),
            format!("{} makes the three-pointer.", shooter.info.short_name()),
        ],
        (ShotDifficulty::Long, Advantage::Defense, true) => vec![
            format!(
                "{} makes the three-pointer under pressure.",
                shooter.info.short_name()
            ),
            format!(
                "{} scores a bomb in the face of {}!",
                shooter.info.short_name(),
                defenders[0].info.short_name()
            ),
            format!(
                "{} drills an incredible three over {}.",
                shooter.info.short_name(),
                defenders[0].info.short_name()
            ),
            format!(
                "{} hits a dagger with {} right on {} face.",
                shooter.info.short_name(),
                defenders[0].info.short_name(),
                shooter.info.pronouns.as_possessive()
            ),
        ],
        (ShotDifficulty::Long, Advantage::Attack, false) => vec![
            format!("{} misses the open three!", shooter.info.short_name()),
            format!(
                "{} can't capitalize on the wide-open three.",
                shooter.info.short_name()
            ),
            format!(
                "{} bricks the uncontested three-pointer.",
                shooter.info.short_name()
            ),
        ],
        (ShotDifficulty::Long, Advantage::Neutral, false) => vec![
            format!("{} misses from long range.", shooter.info.short_name()),
            format!(
                "{} can't connect on the deep shot.",
                shooter.info.short_name()
            ),
        ],
        (ShotDifficulty::Long, Advantage::Defense, false) => {
            if let Some(p) = blocked_by {
                vec![
                    format!(
                        "{} misses the three, blocked by {}.",
                        shooter.info.short_name(),
                        p.info.short_name()
                    ),
                    format!(
                        "{} is rejected by {} on the long-range attempt.",
                        shooter.info.short_name(),
                        p.info.short_name()
                    ),
                ]
            } else {
                vec![
                    format!(
                        "{} misses the three, {} was all over {}.",
                        shooter.info.short_name(),
                        defenders[0].info.short_name(),
                        shooter.info.pronouns.as_object()
                    ),
                    format!(
                        "{} misses the long-range attempt, good defense by {}",
                        shooter.info.short_name(),
                        defenders[0].info.short_name()
                    ),
                ]
            }
        }
    };

    let mut description = text.choose(description_rng)?.to_string();
    if let Some(passer) = assist_by {
        let options = match advantage {
            Advantage::Attack => [
                format!(" Nice assist from {}.", passer.info.short_name()),
                format!(" Good pass from {}.", passer.info.short_name()),
                format!(
                    " {} deserves at least half the praise.",
                    passer.info.short_name()
                ),
            ],
            Advantage::Neutral => [
                format!(" Assist from {}.", passer.info.short_name()),
                format!(" Nice assist from {}.", passer.info.short_name()),
                format!(" Good pass from {}.", passer.info.short_name()),
            ],
            Advantage::Defense => [
                format!(" Assist from {}.", passer.info.short_name()),
                format!(
                    " The pass from {} was not perfect, but {} managed to convert it.",
                    passer.info.short_name(),
                    shooter.info.pronouns.as_subject()
                ),
                format!(
                    " {} managed to covert {}'s pass.",
                    shooter.info.pronouns.as_subject(),
                    passer.info.short_name()
                ),
            ],
        };
        let assist_description = options.choose(description_rng)?;
        description.push_str(assist_description);
    };
    Some(description)
}

fn execute_shot(
    input: &ActionOutput,
    game: &Game,
    action_rng: &mut ChaCha8Rng,
    description_rng: &mut ChaCha8Rng,
    shot: ShotDifficulty,
) -> Option<ActionOutput> {
    let attacking_players = game.attacking_players();
    let defending_players = game.defending_players();

    assert!(input.attackers.len() == 1);
    let shooter_idx = input.attackers[0];
    let shooter = attacking_players[shooter_idx];

    if input.advantage == Advantage::Defense {
        assert!(!input.defenders.is_empty());
    }
    let defenders = input
        .defenders
        .iter()
        .map(|&idx| defending_players[idx])
        .collect::<Vec<&Player>>();

    let atk_skill = match shot {
        ShotDifficulty::Close => shooter.offense.close_range.game_value(),
        ShotDifficulty::Medium => shooter.offense.medium_range.game_value(),
        ShotDifficulty::Long => shooter.offense.long_range.game_value(),
    };
    let def_skill = defenders
        .iter()
        .map(|&p| {
            p.roll(action_rng) / defenders.len() as i16
                + if p.is_knocked_out() {
                    0
                } else {
                    p.defense.block.game_value()
                }
        })
        .sum::<i16>();

    let roll = match input.advantage {
        Advantage::Attack => (shooter.roll(action_rng) + atk_skill) - (shot as i16),
        Advantage::Neutral => {
            (shooter.roll(action_rng) + atk_skill) - (shot as i16 + def_skill / 2)
        }
        Advantage::Defense => (shooter.roll(action_rng) + atk_skill) - (shot as i16 + def_skill),
    };

    let success = roll > 0;
    let blocked_by =
        if !success && input.advantage == Advantage::Defense && roll <= ADV_DEFENSE_LIMIT {
            Some(defenders[0])
        } else {
            None
        };

    let assist_by = if success && input.assist_from.is_some() {
        Some(attacking_players[input.assist_from?])
    } else {
        None
    };

    let description = description(
        description_rng,
        shooter,
        assist_by,
        blocked_by,
        defenders.clone(),
        shot,
        input.advantage,
        success,
    )?;

    let mut result = match success {
        false => {
            // Attackers will get a malus in the rebound action.
            let advantage = match input.advantage {
                Advantage::Attack => Advantage::Neutral,
                Advantage::Neutral | Advantage::Defense => Advantage::Defense,
            };
            ActionOutput {
                advantage,
                possession: input.possession,
                attackers: vec![shooter_idx],
                defenders: input.defenders.clone(),
                situation: ActionSituation::MissedShot,
                description,
                start_at: input.end_at,
                end_at: input.end_at.plus(1 + action_rng.random_range(0..=2)),
                home_score: input.home_score,
                away_score: input.away_score,
                ..Default::default()
            }
        }
        true => {
            let score_change = match shot {
                ShotDifficulty::Close => 2,
                ShotDifficulty::Medium => 2,
                ShotDifficulty::Long => 3,
            };
            ActionOutput {
                score_change,
                home_score: match input.possession {
                    Possession::Home => input.home_score + score_change,
                    Possession::Away => input.home_score,
                },
                away_score: match input.possession {
                    Possession::Home => input.away_score,
                    Possession::Away => input.away_score + score_change,
                },
                possession: !input.possession,
                situation: ActionSituation::BallInBackcourt,
                description,
                start_at: input.end_at,
                end_at: input.end_at.plus(12 + action_rng.random_range(0..=6)),
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
                    let (x, y) = *HOME_CLOSE_SHOT_POSITIONS.choose(action_rng)?;
                    Some((x, y, result.score_change > 0))
                }
                Possession::Away => {
                    let (x, y) = *AWAY_CLOSE_SHOT_POSITIONS.choose(action_rng)?;
                    Some((x, y, result.score_change > 0))
                }
            }
        }
        ShotDifficulty::Medium => {
            shooter_update.attempted_2pt = 1;
            shooter_update.extra_tiredness = TirednessCost::MEDIUM;
            shooter_update.last_action_shot = match game.possession {
                Possession::Home => {
                    let (x, y) = *HOME_MEDIUM_SHOT_POSITIONS.choose(action_rng)?;
                    Some((x, y, result.score_change > 0))
                }
                Possession::Away => {
                    let (x, y) = *AWAY_MEDIUM_SHOT_POSITIONS.choose(action_rng)?;
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
                        let (x, y) = *HOME_IMPOSSIBLE_SHOT_POSITIONS.choose(action_rng)?;
                        Some((x, y, result.score_change > 0))
                    }
                    Possession::Away => {
                        let (x, y) = *AWAY_IMPOSSIBLE_SHOT_POSITIONS.choose(action_rng)?;
                        Some((x, y, result.score_change > 0))
                    }
                },
                _ => match game.possession {
                    Possession::Home => {
                        let (x, y) = *HOME_LONG_SHOT_POSITIONS.choose(action_rng)?;
                        Some((x, y, result.score_change > 0))
                    }
                    Possession::Away => {
                        let (x, y) = *AWAY_LONG_SHOT_POSITIONS.choose(action_rng)?;
                        Some((x, y, result.score_change > 0))
                    }
                },
            }
        }
    };

    if success {
        shooter_update.points = result.score_change;
        shooter_update.extra_morale += MoraleModifier::HIGH_BONUS;

        match shot {
            ShotDifficulty::Close => shooter_update.made_2pt = 1,
            ShotDifficulty::Medium => shooter_update.made_2pt = 1,
            ShotDifficulty::Long => shooter_update.made_3pt = 1,
        };
        if let Some(passer_index) = input.assist_from {
            let passer_update = GameStats {
                assists: 1,
                extra_morale: MoraleModifier::SMALL_BONUS,
                ..Default::default()
            };
            let passer_id = attacking_players[passer_index].id;
            attack_stats_update.insert(passer_id, passer_update);
        }
    } else if blocked_by.is_some() {
        shooter_update.extra_morale += MoraleModifier::HIGH_MALUS;
    } else {
        shooter_update.extra_morale += MoraleModifier::MEDIUM_MALUS;
    }

    attack_stats_update.insert(shooter.id, shooter_update);

    for defender in defenders.iter() {
        let mut defender_update = GameStats {
            extra_tiredness: TirednessCost::MEDIUM,
            ..Default::default()
        };
        if input.advantage == Advantage::Defense {
            if matches!(blocked_by, Some(player) if player.id == defender.id) {
                defender_update.blocks = 1;
                defender_update.extra_morale += MoraleModifier::MEDIUM_BONUS;
            } else {
                // Help consumes less energy
                defender_update.extra_tiredness = TirednessCost::LOW;
            }
        }
        defense_stats_update.insert(defender.id, defender_update);
    }
    result.attack_stats_update = Some(attack_stats_update);
    result.defense_stats_update = Some(defense_stats_update);
    Some(result)
}
