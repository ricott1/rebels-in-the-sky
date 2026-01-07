use super::{action::*, constants::*, game::Game, types::*};
use crate::core::{
    constants::{MoraleModifier, TirednessCost},
    skill::GameSkill,
    Pronoun,
};
use rand::Rng;
use rand_chacha::ChaCha8Rng;
use std::collections::HashMap;

pub(crate) fn execute(
    input: &ActionOutput,
    game: &Game,
    action_rng: &mut ChaCha8Rng,
    _description_rng: &mut ChaCha8Rng,
) -> ActionOutput {
    let attacking_players_array = game.attacking_players_array();
    let defending_players_array = game.defending_players_array();

    assert!(input.attackers.len() == 1);
    let play_idx = input.attackers[0];

    let playmaker = attacking_players_array[play_idx];
    let playmaker_defender = defending_players_array[play_idx];

    let mut attack_stats_update: GameStatsMap = HashMap::new();
    let mut defense_stats_update: GameStatsMap = HashMap::new();

    let timer_increase = 3 + action_rng.random_range(0..=2);

    let mut result = ActionOutput {
        possession: input.possession,
        start_at: input.end_at,
        end_at: input.end_at.plus(timer_increase),
        home_score: input.home_score,
        away_score: input.away_score,
        ..Default::default()
    };

    // Playmaker plays alone
    let atk_result = playmaker.roll(action_rng)
        + playmaker.athletics.quickness.game_value()
        + (0.5 * playmaker.technical.ball_handling + 0.5 * playmaker.mental.aggression)
            .game_value()
        + game
            .attacking_team()
            .tactic
            .attack_roll_bonus(&Action::Fastbreak);

    let def_result = playmaker_defender.roll(action_rng)
        + playmaker_defender.athletics.quickness.game_value()
        + (0.5 * playmaker_defender.mental.aggression
            + 0.25 * playmaker_defender.mental.intuition
            + 0.25 * playmaker_defender.defense.steal)
            .game_value()
        + game
            .defending_team()
            .tactic
            .defense_roll_bonus(&Action::Fastbreak);

    let mut playmaker_update = GameStats {
        extra_tiredness: TirednessCost::MEDIUM,
        ..Default::default()
    };

    match atk_result - def_result {
        x if x >= ADV_ATTACK_LIMIT => {
            result.advantage = Advantage::Attack;
            result.attackers = vec![play_idx];
            result.situation = ActionSituation::CloseShot;
            result.description = format!(
                "{} quickly brings the ball to the other side: {} {} all alone at the basket.",
                playmaker.info.short_name(),
                playmaker.info.pronouns.as_subject().to_lowercase(),
                playmaker.info.pronouns.to_be()
            );
        }
        x if x >= ADV_NEUTRAL_LIMIT => {
            result.advantage = Advantage::Neutral;
            result.attackers = vec![play_idx];
            result.situation = ActionSituation::CloseShot;
            result.description = format!(
                "{} quickly brings the ball to the other side.",
                playmaker.info.short_name(),
            );
        }
        x if x > ADV_DEFENSE_LIMIT => {
            // Playmaker could pass to target to avoid disadvantage
            let playmaker_defender_update = GameStats {
                extra_tiredness: TirednessCost::MEDIUM,
                ..Default::default()
            };
            defense_stats_update.insert(playmaker_defender.id, playmaker_defender_update);
            if playmaker.mental.intuition.game_value() + x > ADV_NEUTRAL_LIMIT {
                let target_idx = {
                    let mut weights = [3, 3, 2, 2, 1];
                    weights[play_idx] = 0;
                    sample_player_index(action_rng, weights, attacking_players_array)
                };

                if let Some(idx) = target_idx {
                    let target = attacking_players_array[idx];
                    result.advantage = Advantage::Neutral;
                    result.attackers = vec![idx];
                    result.defenders = vec![play_idx];
                    result.assist_from = Some(play_idx);
                    result.situation = ActionSituation::CloseShot;
                    result.description = format!(
                        "{} brings the ball to the other side, but {} catches up and {} decide{} to pass to {}.",
                        playmaker.info.short_name(),
                        playmaker_defender.info.short_name(),
                        playmaker.info.pronouns.as_subject().to_lowercase(),
                        if playmaker.info.pronouns == Pronoun::They {""} else {"s"},
                        target.info.short_name()

                    );
                }
            }

            result.advantage = Advantage::Defense;
            result.attackers = vec![play_idx];
            result.defenders = vec![play_idx];
            result.situation = ActionSituation::MediumShot;
            result.description = format!(
                "{} tries to bring the ball to the other side as fast as possible, but {} catches up.",
                playmaker.info.short_name(),
                playmaker_defender.info.short_name()
            );
        }
        _ => {
            playmaker_update.turnovers = 1;
            playmaker_update.extra_morale += MoraleModifier::MEDIUM_MALUS;

            let playmaker_defender_update = GameStats {
                extra_tiredness: TirednessCost::MEDIUM,
                extra_morale: MoraleModifier::SMALL_BONUS,
                ..Default::default()
            };
            defense_stats_update.insert(playmaker_defender.id, playmaker_defender_update);

            result.advantage = Advantage::Neutral;
            result.situation = ActionSituation::Turnover;
            result.possession = !result.possession;
            result.description = format!(
                "{} manages to fumble the ball under {}'s pressure while trying a fastbreak.",
                playmaker.info.short_name(),
                playmaker_defender.info.short_name()
            );
        }
    };
    attack_stats_update.insert(playmaker.id, playmaker_update);

    result.attack_stats_update = Some(attack_stats_update);
    result.defense_stats_update = Some(defense_stats_update);
    result
}
