use super::{
    action::{ActionOutput, ActionSituation, Advantage, EngineAction},
    constants::{TirednessCost, ADV_ATTACK_LIMIT, ADV_DEFENSE_LIMIT, ADV_NEUTRAL_LIMIT},
    game::Game,
    types::{GameStats, GameStatsMap},
};
use crate::world::skill::GameSkill;
use rand::Rng;
use rand_chacha::ChaCha8Rng;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct PickAndRoll;

impl EngineAction for PickAndRoll {
    fn execute(input: &ActionOutput, game: &Game, rng: &mut ChaCha8Rng) -> Option<ActionOutput> {
        let attacking_players = game.attacking_players();
        let defending_players = game.defending_players();

        let play_idx = match input.attackers.len() {
            0 => Self::sample(rng, [6, 1, 2, 0, 0])?,
            _ => input.attackers[0],
        };

        let target_idx = match input.attackers.len() {
            0 | 1 => Self::sample(rng, [1, 2, 3, 3, 2])?,
            _ => input.attackers[1],
        };

        let playmaker = attacking_players[play_idx];
        let playmaker_defender = defending_players[play_idx];

        let target = attacking_players[target_idx];
        let target_defender = defending_players[target_idx];

        let mut attack_stats_update: GameStatsMap = HashMap::new();
        let mut playmaker_update = GameStats::default();
        playmaker_update.extra_tiredness = TirednessCost::MEDIUM;

        let mut defense_stats_update: GameStatsMap = HashMap::new();
        let mut playmaker_defender_update = GameStats::default();
        playmaker_defender_update.extra_tiredness = TirednessCost::MEDIUM;

        let mut target_defender_update = GameStats::default();
        target_defender_update.extra_tiredness = TirednessCost::MEDIUM;

        let timer_increase = 2 + rng.gen_range(0..=3);
        let mut result: ActionOutput;

        if play_idx == target_idx {
            let atk_result = playmaker.roll(rng)
                + playmaker.technical.ball_handling.value()
                + playmaker.athleticism.quickness.value()
                + target.mental.vision.value();

            let def_result = playmaker_defender.roll(rng)
                + playmaker_defender.defense.perimeter_defense.value()
                + playmaker_defender.mental.vision.value();

            result = match atk_result as i16 - def_result as i16 {
                x if x > ADV_ATTACK_LIMIT => ActionOutput {
                    possession: input.possession,
                    advantage: Advantage::Attack,
                    attackers: vec![play_idx],
                    defenders: vec![play_idx],
                    situation: ActionSituation::LongShot,
                    description: format!(
                        "{} uses the screen perfectly and is now open for the shot.",
                        playmaker.info.last_name
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
                    attackers: vec![play_idx],
                    defenders: vec![play_idx],
                    situation: ActionSituation::LongShot,
                    description: format!(
                        "They go for the pick'n'roll. {} goes through the screen and manages to get a bit of space to shot.",
                        playmaker.info.last_name,
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
                    attackers: vec![play_idx],
                    defenders: vec![play_idx],
                    situation: ActionSituation::LongShot,
                    description: format!(
                        "{} tries to use the screen but {} slides nicely to cover.",
                        playmaker.info.last_name, target_defender.info.last_name
                    ),
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
                        possession: !input.possession,
                        description: format!(
                            "{} tries to use the screen but {} snatches the ball from {} hands.",
                            playmaker.info.last_name, target_defender.info.last_name, playmaker.info.pronouns.as_possessive()
                        ),
                        start_at: input.end_at,
                end_at: input.end_at.plus(3),
                home_score: input.home_score,
                    away_score: input.away_score,
                        ..Default::default()
                    }
                }
            };
        } else {
            let atk_result = playmaker.roll(rng)
                + playmaker.technical.ball_handling.value()
                + playmaker.technical.passing.value()
                + target.mental.off_ball_movement.value();

            let def_result = playmaker_defender.roll(rng)
                + playmaker_defender.defense.perimeter_defense.value()
                + target_defender.athleticism.quickness.value();

            result = match atk_result as i16 - def_result as i16 {
            x if x > ADV_ATTACK_LIMIT => ActionOutput {
                possession: input.possession,
                advantage: Advantage::Attack,
                attackers: vec![target_idx],
                defenders: vec![play_idx], //got the switch
                situation: ActionSituation::CloseShot,
                description: format!(
                    "{} and {} execute the pick'n'roll perfectly! {} is now open for the shot.",
                    playmaker.info.last_name, target.info.last_name, target.info.last_name
                ),
                assist_from: Some(play_idx),
                start_at: input.end_at,
                        end_at: input.end_at.plus(timer_increase),
                        home_score: input.home_score,
                    away_score: input.away_score,
                ..Default::default()
            },
            x if x > 0 => ActionOutput {
                possession: input.possession,
                advantage: Advantage::Neutral,
                attackers: vec![target_idx],
                defenders: vec![play_idx], //got the switch
                situation: ActionSituation::CloseShot,
                description:format!(
                    "They go for the pick'n'roll, nice move. {} passes to {} and is now ready to shoot.",
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
                possession: input.possession,
                advantage: Advantage::Defense,
                attackers: vec![target_idx],
                defenders: vec![target_idx], //no switch
                situation: ActionSituation::MediumShot,
                description:format!(
                    "They go for the pick'n'roll. {} passes to {} but {} is all over him.",
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
                playmaker_defender_update.steals = 1;

                ActionOutput {
                    situation: ActionSituation::Turnover,
                    possession: !input.possession,
                    description:format!(
                        "They go for the pick'n'roll but the defender reads that perfectly. {} tries to pass to {} but {} blocks the pass.",
                        playmaker.info.last_name, target.info.last_name, playmaker_defender.info.last_name
                    ),
                    start_at: input.end_at,
                end_at: input.end_at.plus(2),
                home_score: input.home_score,
                    away_score: input.away_score,
                    ..Default::default()
                }
            }
        };
        }
        attack_stats_update.insert(playmaker.id, playmaker_update);
        defense_stats_update.insert(playmaker_defender.id, playmaker_defender_update);
        defense_stats_update.insert(target_defender.id, target_defender_update);
        result.attack_stats_update = Some(attack_stats_update);
        result.defense_stats_update = Some(defense_stats_update);
        Some(result)
    }
}
