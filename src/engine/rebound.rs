use super::{
    action::{ActionOutput, ActionSituation, EngineAction},
    game::Game,
    types::GameStats,
};
use crate::engine::{
    action::Advantage,
    constants::{TirednessCost, ADV_ATTACK_LIMIT},
    types::GameStatsMap,
};
use rand::Rng;
use rand_chacha::ChaCha8Rng;
use std::{
    cmp::{max, min},
    collections::HashMap,
};

const MIN_REBOUND_VALUE: u16 = 20;

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
                (attack_rebounds[idx] as f32 * (20.0 + idx as f32) / 20.0) as u16;
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
                (defense_rebounds[idx] as f32 * (20.0 + idx as f32) / 20.0) as u16;
            //add random roll
            defense_rebounds[idx] += defending_players[idx].roll(rng) as u16;
        }

        let attack_result = *attack_rebounds.iter().max().unwrap();
        let defence_result = *defense_rebounds.iter().max().unwrap();

        let attack_rebounder_idx = attack_rebounds.iter().position(|&r| r == attack_result)?;
        let defence_rebounder_idx = defense_rebounds.iter().position(|&r| r == defence_result)?;

        let attack_rebounder = attacking_players[attack_rebounder_idx];
        let defence_rebounder = defending_players[defence_rebounder_idx];

        //FIXME: add more random situations
        let result = match attack_result as i16 - defence_result as i16 {
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
                    description = format!(
                        "{} catches {} own rebound.",
                        attack_rebounder.info.last_name,
                        attack_rebounder.info.pronouns.as_possessive()
                    )
                } else {
                    description = format!(
                        "{} jumps high and gets the offensive rebound.",
                        attack_rebounder.info.last_name,
                    )
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
                let description: String;
                description = format!(
                    "The ball got to {} that can restart the offensive action.",
                    attack_rebounder.info.last_name,
                );
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
                    description: format!(
                        "{} jumps high and gets the defensive rebound.",
                        defence_rebounder.info.last_name,
                    ),
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
                situation: ActionSituation::BallInBackcourt,
                description: "Nobody got the rebound, ball goes to defence.".into(),
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
