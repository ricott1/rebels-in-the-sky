use super::{
    action::{ActionOutput, ActionSituation, EngineAction},
    game::Game,
    types::GameStats,
    utils::roll,
};
use crate::engine::{action::Advantage, types::GameStatsMap};
use rand::Rng;
use rand_chacha::ChaCha8Rng;
use std::{
    cmp::{max, min},
    collections::HashMap,
};

#[derive(Debug, Default)]
pub struct Rebound;

impl EngineAction for Rebound {
    fn execute(input: &ActionOutput, game: &Game, rng: &mut ChaCha8Rng) -> Option<ActionOutput> {
        let attacking_players = game.attacking_players();
        let defending_players = game.defending_players();
        let attacking_stats = game.attacking_stats();
        let defending_stats = game.defending_stats();

        // let rebound = |player: &Player| player.technical.rebounding;
        let mut attack_rebounds: Vec<u16> = attacking_players
            .iter()
            .map(|&p| p.technical.rebounding as u16)
            .collect();
        let mut defense_rebounds: Vec<u16> = defending_players
            .iter()
            .map(|&p| p.technical.rebounding as u16)
            .collect();

        // apply reduction for shooter rebounding
        assert!(input.attackers.len() == 1);
        attack_rebounds[input.attackers[0]] = attack_rebounds[input.attackers[0]] * 2 / 3;

        for idx in 0..attacking_players.len() {
            // apply bonus based on position
            attack_rebounds[idx] = attack_rebounds[idx] * (10 + idx as u16) / 10;
            //add random roll
            let atk_stats = attacking_stats.get(&attacking_players[idx].id)?;
            match input.advantage {
                Advantage::Attack => {
                    attack_rebounds[idx] += max(
                        roll(rng, atk_stats.tiredness) as u16,
                        roll(rng, atk_stats.tiredness) as u16,
                    );
                }
                Advantage::Neutral => {
                    attack_rebounds[idx] += roll(rng, atk_stats.tiredness) as u16;
                }
                Advantage::Defense => {
                    attack_rebounds[idx] += min(
                        roll(rng, atk_stats.tiredness) as u16,
                        roll(rng, atk_stats.tiredness) as u16,
                    );
                }
            }
        }
        for idx in 0..defending_players.len() {
            // apply reduction for defender rebounding.
            if input.defenders.contains(&idx) {
                defense_rebounds[idx] = defense_rebounds[idx] * 3 / 4;
            }

            defense_rebounds[idx] = defense_rebounds[idx] * (10 + idx as u16) / 10;
            let def_stats = defending_stats.get(&defending_players[idx].id)?;
            defense_rebounds[idx] += roll(rng, def_stats.tiredness) as u16;
        }

        let attack_result = *attack_rebounds.iter().max().unwrap();
        let defence_result = *defense_rebounds.iter().max().unwrap();

        let attack_rebounder_idx = attack_rebounds.iter().position(|&r| r == attack_result)?;
        let defence_rebounder_idx = defense_rebounds.iter().position(|&r| r == defence_result)?;

        let attack_rebounder = attacking_players[attack_rebounder_idx];
        let defence_rebounder = defending_players[defence_rebounder_idx];

        //FIXME: add more random situations
        let result = match attack_result as i16 - defence_result as i16 {
            x if x > 0 => {
                let mut attack_stats_update: GameStatsMap = HashMap::new();
                let mut rebounder_update = GameStats::default();
                rebounder_update.offensive_rebounds = 1;
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
                    end_at: input.end_at.plus(1 + rng.gen_range(0..=2)),
                    home_score: input.home_score,
                    away_score: input.away_score,
                    ..Default::default()
                }
            }
            _ => {
                let mut defence_stats_update: GameStatsMap = HashMap::new();
                let mut rebounder_update = GameStats::default();
                rebounder_update.defensive_rebounds = 1;
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
                    end_at: input.end_at.plus(4 + rng.gen_range(0..=4)),
                    home_score: input.home_score,
                    away_score: input.away_score,
                    ..Default::default()
                }
            }
        };
        Some(result)
    }
}
