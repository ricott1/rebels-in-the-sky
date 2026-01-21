use crate::{
    game_engine::{
        game::{Game, GameSummary},
        types::Possession,
    },
    types::{TeamId, Tick},
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
            "vs".to_string()
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

pub fn number_of_rounds(participants: usize) -> usize {
    (participants as f32).log2().ceil() as usize
}

pub fn current_round(participants: usize, games_played: usize) -> usize {
    let round_sizes = compute_round_sizes(participants);
    let mut counter = 0;
    for (idx, round_size) in round_sizes.iter().enumerate() {
        counter += round_size;

        if games_played <= counter {
            return idx;
        }
    }

    unreachable!()
}

pub fn compute_round_sizes(participants: usize) -> Vec<usize> {
    let mut remaining_players = participants;
    let mut rounds = Vec::new();

    while remaining_players > 1 {
        let games = remaining_players / 2;
        rounds.push(games);
        remaining_players -= games;
    }

    rounds
}

fn get_round_lines(
    round_idx: usize,
    round_description: Vec<TournamentDescription>,
    odd_participants: bool,
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
        let bracket_idx = if odd_participants && round_idx == 0 {
            idx + 1
        } else {
            idx
        };
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
    winner_team_name: Option<String>,
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
    let mut lines = Vec::with_capacity(round_sizes.len());

    'outer: for (round_idx, round_size) in round_sizes.into_iter().enumerate() {
        let mut round_description = vec![];

        for _ in (0..round_size).rev() {
            let description = if idx < past_game_summaries.len() {
                let game = past_game_summaries[idx];
                game.tournament_description(own_team_id, timestamp)
            } else if idx < all_games_len {
                let game = active_games[idx - past_game_summaries.len()];
                game.tournament_description(own_team_id, timestamp)
            } else {
                break 'outer;
            };
            round_description.push(description);
            idx += 1;
        }

        lines.push(get_round_lines(
            round_idx,
            round_description,
            !num_participants.is_multiple_of(2),
        ));
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
    use crossterm::event::{self, Event};
    use itertools::Itertools;
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
        let number_of_rounds = number_of_rounds(tournament.participants.len());
        let current_round = current_round(
            tournament.participants.len(),
            past_game_summaries.len() + active_games.len(),
        ) + 1;

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
                    .clone()
            }),
            tournament.participants.len(),
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
            let mut tournament = Tournament::test(6, 8);
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

                let new_games = tournament.generate_next_games(current_tick, &games)?;
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
            let mut tournament = Tournament::test(5, 16);
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

                let new_games = tournament.generate_next_games(current_tick, &games)?;

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
        assert_eq!(compute_round_sizes(5), vec![2, 1, 1]);
        assert_eq!(compute_round_sizes(6), vec![3, 1, 1]);
        assert_eq!(compute_round_sizes(7), vec![3, 2, 1]);
        assert_eq!(compute_round_sizes(8), vec![4, 2, 1]);
        assert_eq!(compute_round_sizes(9), vec![4, 2, 1, 1]);
        assert_eq!(compute_round_sizes(10), vec![5, 2, 1, 1]);
        assert_eq!(compute_round_sizes(11), vec![5, 3, 1, 1]);
        assert_eq!(compute_round_sizes(12), vec![6, 3, 1, 1]);
        assert_eq!(compute_round_sizes(13), vec![6, 3, 2, 1]);
        assert_eq!(compute_round_sizes(14), vec![7, 3, 2, 1]);
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
        // games_played: 0..=3

        assert_eq!(current_round(4, 0), 0);
        assert_eq!(current_round(4, 1), 0);
        assert_eq!(current_round(4, 2), 0); // first round complete
        assert_eq!(current_round(4, 3), 1); // final
    }

    #[test]
    fn test_current_round_progression_5_participants() {
        assert_eq!(current_round(5, 0), 0);
        assert_eq!(current_round(5, 1), 0);

        assert_eq!(current_round(5, 2), 0);
        assert_eq!(current_round(5, 3), 1);

        assert_eq!(current_round(5, 4), 2);
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
}
