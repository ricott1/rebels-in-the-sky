use super::{
    action::{ActionOutput, EngineAction},
    constants::TirednessCost,
    game::Game,
    types::GameStats,
    utils::roll,
};
use crate::world::skill::GameSkill;
use rand::Rng;
use rand_chacha::ChaCha8Rng;
use std::collections::HashMap;

#[derive(Debug)]
pub struct Brawl;

impl EngineAction for Brawl {
    fn execute(input: &ActionOutput, game: &Game, rng: &mut ChaCha8Rng) -> Option<ActionOutput> {
        let (attacking_players, defending_players) =
            (game.attacking_players(), game.defending_players());
        let (attacking_stats, defending_stats) = (game.attacking_stats(), game.defending_stats());
        let weights = attacking_players
            .iter()
            .map(|p| {
                let stats = attacking_stats.get(&p.id).unwrap();
                if stats.is_knocked_out() {
                    0
                } else {
                    1
                }
            })
            .collect::<Vec<u8>>()
            .try_into()
            .unwrap();

        // This will return None if all players are knocked out
        let attacker_idx = Self::sample(rng, weights)?;

        let weights = defending_players
            .iter()
            .map(|p| {
                let stats = defending_stats.get(&p.id).unwrap();
                if stats.is_knocked_out() {
                    0
                } else {
                    1
                }
            })
            .collect::<Vec<u8>>()
            .try_into()
            .unwrap();

        // This will return None if all players are knocked out
        let defender_idx = Self::sample(rng, weights)?;

        let attacker = attacking_players[attacker_idx];
        let defender = defending_players[defender_idx];

        let attacker_stats = attacking_stats.get(&attacker.id)?;
        let defender_stats = defending_stats.get(&defender.id)?;

        let mut attack_stats_update = HashMap::new();
        let mut attacker_update = GameStats::default();
        attacker_update.add_tiredness(TirednessCost::HIGH, attacker.athleticism.stamina);

        let mut defense_stats_update = HashMap::new();
        let mut defender_update = GameStats::default();
        defender_update.add_tiredness(TirednessCost::MEDIUM, defender.athleticism.stamina);

        let atk_result = roll(rng, attacker_stats.tiredness)
            + attacker.athleticism.strength.value()
            + attacker.mental.aggression.value();
        let def_result = roll(rng, defender_stats.tiredness)
            + defender.athleticism.strength.value()
            + defender.mental.aggression.value();

        let description = match atk_result as i16 - def_result as i16 {
            x if x > 0 => {
                defender_update
                    .add_tiredness(TirednessCost::CRITICAL, defender.athleticism.stamina);
                format!(
                    "A brawl between {} and {}! {} seems to have gotten the upper hand.",
                    attacker.info.last_name, defender.info.last_name, attacker.info.last_name
                )
            }
            x if x == 0 => {
                attacker_update.add_tiredness(TirednessCost::HIGH, attacker.athleticism.stamina);
                defender_update.add_tiredness(TirednessCost::HIGH, defender.athleticism.stamina);
                format!(
                    "A brawl between {} and {}! They both got some damage.",
                    attacker.info.last_name, defender.info.last_name
                )
            }
            _ => {
                attacker_update
                    .add_tiredness(TirednessCost::CRITICAL, attacker.athleticism.stamina);
                format!(
                    "A brawl between {} and {}! {} seems to have gotten the upper hand.",
                    attacker.info.last_name, defender.info.last_name, defender.info.last_name
                )
            }
        };

        let timer_increase = 5 + rng.gen_range(0..=5);

        let mut result = ActionOutput {
            possession: input.possession,
            advantage: input.advantage,
            attackers: input.attackers.clone(),
            defenders: input.defenders.clone(),
            situation: input.situation,
            description,
            start_at: input.end_at,
            end_at: input.end_at.plus(timer_increase),
            home_score: input.home_score,
            away_score: input.away_score,
            ..Default::default()
        };

        attack_stats_update.insert(attacker.id, attacker_update);
        defense_stats_update.insert(defender.id, defender_update);

        result.attack_stats_update = Some(attack_stats_update);
        result.defense_stats_update = Some(defense_stats_update);

        Some(result)
    }
}
