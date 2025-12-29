use super::{action::*, constants::*, game::Game, types::*};
use crate::core::{
    constants::{MoraleModifier, TirednessCost},
    skill::GameSkill,
};
use rand::{seq::IndexedRandom, Rng};
use rand_chacha::ChaCha8Rng;
use std::collections::HashMap;

pub(crate) fn execute(
    input: &ActionOutput,
    game: &Game,
    action_rng: &mut ChaCha8Rng,
    description_rng: &mut ChaCha8Rng,
) -> ActionOutput {
    let attacking_players_array = game.attacking_players_array();
    let defending_players_array = game.defending_players_array();

    let weights = [4, 5, 4, 3, 1];
    let iso_idx = if let Some(idx) =
        sample_player_index(action_rng, weights, attacking_players_array)
    {
        idx
    } else {
        return ActionOutput {
                situation: ActionSituation::Turnover,
                possession: !input.possession,
                description: format!(
                    "Oh no! No player of {} is left standing, they just turned the ball over like that!",
                    game.attacking_team().name,
                ),
                start_at: input.end_at,
                end_at: input.end_at.plus(4 + action_rng.random_range(0..=3)),
                home_score: input.home_score,
                away_score: input.away_score,
                ..Default::default()
            };
    };

    let iso = attacking_players_array[iso_idx];
    let defender = defending_players_array[iso_idx];

    let timer_increase = 2 + action_rng.random_range(0..=3);

    let mut attack_stats_update = HashMap::new();
    let mut iso_update = GameStats {
        extra_tiredness: TirednessCost::HIGH,
        ..Default::default()
    };

    let mut defense_stats_update = HashMap::new();
    let mut defender_update = GameStats {
        extra_tiredness: TirednessCost::MEDIUM,
        ..Default::default()
    };

    let atk_result = iso.roll(action_rng)
        + iso.technical.ball_handling.game_value()
        + (0.75 * iso.athletics.quickness + 0.25 * iso.mental.aggression).game_value()
        + game
            .attacking_team()
            .tactic
            .attack_roll_bonus(&Action::Isolation);

    let def_result = defender.roll(action_rng)
        + defender.defense.perimeter_defense.game_value()
        + (0.75 * defender.athletics.quickness + 0.25 * defender.defense.steal).game_value()
        + game
            .defending_team()
            .tactic
            .defense_roll_bonus(&Action::Isolation);

    let mut result = match atk_result  - def_result  {
            x if x >= ADV_ATTACK_LIMIT => ActionOutput {
                possession: input.possession,
                advantage: Advantage::Attack,
                attackers: vec![iso_idx],
                defenders: vec![iso_idx],
                situation: ActionSituation::CloseShot,
                description: [
                    format!(
                        "{} breaks {}'s ankles and is now alone at the basket.",
                        iso.info.short_name(),
                        defender.info.short_name()
                    ),
                    format!(
                        "{} blows by {} with a lightning-quick crossover and soars for the dunk.",
                        iso.info.short_name(),
                        defender.info.short_name()
                    ),
                    format!(
                        "{} fakes out {} with a smooth hesitation dribble and glides to the rim.",
                        iso.info.short_name(),
                        defender.info.short_name()
                    ),
                    format!(
                        "{} spins past {} effortlessly.",
                        iso.info.short_name(),
                        defender.info.short_name()
                    ),
                    format!(
                        "{} crosses over {}, leaving {} stumbling, and goes for the open jumper.",
                        iso.info.short_name(),
                        defender.info.short_name(),
                        defender.info.pronouns.as_object()
                    ),
                    format!(
                        "{} uses a killer step-back move to create space from {}.",
                        iso.info.short_name(),
                        defender.info.short_name()
                    ),

                    format!(
                        "{} weaves through traffic, leaving {} behind.",
                        iso.info.short_name(),
                        defender.info.short_name()
                    ),
                    format!(
                        "{} fakes out {} with a jab step and drives straight to the basket.",
                        iso.info.short_name(),
                        defender.info.short_name()
                    ),
                    format!(
                        "{} cuts through {} and the help defense.",
                        iso.info.short_name(),
                        defender.info.short_name()
                    ),
                    format!(
                        "{} shakes off {} with a crafty behind-the-back dribble and goes for a clean jumper.",
                        iso.info.short_name(),
                        defender.info.short_name()
                    ),
                ].choose(description_rng).expect("There should be one option").clone(),
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
                description: [
                    format!(
                        "{} gets through {} and gathers the ball to shoot.",
                        iso.info.short_name(),
                        defender.info.short_name()
                    ),
                    format!(
                        "{} crosses over {}, creating a bit of space to rise for the jumper.",
                        iso.info.short_name(),
                        defender.info.short_name()
                    ),
                    format!(
                        "{} blows past {} with a quick first step and attacks the rim.",
                        iso.info.short_name(),
                        defender.info.short_name()
                    ),
                    format!(
                        "{} spins around {} and lines up for a clean look at the basket.",
                        iso.info.short_name(),
                        defender.info.short_name()
                    ),
                    format!(
                        "{} uses a hesitation move to freeze {} and drives to the hoop.",
                        iso.info.short_name(),
                        defender.info.short_name()
                    ),
                    format!(
                        "{} accelerates past {} and floats a shot over the defense.",
                        iso.info.short_name(),
                        defender.info.short_name()
                    ),
                    format!(
                        "{} gets a step on {}, pivots, and pulls up for a shot.",
                        iso.info.short_name(),
                        defender.info.short_name()
                    ),
                    format!(
                        "{} shakes off {} with a step-back dribble and fires.",
                        iso.info.short_name(),
                        defender.info.short_name()
                    ),
                ].choose(description_rng).expect("There should be one option").clone(),
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
                description:  [
                    format!(
                        "{} tries to dribble past {} but {} is all over {}.",
                        iso.info.short_name(),
                        defender.info.short_name(),
                        defender.info.short_name(),
                        iso.info.pronouns.as_object()
                    ),
                    format!(
                        "{} attempts a quick crossover on {}, but {} anticipates the move perfectly.",
                        iso.info.short_name(),
                        defender.info.short_name(),
                        defender.info.short_name()
                    ),
                    format!(
                        "{} goes for a step-back jumper against {}, but {} contests the shot heavily.",
                        iso.info.short_name(),
                        defender.info.short_name(),
                        defender.info.short_name()
                    ),
                    format!(
                        "{} spins into the lane trying to shake {}, but {} holds {} ground.",
                        iso.info.short_name(),
                        defender.info.short_name(),
                        defender.info.short_name(),
                        defender.info.pronouns.as_possessive()
                    ),
                    format!(
                        "{} executes a behind-the-back dribble to beat {}, but {} recovers quickly.",
                        iso.info.short_name(),
                        defender.info.short_name(),
                        defender.info.short_name()
                    ),
                    format!(
                        "{} pulls up for a mid-range shot over {}, but {} contests it fiercely.",
                        iso.info.short_name(),
                        defender.info.short_name(),
                        defender.info.short_name()
                    ),
                    format!(
                        "{} drives baseline against {}, but {} cuts off the angle superbly.",
                        iso.info.short_name(),
                        defender.info.short_name(),
                        defender.info.short_name()
                    ),
                ].choose(description_rng).expect("There should be one option").clone(),
                start_at: input.end_at,
                end_at: input.end_at.plus(timer_increase),
                home_score: input.home_score,
                away_score: input.away_score,
                ..Default::default()
            },
            _ => {
                iso_update.turnovers = 1;
                iso_update.extra_morale += MoraleModifier::MEDIUM_MALUS;
                defender_update.extra_morale += MoraleModifier::SMALL_BONUS;

                // Equivalent to `- def_result - target_defender.defense.steal.game_value() <= STEAL_LIMIT`
                let with_steal = def_result + defender.defense.steal.game_value() >= -STEAL_LIMIT;


            if with_steal {
                defender_update.steals = 1;
                defender_update.extra_morale += MoraleModifier::MEDIUM_BONUS;
                iso_update.extra_morale += MoraleModifier::MEDIUM_MALUS;
            }

            let situation = if with_steal && action_rng.random_bool(FASTBREAK_ACTION_PROBABILITY * game.defending_team().tactic.fastbreak_probability_modifier()){
                ActionSituation::Fastbreak
            } else {
                ActionSituation::Turnover
            };

            let attackers = if with_steal {
                vec![iso_idx]
            } else {vec![]};

            let end_at = if with_steal {
                input.end_at.plus(1 +  action_rng.random_range(0..=2))
            } else {
                input.end_at.plus(4 + action_rng.random_range(0..=2))
            };

            let description = if with_steal {
                        [
                        format!(
                            "{} tries to dribble past {} but {} steals the ball. Terrible choice.",
                            iso.info.short_name(),
                            defender.info.short_name(),
                            defender.info.short_name()
                        ),
                        format!(
                            "{} goes for a flashy behind-the-back pass, but {} intercepts it easily. Risky decision!",
                            iso.info.short_name(),
                            defender.info.short_name(),
                        ),
                        format!(
                            "{} attempts a spin move to beat {}, but {} strips the ball mid-spin. Incredible defense!",
                            iso.info.short_name(),
                            defender.info.short_name(),
                            defender.info.short_name()
                        ),
                        format!(
                            "{} tries a fadeaway jumper over {}, but {} contests it perfectly. Poor shot selection!",
                            iso.info.short_name(),
                            defender.info.short_name(),
                            defender.info.short_name()
                        ),
                        format!(
                            "{} charges down the lane, hoping to outmuscle {}, but {} blocks the attempt. Denied!",
                            iso.info.short_name(),
                            defender.info.short_name(),
                            defender.info.short_name()
                        ),
                        format!(
                            "{} attempts a quick crossover to get past {}, but {} picks {} pocket clean. Too predictable!",
                            iso.info.short_name(),
                            defender.info.short_name(),
                            defender.info.short_name(),
                            iso.info.pronouns.as_possessive()
                        ),
                    ]}else{[
                        format!(
                            "{} tries to dribble past {} but {}'s pressure makes {} fumble the ball.",
                            iso.info.short_name(),
                            defender.info.short_name(),
                            defender.info.short_name(), iso.info.pronouns.as_object(),
                        ),
                        format!(
                            "{} loses the ball while trying a flashy behind-the-back pass to get off of {}'s defense. Risky decision!",
                            iso.info.short_name(),
                            defender.info.short_name(),
                        ),
                        format!(
                            "{} attempts a spin move to beat {}, but {} forces the turnover.",
                            iso.info.short_name(),
                            defender.info.short_name(),
                            defender.info.short_name()
                        ),
                        format!(
                            "{} attempts a fancy between-the-legs move to get past {}, but {} forces the turnover.",
                            iso.info.short_name(),
                            defender.info.short_name(),
                            defender.info.short_name()
                        ),
                        format!(
                            "{} attempts a quick crossover to get past {}, but trips on the floor!",
                            iso.info.short_name(),
                            defender.info.short_name(),
                        ),
                        format!(
                            "{} goes for an ill-advised lob pass, the ball is lost on the backboard...",
                            iso.info.short_name(),
                        ),
                    ]}.choose(description_rng).expect("There should be one option").clone();

                ActionOutput {
                    situation,
                    possession: !input.possession,
                    description,
                    start_at: input.end_at,
                    end_at,
                    home_score: input.home_score,
                    away_score: input.away_score,
                    attackers,
                    ..Default::default()
                }
            }
        };
    attack_stats_update.insert(iso.id, iso_update);
    defense_stats_update.insert(defender.id, defender_update);
    result.attack_stats_update = Some(attack_stats_update);
    result.defense_stats_update = Some(defense_stats_update);
    result
}
