use super::{
    action::{ActionOutput, ActionSituation, Advantage, EngineAction},
    constants::{TirednessCost, ADV_ATTACK_LIMIT, ADV_DEFENSE_LIMIT, ADV_NEUTRAL_LIMIT},
    game::Game,
    types::GameStats,
    utils::roll,
};
use crate::world::skill::GameSkill;
use rand::Rng;
use rand_chacha::ChaCha8Rng;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct Isolation;

impl EngineAction for Isolation {
    fn execute(input: &ActionOutput, game: &Game, rng: &mut ChaCha8Rng) -> Option<ActionOutput> {
        let attacking_players = game.attacking_players();
        let defending_players = game.defending_players();
        let attacking_stats = game.attacking_stats();
        let defending_stats = game.defending_stats();

        let iso_idx = Self::sample(rng, [2, 3, 2, 1, 0])?;

        let iso = attacking_players[iso_idx];
        let iso_stats = attacking_stats.get(&iso.id)?;
        let defender = defending_players[iso_idx];
        let defender_stats = defending_stats.get(&defender.id)?;

        let timer_increase = 2 + rng.gen_range(0..=3);

        let mut attack_stats_update = HashMap::new();
        let mut iso_update = GameStats::default();
        iso_update.add_tiredness(TirednessCost::MEDIUM, iso.athleticism.stamina);

        let mut defense_stats_update = HashMap::new();
        let mut defender_update = GameStats::default();
        defender_update.add_tiredness(TirednessCost::MEDIUM, defender.athleticism.stamina);

        let atk_result = roll(rng, iso_stats.tiredness)
            + iso.technical.ball_handling.value()
            + iso.athleticism.quickness.value();

        let def_result = roll(rng, defender_stats.tiredness)
            + defender.defense.perimeter_defense.value()
            + defender.athleticism.quickness.value();

        let mut result = match atk_result as i16 - def_result as i16 {
            x if x > ADV_ATTACK_LIMIT => ActionOutput {
                possession: input.possession,
                advantage: Advantage::Attack,
                attackers: vec![iso_idx],
                defenders: vec![iso_idx],
                situation: ActionSituation::CloseShot,
                description: format!(
                    "{} breaks {}'s ankles and is now at the basket.",
                    iso.info.last_name, defender.info.last_name
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
                    iso.info.last_name, defender.info.last_name,
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
                    iso.info.last_name, defender.info.last_name, defender.info.last_name
                ),
                start_at: input.end_at,
                end_at: input.end_at.plus(timer_increase),
                home_score: input.home_score,
                away_score: input.away_score,
                ..Default::default()
            },
            _ => {
                iso_update.turnovers = 1;
                defender_update.steals = 1;

                ActionOutput {
                    situation: ActionSituation::Turnover,
                    possession: !input.possession,
                    description: format!(
                        "{} tries to dribble past {} but {} steals the ball. Terrible choice.",
                        iso.info.last_name, defender.info.last_name, defender.info.last_name
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
