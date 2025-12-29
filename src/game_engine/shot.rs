use super::{
    action::{ActionOutput, ActionSituation, Advantage},
    constants::*,
    game::Game,
    types::*,
};
use crate::core::{
    constants::{MoraleModifier, TirednessCost},
    player::Player,
    skill::GameSkill,
    CrewRole, TeamBonus, Trait, MAX_SKILL,
};
use rand::{seq::IndexedRandom, Rng};
use rand_chacha::ChaCha8Rng;
use std::collections::HashMap;

pub(crate) fn execute_close_shot(
    input: &ActionOutput,
    game: &Game,
    action_rng: &mut ChaCha8Rng,
    description_rng: &mut ChaCha8Rng,
) -> ActionOutput {
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
) -> ActionOutput {
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
) -> ActionOutput {
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
    with_dunk: bool,
    defenders: Vec<&Player>,
    shot_difficulty: ShotDifficulty,
    advantage: Advantage,
    success: bool,
) -> String {
    let text = match (shot_difficulty, advantage, success) {
        (ShotDifficulty::Close, Advantage::Attack, true) => {
            if with_dunk {
                vec![
                    format!(
                        "{} slams the ball in the basket! What a move!",
                        shooter.info.short_name()
                    ),
                    format!("{} dunks it with two hands", shooter.info.short_name()),
                    format!(
                        "{} slams the ball with a spectacular jump.",
                        shooter.info.short_name()
                    ),
                    format!(
                        "Reverse dunk from {}! Everyone is on their feet!",
                        shooter.info.short_name()
                    ),
                    format!(
                        "{} glides through the air and slams it with one hand!",
                        shooter.info.short_name()
                    ),
                ]
            } else {
                vec![
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
                ]
            }
        }

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
                shooter.info.pronouns.as_subject()
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
                            "{} tries to force a layup against {}, but {} stuffs it at the rim. No chance!",
                            shooter.info.short_name(),
                            p.info.short_name(),
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
                        "{} is denied by {} on the mid-range attempt.",
                        shooter.info.short_name(),
                        p.info.short_name()
                    ),
                    format!(
                            "{} tries a fadeaway jumper over {}, but {} contests it perfectly. Poor shot selection!",
                            shooter.info.short_name(),
                            p.info.short_name(),
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

    let mut description = text
        .choose(description_rng)
        .expect("There should be a description")
        .to_string();
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
        let assist_description = options
            .choose(description_rng)
            .expect("There should be a description");
        description.push_str(assist_description);
    };
    description
}

fn execute_shot(
    input: &ActionOutput,
    game: &Game,
    action_rng: &mut ChaCha8Rng,
    description_rng: &mut ChaCha8Rng,
    shot_difficulty: ShotDifficulty,
) -> ActionOutput {
    let attacking_players_array = game.attacking_players_array();
    let defending_players_array = game.defending_players_array();

    assert!(input.attackers.len() == 1);
    let shooter_idx = input.attackers[0];
    let shooter = attacking_players_array[shooter_idx];

    if input.advantage == Advantage::Defense {
        assert!(!input.defenders.is_empty());
    }

    assert!(input.defenders.len() < 2); // FIXME: in the future we should allow this
    let defenders = input
        .defenders
        .iter()
        .map(|&idx| defending_players_array[idx])
        .collect::<Vec<&Player>>();

    let atk_skill = match shot_difficulty {
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
        Advantage::Attack => {
            (shooter.roll(action_rng).max(shooter.roll(action_rng)) + atk_skill)
                - (shot_difficulty as i16 + def_skill)
        }
        Advantage::Neutral => {
            (shooter.roll(action_rng) + atk_skill) - (shot_difficulty as i16 + def_skill)
        }
        Advantage::Defense => {
            (shooter.roll(action_rng).min(shooter.roll(action_rng)) + atk_skill)
                - (shot_difficulty as i16 + def_skill)
        }
    };

    let success = roll > 0;
    let blocked_by =
        if !success && input.advantage == Advantage::Defense && roll <= ADV_DEFENSE_LIMIT {
            Some(defenders[0])
        } else {
            None
        };

    let with_dunk = success
        && input.advantage == Advantage::Attack
        && shot_difficulty == ShotDifficulty::Close
        && action_rng.random_bool(
            (DUNK_PROBABILITY
                * if matches!(shooter.special_trait, Some(Trait::Showpirate)) {
                    2.0
                } else {
                    1.0
                }
                * ((0.25 * (shooter.info.height - 150.0)).bound() / MAX_SKILL) as f64
                * (shooter.athletics.vertical / MAX_SKILL) as f64)
                .clamp(0.0, 1.0),
        );

    let assist_by = if success {
        input.assist_from.map(|idx| attacking_players_array[idx])
    } else {
        None
    };

    let description = description(
        description_rng,
        shooter,
        assist_by,
        blocked_by,
        with_dunk,
        defenders.clone(),
        shot_difficulty,
        input.advantage,
        success,
    );

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
            let score_change = match shot_difficulty {
                ShotDifficulty::Close | ShotDifficulty::Medium => 2,
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

    shooter_update.extra_tiredness = match input.advantage {
        Advantage::Attack => TirednessCost::LOW,
        _ => TirednessCost::MEDIUM,
    };

    match shot_difficulty {
        ShotDifficulty::Close => {
            shooter_update.attempted_2pt = 1;
            shooter_update.last_action_shot = match game.possession {
                Possession::Home => {
                    let (x, y) = *HOME_CLOSE_SHOT_POSITIONS
                        .choose(action_rng)
                        .expect("There should be a shooting position");
                    Some((x, y, result.score_change > 0))
                }
                Possession::Away => {
                    let (x, y) = *AWAY_CLOSE_SHOT_POSITIONS
                        .choose(action_rng)
                        .expect("There should be a shooting position");
                    Some((x, y, result.score_change > 0))
                }
            }
        }
        ShotDifficulty::Medium => {
            shooter_update.attempted_2pt = 1;
            shooter_update.last_action_shot = match game.possession {
                Possession::Home => {
                    let (x, y) = *HOME_MEDIUM_SHOT_POSITIONS
                        .choose(action_rng)
                        .expect("There should be a shooting position");
                    Some((x, y, result.score_change > 0))
                }
                Possession::Away => {
                    let (x, y) = *AWAY_MEDIUM_SHOT_POSITIONS
                        .choose(action_rng)
                        .expect("There should be a shooting position");
                    Some((x, y, result.score_change > 0))
                }
            }
        }
        ShotDifficulty::Long => {
            shooter_update.attempted_3pt = 1;
            shooter_update.last_action_shot = match input.advantage {
                Advantage::Defense => match game.possession {
                    Possession::Home => {
                        let (x, y) = *HOME_IMPOSSIBLE_SHOT_POSITIONS
                            .choose(action_rng)
                            .expect("There should be a shooting position");
                        Some((x, y, result.score_change > 0))
                    }
                    Possession::Away => {
                        let (x, y) = *AWAY_IMPOSSIBLE_SHOT_POSITIONS
                            .choose(action_rng)
                            .expect("There should be a shooting position");
                        Some((x, y, result.score_change > 0))
                    }
                },
                _ => match game.possession {
                    Possession::Home => {
                        let (x, y) = *HOME_LONG_SHOT_POSITIONS
                            .choose(action_rng)
                            .expect("There should be a shooting position");
                        Some((x, y, result.score_change > 0))
                    }
                    Possession::Away => {
                        let (x, y) = *AWAY_LONG_SHOT_POSITIONS
                            .choose(action_rng)
                            .expect("There should be a shooting position");
                        Some((x, y, result.score_change > 0))
                    }
                },
            }
        }
    };

    if success {
        shooter_update.points = result.score_change;
        shooter_update.extra_morale += match input.advantage {
            Advantage::Defense => MoraleModifier::HIGH_BONUS,
            Advantage::Neutral => MoraleModifier::MEDIUM_BONUS,
            Advantage::Attack => MoraleModifier::SMALL_BONUS,
        };

        match shot_difficulty {
            ShotDifficulty::Close | ShotDifficulty::Medium => shooter_update.made_2pt = 1,
            ShotDifficulty::Long => shooter_update.made_3pt = 1,
        };
        if let Some(passer_index) = input.assist_from {
            let passer_update = GameStats {
                assists: 1,
                extra_morale: MoraleModifier::SMALL_BONUS,
                ..Default::default()
            };
            let passer_id = attacking_players_array[passer_index].id;
            attack_stats_update.insert(passer_id, passer_update);
        }
    } else {
        shooter_update.extra_morale += match input.advantage {
            Advantage::Defense => MoraleModifier::SMALL_MALUS,
            Advantage::Neutral => MoraleModifier::MEDIUM_MALUS,
            Advantage::Attack => MoraleModifier::HIGH_MALUS,
        };

        if blocked_by.is_some() {
            shooter_update.extra_morale += MoraleModifier::MEDIUM_MALUS;
        }
    }

    attack_stats_update.insert(shooter.id, shooter_update);

    for defender in defenders.iter() {
        let mut defender_update = GameStats::default();
        if input.advantage == Advantage::Defense {
            if matches!(blocked_by, Some(player) if player.id == defender.id) {
                defender_update.blocks = 1;
                defender_update.extra_morale += MoraleModifier::HIGH_BONUS;
                defender_update.extra_tiredness = TirednessCost::MEDIUM;
            } else {
                // Help consumes less energy
                defender_update.extra_tiredness = TirednessCost::LOW;
            }
        }
        defense_stats_update.insert(defender.id, defender_update);
    }

    // Add morale modifiers if team scored.
    // These modifiers are applied to the whole team, not only playing players.
    if success {
        // Conditions for extra morale boost:
        // shot success, team is losing at most by a certain margin.
        let team_captain = game
            .all_attacking_players()
            .values()
            .find(|&p| p.info.crew_role == CrewRole::Captain);
        let losing_margin = 5 * team_captain
            .map(|p| TeamBonus::Reputation.current_player_bonus(p))
            .unwrap_or(1.0) as u16;
        // // Note: this is the score BEFORE the result is applied to the score.
        let score = game.get_score();
        let attacking_team_was_losing_by_margin = if input.possession == Possession::Home {
            score.0 < score.1 && score.1 - score.0 <= losing_margin
        } else {
            score.1 < score.0 && score.0 - score.1 <= losing_margin
        };

        let extra_morale = if attacking_team_was_losing_by_margin {
            MoraleModifier::MEDIUM_BONUS
        } else {
            MoraleModifier::SMALL_BONUS
        };

        for player in game.all_attacking_players().values() {
            attack_stats_update
                .entry(player.id)
                .and_modify(|stats| stats.extra_morale += extra_morale)
                .or_insert(GameStats {
                    extra_morale,
                    ..Default::default()
                });
        }

        for player in game.all_defending_players().values() {
            let extra_morale = if with_dunk {
                MoraleModifier::HIGH_MALUS
            } else {
                MoraleModifier::SMALL_MALUS
            };

            defense_stats_update
                .entry(player.id)
                .and_modify(|stats| stats.extra_morale += extra_morale)
                .or_insert(GameStats {
                    extra_morale,
                    ..Default::default()
                });
        }
    }

    result.attack_stats_update = Some(attack_stats_update);
    result.defense_stats_update = Some(defense_stats_update);
    result
}

#[cfg(test)]
mod tests {
    use itertools::Itertools;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;
    use strum::IntoEnumIterator;

    use crate::{
        core::{Player, Team, MAX_PLAYERS_PER_GAME},
        game_engine::{
            action::{ActionOutput, ActionSituation, Advantage},
            game::Game,
            shot::{execute_close_shot, execute_long_shot, execute_medium_shot},
            types::TeamInGame,
        },
        types::{AppResult, PlayerMap, TeamId},
    };

    fn generate_team_in_game(shoot_skill: f32, block_skill: f32) -> TeamInGame {
        let team = Team {
            id: TeamId::new_v4(),
            ..Default::default()
        };

        let mut players = PlayerMap::new();
        for _ in 0..MAX_PLAYERS_PER_GAME {
            let mut player = Player::default().randomize(None);
            player.offense.close_range = shoot_skill;
            player.offense.medium_range = shoot_skill;
            player.offense.long_range = shoot_skill;
            player.defense.block = block_skill;
            players.insert(player.id, player);
        }

        TeamInGame::new(&team, players)
    }

    #[test]
    fn test_shooting() -> AppResult<()> {
        const N: usize = 40_000;

        let shoot_skill = 20.0;
        let block_skill = 20.0;

        let home_team_in_game = generate_team_in_game(shoot_skill, block_skill);
        let away_team_in_game = generate_team_in_game(shoot_skill, block_skill);
        let game = &Game::test(home_team_in_game, away_team_in_game);

        let action_rng = &mut ChaCha8Rng::from_os_rng();
        let description_rng = &mut ChaCha8Rng::from_os_rng();

        for situation in [
            ActionSituation::CloseShot,
            ActionSituation::MediumShot,
            ActionSituation::LongShot,
        ] {
            println!("{:#?}", situation);

            for advantage in Advantage::iter() {
                println!("....{:<7} ", format!("{:#?}", advantage));
                for num_defenders in 0..=1 {
                    if advantage == Advantage::Defense && num_defenders == 0 {
                        continue;
                    }
                    let defenders = (0..num_defenders).collect_vec();
                    let input = ActionOutput {
                        advantage,
                        attackers: vec![0],
                        defenders,
                        situation,
                        ..Default::default()
                    };

                    let mut made = 0;
                    let mut attempted = 0;
                    for _ in 0..N {
                        let result = match situation {
                            ActionSituation::CloseShot => {
                                execute_close_shot(&input, game, action_rng, description_rng)
                            }
                            ActionSituation::MediumShot => {
                                execute_medium_shot(&input, game, action_rng, description_rng)
                            }
                            ActionSituation::LongShot => {
                                execute_long_shot(&input, game, action_rng, description_rng)
                            }
                            _ => unreachable!(),
                        };
                        attempted += 1;
                        if result.score_change > 0 {
                            made += 1;
                        }
                    }
                    println!(
                        ".....Def {} => {:.2}%",
                        num_defenders,
                        (made as f32 / attempted as f32) * 100.0
                    );
                }
            }
        }

        Ok(())
    }
}
