use crate::{
    game_engine::{
        game::{Game, GameSummary},
        types::Possession,
    },
    types::{SystemTimeTick, TeamId, Tick},
    ui::constants::UiStyle,
};
use itertools::Itertools;
use ratatui::{
    style::Style,
    text::{Line, Span},
};

const MAX_ROUND_LINES: usize = 8;
const BLOCK_HEIGHT: usize = 3;
const GAP: usize = 1;
const COL_WIDTH: usize = 20;
const ROUND_LINES_LEN: usize = MAX_ROUND_LINES * (BLOCK_HEIGHT + GAP);

struct TournamentDescription {
    home_team_name: String,
    home_team_style: Style,
    away_team_name: String,
    away_team_style: Style,
    result: String,
    winner: Option<Possession>,
}

impl TournamentDescription {
    fn winner_name(&self) -> Option<&str> {
        self.winner.map(|p| match p {
            Possession::Home => self.home_team_name.as_str(),
            Possession::Away => self.away_team_name.as_str(),
        })
    }
}

trait TournamentDescriptionTrait {
    fn tournament_description(&self, own_team_id: TeamId, timestamp: Tick)
        -> TournamentDescription;
}

impl TournamentDescriptionTrait for Game {
    fn tournament_description(
        &self,
        own_team_id: TeamId,
        timestamp: Tick,
    ) -> TournamentDescription {
        let score = self.get_score();

        let home_team_name = self.home_team_in_game.name.to_string();
        let home_team_style = if self.home_team_in_game.team_id == own_team_id {
            UiStyle::OWN_TEAM
        } else if self.is_network() {
            UiStyle::NETWORK
        } else {
            UiStyle::DEFAULT
        };

        let away_team_name = self.away_team_in_game.name.to_string();
        let away_team_style = if self.away_team_in_game.team_id == own_team_id {
            UiStyle::OWN_TEAM
        } else if self.is_network() {
            UiStyle::NETWORK
        } else {
            UiStyle::DEFAULT
        };

        let result = if self.has_started(timestamp) {
            format!("{} {:>3}-{:<3}", self.timer.format(), score.0, score.1)
        } else {
            let countdown = (self.starting_at.saturating_sub(timestamp)).formatted();
            format!("Starting in {}", countdown)
        };

        TournamentDescription {
            home_team_name,
            home_team_style,
            away_team_name,
            away_team_style,
            result,
            winner: self.winner.map(|id| {
                if id == self.home_team_in_game.team_id {
                    Possession::Home
                } else {
                    Possession::Away
                }
            }),
        }
    }
}

impl TournamentDescriptionTrait for GameSummary {
    fn tournament_description(
        &self,
        own_team_id: TeamId,
        _timestamp: Tick,
    ) -> TournamentDescription {
        let home_team_name = self.home_team_name.clone();
        let home_team_style = if self.home_team_id == own_team_id {
            UiStyle::OWN_TEAM
        } else if self.is_network {
            UiStyle::NETWORK
        } else {
            UiStyle::DEFAULT
        };

        let away_team_name = self.away_team_name.clone();
        let away_team_style = if self.away_team_id == own_team_id {
            UiStyle::OWN_TEAM
        } else if self.is_network {
            UiStyle::NETWORK
        } else {
            UiStyle::DEFAULT
        };

        let score = self.get_score();
        let result = format!("{:>3}-{:<3}", score.0, score.1);

        TournamentDescription {
            home_team_name,
            home_team_style,
            away_team_name,
            away_team_style,
            result,
            winner: self.winner.map(|id| {
                if id == self.home_team_id {
                    Possession::Home
                } else {
                    Possession::Away
                }
            }),
        }
    }
}

pub fn number_of_rounds(num_participants: usize) -> usize {
    (num_participants as f32).log2().ceil() as usize
}

pub fn current_round(num_participants: usize, games_completed: usize) -> usize {
    let round_sizes = compute_round_sizes(num_participants);
    let mut counter = 0;
    for (idx, round_size) in round_sizes.iter().enumerate() {
        counter += round_size;

        if games_completed < counter {
            return idx;
        }
    }

    round_sizes.len() - 1
}

pub fn compute_round_sizes(participants: usize) -> Vec<usize> {
    let num_rounds = number_of_rounds(participants);
    let mut rounds = Vec::with_capacity(num_rounds);
    let next_pot = 1usize << (num_rounds - 1);
    rounds.push(participants - next_pot);
    let mut remaining = next_pot;
    while remaining > 1 {
        rounds.push(remaining / 2);
        remaining /= 2;
    }
    rounds
}

fn get_round_lines(
    round_idx: usize,
    round_description: Vec<TournamentDescription>,
    bracket_positions: &[usize],
) -> Vec<Line<'static>> {
    let mut lines: Vec<Line> = (0..ROUND_LINES_LEN).map(|_| Line::default()).collect_vec();

    let num_blank_lines = (1usize << round_idx).saturating_sub(1);

    for (
        idx,
        TournamentDescription {
            home_team_name,
            home_team_style,
            away_team_name,
            away_team_style,
            result,
            winner,
        },
    ) in round_description.into_iter().enumerate()
    {
        let bracket_idx = bracket_positions[idx];
        let stride = (BLOCK_HEIGHT + GAP) * (1 << round_idx);
        let central_line_index = bracket_idx * stride + stride / 2 - GAP / 2;
        let l = COL_WIDTH.saturating_sub(home_team_name.len()) / 2;

        let style = if matches!(winner, Some(p) if p == Possession::Home) {
            UiStyle::OK
        } else {
            UiStyle::DEFAULT
        };
        lines[central_line_index - num_blank_lines - 1] = Line::from(vec![
            Span::styled("═".repeat(l), style),
            Span::raw(" "),
            Span::styled(home_team_name.clone(), home_team_style),
            Span::raw(" "),
            Span::styled(
                "═".repeat(l + COL_WIDTH.saturating_sub(home_team_name.len()) % 2),
                style,
            ),
            Span::styled("╗   ", style),
        ]);

        let blank_line = Line::from(vec![
            Span::raw(" ".repeat(COL_WIDTH + 2)),
            Span::styled("║", style),
        ]);

        for b_idx in 0..num_blank_lines {
            lines[central_line_index - num_blank_lines + b_idx] = blank_line.clone();
        }

        let result_width = COL_WIDTH + 2;

        if matches!(winner, Some(p)  if p == Possession::Home) {
            lines[central_line_index] = Line::from(vec![
                Span::raw(format!("{:^result_width$}", result.clone())),
                Span::styled("╚═", UiStyle::OK),
            ]);
        } else if matches!(winner, Some(p)  if p == Possession::Away) {
            lines[central_line_index] = Line::from(vec![
                Span::raw(format!("{:^result_width$}", result.clone())),
                Span::styled("╔═", UiStyle::OK),
            ]);
        } else {
            lines[central_line_index] = Line::from(vec![
                Span::raw(format!("{:^result_width$}", result.clone())),
                Span::raw("╠═"),
            ]);
        };

        let style = if matches!(winner,Some(p)  if p == Possession::Away) {
            UiStyle::OK
        } else {
            UiStyle::DEFAULT
        };

        let blank_line = Line::from(vec![
            Span::raw(" ".repeat(COL_WIDTH + 2)),
            Span::styled("║", style),
        ]);
        for b_idx in 0..num_blank_lines {
            lines[central_line_index + 1 + b_idx] = blank_line.clone();
        }
        let l = COL_WIDTH.saturating_sub(away_team_name.len()) / 2;

        lines[central_line_index + num_blank_lines + 1] = Line::from(vec![
            Span::styled("═".repeat(l), style),
            Span::raw(" "),
            Span::styled(away_team_name.clone(), away_team_style),
            Span::raw(" "),
            Span::styled(
                "═".repeat(l + COL_WIDTH.saturating_sub(away_team_name.len()) % 2),
                style,
            ),
            Span::styled("╝   ", style),
        ]);
    }

    lines
}

pub fn get_bracket_lines(
    winner_team_name: Option<&str>,
    num_participants: usize,
    active_games: &[&Game],
    past_game_summaries: &[&GameSummary],
    own_team_id: TeamId,
    timestamp: Tick,
) -> Vec<Vec<Line<'static>>> {
    // Round sizes define the tournament structure.
    // For instance, 5 participants would give round_sizes = [1, 2, 1];
    let round_sizes = compute_round_sizes(num_participants);
    let num_round = round_sizes.len();

    // We start filling in the descriptions using the past_game_summaries (older active_games).
    let all_games_len = past_game_summaries.len() + active_games.len();
    let mut idx = 0;
    let mut round_descriptions: Vec<Vec<TournamentDescription>> = Vec::with_capacity(num_round);

    for (_round_idx, round_size) in round_sizes.iter().copied().enumerate() {
        let mut round_description = vec![];

        for _ in 0..round_size {
            let description = if idx < past_game_summaries.len() {
                let game = past_game_summaries[idx];
                game.tournament_description(own_team_id, timestamp)
            } else if idx < all_games_len {
                let game = active_games[idx - past_game_summaries.len()];
                game.tournament_description(own_team_id, timestamp)
            } else {
                break;
            };
            round_description.push(description);
            idx += 1;
        }

        if round_description.is_empty() {
            break;
        }
        round_descriptions.push(round_description);
    }

    // Backward pass: reorder game blocks within each round so that winners flow
    // correctly into the next round's bracket positions.
    // In a binary bracket, round R positions 2*p and 2*p+1 feed into round R+1 position p.
    // The winner at position 2*p becomes the home (top) team, 2*p+1 becomes away (bottom).
    for r in (0..round_descriptions.len().saturating_sub(1)).rev() {
        let next_round_teams: Vec<(String, String)> = round_descriptions[r + 1]
            .iter()
            .map(|d| (d.home_team_name.clone(), d.away_team_name.clone()))
            .collect();

        let current = &mut round_descriptions[r];
        let len = current.len();
        let mut target: Vec<Option<usize>> = vec![None; len];
        let mut used = vec![false; len];

        for (p, (home_name, away_name)) in next_round_teams.iter().enumerate() {
            let top_pos = 2 * p;
            let bot_pos = 2 * p + 1;

            if top_pos < len {
                if let Some(i) = current
                    .iter()
                    .enumerate()
                    .position(|(i, g)| !used[i] && g.winner_name() == Some(home_name.as_str()))
                {
                    target[top_pos] = Some(i);
                    used[i] = true;
                }
            }

            if bot_pos < len {
                if let Some(i) = current
                    .iter()
                    .enumerate()
                    .position(|(i, g)| !used[i] && g.winner_name() == Some(away_name.as_str()))
                {
                    target[bot_pos] = Some(i);
                    used[i] = true;
                }
            }
        }

        // Fill remaining positions with unplaced games (preserving original order).
        let mut unplaced: Vec<usize> = (0..len).filter(|i| !used[*i]).collect();
        for pos in 0..len {
            if target[pos].is_none() {
                if let Some(i) = unplaced.pop() {
                    target[pos] = Some(i);
                }
            }
        }

        // Apply the reordering.
        let mut old: Vec<Option<TournamentDescription>> =
            std::mem::take(current).into_iter().map(Some).collect();
        *current = target
            .into_iter()
            .filter_map(|opt| old.get_mut(opt?).and_then(|slot| slot.take()))
            .collect();
    }

    // Compute bracket positions for each round.
    // For the play-in round (round 0 when it has fewer games than the expected next round),
    // we compute positions by matching each game's winner to the next round's teams.
    // For full rounds, positions are sequential 0..n.
    let mut all_bracket_positions: Vec<Vec<usize>> = Vec::with_capacity(round_descriptions.len());
    for (round_idx, round_desc) in round_descriptions.iter().enumerate() {
        // Use the expected next round size from round_sizes, not the actual (possibly partial) size.
        let expected_next_size = round_sizes.get(round_idx + 1).copied().unwrap_or(0);
        let full_size = expected_next_size * 2;

        if round_idx == 0
            && round_idx + 1 < round_descriptions.len()
            && round_desc.len() < full_size
        {
            // Play-in round with gaps: match each game to its next-round position.
            let next_round = &round_descriptions[round_idx + 1];
            let mut positions = Vec::with_capacity(round_desc.len());
            let mut used_positions = vec![false; full_size];

            for desc in round_desc.iter() {
                let mut found = false;

                // Try matching winner name to next round's home/away teams.
                if let Some(winner) = desc.winner_name() {
                    for (p, next_desc) in next_round.iter().enumerate() {
                        if !used_positions[2 * p] && next_desc.home_team_name == winner {
                            used_positions[2 * p] = true;
                            positions.push(2 * p);
                            found = true;
                            break;
                        }
                        if !used_positions[2 * p + 1] && next_desc.away_team_name == winner {
                            used_positions[2 * p + 1] = true;
                            positions.push(2 * p + 1);
                            found = true;
                            break;
                        }
                    }
                }

                // For in-progress games, try matching team names.
                if !found {
                    for (p, next_desc) in next_round.iter().enumerate() {
                        if !used_positions[2 * p]
                            && (next_desc.home_team_name == desc.home_team_name
                                || next_desc.home_team_name == desc.away_team_name)
                        {
                            used_positions[2 * p] = true;
                            positions.push(2 * p);
                            found = true;
                            break;
                        }
                        if !used_positions[2 * p + 1]
                            && (next_desc.away_team_name == desc.home_team_name
                                || next_desc.away_team_name == desc.away_team_name)
                        {
                            used_positions[2 * p + 1] = true;
                            positions.push(2 * p + 1);
                            found = true;
                            break;
                        }
                    }
                }

                // Fallback: place at a position feeding into an unoccupied next-round slot.
                // Prefer slots beyond existing next-round games (those games are independent).
                if !found {
                    let pos = (0..full_size)
                        .rev()
                        .find(|p| !used_positions[*p])
                        .unwrap_or(0);
                    used_positions[pos] = true;
                    positions.push(pos);
                }
            }

            all_bracket_positions.push(positions);
        } else {
            all_bracket_positions.push((0..round_desc.len()).collect());
        }
    }

    // Render each round.
    let mut lines = Vec::with_capacity(round_descriptions.len());
    for (round_idx, round_description) in round_descriptions.into_iter().enumerate() {
        let positions = &all_bracket_positions[round_idx];
        lines.push(get_round_lines(round_idx, round_description, positions));
    }

    if let Some(winner) = winner_team_name {
        let mut winner_lines: Vec<Line> =
            (0..ROUND_LINES_LEN).map(|_| Line::default()).collect_vec();

        let stride = (BLOCK_HEIGHT + GAP) * (1 << (num_round - 1));
        let central_line_index = stride / 2 - GAP / 2;

        winner_lines[central_line_index] = Line::from(vec![
            Span::styled("══ ", UiStyle::OK),
            Span::raw(winner.to_string()),
        ]);
        lines.push(winner_lines);
    }

    lines
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::{
        core::TickInterval,
        game_engine::Tournament,
        types::{AppResult, GameMap, GameSummaryMap, SystemTimeTick, TeamId, Tick},
    };
    use itertools::Itertools;
    use ratatui::crossterm::event::{self, Event};
    use ratatui::{
        layout::{Constraint, Layout},
        widgets::Paragraph,
    };
    use ratatui::{DefaultTerminal, Frame};

    fn render(
        frame: &mut Frame,
        tournament: &Tournament,
        own_team_id: TeamId,
        active_games: Vec<&Game>,
        past_game_summaries: Vec<&GameSummary>,
        current_tick: Tick,
    ) {
        let num_participants = tournament.participants.len();
        let number_of_rounds = number_of_rounds(num_participants);
        let current_round = current_round(num_participants, past_game_summaries.len()) + 1;

        let split =
            Layout::vertical([Constraint::Length(3), Constraint::Fill(1)]).split(frame.area());

        let brackets_split =
            Layout::horizontal([Constraint::Length(24)].repeat(number_of_rounds + 1))
                .split(split[1]);

        let brackets = super::get_bracket_lines(
            tournament.winner.map(|id| {
                tournament
                    .participants
                    .get(&id)
                    .expect("Winner should be a participant")
                    .name
                    .as_str()
            }),
            num_participants,
            &active_games,
            &past_game_summaries,
            own_team_id,
            current_tick,
        );

        frame.render_widget(
            Paragraph::new(format!(
                "Rounds {}/{} - tick {} - brackets {}",
                current_round,
                number_of_rounds,
                current_tick.formatted_as_time(),
                brackets.len()
            )),
            split[0],
        );
        for (round_idx, lines) in brackets.iter().enumerate() {
            frame.render_widget(Paragraph::new(lines.clone()), brackets_split[round_idx]);
        }
    }

    #[test]
    #[ignore]
    fn test_rendering_live_tournament() -> AppResult<()> {
        fn run(mut terminal: DefaultTerminal) -> AppResult<()> {
            let mut tournament = Tournament::test(11, 16);
            let mut games = GameMap::new();
            let mut past_games = GameSummaryMap::new();

            for game in tournament.initialize() {
                games.insert(game.id, game);
            }

            let mut current_tick = Tick::now();

            while !tournament.has_ended() {
                for game in games.values_mut() {
                    game.tick(current_tick);
                }

                let new_games =
                    tournament.generate_next_games(current_tick, &games, &past_games)?;
                let own_team_id = tournament.participants.keys().collect_vec()[0].clone();

                let active_games = tournament.active_games(&games);
                let game_summaries = tournament.past_game_summaries(&past_games);

                if let Ok(true) = event::poll(std::time::Duration::from_millis(2)) {
                    if matches!(event::read().unwrap(), Event::Key(_)) {
                        break;
                    }
                }

                terminal.draw(|frame| {
                    render(
                        frame,
                        &tournament,
                        own_team_id,
                        active_games,
                        game_summaries,
                        current_tick,
                    );
                })?;

                for game in games.values().filter(|g| g.has_ended()) {
                    past_games.insert(game.id, GameSummary::from_game(game));
                }

                games.retain(|_, g| !g.has_ended());

                for game in new_games {
                    games.insert(game.id, game);
                }

                current_tick += TickInterval::SHORT;

                if tournament.has_ended() {
                    if matches!(event::read().unwrap(), Event::Key(_)) {
                        break;
                    }
                }
            }

            Ok(())
        }

        let terminal = ratatui::init();
        let result = run(terminal);
        ratatui::restore();
        result
    }

    #[test]
    #[ignore]
    fn test_rendering_ended_tournament() -> AppResult<()> {
        fn run() -> AppResult<()> {
            let mut tournament = Tournament::test(7, 16);
            let own_team_id = tournament.participants.keys().collect_vec()[0].clone();
            let mut games = GameMap::new();
            let mut past_games = GameSummaryMap::new();

            for game in tournament.initialize() {
                games.insert(game.id, game);
            }

            let mut current_tick = tournament.registrations_closing_at;

            while !tournament.has_ended() {
                for game in games.values_mut() {
                    game.tick(current_tick);
                }

                let new_games =
                    tournament.generate_next_games(current_tick, &games, &past_games)?;

                for game in games.values().filter(|g| g.has_ended()) {
                    past_games.insert(game.id, GameSummary::from_game(game));
                }

                games.retain(|_, g| !g.has_ended());

                for game in new_games {
                    games.insert(game.id, game);
                }

                current_tick += TickInterval::SHORT;
            }

            let mut terminal = ratatui::init();

            loop {
                let active_games = tournament.active_games(&games);
                let game_summaries = tournament.past_game_summaries(&past_games);
                assert!(active_games.is_empty());

                terminal.draw(|frame| {
                    render(
                        frame,
                        &tournament,
                        own_team_id,
                        active_games,
                        game_summaries,
                        current_tick,
                    );
                })?;
                if matches!(event::read().unwrap(), Event::Key(_)) {
                    break;
                }
            }

            Ok(())
        }

        let result = run();
        ratatui::restore();
        result
    }

    #[test]
    fn test_number_of_rounds() {
        assert_eq!(number_of_rounds(2), 1);
        assert_eq!(number_of_rounds(3), 2);
        assert_eq!(number_of_rounds(4), 2);
        assert_eq!(number_of_rounds(5), 3);
        assert_eq!(number_of_rounds(6), 3);
        assert_eq!(number_of_rounds(7), 3);
        assert_eq!(number_of_rounds(8), 3);
    }

    #[test]
    fn test_compute_round_sizes_basic() {
        assert_eq!(compute_round_sizes(2), vec![1]);
        assert_eq!(compute_round_sizes(3), vec![1, 1]);
        assert_eq!(compute_round_sizes(4), vec![2, 1]);
        assert_eq!(compute_round_sizes(5), vec![1, 2, 1]);
        assert_eq!(compute_round_sizes(6), vec![2, 2, 1]);
        assert_eq!(compute_round_sizes(7), vec![3, 2, 1]);
        assert_eq!(compute_round_sizes(8), vec![4, 2, 1]);
        assert_eq!(compute_round_sizes(9), vec![1, 4, 2, 1]);
        assert_eq!(compute_round_sizes(10), vec![2, 4, 2, 1]);
        assert_eq!(compute_round_sizes(11), vec![3, 4, 2, 1]);
        assert_eq!(compute_round_sizes(12), vec![4, 4, 2, 1]);
        assert_eq!(compute_round_sizes(13), vec![5, 4, 2, 1]);
        assert_eq!(compute_round_sizes(14), vec![6, 4, 2, 1]);
        assert_eq!(compute_round_sizes(15), vec![7, 4, 2, 1]);
        assert_eq!(compute_round_sizes(16), vec![8, 4, 2, 1]);
    }

    #[test]
    fn test_compute_round_sizes_sum_matches_games() {
        for participants in 2..=8 {
            let sizes = compute_round_sizes(participants);
            let total_games: usize = sizes.iter().sum();

            // In a single-elimination tournament:
            assert_eq!(total_games, participants - 1);
        }
    }

    #[test]
    fn test_current_round_progression_4_participants() {
        // 4 participants -> round sizes [2, 1]
        // games_completed: 0..=3

        assert_eq!(current_round(4, 0), 0); // no games done, in round 0
        assert_eq!(current_round(4, 1), 0); // 1 of 2 round-0 games done
        assert_eq!(current_round(4, 2), 1); // round 0 complete, in round 1
        assert_eq!(current_round(4, 3), 1); // all done, last round
    }

    #[test]
    fn test_current_round_progression_5_participants() {
        // 5 participants -> round sizes [1, 2, 1]
        assert_eq!(current_round(5, 0), 0); // no games done, in play-in
        assert_eq!(current_round(5, 1), 1); // play-in complete, in semis

        assert_eq!(current_round(5, 2), 1); // 1 of 2 semis done
        assert_eq!(current_round(5, 3), 2); // semis complete, in final

        assert_eq!(current_round(5, 4), 2); // all done, last round
    }

    #[test]
    fn test_current_round_last_round_when_all_games_played() {
        for participants in 2..=8 {
            let sizes = compute_round_sizes(participants);
            let total_games: usize = sizes.iter().sum();
            let last_round = sizes.len() - 1;

            assert_eq!(current_round(participants, total_games), last_round);
        }
    }

    #[test]
    fn test_round_sizes_length_matches_number_of_rounds() {
        for participants in 2..=8 {
            assert_eq!(
                compute_round_sizes(participants).len(),
                number_of_rounds(participants)
            );
        }
    }

    #[test]
    fn test_get_bracket_lines_all_sizes() -> AppResult<()> {
        for n in 2..=8usize {
            let mut tournament = Tournament::test(n, 8);
            let mut games = GameMap::new();
            let mut past_games = GameSummaryMap::new();

            for game in tournament.initialize() {
                games.insert(game.id, game);
            }

            let mut current_tick = tournament.registrations_closing_at;

            while !tournament.has_ended() {
                for game in games.values_mut() {
                    game.tick(current_tick);
                }

                let new_games =
                    tournament.generate_next_games(current_tick, &games, &past_games)?;

                for game in games.values().filter(|g| g.has_ended()) {
                    past_games.insert(game.id, GameSummary::from_game(game));
                }

                games.retain(|_, g| !g.has_ended());

                for game in new_games {
                    games.insert(game.id, game);
                }

                current_tick += TickInterval::SHORT;
            }

            let own_team_id = tournament.participants.keys().next().unwrap().clone();
            let active_games = tournament.active_games(&games);
            let past_game_summaries = tournament.past_game_summaries(&past_games);

            let total_rendered_games = past_game_summaries.len() + active_games.len();

            let brackets = get_bracket_lines(
                tournament.winner.map(|id| {
                    tournament
                        .participants
                        .get(&id)
                        .expect("Winner should be a participant")
                        .name
                        .as_str()
                }),
                n,
                &active_games,
                &past_game_summaries,
                own_team_id,
                current_tick,
            );

            let expected_columns = number_of_rounds(n) + 1;
            assert_eq!(
                brackets.len(),
                expected_columns,
                "N={n}: expected {expected_columns} bracket columns, got {}",
                brackets.len()
            );

            for (round_idx, round_lines) in brackets.iter().enumerate() {
                assert_eq!(
                    round_lines.len(),
                    ROUND_LINES_LEN,
                    "N={n}, round {round_idx}: expected {ROUND_LINES_LEN} lines, got {}",
                    round_lines.len()
                );
            }

            assert_eq!(
                total_rendered_games,
                n - 1,
                "N={n}: expected {} total games, got {total_rendered_games}",
                n - 1
            );
        }

        Ok(())
    }
}
