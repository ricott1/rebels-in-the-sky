use super::{action::*, constants::*, game::Game, types::*};
use crate::core::{
    constants::{MoraleModifier, TirednessCost},
    skill::GameSkill,
    Player, MAX_SKILL,
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
    let play_idx = match input.attackers.len() {
        0 => {
            let weights = [70, 15, 25, 2, 1];
            if let Some(idx) = sample_player_index(action_rng, weights, attacking_players_array) {
                idx
            } else {
                return ActionOutput {
                    situation: ActionSituation::Turnover,
                    possession: !input.possession,
                    description: format!(
                    "Wow! No player of {} is left standing, they just turned the ball over like that!",
                    game.attacking_team().name,
                ),
                    start_at: input.end_at,
                    end_at: input.end_at.plus(4 + action_rng.random_range(0..=3)),
                    home_score: input.home_score,
                    away_score: input.away_score,
                    ..Default::default()
                };
            }
        }
        _ => input.attackers[0],
    };

    let target_idx = match input.attackers.len() {
        0 | 1 => {
            let weights = [1, 2, 3, 3, 2];
            sample_player_index(action_rng, weights, attacking_players_array)
                .expect("Since we reached this selection, there should be at least one player.")
        }
        _ => input.attackers[1],
    };

    let mut attack_stats_update: GameStatsMap = HashMap::new();
    let mut defense_stats_update: GameStatsMap = HashMap::new();

    // Playmaker uses the screen to go to basket
    let mut result = if play_idx == target_idx {
        playmaker_uses_the_screen(
            play_idx,
            &mut attack_stats_update,
            &mut defense_stats_update,
            input,
            game,
            action_rng,
            description_rng,
        )
    }
    // Playmaker tries to pass to player who set the screen
    else {
        playmaker_passes_to_target(
            play_idx,
            target_idx,
            &mut attack_stats_update,
            &mut defense_stats_update,
            input,
            game,
            action_rng,
            description_rng,
        )
    };

    result.attack_stats_update = Some(attack_stats_update);
    result.defense_stats_update = Some(defense_stats_update);
    result
}

fn playmaker_uses_the_screen(
    play_idx: usize,
    attack_stats_update: &mut GameStatsMap,
    defense_stats_update: &mut GameStatsMap,
    input: &ActionOutput,
    game: &Game,
    action_rng: &mut ChaCha8Rng,
    description_rng: &mut ChaCha8Rng,
) -> ActionOutput {
    let attacking_players_array = game.attacking_players_array();
    let defending_players_array = game.defending_players_array();
    let playmaker = attacking_players_array[play_idx];
    let playmaker_defender = defending_players_array[play_idx];

    // Select a screener
    let screener_idx = {
        let mut weights = [1, 2, 3, 3, 2];
        weights[play_idx] = 0;
        if let Some(idx) = sample_player_index(action_rng, weights, attacking_players_array) {
            idx
        } else {
            return ActionOutput {
                situation: ActionSituation::Turnover,
                possession: !input.possession,
                description: format!(
                    "Oh no! No player of {} set up the screen and they just turned the ball over!",
                    game.attacking_team().name,
                ),
                start_at: input.end_at,
                end_at: input.end_at.plus(4 + action_rng.random_range(0..=3)),
                home_score: input.home_score,
                away_score: input.away_score,
                ..Default::default()
            };
        }
    };

    let screener = attacking_players_array[screener_idx];
    let screener_update = GameStats {
        extra_tiredness: TirednessCost::LOW,
        ..Default::default()
    };

    let screener_defender = defending_players_array[screener_idx];
    let screener_defender_update = GameStats {
        extra_tiredness: TirednessCost::LOW,
        ..Default::default()
    };

    let mut playmaker_update = GameStats {
        extra_tiredness: TirednessCost::MEDIUM,
        ..Default::default()
    };

    // FIXME: playmaker defender and target defender can be same player
    let mut playmaker_defender_update = GameStats {
        extra_tiredness: TirednessCost::MEDIUM,
        ..Default::default()
    };

    let atk_result = playmaker.roll(action_rng)
        + (0.75 * playmaker.technical.ball_handling + 0.25 * playmaker.athletics.quickness)
            .game_value()
        + (0.5 * screener.athletics.strength + 0.5 * playmaker.mental.intuition).game_value()
        + game
            .attacking_team()
            .tactic
            .attack_roll_bonus(&Action::PickAndRoll);

    let def_result = playmaker_defender.roll(action_rng)
        + playmaker_defender.defense.perimeter_defense.game_value()
        + (0.25 * playmaker_defender.defense.steal
            + 0.5 * playmaker_defender.athletics.quickness
            + 0.25 * playmaker_defender.athletics.strength)
            .game_value()
        + game
            .defending_team()
            .tactic
            .defense_roll_bonus(&Action::PickAndRoll);

    // Split: if playmaker has good vision and passing, it passes to another player off-the-screen
    let num_ok_players = game
        .attacking_players_array()
        .iter()
        .filter(|p| !p.is_knocked_out())
        .count();
    let result = if num_ok_players > 1
        && action_rng.random_bool(
            ((0.5 * playmaker.mental.vision + 0.5 * playmaker.technical.passing) / MAX_SKILL)
                as f64,
        ) {
        let mut weights = [3, 3, 3, 3, 1];
        weights[play_idx] = 0;
        let off_screen_idx = sample_player_index(action_rng, weights, attacking_players_array)
            .expect("There should be another ok player");
        let off_screen_player: &Player = attacking_players_array[off_screen_idx];
        ActionOutput {
            possession: input.possession,
            advantage: Advantage::Neutral,
            attackers: vec![play_idx, off_screen_idx],
            defenders: vec![],
            situation: ActionSituation::ForcedOffTheScreenAction,
            description: [
                format!(
                    "{} fakes using the pick'n'roll and gives the ball to {} in the corner at the last moment.",
                    playmaker.info.short_name(), off_screen_player.info.short_name()
                ),
                format!(
                    "{} is confronted by {}'s sticky defense, but suddenly passes to {} who's cutting through.",
                    playmaker.info.short_name(), playmaker_defender.info.short_name(), off_screen_player.info.short_name()
                ),
                format!(
                    "{} fakes the penetration and decides to give the ball to {} instead.",
                    playmaker.info.short_name(),  off_screen_player.info.short_name()
                ),
            ].choose(description_rng)
            .expect("There should be one option")
            .clone(),
            start_at: input.end_at,
            end_at: input.end_at.plus(1 + action_rng.random_range(0..=2)),
            home_score: input.home_score,
            away_score: input.away_score,
            assist_from: Some(play_idx),
            ..Default::default()
        }
    } else {
        let timer_increase = 3 + action_rng.random_range(0..=3);
        match atk_result - def_result {
                x if x >= ADV_ATTACK_LIMIT => ActionOutput {
                    possession: input.possession,
                    advantage: Advantage::Attack,
                    attackers: vec![play_idx],
                    defenders: vec![play_idx],
                    situation: ActionSituation::LongShot,
                    description: [
                        format!(
                            "{} uses the screen perfectly and is now open for the shot.",
                            playmaker.info.short_name()
                        ),
                        format!(
                            "{} navigates {}'s screen flawlessly and gets wide open for the shot.",
                            playmaker.info.short_name(), screener.info.short_name()
                        ),
                        format!(
                            "{} reads the defense well and uses the screen to get free for an open shot.",
                            playmaker.info.short_name()
                        ),
                        format!(
                            "{} uses the pick to perfection, getting a clean look at the basket.",
                            playmaker.info.short_name()
                        ),
                        format!(
                            "{} takes full advantage of {}'s screen and has an easy opportunity for a shot.",
                            playmaker.info.short_name(), screener.info.short_name()
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
                    attackers: vec![play_idx],
                    defenders: vec![play_idx],
                    situation: ActionSituation::LongShot,
                    description: [
                        format!(
                            "They go for the pick'n'roll. {} goes through {}'s screen and manages to get a bit of space to shot.",
                            playmaker.info.short_name(), screener.info.short_name()
                        ),
                        format!(
                            "The pick'n'roll is set up. {} uses {}'s screen to create just enough separation for a shot.",
                            playmaker.info.short_name(), screener.info.short_name()
                        ),
                        format!(
                            "The pick'n'roll play is in motion. {} fights through {}'s screen and gets a little space to shoot.",
                            playmaker.info.short_name(), screener.info.short_name()
                        ),
                        format!(
                            "They run the pick'n'roll. {} navigates through {}'s screen and manages a slight opening for the shot.",
                            playmaker.info.short_name(), screener.info.short_name()
                        ),
                        format!(
                            "In the pick'n'roll, {} uses {}'s screen and finds just enough room to take a shot.",
                            playmaker.info.short_name(), screener.info.short_name()
                        ),
                    ].choose(description_rng).expect("There should be one option").clone(),
                    start_at: input.end_at,
                        end_at: input.end_at.plus(timer_increase),
                        home_score: input.home_score,
                    away_score: input.away_score,
                    ..Default::default()
                },
                x if x > ADV_DEFENSE_LIMIT => {
                    match action_rng.random_bool(0.5) {
                        false => ActionOutput {
                            possession: input.possession,
                            advantage: Advantage::Defense,
                            attackers: vec![play_idx],
                            defenders: vec![screener_idx],
                            situation: ActionSituation::LongShot,
                            description:[
                                format!(
                                    "{} tries to use the screen but {} slides nicely to cover.",
                                    playmaker.info.short_name(), screener_defender.info.short_name()
                                ),
                                format!(
                                    "{} eludes {}'s screen and slides to cover {}.",
                                    screener_defender.info.short_name(), screener.info.short_name(),playmaker.info.short_name()
                                ),
                                format!(
                                    "{} tries to move past {} using the screen but {} swaps cover and is all over {}.",
                                    playmaker.info.short_name(), playmaker_defender.info.short_name(), screener_defender.info.short_name(),playmaker.info.pronouns.as_object()
                                ),
                            ] .choose(description_rng).expect("There should be one option").clone(),
                            start_at: input.end_at,
                                end_at: input.end_at.plus(timer_increase),
                                home_score: input.home_score,
                            away_score: input.away_score,
                            ..Default::default()
                        },
                        true => ActionOutput {
                            possession: input.possession,
                            advantage: Advantage::Defense,
                            attackers: vec![play_idx],
                            defenders: vec![play_idx],
                            situation: ActionSituation::LongShot,
                            description:[
                                format!(
                                    "{} attempts to navigate the screen, but {} stays right with {}, denying the space.",
                                    playmaker.info.short_name(), playmaker_defender.info.short_name(), playmaker.info.pronouns.as_object()
                                ),
                                format!(
                                    "{} goes for the screen, but {} expertly fights through, staying tight on defense.",
                                    playmaker.info.short_name(), playmaker_defender.info.short_name()
                                ),
                                format!(
                                    "{} tries to use the pick, but {} anticipates the move and stays in front.",
                                    playmaker.info.short_name(), playmaker_defender.info.short_name()
                                ),
                                format!(
                                    "{} tries to get open off the screen, but {} moves with him step for step, preventing any separation.",
                                    playmaker.info.short_name(), playmaker_defender.info.short_name()
                                ),
                            ] .choose(description_rng).expect("There should be one option").clone(),
                            start_at: input.end_at,
                                end_at: input.end_at.plus(timer_increase),
                                home_score: input.home_score,
                            away_score: input.away_score,
                            ..Default::default()
                        }
                    }
            },
                _ => {
                    playmaker_update.turnovers = 1;
                    playmaker_update.extra_morale += MoraleModifier::SMALL_MALUS;
                    playmaker_defender_update.extra_morale += MoraleModifier::SMALL_BONUS;
                    // Equivalent to `- def_result - target_defender.defense.steal.game_value() <= STEAL_LIMIT`
                    let with_steal = def_result + playmaker_defender.defense.steal.game_value() >= -STEAL_LIMIT;

                    if with_steal {
                        playmaker_defender_update.steals = 1;
                        playmaker_defender_update.extra_morale += MoraleModifier::MEDIUM_BONUS;
                        playmaker_update.extra_morale += MoraleModifier::MEDIUM_MALUS;
                    }

                    let situation = if with_steal && action_rng.random_bool(FASTBREAK_ACTION_PROBABILITY * game.defending_team().tactic.fastbreak_probability_modifier()){
                        ActionSituation::Fastbreak
                    } else {
                        ActionSituation::Turnover
                    };

                    let attackers = if with_steal {
                        vec![play_idx]
                    } else {vec![]};

                    let end_at = if with_steal {
                        input.end_at.plus(1 +  action_rng.random_range(0..=2))
                    } else {
                        input.end_at.plus(4 + action_rng.random_range(0..=2))
                    };

                    let description =  if with_steal{[
                            format!(
                                "{} tries to use the screen but {} snatches the ball from {} hands.",
                                playmaker.info.short_name(), playmaker_defender.info.short_name(), playmaker.info.pronouns.as_possessive()
                            ),
                            format!(
                                "{} attempts to use the screen, but {} swipes the ball right out of {} hands.",
                                playmaker.info.short_name(), playmaker_defender.info.short_name(), playmaker.info.pronouns.as_possessive()
                            ),
                            format!(
                                "{} tries to get open with the screen, but {} anticipates the play and steals the ball.",
                                playmaker.info.short_name(), playmaker_defender.info.short_name()
                            ),
                            format!(
                                "{} goes for the screen, but {} is quick to jump in, stealing the ball away from {}.",
                                playmaker.info.short_name(), playmaker_defender.info.short_name(), playmaker.info.pronouns.as_possessive()
                            ),
                        ]}else{[
                            format!(
                                "{} tries to use the screen fumbles the ball.",
                                playmaker.info.short_name()
                            ),
                            format!(
                                "{} attempts to use the screen, but {} forces the turnover.",
                                playmaker.info.short_name(), playmaker_defender.info.short_name(),
                            ),
                            format!(
                                "{} tries to get open with the screen, but {} anticipates the play and forces the turnover.",
                                playmaker.info.short_name(), playmaker_defender.info.short_name()
                            ),
                            format!(
                                "{} attempts the screen play, but {} closes the passing lane, making {} lose the ball.",
                                playmaker.info.short_name(), playmaker_defender.info.short_name(),playmaker.info.short_name()
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
            }
    };

    attack_stats_update.insert(screener.id, screener_update);
    defense_stats_update.insert(screener_defender.id, screener_defender_update);
    attack_stats_update.insert(playmaker.id, playmaker_update);
    defense_stats_update.insert(playmaker_defender.id, playmaker_defender_update);

    result
}

fn playmaker_passes_to_target(
    play_idx: usize,
    target_idx: usize,
    attack_stats_update: &mut GameStatsMap,
    defense_stats_update: &mut GameStatsMap,
    input: &ActionOutput,
    game: &Game,
    action_rng: &mut ChaCha8Rng,
    description_rng: &mut ChaCha8Rng,
) -> ActionOutput {
    let attacking_players_array = game.attacking_players_array();
    let defending_players_array = game.defending_players_array();
    let playmaker = attacking_players_array[play_idx];
    let playmaker_defender = defending_players_array[play_idx];

    let mut playmaker_update = GameStats {
        extra_tiredness: TirednessCost::MEDIUM,
        ..Default::default()
    };

    // FIXME: playmaker defender and target defender can be same player
    let mut playmaker_defender_update = GameStats {
        extra_tiredness: TirednessCost::MEDIUM,
        ..Default::default()
    };

    let target = attacking_players_array[target_idx];
    let target_defender = defending_players_array[target_idx];
    let target_update = GameStats {
        extra_tiredness: TirednessCost::MEDIUM,
        ..Default::default()
    };
    let target_defender_update = GameStats {
        extra_tiredness: TirednessCost::MEDIUM,
        ..Default::default()
    };

    let atk_result = playmaker.roll(action_rng)
        + (0.25 * playmaker.technical.ball_handling
            + 0.25 * playmaker.mental.vision
            + 0.5 * target.mental.intuition)
            .game_value()
        + playmaker.technical.passing.game_value()
        + game
            .attacking_team()
            .tactic
            .attack_roll_bonus(&Action::PickAndRoll);

    let def_result = playmaker_defender.roll(action_rng)
        + playmaker_defender.defense.perimeter_defense.game_value()
        + (0.25 * target_defender.athletics.quickness
            + 0.5 * target_defender.mental.intuition
            + 0.25 * playmaker_defender.defense.steal)
            .game_value()
        + game
            .defending_team()
            .tactic
            .defense_roll_bonus(&Action::PickAndRoll);

    let timer_increase = 4 + action_rng.random_range(0..=3);

    let result = match atk_result  - def_result {
            x if x >= ADV_ATTACK_LIMIT => ActionOutput {
                possession: input.possession,
                advantage: Advantage::Attack,
                attackers: vec![target_idx],
                defenders: vec![play_idx],
                situation: ActionSituation::CloseShot,
                description: [
                    format!(
                        "{} gives the ball to {} using the pick'n'roll perfectly! {} is now open for the shot.",
                        playmaker.info.short_name(), target.info.short_name(), target.info.short_name()
                    ),
                    format!(
                        "{} and {} run the pick'n'roll to perfection, and now {} has a wide-open shot.",
                        playmaker.info.short_name(), target.info.short_name(), target.info.short_name()
                    ),
                    format!(
                        "{} and {} work the pick'n'roll flawlessly! {} is left with a clean look at the basket.",
                        playmaker.info.short_name(), target.info.short_name(), target.info.short_name()
                    ),
                    format!(
                        "{} and {} execute the pick'n'roll to perfection, freeing {} for an easy shot attempt.",
                        playmaker.info.short_name(), target.info.short_name(), target.info.short_name()
                    ),
                    format!(
                        "{} passes to {} after a flawless pick'n'roll, and now {} is in prime position for the shot.",
                        playmaker.info.short_name(), target.info.short_name(), target.info.short_name()
                    ),
                ].choose(description_rng).expect("There should be one option").clone(),
                assist_from: Some(play_idx),
                start_at: input.end_at,
                        end_at: input.end_at.plus(timer_increase),
                        home_score: input.home_score,
                    away_score: input.away_score,
                ..Default::default()
            },
            x if x > ADV_NEUTRAL_LIMIT => ActionOutput {
                possession: input.possession,
                advantage: Advantage::Neutral,
                attackers: vec![target_idx],
                defenders: vec![play_idx],
                situation: if action_rng.random_bool(((target.athletics.quickness - 0.5 * target_defender.defense.interior_defense).bound()/MAX_SKILL)as f64) {ActionSituation::CloseShot} else {ActionSituation::MediumShot},
                description:[
                    format!(
                        "After setting up the screen, {} gets the pass from {} and is now ready to shoot.",
                       target.info.short_name(), playmaker.info.short_name(),
                    ),
                    format!(
                        "The pick'n'roll is executed smoothly. {} passes to {} who now has an opportunity to shoot.",
                        playmaker.info.short_name(), target.info.short_name(),
                    ),
                    format!(
                        "They run the pick'n'roll, and after a solid pass from {} to {}, the shot is ready.",
                        playmaker.info.short_name(), target.info.short_name(),
                    ),
                    format!(
                        "Nice pick'n'roll execution. {} delivers a pass to {} who's now in position to take the shot.",
                        playmaker.info.short_name(), target.info.short_name(),
                    ),
                    format!(
                        "The pick'n'roll is set up well, and {} passes to {} who prepares for the shot.",
                        playmaker.info.short_name(), target.info.short_name(),
                    ),
                ].choose(description_rng).expect("There should be one option").clone(),
                assist_from: Some(play_idx),
                start_at: input.end_at,
                        end_at: input.end_at.plus(timer_increase),
                        home_score: input.home_score,
                    away_score: input.away_score,
                ..Default::default()
            },
            x if x > ADV_DEFENSE_LIMIT =>   {
            // Split: if playmaker has good vision and passing, it passes to a third player
            let num_ok_players = game
                .attacking_players_array()
                .iter()
                .filter(|p| !p.is_knocked_out())
                .count();
            if num_ok_players > 2 && (0.75 * playmaker.mental.vision + 0.25 * playmaker.technical.passing).game_value() + x
                > ADV_NEUTRAL_LIMIT {

                let mut weights = [3, 3, 2, 2, 1];
                weights[play_idx] = 0;
                weights[target_idx] = 0;
                let off_screen_idx = sample_player_index(action_rng, weights, attacking_players_array)
                .expect("There should be another ok player");
                let off_screen_player: &Player = attacking_players_array[off_screen_idx];
                ActionOutput {
                        possession: input.possession,
                        advantage: Advantage::Neutral,
                        attackers: vec![play_idx, off_screen_idx,target_idx],
                        defenders: vec![],
                        situation: ActionSituation::ForcedOffTheScreenAction,
                        description: [
                            format!(
                                "{} tries to go through {} with {}'s screen, but sees {} in the corner at the last moment.",
                                playmaker.info.short_name(), playmaker_defender.info.short_name(), target.info.short_name(), off_screen_player.info.short_name()
                            ),
                            format!(
                                "{} can't shake off {}'s sticky defense, so the ball is passed to {}.",
                                playmaker.info.short_name(), playmaker_defender.info.short_name(), off_screen_player.info.short_name()
                            ),
                            format!(
                                "{} was ready to get the pass by {} but {} decides to give the ball to {} instead.",
                                target.info.short_name(), playmaker.info.short_name(), playmaker.info.pronouns.as_subject(), off_screen_player.info.short_name()
                            ),
                        ].choose(description_rng)
                        .expect("There should be one option")
                        .clone(),
                        start_at: input.end_at,
                        end_at: input.end_at.plus(1 + action_rng.random_range(0..=2)),
                        home_score: input.home_score,
                        away_score: input.away_score,
                        assist_from: Some(play_idx),
                        ..Default::default()
                    }
                }
            else {
                ActionOutput {
                    possession: input.possession,
                    advantage: Advantage::Defense,
                    attackers: vec![target_idx],
                    defenders: vec![target_idx],
                    situation: ActionSituation::MediumShot,
                    description:[
                        format!(
                            "They go for the pick'n'roll. {} passes to {} but {} is all over {}.",
                            playmaker.info.short_name(), target.info.short_name(), target_defender.info.short_name(), target.info.pronouns.as_object()
                        ),
                        format!(
                            "The pick'n'roll is executed, but {} is quick to cover as {} passes to {}.",
                        target_defender.info.short_name(), playmaker.info.short_name(), target.info.short_name(),
                        ),
                        format!(
                            "They try the pick'n'roll. {} passes to {} but {} sticks to {} like glue.",
                            playmaker.info.short_name(), target.info.short_name(), target_defender.info.short_name(), target.info.pronouns.as_object()
                        ),
                        format!(
                            "They run the pick'n'roll, but {} anticipates it perfectly, covering {} as soon as the pass is made.",
                            target_defender.info.short_name(), target.info.short_name()
                        ),
                        format!(
                            "On the pick'n'roll, {} passes to {} but {} is right there, denying any space for a shot.",
                            playmaker.info.short_name(), target.info.short_name(), target_defender.info.short_name()
                        ),
                    ].choose(description_rng).expect("There should be one option").clone(),
                    assist_from: Some(play_idx),
                    start_at: input.end_at,
                            end_at: input.end_at.plus(timer_increase),
                            home_score: input.home_score,
                        away_score: input.away_score,
                    ..Default::default()
                }
            }
        },
            _ => {
                playmaker_update.turnovers = 1;
                playmaker_update.extra_morale += MoraleModifier::SMALL_MALUS;
                playmaker_defender_update.extra_morale += MoraleModifier::SMALL_BONUS;

                // Equivalent to `- def_result - target_defender.defense.steal.game_value() <= STEAL_LIMIT`
                let with_steal = def_result + playmaker_defender.defense.steal.game_value() >= -STEAL_LIMIT;

                if with_steal {
                    playmaker_defender_update.steals = 1;
                    playmaker_defender_update.extra_morale += MoraleModifier::MEDIUM_BONUS;
                    playmaker_update.extra_morale += MoraleModifier::MEDIUM_MALUS;
                }

                let situation = if with_steal && action_rng.random_bool(FASTBREAK_ACTION_PROBABILITY * game.defending_team().tactic.fastbreak_probability_modifier()){
                    ActionSituation::Fastbreak
                } else {
                    ActionSituation::Turnover
                };

                let attackers = if with_steal {
                    vec![play_idx]
                } else {vec![]};

                let end_at = if with_steal {
                    input.end_at.plus(1 +  action_rng.random_range(0..=2))
                } else {
                    input.end_at.plus(4 + action_rng.random_range(0..=2))
                };

                let description = if with_steal{[
                        format!(
                            "They go for the pick'n'roll but the defender read that perfectly. {} tries to pass to {} but {} blocks the pass.",
                            playmaker.info.short_name(), target.info.short_name(), playmaker_defender.info.short_name()
                        ),
                        format!(
                            "The pick'n'roll is set, but {} sees it coming and blocks the pass to {}.",
                            playmaker_defender.info.short_name(), target.info.short_name()
                        ),
                        format!(
                            "They try the pick'n'roll, but {} reads the move perfectly, blocking {}'s pass to {}.",
                            playmaker_defender.info.short_name(), playmaker.info.short_name(), target.info.short_name()
                        ),
                    ]}else{[
                        format!(
                            "On the pick'n'roll, {} anticipates the play perfectly and pushes {} to an unforced error.",
                           playmaker_defender.info.short_name(),  playmaker.info.short_name(),
                        ),
                        format!(
                            "They attempt the pick'n'roll, but {}'s good defense forces the turnover.",
                            playmaker_defender.info.short_name()
                        ),
                        format!(
                            "The pick'n'roll is set but {} trips and loses the ball under {}'s pressure.",
                            playmaker.info.short_name(), playmaker_defender.info.short_name()
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

    attack_stats_update.insert(target.id, target_update);
    defense_stats_update.insert(target_defender.id, target_defender_update);

    attack_stats_update.insert(playmaker.id, playmaker_update);
    defense_stats_update.insert(playmaker_defender.id, playmaker_defender_update);

    result
}
