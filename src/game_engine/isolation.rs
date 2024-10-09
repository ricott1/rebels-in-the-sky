use super::{
    action::{ActionOutput, ActionSituation, Advantage, EngineAction},
    constants::*,
    game::Game,
    types::{GameStats, Possession},
};
use crate::world::{
    constants::{MoraleModifier, TirednessCost},
    skill::GameSkill,
};
use rand::Rng;
use rand_chacha::ChaCha8Rng;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct Isolation;

impl EngineAction for Isolation {
    fn execute(input: &ActionOutput, game: &Game, rng: &mut ChaCha8Rng) -> Option<ActionOutput> {
        let attacking_players = game.attacking_players();
        let defending_players = game.defending_players();

        let mut weights = [4, 5, 4, 3, 1];
        for (idx, player) in attacking_players.iter().enumerate() {
            if player.is_knocked_out() {
                weights[idx] = 0
            }
        }

        if weights.iter().sum::<u8>() == 0 {
            let name = if game.possession == Possession::Home {
                game.home_team_in_game.name.clone()
            } else {
                game.away_team_in_game.name.clone()
            };
            return Some(ActionOutput {
                situation: ActionSituation::Turnover,
                possession: !input.possession,
                description: format!(
                    "Oh no! The whole team is wasted! {name} just turned the ball over like that.",
                ),
                start_at: input.end_at,
                end_at: input.end_at.plus(4),
                home_score: input.home_score,
                away_score: input.away_score,
                ..Default::default()
            });
        }

        let iso_idx = Self::sample(rng, weights)?;

        let iso = attacking_players[iso_idx];
        let defender = defending_players[iso_idx];

        let timer_increase = 2 + rng.gen_range(0..=3);

        let mut attack_stats_update = HashMap::new();
        let mut iso_update = GameStats::default();
        iso_update.extra_tiredness = TirednessCost::MEDIUM;

        let mut defense_stats_update = HashMap::new();
        let mut defender_update = GameStats::default();
        defender_update.extra_tiredness = TirednessCost::MEDIUM;

        let atk_result =
            iso.roll(rng) + iso.technical.ball_handling.value() + iso.athletics.quickness.value();

        let def_result = defender.roll(rng)
            + defender.defense.perimeter_defense.value()
            + defender.athletics.quickness.value();

        let mut result = match atk_result as i16 - def_result as i16 {
            x if x > ADV_ATTACK_LIMIT => ActionOutput {
                possession: input.possession,
                advantage: Advantage::Attack,
                attackers: vec![iso_idx],
                defenders: vec![iso_idx],
                situation: ActionSituation::CloseShot,
                description: format!(
                    "{} breaks {}'s ankles and is now at the basket.",
                    iso.info.shortened_name(),
                    defender.info.shortened_name()
                ),
                start_at: input.end_at,
                end_at: input.end_at.plus(timer_increase),
                home_score: input.home_score,
                away_score: input.away_score,
                ..Default::default()
            },
            x if x > ADV_NEUTRAL_LIMIT => ActionOutput {
                possession: input.possession,
                advantage: Advantage::Neutral,
                attackers: vec![iso_idx],
                defenders: vec![iso_idx], //got the switch
                situation: ActionSituation::CloseShot,
                description: format!(
                    "{} gets through {} and gathers the ball to shoot.",
                    iso.info.shortened_name(),
                    defender.info.shortened_name(),
                ),
                start_at: input.end_at,
                end_at: input.end_at.plus(timer_increase),
                home_score: input.home_score,
                away_score: input.away_score,
                ..Default::default()
            },
            x if x > ADV_DEFENSE_LIMIT => ActionOutput {
                possession: input.possession,
                advantage: Advantage::Defense,
                attackers: vec![iso_idx],
                defenders: vec![iso_idx], //no switch
                situation: ActionSituation::MediumShot,
                description: format!(
                    "{} tries to dribble past {} but {} is all over him.",
                    iso.info.shortened_name(),
                    defender.info.shortened_name(),
                    defender.info.shortened_name()
                ),
                start_at: input.end_at,
                end_at: input.end_at.plus(timer_increase),
                home_score: input.home_score,
                away_score: input.away_score,
                ..Default::default()
            },
            _ => {
                iso_update.turnovers = 1;
                iso_update.extra_morale += MoraleModifier::MEDIUM_MALUS;
                defender_update.steals = 1;
                defender_update.extra_morale += MoraleModifier::MEDIUM_BONUS;

                ActionOutput {
                    situation: ActionSituation::Turnover,
                    possession: !input.possession,
                    description: format!(
                        "{} tries to dribble past {} but {} steals the ball. Terrible choice.",
                        iso.info.shortened_name(),
                        defender.info.shortened_name(),
                        defender.info.shortened_name()
                    ),
                    start_at: input.end_at,
                    end_at: input.end_at.plus(2),
                    home_score: input.home_score,
                    away_score: input.away_score,
                    ..Default::default()
                }
            }
        };
        attack_stats_update.insert(iso.id, iso_update);
        defense_stats_update.insert(defender.id, defender_update);
        result.attack_stats_update = Some(attack_stats_update);
        result.defense_stats_update = Some(defense_stats_update);
        Some(result)
    }
}
