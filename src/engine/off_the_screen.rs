use super::{
    action::{ActionOutput, ActionSituation, Advantage},
    constants::{TirednessCost, ADV_ATTACK_LIMIT, ADV_DEFENSE_LIMIT},
    game::Game,
    types::{GameStats, GameStatsMap},
    utils::roll,
};
use crate::world::{player::Player, skill::GameSkill};
use rand::Rng;
use rand_chacha::ChaCha8Rng;
use rand_distr::{Distribution, WeightedIndex};
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct OffTheScreen;

impl OffTheScreen {
    pub fn execute(
        &self,
        input: &ActionOutput,
        game: &Game,
        rng: &mut ChaCha8Rng,
    ) -> Option<ActionOutput> {
        let attacking_players = game.attacking_players();
        let defending_players = game.defending_players();
        let attacking_stats = game.attacking_stats();
        let defending_stats = game.defending_stats();

        let play_idx: usize;
        let target_idx: usize;
        match input.attackers.len() {
            0 => {
                play_idx = WeightedIndex::new(&[6, 1, 2, 0, 0]).ok()?.sample(rng);
                target_idx = WeightedIndex::new(&[1, 2, 3, 3, 2]).ok()?.sample(rng);
            }
            1 => {
                play_idx = input.attackers[0];
                let mut weights = [1, 2, 3, 3, 2];
                weights[play_idx] = 0;
                target_idx = WeightedIndex::new(&weights).ok()?.sample(rng);
            }
            _ => {
                play_idx = input.attackers[0];
                target_idx = input.attackers[1];
            }
        }

        let playmaker: &Player = attacking_players[play_idx];
        let playmaker_defender = defending_players[play_idx];
        let play_stats = attacking_stats.get(&playmaker.id)?;
        let defender_stats = defending_stats.get(&playmaker_defender.id)?;

        let target = attacking_players[target_idx];
        let target_defender = defending_players[target_idx];

        let mut attack_stats_update: GameStatsMap = HashMap::new();
        let mut playmaker_update = GameStats::default();
        playmaker_update.add_tiredness(TirednessCost::MEDIUM, playmaker.athleticism.stamina);

        let mut defense_stats_update: GameStatsMap = HashMap::new();
        let mut target_defender_update = GameStats::default();
        target_defender_update
            .add_tiredness(TirednessCost::MEDIUM, target_defender.athleticism.stamina);

        let timer_increase = 3 + rng.gen_range(0..=1);
        let mut result: ActionOutput;

        let atk_result = roll(rng, play_stats.tiredness)
            + playmaker.mental.vision.value()
            + playmaker.technical.passing.value()
            + target.mental.off_ball_movement.value();

        let def_result = roll(rng, defender_stats.tiredness)
            + target_defender.defense.perimeter_defense.value()
            + target_defender.athleticism.quickness.value();

        result = match atk_result as i16 - def_result as i16 {
            x if x > ADV_ATTACK_LIMIT => ActionOutput {
                possession: input.possession.clone(),
                advantage: Advantage::Attack,
                attackers: vec![target_idx],
                defenders: vec![target_idx],
                situation: ActionSituation::LongShot,
                description: format!(
                    "{} gets the pass from {} and is now open for the shot.",
                    target.info.last_name, playmaker.info.last_name,
                ),
                assist_from: Some(play_idx),
                start_at: input.end_at,
                end_at: input.end_at.plus(timer_increase),
                home_score: input.home_score,
                    away_score: input.away_score,
                ..Default::default()
            },
            x if x > 0 => ActionOutput {
                possession: input.possession.clone(),
                advantage: Advantage::Neutral,
                attackers: vec![target_idx],
                defenders: vec![target_idx],
                situation: ActionSituation::LongShot,
                description: format!(
                    "{} passes to {} after the screen.",
                    playmaker.info.last_name, target.info.last_name,
                ),
                assist_from: Some(play_idx),
                start_at: input.end_at,
                end_at: input.end_at.plus(timer_increase),
                home_score: input.home_score,
                    away_score: input.away_score,
                ..Default::default()
            },
            x if x > ADV_DEFENSE_LIMIT => ActionOutput {
                possession: input.possession.clone(),
                advantage: Advantage::Defense,
                attackers: vec![target_idx],
                defenders: vec![target_idx],
                situation: ActionSituation::MediumShot,
                description: format!(
                    "{} passes to {} who tried to get free using the screen, but {} is all over him.",
                    playmaker.info.last_name, target.info.last_name, target_defender.info.last_name
                ),
                assist_from: Some(play_idx),
                start_at: input.end_at,
                end_at: input.end_at.plus(timer_increase),
                home_score: input.home_score,
                    away_score: input.away_score,
                ..Default::default()
            },
            _ => {
                playmaker_update.turnovers = 1;
                target_defender_update.steals = 1;

                ActionOutput {
                    situation: ActionSituation::Turnover,
                    possession: !input.possession.clone(),
                    description:format!(
                        "{} tries to pass to {} off-the-screen but {} blocks the pass.",
                        playmaker.info.last_name, target.info.last_name, target_defender.info.last_name
                    ),
                    start_at: input.end_at,
                end_at: input.end_at.plus(2),
                home_score: input.home_score,
                    away_score: input.away_score,
                    ..Default::default()
                }
            }
        };

        attack_stats_update.insert(playmaker.id, playmaker_update);
        defense_stats_update.insert(target_defender.id, target_defender_update);
        result.attack_stats_update = Some(attack_stats_update);
        result.defense_stats_update = Some(defense_stats_update);
        Some(result)
    }
}
