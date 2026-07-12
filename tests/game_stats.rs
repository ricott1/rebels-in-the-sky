#[cfg(test)]
mod tests {
    use itertools::Itertools;
    use rayon::prelude::*;
    use rebels::core::{Player, Rated, Skill, Team, TickInterval, MAX_PLAYERS_PER_GAME};
    use rebels::game_engine::action::{ActionOutput, ActionSituation, Advantage};
    use rebels::game_engine::game::Game;
    use rebels::game_engine::tactic::Tactic;
    use rebels::game_engine::types::{
        GamePositionFluidity, GameStats, GameStatsMap, Possession, SubstitutionTendency, TeamInGame,
    };
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
        home_team_settings: (Tactic, SubstitutionTendency, GamePositionFluidity),
        away_team_settings: (Tactic, SubstitutionTendency, GamePositionFluidity),
        bins: Vec<BinResult>,
    }

    struct GameSample {
        rating_diff: f32,
        winner: Option<Possession>,
        home_values: Vec<f32>,
        away_values: Vec<f32>,
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

        let binned = compute_binned_stats(samples, bin_size);

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

    /// Compute the per-team entry vector for a finished game:
    /// 14 stat selectors, then attack/neutral/defense advantage counts,
    /// fastbreak count and substitution count.
    fn team_values(
        stats: &GameStatsMap,
        players: &PlayerMap,
        action_outputs: &[ActionOutput],
        possession: Possession,
    ) -> Vec<f32> {
        let selectors: Vec<fn(&GameStats, &Player) -> f32> = vec![
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

        let mut values: Vec<f32> = selectors
            .iter()
            .map(|&selector| team_stat_sum(stats, players, selector))
            .collect();

        for &advantage in [Advantage::Attack, Advantage::Neutral, Advantage::Defense].iter() {
            let advantage_count = action_outputs
                .iter()
                .filter(|output| output.possession == possession && output.advantage == advantage)
                .count() as f32;
            values.push(advantage_count);
        }

        let fastbreak_count = action_outputs
            .iter()
            .filter(|output| {
                output.possession == possession && output.situation == ActionSituation::Fastbreak
            })
            .count() as f32;
        values.push(fastbreak_count);

        let substitution_count = action_outputs
            .iter()
            .filter(|output| {
                output.situation == ActionSituation::AfterSubstitution
                    && ((output.attack_stats_update.is_some() && output.possession == possession)
                        || (output.defense_stats_update.is_some()
                            && output.possession == !possession))
            })
            .count() as f32;
        values.push(substitution_count);

        values
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
        home_team_settings: (Tactic, SubstitutionTendency, GamePositionFluidity),
        away_team_settings: (Tactic, SubstitutionTendency, GamePositionFluidity),
        with_fixed_stamina: Option<Skill>,
    ) -> Vec<GameSample> {
        let mut samples = Vec::with_capacity(n_games);
        for i in 0..n_games {
            let sign = if i % 2 == 0 { 1.0 } else { -1.0 };
            let home_team_base_level =
                if max_delta_rating > 0.0 && i <= n_games / (2 * max_delta_rating as usize) {
                    0.0
                } else {
                    sign * max_delta_rating * i as f32 / n_games as f32
                };
            let away_team_base_level =
                if max_delta_rating > 0.0 && i <= n_games / (2 * max_delta_rating as usize) {
                    0.0
                } else {
                    -sign * max_delta_rating * i as f32 / n_games as f32
                };

            let (mut home_team_in_game, mut away_team_in_game) =
                if home_team_base_level == away_team_base_level {
                    generate_identical_team_in_game(home_team_base_level, with_fixed_stamina)
                } else {
                    (
                        generate_team_in_game(home_team_base_level, with_fixed_stamina),
                        generate_team_in_game(away_team_base_level, with_fixed_stamina),
                    )
                };

            let rating_diff = home_team_in_game.rating() - away_team_in_game.rating();

            home_team_in_game.tactic = home_team_settings.0;
            home_team_in_game.substitution_tendency = home_team_settings.1;
            home_team_in_game.game_position_fluidity = home_team_settings.2;
            away_team_in_game.tactic = away_team_settings.0;
            away_team_in_game.substitution_tendency = away_team_settings.1;
            away_team_in_game.game_position_fluidity = away_team_settings.2;
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

            let home_values = team_values(
                &game.home_team_in_game.stats,
                &game.home_team_in_game.players,
                &game.action_results,
                Possession::Home,
            );
            let away_values = team_values(
                &game.away_team_in_game.stats,
                &game.away_team_in_game.players,
                &game.action_results,
                Possession::Away,
            );

            samples.push(GameSample {
                rating_diff,
                winner,
                home_values,
                away_values,
            })
        }

        samples
    }

    fn compute_binned_stats(
        samples: &[GameSample],
        bin_size: f32,
    ) -> BTreeMap<i32, ((Vec<f32>, Vec<f32>), (Vec<f32>, Vec<f32>), usize)>
// Returns bin_center --> ((home mean, home stddev) for each entry, (away mean, away stddev) for each entry, count)
    {
        let entry_length = match samples.first() {
            Some(sample) => sample.home_values.len(),
            None => return BTreeMap::new(),
        };

        // First pass: sum and count for each entry
        let default_entry = (
            vec![0.0f32; entry_length],
            vec![0.0f32; entry_length],
            0usize,
        );
        let mut sums: BTreeMap<i32, (Vec<f32>, Vec<f32>, usize)> = BTreeMap::new();
        for sample in samples {
            let bin = (sample.rating_diff / bin_size).round() as i32;
            let entry = sums.entry(bin).or_insert(default_entry.clone());
            for idx in 0..entry_length {
                entry.0[idx] += sample.home_values[idx];
                entry.1[idx] += sample.away_values[idx];
            }
            entry.2 += 1;
        }

        // Means
        let mut means: BTreeMap<i32, (Vec<f32>, Vec<f32>)> = BTreeMap::new();
        for (bin, (home_sums, away_sums, count)) in &sums {
            let home_means = home_sums.iter().map(|s| s / *count as f32).collect();
            let away_means = away_sums.iter().map(|s| s / *count as f32).collect();
            means.insert(*bin, (home_means, away_means));
        }

        // Second pass: sum squared deviations
        let default_entry = (vec![0.0f32; entry_length], vec![0.0f32; entry_length]);
        let mut sqdevs: BTreeMap<i32, (Vec<f32>, Vec<f32>)> = BTreeMap::new();
        for sample in samples {
            let bin = (sample.rating_diff / bin_size).round() as i32;
            let (home_means, away_means) = &means[&bin];
            let entry = sqdevs.entry(bin).or_insert(default_entry.clone());
            for idx in 0..entry_length {
                entry.0[idx] += (sample.home_values[idx] - home_means[idx]).powi(2);
                entry.1[idx] += (sample.away_values[idx] - away_means[idx]).powi(2);
            }
        }

        // Final assembly: compute sample variance (N-1), stddev, and count
        let mut out = BTreeMap::new();
        for (bin, (_, _, count)) in sums {
            let (home_means, away_means) = means[&bin].clone();
            let (home_ss, away_ss) = sqdevs.get(&bin).unwrap();
            let home_stds = home_ss
                .iter()
                .map(|s| {
                    if count > 1 {
                        (s / (count as f32 - 1.0)).sqrt()
                    } else {
                        0.0
                    }
                })
                .collect_vec();
            let away_stds = away_ss
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
                ((home_means, home_stds), (away_means, away_stds), count),
            );
        }
        out
    }

    fn print_stats_report(result: &MatchupResult, cutoff: usize) {
        println!(
            "Result for {:#?} vs {:#?}",
            result.home_team_settings, result.away_team_settings
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
        const N: usize = 30_000;
        const BIN_SIZE: f32 = 1.0;
        let max_delta_rating: f32 = 2.0;
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
                let home_team_settings = (
                    home_tactic,
                    SubstitutionTendency::Normal,
                    GamePositionFluidity::Normal,
                );
                let away_team_settings = (
                    away_tactic,
                    SubstitutionTendency::Normal,
                    GamePositionFluidity::Normal,
                );
                let samples = get_simulated_game_samples(
                    N,
                    max_delta_rating,
                    home_team_settings,
                    away_team_settings,
                    with_fixed_stamina,
                );
                let bins = process_stats(&samples, BIN_SIZE);

                MatchupResult {
                    home_team_settings,
                    away_team_settings,
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
