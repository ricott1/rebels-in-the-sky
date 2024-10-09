use super::{
    action::{ActionOutput, EngineAction},
    game::Game,
    types::GameStats,
};
use crate::world::{
    constants::{MoraleModifier, TirednessCost},
    player::Trait,
    skill::GameSkill,
};
use rand::Rng;
use rand_chacha::ChaCha8Rng;
use std::collections::HashMap;

#[derive(Debug)]
pub struct Brawl;

impl EngineAction for Brawl {
    fn execute(input: &ActionOutput, game: &Game, rng: &mut ChaCha8Rng) -> Option<ActionOutput> {
        let (attacking_players, defending_players) =
            (game.attacking_players(), game.defending_players());
        let weights = attacking_players
            .iter()
            .map(|p| {
                if p.is_knocked_out() {
                    0
                } else if p.special_trait == Some(Trait::Killer) {
                    p.mental.aggression.value() * 2
                } else {
                    p.mental.aggression.value()
                }
            })
            .collect::<Vec<u8>>()
            .try_into()
            .ok()?;

        // This will return None if all players are knocked out
        let attacker_idx = Self::sample(rng, weights)?;

        let weights = defending_players
            .iter()
            .map(|p| {
                if p.is_knocked_out() {
                    0
                } else if p.special_trait == Some(Trait::Killer) {
                    p.mental.aggression.value() * 2
                } else {
                    p.mental.aggression.value()
                }
            })
            .collect::<Vec<u8>>()
            .try_into()
            .ok()?;

        // This will return None if all players are knocked out
        let defender_idx = Self::sample(rng, weights)?;

        let attacker = attacking_players[attacker_idx];
        let defender = defending_players[defender_idx];

        let mut attack_stats_update = HashMap::new();
        let mut attacker_update = GameStats::default();
        attacker_update.extra_tiredness = TirednessCost::HIGH;

        let mut defense_stats_update = HashMap::new();
        let mut defender_update = GameStats::default();
        defender_update.extra_tiredness = TirednessCost::MEDIUM;

        let mut atk_result = attacker.roll(rng)
            + attacker.athletics.strength.value() / 2
            + attacker.mental.aggression.value() / 2
            + attacker.offense.brawl.value();

        if attacker.special_trait == Some(Trait::Killer) {
            atk_result += attacker.reputation.value();
        }

        let mut def_result = defender.roll(rng)
            + defender.athletics.strength.value() / 2
            + defender.mental.aggression.value() / 2
            + defender.offense.brawl.value();

        if defender.special_trait == Some(Trait::Killer) {
            def_result += defender.reputation.value();
        }

        let description = match atk_result as i16 - def_result as i16 {
            x if x > 0 => {
                defender_update.extra_morale += MoraleModifier::SEVERE_MALUS;
                attacker_update.extra_morale += MoraleModifier::SEVERE_BONUS;

                if attacker.has_hook() {
                    defender_update.extra_tiredness += TirednessCost::CRITICAL;
                    format!(
                        "A brawl between {} and {}! {} seems to have gotten the upper hand.",
                        defender.info.shortened_name(),
                        attacker.info.shortened_name(),
                        attacker.info.shortened_name()
                    )
                } else {
                    defender_update.extra_tiredness += TirednessCost::SEVERE;
                    format!(
                        "A brawl between {} and {}! {} got {} good with the hook! That'll be an ugly scar.",
                        defender.info.shortened_name(), attacker.info.shortened_name(), attacker.info.shortened_name(), defender.info.pronouns.as_object()
                    )
                }
            }
            x if x == 0 => {
                attacker_update.extra_tiredness += TirednessCost::HIGH;
                defender_update.extra_tiredness += TirednessCost::HIGH;
                defender_update.extra_morale += MoraleModifier::SMALL_MALUS;
                attacker_update.extra_morale += MoraleModifier::SMALL_MALUS;
                match rng.gen_range(0..=1) {
                    0 => format!(
                        "A brawl between {} and {}! They both got some damage.",
                        attacker.info.shortened_name(),
                        defender.info.shortened_name()
                    ),
                    _ => format!(
                        "A brawl between {} and {}! An even match.",
                        defender.info.shortened_name(),
                        attacker.info.shortened_name()
                    ),
                }
            }
            _ => {
                defender_update.extra_morale += MoraleModifier::SEVERE_BONUS;
                attacker_update.extra_morale += MoraleModifier::SEVERE_MALUS;
                if defender.has_hook() {
                    attacker_update.extra_tiredness += TirednessCost::CRITICAL;
                    format!(
                        "A brawl between {} and {}! {} seems to have gotten the upper hand.",
                        attacker.info.shortened_name(),
                        defender.info.shortened_name(),
                        defender.info.shortened_name()
                    )
                } else {
                    attacker_update.extra_tiredness += TirednessCost::SEVERE;
                    format!(
                        "A brawl between {} and {}! {} got {} good with the hook! That'll be an ugly scar.",
                        attacker.info.shortened_name(), defender.info.shortened_name(), defender.info.shortened_name(), attacker.info.pronouns.as_object()
                    )
                }
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
