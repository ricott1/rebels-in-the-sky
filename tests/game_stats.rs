#[cfg(test)]
mod tests {
    use itertools::Itertools;
    use rayon::prelude::*;
    use rebels::core::{Player, Rated, Skill, Team, TickInterval, MAX_PLAYERS_PER_GAME};
    use rebels::game_engine::action::{ActionOutput, ActionSituation, Advantage};
    use rebels::game_engine::game::Game;
    use rebels::game_engine::tactic::Tactic;
    use rebels::game_engine::types::{GameStats, GameStatsMap, Possession, TeamInGame};
    use rebels::types::{AppResult, PlayerId, PlayerMap, SystemTimeTick, TeamId, Tick};
    use std::collections::{BTreeMap, HashMap};
    use strum::IntoEnumIterator;

    #[derive(Debug)]
    struct BinResult {
        center: i32,
        count: usize,
        win_count: usize,
        loss_count: usize,
        draw_count: usize,
        home_avg: Vec<f32>,
        home_std: Vec<f32>,
        away_avg: Vec<f32>,
        away_std: Vec<f32>,
    }

    #[derive(Debug)]
    struct MatchupResult {
        home_tactic: Tactic,
        away_tactic: Tactic,
        bins: Vec<BinResult>,
    }

    struct GameSample {
        rating_diff: f32,
        winner: Option<Possession>,
        home_stats: GameStatsMap,
        away_stats: GameStatsMap,
        home_players: PlayerMap,
        away_players: PlayerMap,
        action_outputs: Vec<ActionOutput>,
    }

    fn process_stats(samples: &[GameSample], bin_size: f32) -> Vec<BinResult> {
        // Map samples to vec of bins afetr filtering only wins
        let win_samples = samples
            .iter()
            .filter(|sample| matches!(sample.winner, Some(possession) if possession == Possession::Home))
            .map(|sample| (sample.rating_diff / bin_size).round() as i32);

        let mut win_counts: HashMap<i32, usize> = HashMap::new();
        for bin in win_samples {
            *win_counts.entry(bin).or_default() += 1;
        }

        let loss_samples = samples
            .iter()
            .filter(|sample| {
                matches!(sample.winner, Some(possession) if possession == Possession::Away)
            })
            .map(|sample| (sample.rating_diff / bin_size).round() as i32);

        let mut loss_counts: HashMap<i32, usize> = HashMap::new();
        for bin in loss_samples {
            *loss_counts.entry(bin).or_default() += 1;
        }

        // Compute selector stats (points, 2pt, 3pt, rebounds, assists, etc.)
        let selectors = vec![
            |s: &GameStats, _: &Player| 2.0 * s.made_2pt as f32 + 3.0 * s.made_3pt as f32,
            |s: &GameStats, _: &Player| s.made_2pt as f32,
            |s: &GameStats, _: &Player| s.attempted_2pt as f32,
            |s: &GameStats, _: &Player| s.made_3pt as f32,
            |s: &GameStats, _: &Player| s.attempted_3pt as f32,
            |s: &GameStats, _: &Player| s.defensive_rebounds as f32,
            |s: &GameStats, _: &Player| s.offensive_rebounds as f32,
            |s: &GameStats, _: &Player| s.assists as f32,
            |s: &GameStats, _: &Player| s.turnovers as f32,
            |s: &GameStats, _: &Player| s.steals as f32,
            |s: &GameStats, _: &Player| s.blocks as f32,
            |s: &GameStats, _: &Player| s.brawls[0] as f32 + 0.5 * s.brawls[1] as f32,
            |_: &GameStats, p: &Player| p.tiredness,
            |_: &GameStats, p: &Player| p.morale,
        ];

        let binned = compute_binned_stats(samples, bin_size, selectors);

        let mut bins = Vec::new();

        for (center, ((home_avg, home_std), (away_avg, away_std), count)) in binned {
            let win_count = win_counts.get(&center).copied().unwrap_or(0);
            let loss_count = loss_counts.get(&center).copied().unwrap_or(0);
            let draw_count = count - win_count - loss_count;

            bins.push(BinResult {
                center,
                count,
                win_count,
                loss_count,
                draw_count,
                home_avg,
                home_std,
                away_avg,
                away_std,
            });
        }

        bins
    }

    /// Sum the provided selector over a team's GameStatsMap.
    /// `stats` is a GameStatsMap (player_id -> GameStats).
    fn team_stat_sum<F>(stats: &GameStatsMap, players: &PlayerMap, selector: F) -> f32
    where
        F: Fn(&GameStats, &Player) -> f32,
    {
        stats
            .iter()
            .map(|(id, stat)| {
                let player = players.get(id).unwrap();
                selector(stat, player)
            })
            .sum()
    }

    fn generate_team_in_game(
        team_base_level: f32,
        with_fixed_stamina: Option<Skill>,
    ) -> TeamInGame {
        let team = Team {
            id: TeamId::new_v4(),
            ..Default::default()
        };

        let mut players = PlayerMap::new();
        for _ in 0..MAX_PLAYERS_PER_GAME {
            let mut player = Player::default()
                .with_base_level(team_base_level)
                .randomize(None);
            if let Some(value) = with_fixed_stamina {
                player.athletics.stamina = value;
            }
            players.insert(player.id, player);
        }

        TeamInGame::new(&team, players)
    }

    fn generate_identical_team_in_game(
        team_base_level: f32,
        with_fixed_stamina: Option<Skill>,
    ) -> (TeamInGame, TeamInGame) {
        let home_team = Team {
            id: TeamId::new_v4(),
            ..Default::default()
        };
        let away_team = Team {
            id: TeamId::new_v4(),
            ..Default::default()
        };
        let mut home_players = PlayerMap::new();
        let mut away_players = PlayerMap::new();
        for _ in 0..MAX_PLAYERS_PER_GAME {
            let mut home_player = Player::default()
                .with_base_level(team_base_level)
                .randomize(None);
            let mut away_player = home_player.clone();

            if let Some(value) = with_fixed_stamina {
                home_player.athletics.stamina = value;
                away_player.athletics.stamina = value;
            }

            away_player.id = PlayerId::new_v4();
            home_players.insert(home_player.id, home_player);
            away_players.insert(away_player.id, away_player);
        }

        (
            TeamInGame::new(&home_team, home_players),
            TeamInGame::new(&away_team, away_players),
        )
    }

    fn get_simulated_game_samples(
        n_games: usize,
        max_delta_rating: f32,
        home_tactic: Tactic,
        away_tactic: Tactic,
        with_fixed_stamina: Option<Skill>,
    ) -> Vec<GameSample> {
        let mut samples = Vec::with_capacity(n_games);
        for i in 0..n_games {
            let home_team_base_level =
                if max_delta_rating > 0.0 && i <= n_games / (2 * max_delta_rating as usize) {
                    0.0
                } else {
                    max_delta_rating * i as f32 / n_games as f32
                };
            let away_team_base_level =
                if max_delta_rating > 0.0 && i <= n_games / (2 * max_delta_rating as usize) {
                    0.0
                } else {
                    -max_delta_rating * i as f32 / n_games as f32
                };

            let (home_team_in_game, away_team_in_game) =
                if home_team_base_level == away_team_base_level {
                    generate_identical_team_in_game(home_team_base_level, with_fixed_stamina)
                } else {
                    (
                        generate_team_in_game(home_team_base_level, with_fixed_stamina),
                        generate_team_in_game(away_team_base_level, with_fixed_stamina),
                    )
                };

            let home_rating = home_team_in_game.rating();
            let away_rating = away_team_in_game.rating();

            // Reorder so home team is always higher rated one.
            let (mut home_team_in_game, mut away_team_in_game, rating_diff) =
                if home_rating >= away_rating {
                    (
                        home_team_in_game,
                        away_team_in_game,
                        (home_rating - away_rating) as f32,
                    )
                } else {
                    (
                        away_team_in_game,
                        home_team_in_game,
                        (away_rating - home_rating) as f32,
                    )
                };

            home_team_in_game.tactic = home_tactic;
            away_team_in_game.tactic = away_tactic;

            let mut current_tick = Tick::now();
            let mut game = Game::test(home_team_in_game, away_team_in_game);

            // Simulate until finished
            while !game.has_ended() {
                if game.has_started(current_tick) {
                    game.tick(current_tick);
                }
                current_tick += TickInterval::SHORT;
            }

            let winner = match game.winner {
                Some(id) if id == game.home_team_in_game.team_id => Some(Possession::Home),
                Some(id) if id == game.away_team_in_game.team_id => Some(Possession::Away),
                None => None,
                _ => unreachable!(),
            };

            let action_outputs = game.action_results;

            samples.push(GameSample {
                rating_diff,
                winner,
                home_stats: game.home_team_in_game.stats,
                away_stats: game.away_team_in_game.stats,
                home_players: game.home_team_in_game.players,
                away_players: game.away_team_in_game.players,
                action_outputs,
            })
        }

        samples
    }

    fn compute_binned_stats<F>(
        samples: &[GameSample], // (rating_diff, home_stats, away_stats)
        bin_size: f32,
        selectors: Vec<F>,
    ) -> BTreeMap<i32, ((Vec<f32>, Vec<f32>), (Vec<f32>, Vec<f32>), usize)>
    // Returns bin_center --> ((home mean, home stddev) for each selector, (away mean, away stddev) for each selector, count)
    where
        F: Fn(&GameStats, &Player) -> f32,
    {
        let entry_length = selectors.len() + 3 + 1 + 1; // +3 for advantages + 1 for fastbreak +1 for substitution

        // First pass: sum and count for each selector
        let default_entry = (
            vec![0.0f32].repeat(entry_length),
            vec![0.0f32].repeat(entry_length),
            0usize,
        );
        let mut sums: BTreeMap<i32, (Vec<f32>, Vec<f32>, usize)> = BTreeMap::new();
        for sample in samples {
            let bin = ((sample.rating_diff) / bin_size).round() as i32;
            let entry = sums.entry(bin).or_insert(default_entry.clone());
            for (idx, selector) in selectors.iter().enumerate() {
                entry.0[idx] += team_stat_sum(&sample.home_stats, &sample.home_players, selector);
                entry.1[idx] += team_stat_sum(&sample.away_stats, &sample.away_players, selector);
            }

            // Push data from sample.action_outputs
            for &possession in [Possession::Home, Possession::Away].iter() {
                for (idx, &advantage) in [Advantage::Attack, Advantage::Neutral, Advantage::Defense]
                    .iter()
                    .enumerate()
                {
                    let advantage_count = sample
                        .action_outputs
                        .iter()
                        .filter(|output| {
                            output.possession == possession && output.advantage == advantage
                        })
                        .count() as f32;
                    if possession == Possession::Home {
                        entry.0[selectors.len() + idx] += advantage_count;
                    } else {
                        entry.1[selectors.len() + idx] += advantage_count;
                    }
                }

                let fastbreak_count = sample
                    .action_outputs
                    .iter()
                    .filter(|output| {
                        output.possession == possession
                            && output.situation == ActionSituation::Fastbreak
                    })
                    .count() as f32;
                if possession == Possession::Home {
                    entry.0[selectors.len() + 3] += fastbreak_count;
                } else {
                    entry.1[selectors.len() + 3] += fastbreak_count;
                }

                let home_sub_count = sample
                    .action_outputs
                    .iter()
                    .filter(|output| {
                        output.situation == ActionSituation::AfterSubstitution
                            && ((output.attack_stats_update.is_some()
                                && output.possession == Possession::Home)
                                || (output.defense_stats_update.is_some()
                                    && output.possession == Possession::Away))
                    })
                    .count() as f32;

                let away_sub_count = sample
                    .action_outputs
                    .iter()
                    .filter(|output| {
                        output.situation == ActionSituation::AfterSubstitution
                            && ((output.attack_stats_update.is_some()
                                && output.possession == Possession::Away)
                                || (output.defense_stats_update.is_some()
                                    && output.possession == Possession::Home))
                    })
                    .count() as f32;

                if possession == Possession::Home {
                    entry.0[selectors.len() + 4] += home_sub_count;
                } else {
                    entry.1[selectors.len() + 4] += away_sub_count;
                }
            }

            // Increase count
            entry.2 += 1;
        }

        // Means
        let mut means: BTreeMap<i32, (Vec<f32>, Vec<f32>)> = BTreeMap::new();
        for (bin, (home_sums, away_sums, count)) in &sums {
            let home_means = (0..entry_length)
                .map(|idx| home_sums[idx] / *count as f32)
                .collect();

            let away_means = (0..entry_length)
                .map(|idx| away_sums[idx] / *count as f32)
                .collect();

            means.insert(*bin, (home_means, away_means));
        }

        // Second pass: sum squared deviations

        let default_entry = (
            vec![0.0f32].repeat(entry_length),
            vec![0.0f32].repeat(entry_length),
        );

        assert!(default_entry.0.len() > selectors.len());

        let mut sqdevs: BTreeMap<i32, (Vec<f32>, Vec<f32>)> = BTreeMap::new();
        for sample in samples {
            let bin = (sample.rating_diff / bin_size).round() as i32;
            let (home_means, away_means) = means[&bin].clone();
            let entry = sqdevs.entry(bin).or_insert(default_entry.clone());

            for (idx, selector) in selectors.iter().enumerate() {
                entry.0[idx] += (team_stat_sum(&sample.home_stats, &sample.home_players, selector)
                    - home_means[idx])
                    .powi(2);
                entry.1[idx] += (team_stat_sum(&sample.away_stats, &sample.away_players, selector)
                    - away_means[idx])
                    .powi(2);
            }

            // Push data from sample.action_outputs
            for &possession in [Possession::Home, Possession::Away].iter() {
                for (idx, &advantage) in [Advantage::Attack, Advantage::Neutral, Advantage::Defense]
                    .iter()
                    .enumerate()
                {
                    let advantage_count = sample
                        .action_outputs
                        .iter()
                        .filter(|output| {
                            output.possession == possession && output.advantage == advantage
                        })
                        .count() as f32;
                    if possession == Possession::Home {
                        entry.0[selectors.len() + idx] +=
                            (advantage_count - home_means[selectors.len() + idx]).powi(2);
                    } else {
                        entry.1[selectors.len() + idx] +=
                            (advantage_count - away_means[selectors.len() + idx]).powi(2);
                    }
                }

                let fastbreak_count = sample
                    .action_outputs
                    .iter()
                    .filter(|output| {
                        output.possession == possession
                            && output.situation == ActionSituation::Fastbreak
                    })
                    .count() as f32;
                if possession == Possession::Home {
                    entry.0[selectors.len() + 3] +=
                        (fastbreak_count - home_means[selectors.len() + 3]).powi(2);
                } else {
                    entry.1[selectors.len() + 3] +=
                        (fastbreak_count - away_means[selectors.len() + 3]).powi(2);
                }

                let home_sub_count = sample
                    .action_outputs
                    .iter()
                    .filter(|output| {
                        output.situation == ActionSituation::AfterSubstitution
                            && ((output.attack_stats_update.is_some()
                                && output.possession == Possession::Home)
                                || (output.defense_stats_update.is_some()
                                    && output.possession == Possession::Away))
                    })
                    .count() as f32;

                let away_sub_count = sample
                    .action_outputs
                    .iter()
                    .filter(|output| {
                        output.situation == ActionSituation::AfterSubstitution
                            && ((output.attack_stats_update.is_some()
                                && output.possession == Possession::Away)
                                || (output.defense_stats_update.is_some()
                                    && output.possession == Possession::Home))
                    })
                    .count() as f32;

                if possession == Possession::Home {
                    entry.0[selectors.len() + 4] +=
                        (home_sub_count - home_means[selectors.len() + 4]).powi(2);
                } else {
                    entry.1[selectors.len() + 4] +=
                        (away_sub_count - away_means[selectors.len() + 4]).powi(2);
                }
            }
        }

        // Final assembly: compute sample variance (N-1), stddev, and count
        let mut out = BTreeMap::new();
        for (bin, (_, _, count)) in sums {
            let (home_means, away_means) = means[&bin].clone();
            let (home_ss, away_ss) = sqdevs.get(&bin).unwrap();
            let home_variances = home_ss
                .iter()
                .map(|s| {
                    if count > 1 {
                        (s / (count as f32 - 1.0)).sqrt()
                    } else {
                        0.0
                    }
                })
                .collect_vec();
            let away_variances = away_ss
                .iter()
                .map(|s| {
                    if count > 1 {
                        (s / (count as f32 - 1.0)).sqrt()
                    } else {
                        0.0
                    }
                })
                .collect_vec();

            let bin_center = (bin as f32 * bin_size) as i32;
            out.insert(
                bin_center,
                (
                    (home_means, home_variances),
                    (away_means, away_variances),
                    count,
                ),
            );
        }
        out
    }

    fn print_stats_report(result: &MatchupResult, cutoff: usize) {
        println!(
            "Result for {} vs {}",
            result.home_tactic, result.away_tactic
        );

        for bin in &result.bins {
            let count = bin.count;
            if count < cutoff {
                continue;
            }
            println!("Δrating={:+2} ({} samples)", bin.center, count);

            println!(
                "  Win% = {:3.1} ± {:3.1} ({}/{})",
                100.0 * bin.win_count as f32 / count as f32,
                100.0
                    * (((bin.win_count + 1) * (count - bin.win_count + 1)) as f32
                        / ((count + 2).pow(2) * (count + 3)) as f32)
                        .sqrt(),
                bin.win_count,
                count
            );
            println!(
                "  Loss% = {:3.1} ± {:3.1} ({}/{})",
                100.0 * bin.loss_count as f32 / count as f32,
                100.0
                    * (((bin.loss_count + 1) * (count - bin.loss_count + 1)) as f32
                        / ((count + 2).pow(2) * (count + 3)) as f32)
                        .sqrt(),
                bin.loss_count,
                count
            );
            println!(
                "  Draw% = {:3.1} ± {:3.1} ({}/{})",
                100.0 * (bin.draw_count + 1) as f32 / (count + 2) as f32,
                100.0
                    * (((bin.draw_count + 1) * (count - bin.draw_count + 1)) as f32
                        / ((count + 2).pow(2) * (count + 3)) as f32)
                        .sqrt(),
                bin.draw_count,
                count
            );
            println!(
                "  points = {:3.1} ± {:3.1} vs {:3.1} ± {:3.1}",
                bin.home_avg[0], bin.home_std[0], bin.away_avg[0], bin.away_std[0],
            );
            println!(
                "  2pt = {:3.1}/{:3.1} ± {:3.1}/{:3.1} vs {:3.1}/{:3.1} ± {:3.1}/{:3.1}",
                bin.home_avg[1],
                bin.home_avg[2],
                bin.home_std[1],
                bin.home_std[2],
                bin.away_avg[1],
                bin.away_avg[2],
                bin.away_std[1],
                bin.away_std[2],
            );
            println!(
                "  3pt = {:3.1}/{:3.1} ± {:3.1}/{:3.1} vs {:3.1}/{:3.1} ± {:3.1}/{:3.1}",
                bin.home_avg[3],
                bin.home_avg[4],
                bin.home_std[3],
                bin.home_std[4],
                bin.away_avg[3],
                bin.away_avg[4],
                bin.away_std[3],
                bin.away_std[4],
            );

            println!(
                "  Def/Off Rebounds = {:3.1}/{:3.1} ± {:3.1}/{:3.1} vs {:3.1}/{:3.1} ± {:3.1}/{:3.1}",
                bin.home_avg[5],
                bin.home_avg[6],
                bin.home_std[5],
                bin.home_std[6],
                bin.away_avg[5],
                bin.away_avg[6],
                bin.away_std[5],
                bin.away_std[6],
            );

            println!(
                "  Assists/Turnovers = {:3.1}/{:3.1} ± {:3.1}/{:3.1} vs {:3.1}/{:3.1} ± {:3.1}/{:3.1}",
                bin.home_avg[7],
                bin.home_avg[8],
                bin.home_std[7],
                bin.home_std[8],
                bin.away_avg[7],
                bin.away_avg[8],
                bin.away_std[7],
                bin.away_std[8],
            );

            println!(
                "  Steals/Blocks = {:3.1}/{:3.1} ± {:3.1}/{:3.1} vs {:3.1}/{:3.1} ± {:3.1}/{:3.1}",
                bin.home_avg[9],
                bin.home_avg[10],
                bin.home_std[9],
                bin.home_std[10],
                bin.away_avg[9],
                bin.away_avg[10],
                bin.away_std[9],
                bin.away_std[10],
            );

            println!(
                "  Brawls = {:3.1} ± {:3.1} vs {:3.1} ± {:3.1}",
                bin.home_avg[11], bin.home_std[11], bin.away_avg[11], bin.away_std[11],
            );

            println!(
                "  Fastbreaks = {:3.1} ± {:3.1} vs {:3.1} ± {:3.1}",
                bin.home_avg[17], bin.home_std[17], bin.away_avg[17], bin.away_std[17],
            );

            println!(
                "  Substitutions = {:3.1} ± {:3.1} vs {:3.1} ± {:3.1}",
                bin.home_avg[18], bin.home_std[18], bin.away_avg[18], bin.away_std[18],
            );

            println!(
                "  Tiredness = {:3.1} ± {:3.1} vs {:3.1} ± {:3.1}",
                bin.home_avg[12] / MAX_PLAYERS_PER_GAME as f32,
                bin.home_std[12] / MAX_PLAYERS_PER_GAME as f32,
                bin.away_avg[12] / MAX_PLAYERS_PER_GAME as f32,
                bin.away_std[12] / MAX_PLAYERS_PER_GAME as f32,
            );
            println!(
                "  Morale = {:3.1} ± {:3.1} vs {:3.1} ± {:3.1}",
                bin.home_avg[13] / MAX_PLAYERS_PER_GAME as f32,
                bin.home_std[13] / MAX_PLAYERS_PER_GAME as f32,
                bin.away_avg[13] / MAX_PLAYERS_PER_GAME as f32,
                bin.away_std[13] / MAX_PLAYERS_PER_GAME as f32,
            );

            println!(
                "  Advantage = {:3.1}/{:3.1}/{:3.1} vs {:3.1}/{:3.1}/{:3.1}",
                bin.home_avg[14],
                bin.home_avg[15],
                bin.home_avg[16],
                bin.away_avg[14],
                bin.away_avg[15],
                bin.away_avg[16],
            );

            println!("");
        }
    }

    #[ignore]
    #[test]
    fn test_multiple_games() -> AppResult<()> {
        const N: usize = 6_000;
        const BIN_SIZE: f32 = 1.0;
        let max_delta_rating: f32 = 3.0;
        let with_fixed_stamina = None; //Some(10.0);

        let tactic_pairs = Tactic::iter()
            .enumerate()
            .flat_map(|(i, home)| {
                Tactic::iter()
                    .enumerate()
                    .filter(move |(j, _)| j >= &i) // <-- keep only pairs where j >= i
                    .map(move |(_, away)| (home, away))
            })
            .collect_vec();

        let results: Vec<MatchupResult> = tactic_pairs
            .par_iter()
            .map(|&(home_tactic, away_tactic)| {
                let samples = get_simulated_game_samples(
                    N,
                    max_delta_rating,
                    home_tactic,
                    away_tactic,
                    with_fixed_stamina,
                );
                let bins = process_stats(&samples, BIN_SIZE);
                MatchupResult {
                    home_tactic,
                    away_tactic,
                    bins,
                }
            })
            .collect();

        let cutoff = N / 10;
        for result in &results {
            print_stats_report(result, cutoff);
        }

        Ok(())
    }
}

//cargo test test_multiple_games -- --nocapture --ignored > tests/game_engine_data/game_stats_v1.5.x.data
