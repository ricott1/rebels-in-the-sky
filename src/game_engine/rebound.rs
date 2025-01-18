use super::{
    action::{ActionOutput, ActionSituation, EngineAction},
    game::Game,
    types::*,
};
use crate::{
    game_engine::{
        action::{Action, Advantage},
        constants::*,
        types::GameStatsMap,
    },
    world::constants::TirednessCost,
};
use rand::{seq::SliceRandom, Rng};
use rand_chacha::ChaCha8Rng;
use std::{
    cmp::{max, min},
    collections::HashMap,
};

const MIN_REBOUND_VALUE: u16 = 40;
const REBOUND_POSITION_SCALING: f32 = 12.0;

fn position_rebound_bonus(idx: usize) -> f32 {
    1.0 + idx as f32 / REBOUND_POSITION_SCALING
}

#[derive(Debug, Default)]
pub struct Rebound;

impl EngineAction for Rebound {
    fn execute(input: &ActionOutput, game: &Game, rng: &mut ChaCha8Rng) -> Option<ActionOutput> {
        let attacking_players = game.attacking_players();
        let defending_players = game.defending_players();

        let mut attack_rebounds: Vec<u16> = attacking_players
            .iter()
            .map(|&p| p.technical.rebounds as u16)
            .collect();
        let mut defense_rebounds: Vec<u16> = defending_players
            .iter()
            .map(|&p| p.technical.rebounds as u16)
            .collect();

        // apply reduction for shooter rebounds
        assert!(input.attackers.len() == 1);
        attack_rebounds[input.attackers[0]] =
            (attack_rebounds[input.attackers[0]] as f32 * 2.0 / 3.0) as u16;

        for idx in 0..attacking_players.len() {
            // apply bonus based on position
            attack_rebounds[idx] =
                (attack_rebounds[idx] as f32 * position_rebound_bonus(idx)) as u16;
            //add random roll
            match input.advantage {
                Advantage::Attack => {
                    attack_rebounds[idx] += max(
                        attacking_players[idx].roll(rng) as u16,
                        attacking_players[idx].roll(rng) as u16,
                    );
                }
                Advantage::Neutral => {
                    attack_rebounds[idx] += attacking_players[idx].roll(rng) as u16;
                }
                Advantage::Defense => {
                    attack_rebounds[idx] += min(
                        attacking_players[idx].roll(rng) as u16,
                        attacking_players[idx].roll(rng) as u16,
                    );
                }
            }
        }
        for idx in 0..defending_players.len() {
            // apply reduction for defender rebounds.
            if input.defenders.contains(&idx) {
                defense_rebounds[idx] = (defense_rebounds[idx] as f32 * 3.0 / 4.0) as u16;
            }
            // apply bonus based on position
            defense_rebounds[idx] =
                (defense_rebounds[idx] as f32 * position_rebound_bonus(idx)) as u16;
            //add random roll
            defense_rebounds[idx] += defending_players[idx].roll(rng) as u16;
        }

        let attack_result = *attack_rebounds
            .iter()
            .max()
            .expect("Attack rebounds should be non-empty");
        let defence_result = *defense_rebounds
            .iter()
            .max()
            .expect("Defense rebounds should be non-empty");

        let attack_rebounder_idx = attack_rebounds.iter().position(|&r| r == attack_result)?;
        let defence_rebounder_idx = defense_rebounds.iter().position(|&r| r == defence_result)?;

        let attack_rebounder = attacking_players[attack_rebounder_idx];
        let defence_rebounder = defending_players[defence_rebounder_idx];

        log::debug!(
            "Rebound debugging: {} vs {}, to beat {}",
            attack_result,
            defence_result,
            MIN_REBOUND_VALUE
        );
        let result = match attack_result as i16 - defence_result as i16 + Self::tactic_modifier(game, &Action::Rebound){
            x if x > ADV_ATTACK_LIMIT
                || (x > 0
                    && attack_result >= MIN_REBOUND_VALUE
                    && attack_rebounder_idx == input.attackers[0]) =>
            {
                let mut attack_stats_update: GameStatsMap = HashMap::new();
                let mut rebounder_update = GameStats::default();
                rebounder_update.offensive_rebounds = 1;
                rebounder_update.extra_tiredness = TirednessCost::LOW;
                attack_stats_update.insert(attack_rebounder.id, rebounder_update);
                let description: String;
                if attack_rebounder_idx == input.attackers[0] {
                    description = [
                        format!(
                            "{} grabs {} own rebound with a quick reaction.",
                            attack_rebounder.info.shortened_name(),
                            attack_rebounder.info.pronouns.as_possessive()
                        ),
                        format!(
                            "{} snatches the ball after missing the shot, showing persistence.",
                            attack_rebounder.info.shortened_name(),
                        ),
                        format!(
                            "{} secures {} own miss for a second chance.",
                            attack_rebounder.info.shortened_name(),
                            attack_rebounder.info.pronouns.as_possessive()
                        ),
                        format!(
                            "{} quickly leaps and grabs {} missed shot, avoiding defenders.",
                            attack_rebounder.info.shortened_name(),
                            attack_rebounder.info.pronouns.as_possessive()
                        ),
                        format!(
                            "{} fights through the defenders to secure {} own rebound.",
                            attack_rebounder.info.shortened_name(),
                            attack_rebounder.info.pronouns.as_possessive()
                        ),
                    ]
                    .choose(rng)
                    .expect("There should be an option")
                    .clone()
                } else {
                    description = [
                        format!(
                            "{} leaps above the defenders and snags the offensive rebound.",
                            attack_rebounder.info.shortened_name(),
                        ),
                        format!(
                            "{} outmuscles the competition to grab the offensive rebound.",
                            attack_rebounder.info.shortened_name(),
                        ),
                        format!(
                            "{} beats everyone to the ball, securing the offensive rebound.",
                            attack_rebounder.info.shortened_name(),
                        ),
                        format!(
                            "{} extends high and grabs the ball over the defenders for an offensive rebound.",
                            attack_rebounder.info.shortened_name(),
                        ),
                        format!(
                            "{} crashes the boards and comes down with the offensive rebound.",
                            attack_rebounder.info.shortened_name(),
                        ),
                    ].choose(rng)
                    .expect("There should be an option")
                    .clone()
                }
                ActionOutput {
                    possession: input.possession,
                    situation: ActionSituation::AfterOffensiveRebound,
                    description,
                    attackers: vec![attack_rebounder_idx],
                    attack_stats_update: Some(attack_stats_update),
                    start_at: input.end_at,
                    end_at: input.end_at.plus(1 + rng.gen_range(0..=1)),
                    home_score: input.home_score,
                    away_score: input.away_score,
                    ..Default::default()
                }
            }

            x if x > 0 && attack_result >= MIN_REBOUND_VALUE => {
                let mut attack_stats_update: GameStatsMap = HashMap::new();
                let mut rebounder_update = GameStats::default();
                rebounder_update.offensive_rebounds = 1;
                rebounder_update.extra_tiredness = TirednessCost::LOW;
                attack_stats_update.insert(attack_rebounder.id, rebounder_update);
                let description = [
                    format!(
                        "The ball got to {} who can restart the offensive action.",
                        attack_rebounder.info.shortened_name(),
                    ),
                    format!(
                        "{} secures the offensive rebound and looks to reset the play.",
                        attack_rebounder.info.shortened_name(),
                    ),
                    format!(
                        "{} snags the rebound and reset the offense.",
                        attack_rebounder.info.shortened_name(),
                    ),
                    format!(
                        "{} pulls down the board and surveys the floor for the next move.",
                        attack_rebounder.info.shortened_name(),
                    ),
                ]
                .choose(rng)
                .expect("There should be an option")
                .clone();

                ActionOutput {
                    possession: input.possession,
                    situation: ActionSituation::AfterLongOffensiveRebound,
                    description,
                    attackers: vec![attack_rebounder_idx],
                    attack_stats_update: Some(attack_stats_update),
                    start_at: input.end_at,
                    end_at: input.end_at.plus(2 + rng.gen_range(0..=3)),
                    home_score: input.home_score,
                    away_score: input.away_score,
                    ..Default::default()
                }
            }
            x if x < 0 && defence_result >= MIN_REBOUND_VALUE => {
                let mut defence_stats_update: GameStatsMap = HashMap::new();
                let mut rebounder_update = GameStats::default();
                rebounder_update.defensive_rebounds = 1;
                rebounder_update.extra_tiredness = TirednessCost::LOW;
                defence_stats_update.insert(defence_rebounder.id, rebounder_update);

                ActionOutput {
                    possession: !input.possession,
                    situation: ActionSituation::AfterDefensiveRebound,
                    description: [
                        format!(
                            "{} jumps high and gets the defensive rebound.",
                            defence_rebounder.info.shortened_name(),
                        ),
                        format!(
                            "{} reaches up to snare the ball, grabbing the defensive rebound.",
                            defence_rebounder.info.shortened_name(),
                        ),
                        format!(
                            "{} outmuscles the offense and secures the defensive board.",
                            defence_rebounder.info.shortened_name(),
                        ),
                        format!(
                            "{} claims the rebound, boxing out the attacker and controlling the ball.",
                            defence_rebounder.info.shortened_name(),
                        ),
                        format!(
                            "{} uses great positioning to grab the defensive rebound and take control.",
                            defence_rebounder.info.shortened_name(),
                        ),
                    ] .choose(rng)
                    .expect("There should be an option")
                    .clone(),
                    defense_stats_update: Some(defence_stats_update),
                    start_at: input.end_at,
                    end_at: input.end_at.plus(4 + rng.gen_range(0..=3)),
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
                ].choose(rng)
                .expect("There should be an option")
                .to_string(),
                start_at: input.end_at,
                end_at: input.end_at.plus(5 + rng.gen_range(0..=4)),
                home_score: input.home_score,
                away_score: input.away_score,
                ..Default::default()
            },
        };
        Some(result)
    }
}
