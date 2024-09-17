use super::{
    action::{ActionOutput, ActionSituation, Advantage, EngineAction},
    constants::*,
    game::Game,
    types::GameStats,
};
use crate::world::{
    constants::{MoraleModifier, TirednessCost},
    player::Player,
    skill::GameSkill,
};
use rand::Rng;
use rand_chacha::ChaCha8Rng;
use rand_distr::{Distribution, WeightedIndex};
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct Post;

impl EngineAction for Post {
    fn execute(input: &ActionOutput, game: &Game, rng: &mut ChaCha8Rng) -> Option<ActionOutput> {
        let attacking_players = game.attacking_players();
        let defending_players = game.defending_players();

        let post_idx = match input.attackers.len() {
            0 => Self::sample(rng, [0, 0, 1, 2, 3])?,
            _ => input.attackers[0],
        };

        let poster = attacking_players[post_idx];
        let defender = defending_players[post_idx];

        let timer_increase = 4 + rng.gen_range(0..=5);

        let mut attack_stats_update = HashMap::new();
        let mut post_update = GameStats::default();
        post_update.extra_tiredness = TirednessCost::HIGH;

        let mut defense_stats_update = HashMap::new();
        let mut defender_update = GameStats::default();
        defender_update.extra_tiredness = TirednessCost::MEDIUM;

        let atk_result = poster.roll(rng)
            + poster.technical.post_moves.value()
            + poster.athletics.strength.value();

        let def_result = defender.roll(rng)
            + defender.defense.interior_defense.value()
            + defender.athletics.strength.value();

        let mut result = match atk_result as i16 - def_result as i16 {
            x if x > ADV_ATTACK_LIMIT => ActionOutput {
                possession: input.possession,
                advantage: Advantage::Attack,
                attackers: vec![post_idx],
                defenders: vec![post_idx],
                situation: ActionSituation::CloseShot,
                description: format!(
                    "{} worked {}'s perfectly and got to the basket.",
                    poster.info.last_name, defender.info.last_name
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
                attackers: vec![post_idx],
                defenders: vec![post_idx],
                situation: ActionSituation::CloseShot,
                description: format!(
                    "{} bumps on {} and gathers the ball to shoot.",
                    poster.info.last_name, defender.info.last_name,
                ),
                start_at: input.end_at,
                end_at: input.end_at.plus(timer_increase),
                home_score: input.home_score,
                away_score: input.away_score,
                ..Default::default()
            },
            x if x > ADV_DEFENSE_LIMIT => {
                if poster.mental.vision as i16 + x > ADV_NEUTRAL_LIMIT {
                    let mut weights = [3, 3, 2, 2, 1];
                    weights[post_idx] = 0;
                    let target_idx = WeightedIndex::new(&weights).ok()?.sample(rng);
                    let target: &Player = attacking_players[target_idx];
                    ActionOutput {
                        possession: input.possession,
                        advantage: Advantage::Neutral,
                        attackers: vec![target_idx],
                        defenders: vec![],
                        situation: ActionSituation::BallInMidcourt,
                        description: format!(
                            "{} is struggling from the post due to {}'s defense. The ball is passed to {} to reset.",
                            poster.info.last_name, defender.info.last_name, target.info.last_name
                        ),
                        start_at: input.end_at,
                        end_at: input.end_at.plus(timer_increase/2),
                        home_score: input.home_score,
                        away_score: input.away_score,
                        ..Default::default()
                    }
                } else {
                    ActionOutput {
                        possession: input.possession,
                        advantage: Advantage::Defense,
                        attackers: vec![post_idx],
                        defenders: vec![post_idx],
                        situation: ActionSituation::MediumShot,
                        description: format!(
                        "{} tries to make the post moves work against {} but {} is all over him.",
                        poster.info.last_name, defender.info.last_name, defender.info.last_name
                    ),
                        start_at: input.end_at,
                        end_at: input.end_at.plus(timer_increase),
                        home_score: input.home_score,
                        away_score: input.away_score,
                        ..Default::default()
                    }
                }
            }
            _ => {
                post_update.turnovers = 1;
                post_update.extra_morale += MoraleModifier::SMALL_MALUS;
                defender_update.steals = 1;
                defender_update.extra_morale += MoraleModifier::MEDIUM_BONUS;

                ActionOutput {
                    situation: ActionSituation::Turnover,
                    possession: !input.possession,
                    description: format!(
                        "{} steals the ball from {} on the post.",
                        defender.info.last_name, poster.info.last_name,
                    ),
                    start_at: input.end_at,
                    end_at: input.end_at.plus(3),
                    home_score: input.home_score,
                    away_score: input.away_score,
                    ..Default::default()
                }
            }
        };
        attack_stats_update.insert(poster.id, post_update);
        defense_stats_update.insert(defender.id, defender_update);
        result.attack_stats_update = Some(attack_stats_update);
        result.defense_stats_update = Some(defense_stats_update);
        Some(result)
    }
}
