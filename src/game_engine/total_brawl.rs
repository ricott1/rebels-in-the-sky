use super::{action::*, game::Game, types::*};
use crate::core::{constants::TirednessCost, player::Trait, Skill};
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
    let mut output = ActionOutput {
        situation: input.situation,
        start_at: input.end_at,
        end_at: input.end_at,
        home_score: input.home_score,
        away_score: input.away_score,
        ..Default::default()
    };

    let mut attack_stats_update = HashMap::new();
    for player in attacking_players_array.iter() {
        let update = GameStats {
            extra_tiredness: TirednessCost::MEDIUM,
            ..Default::default()
        };
        attack_stats_update.insert(player.id, update);
    }

    let mut defense_stats_update = HashMap::new();
    for player in defending_players_array.iter() {
        let update = GameStats {
            extra_tiredness: TirednessCost::MEDIUM,
            ..Default::default()
        };
        defense_stats_update.insert(player.id, update);
    }

    // NOTE: we do not use game_value but the Skills directly so that the chance for draws is much lower.
    let atk_result: Skill = attacking_players_array
        .iter()
        .map(|player| {
            let mut result = player.roll(action_rng) as Skill
                + (0.5 * player.athletics.strength + 0.5 * player.mental.aggression)
                + player.offense.brawl;

            if player.special_trait == Some(Trait::Killer) {
                result += player.reputation;
            }
            result
        })
        .sum();

    let def_result: Skill = defending_players_array
        .iter()
        .map(|player| {
            let mut result = player.roll(action_rng) as Skill
                + (0.5 * player.athletics.strength + 0.5 * player.mental.aggression)
                + player.offense.brawl;

            if player.special_trait == Some(Trait::Killer) {
                result += player.reputation;
            }
            result
        })
        .sum();

    match atk_result - def_result {
        _x if _x > 0.0 => {
            // Clear win for attacking team
            output.description = [
                format!(
                    "The game ended in a draw! The crews settle it the pirate way. Total brawl! {} completely dominates the fight!",
                    game.attacking_team().name
                ),
                format!(
                    "It's a tie! Time to settle this with fists! All-out brawl and {} overpowers {} in a one-sided beat down!",
                    game.attacking_team().name,
                    game.defending_team().name
                ),
                format!(
                    "The game ended in a draw! The crews settle it the pirate way. {} crushes {} in the brawl!",
                    game.attacking_team().name,
                    game.defending_team().name
                ),
                format!(
                    "A draw! Total brawl to decide the winner! {} overwhelms {} and takes it!",
                    game.attacking_team().name,
                    game.defending_team().name
                ),
            ]
            .choose(description_rng)
            .expect("There should be an option")
            .clone();

            output.possession = if game.attacking_team().team_id == game.home_team_in_game.team_id {
                Possession::Home
            } else {
                Possession::Away
            };
        }
        _x if _x < 0.0 => {
            // Clear win for defending team
            output.description = [
                format!(
                    "The game ended in a draw! The crews settle it the pirate way. Total brawl! {} completely dominates the fight!",
                    game.defending_team().name
                ),
                format!(
                    "It's a tie! Time to settle this with fists! All-out brawl and {} overpowers {} in a one-sided beat down!",
                    game.defending_team().name,
                    game.attacking_team().name
                ),
                format!(
                    "The game ended in a draw! The crews settle it the pirate way. {} crushes {} in the brawl!",
                    game.defending_team().name,
                    game.attacking_team().name
                ),
                format!(
                    "A draw! Total brawl to decide the winner! {} overwhelms {} and takes it!",
                    game.defending_team().name,
                    game.attacking_team().name
                ),
            ]
            .choose(description_rng)
            .expect("There should be an option")
            .clone();

            output.possession = if game.defending_team().team_id == game.home_team_in_game.team_id {
                Possession::Home
            } else {
                Possession::Away
            };
        }
        _ => {
            if action_rng.random_bool(0.5) {
                // Close win for attacking team
                output.description = [
                    format!(
                        "The game ended in a draw! Total brawl! An absolute war between the crews! {} barely edges it out!",
                        game.attacking_team().name
                    ),
                    format!(
                        "It's a tie! Both crews go at it! It's chaos but {} manages to come out on top by a hair!",
                        game.attacking_team().name
                    ),
                    format!(
                        "A draw! The brawl is too close to call! After a brutal fight, {} scrapes by with the win!",
                        game.attacking_team().name
                    ),
                    format!(
                        "The game ended in a draw! Total brawl! {} and {} are evenly matched, but {} just barely takes it!",
                        game.attacking_team().name,
                        game.defending_team().name,
                        game.attacking_team().name
                    ),
                ]
                .choose(description_rng)
                .expect("There should be an option")
                .clone();

                output.possession =
                    if game.attacking_team().team_id == game.home_team_in_game.team_id {
                        Possession::Home
                    } else {
                        Possession::Away
                    };
            } else {
                // Close win for defending team
                output.description = [
                    format!(
                        "The game ended in a draw! Total brawl! An absolute war between the crews! {} barely edges it out!",
                        game.defending_team().name
                    ),
                    format!(
                        "It's a tie! Both crews go at it! It's chaos but {} manages to come out on top by a hair!",
                        game.defending_team().name
                    ),
                    format!(
                        "A draw! The brawl is too close to call! After a brutal fight, {} scrapes by with the win!",
                        game.defending_team().name
                    ),
                    format!(
                        "The game ended in a draw! Total brawl! {} and {} are evenly matched, but {} just barely takes it!",
                        game.defending_team().name,
                        game.attacking_team().name,
                        game.defending_team().name
                    ),
                ]
                .choose(description_rng)
                .expect("There should be an option")
                .clone();

                output.possession =
                    if game.defending_team().team_id == game.home_team_in_game.team_id {
                        Possession::Home
                    } else {
                        Possession::Away
                    };
            }
        }
    };

    output.attack_stats_update = Some(attack_stats_update);
    output.defense_stats_update = Some(defense_stats_update);

    output
}
