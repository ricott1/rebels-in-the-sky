use super::clickable_list::ClickableListState;
use super::constants::UiStyle;
use super::gif_map::GifMap;
use super::ui_callback::{CallbackRegistry, UiCallbackPreset};
use super::{
    big_numbers::{hyphen, BigNumberFont},
    constants::{IMG_FRAME_WIDTH, LEFT_PANEL_WIDTH},
    traits::{Screen, SplitPanel},
    utils::img_to_lines,
    widgets::{default_block, selectable_list, DOWN_ARROW_SPAN, SWITCH_ARROW_SPAN, UP_ARROW_SPAN},
};
use crate::engine::constants::MAX_TIREDNESS;
use crate::types::AppResult;
use crate::world::planet::PlanetType;
use crate::{
    engine::{
        action::{ActionOutput, ActionSituation, Advantage},
        game::Game,
        timer::{Period, Timer},
        types::{GameStatsMap, Possession},
    },
    image::pitch::{set_shot_pixels, PitchStyle, PITCH_WIDTH},
    image::player::{PLAYER_IMAGE_HEIGHT, PLAYER_IMAGE_WIDTH},
    types::GameId,
    ui::constants::{PrintableKeyCode, UiKey},
    world::{
        player::Player,
        position::{GamePosition, Position},
        world::World,
    },
};
use core::fmt::Debug;
use crossterm::event::KeyCode;
use itertools::Itertools;
use ratatui::layout::Margin;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    prelude::Rect,
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Cell, Paragraph, Row, Table, Wrap},
    Frame,
};
use std::{cell::RefCell, rc::Rc};
use strum_macros::Display;

#[derive(Default, Debug, Clone, PartialEq, Display)]
enum PitchViewFilter {
    #[default]
    All,
    First,
    Second,
    Third,
    Fourth,
}

#[derive(Debug, Default)]
pub struct GamePanel {
    pub index: usize,
    pub games: Vec<GameId>,
    pitch_view: bool,
    pitch_view_filter: PitchViewFilter,
    commentary_index: usize,
    debug_mode: bool,
    action_results: Vec<ActionOutput>,
    tick: usize,
    callback_registry: Rc<RefCell<CallbackRegistry>>,
    gif_map: Rc<RefCell<GifMap>>,
}

impl GamePanel {
    pub fn new(
        callback_registry: Rc<RefCell<CallbackRegistry>>,
        gif_map: Rc<RefCell<GifMap>>,
    ) -> Self {
        Self {
            callback_registry,
            gif_map,
            ..Default::default()
        }
    }

    fn selected_game<'a>(&self, world: &'a World) -> Option<&'a Game> {
        if self.index >= self.games.len() {
            return None;
        }
        world.get_game(self.games[self.index].clone())
    }

    fn build_top_panel(&mut self, frame: &mut Frame, world: &World, area: Rect) -> AppResult<()> {
        // Split into left and right panels
        let split = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(LEFT_PANEL_WIDTH),
                Constraint::Min(IMG_FRAME_WIDTH),
            ])
            .split(area);
        self.build_game_list(frame, world, split[0]);

        if let Some(game) = self.selected_game(world) {
            if self.pitch_view {
                self.build_pitch_panel(frame, world, game, split[1]);
            } else {
                self.build_score_panel(frame, world, game, split[1])?;
            }
        }
        Ok(())
    }

    fn build_game_list(&mut self, frame: &mut Frame, world: &World, area: Rect) {
        let options = self
            .games
            .iter()
            .filter(|&&id| world.get_game(id).is_some())
            .map(|&id| {
                let game = world.get_game(id).unwrap();
                let mut style = UiStyle::DEFAULT;

                if game.home_team_in_game.team_id == world.own_team_id
                    || game.away_team_in_game.team_id == world.own_team_id
                {
                    style = UiStyle::OWN_TEAM
                } else if game.home_team_in_game.peer_id.is_some()
                    || game.away_team_in_game.peer_id.is_some()
                {
                    style = UiStyle::NETWORK
                }

                (
                    format!(
                        "{:>12} {:>3}-{:<3} {:<12}",
                        game.home_team_in_game.name,
                        game.action_results.last().unwrap().home_score,
                        game.action_results.last().unwrap().away_score,
                        game.away_team_in_game.name
                    ),
                    style,
                )
            })
            .collect_vec();

        let list = selectable_list(options, &self.callback_registry);

        frame.render_stateful_widget(
            list.block(default_block().title("Games ↓/↑")),
            area,
            &mut ClickableListState::default().with_selected(Some(self.index)),
        );
    }

    fn build_score_panel(
        &self,
        frame: &mut Frame,
        world: &World,
        game: &Game,
        area: Rect,
    ) -> AppResult<()> {
        let split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(PLAYER_IMAGE_HEIGHT as u16 / 2), // Score Plus images
                                                                    // Constraint::Length(2),                              //floor
            ])
            .split(area);

        let side_length: u16;
        let score_panel_width = 59;
        if area.width > 2 * PLAYER_IMAGE_WIDTH as u16 + score_panel_width {
            side_length = (area.width - 2 * PLAYER_IMAGE_WIDTH as u16 - score_panel_width) / 2;
        } else {
            side_length = 0;
        }
        let top_split = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(side_length),
                Constraint::Length(PLAYER_IMAGE_WIDTH as u16),
                Constraint::Length(score_panel_width),
                Constraint::Length(PLAYER_IMAGE_WIDTH as u16),
                Constraint::Length(side_length),
            ])
            .split(split[0]);

        let margin_height: u16;
        if top_split[2].height > 12 {
            margin_height = (top_split[2].height - 12) / 2;
        } else {
            margin_height = 0;
        }
        let central_split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(margin_height),
                Constraint::Length(2),
                Constraint::Length(2),
                Constraint::Length(8),
                Constraint::Length(margin_height),
            ])
            .split(top_split[2]);

        frame.render_widget(
            Paragraph::new(format!(
                "Playing on {}",
                world.get_planet_or_err(game.location).unwrap().name
            ))
            .alignment(Alignment::Center),
            central_split[2],
        );

        let digit_split = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(8),
                Constraint::Length(1),
                Constraint::Length(8),
                Constraint::Length(1),
                Constraint::Length(8),
                Constraint::Length(1),
                Constraint::Length(5),
                Constraint::Length(1),
                Constraint::Length(8),
                Constraint::Length(1),
                Constraint::Length(8),
                Constraint::Length(1),
                Constraint::Length(8),
            ])
            .split(central_split[3]);

        let action = if self.commentary_index == 0 {
            &game.action_results[game.action_results.len() - 1]
        } else {
            &game.action_results[self.action_results.len() - 1 - self.commentary_index]
        };

        let home_players = game
            .home_team_in_game
            .players
            .values()
            .collect::<Vec<&Player>>();
        let away_players = game
            .away_team_in_game
            .players
            .values()
            .collect::<Vec<&Player>>();
        let home_score = action.home_score;
        let away_score = action.away_score;

        let base_home_player = home_players
            .iter()
            .max_by(|&a, &b| a.total_skills().cmp(&b.total_skills()))
            .unwrap();
        let base_away_player = away_players
            .iter()
            .max_by(|&a, &b| a.total_skills().cmp(&b.total_skills()))
            .unwrap();

        if let Ok(mut lines) = self
            .gif_map
            .borrow_mut()
            .player_frame_lines(&base_home_player, self.tick)
        {
            lines.remove(0);
            let paragraph = Paragraph::new(lines).alignment(Alignment::Center);
            frame.render_widget(paragraph, top_split[1]);
        }
        if let Ok(mut lines) = self
            .gif_map
            .borrow_mut()
            .player_frame_lines(&base_away_player, self.tick)
        {
            lines.remove(0);
            let paragraph = Paragraph::new(lines).alignment(Alignment::Center);
            frame.render_widget(paragraph, top_split[3]);
        }

        let home_dot = if action.possession == Possession::Home {
            "●"
        } else {
            " "
        };
        let away_dot = if action.possession == Possession::Away {
            "●"
        } else {
            " "
        };
        frame.render_widget(
            Paragraph::new(Line::from(format!(
                "{} {} vs {} {}",
                home_dot,
                game.home_team_in_game.name.to_string(),
                game.away_team_in_game.name.to_string(),
                away_dot
            )))
            .alignment(Alignment::Center),
            central_split[1],
        );

        let timer_lines = self.build_timer_lines(world, game);
        frame.render_widget(
            Paragraph::new(timer_lines).alignment(Alignment::Center),
            central_split[4],
        );
        match home_score {
            x if x < 10 => frame.render_widget((home_score % 10).big_font(), digit_split[4]),
            x if x < 100 => {
                frame.render_widget((home_score % 100 / 10).big_font(), digit_split[2]);
                frame.render_widget((home_score % 10).big_font(), digit_split[4]);
            }
            x if x < 1000 => {
                frame.render_widget((home_score / 100).big_font(), digit_split[0]);
                frame.render_widget((home_score % 100 / 10).big_font(), digit_split[2]);
                frame.render_widget((home_score % 10).big_font(), digit_split[4]);
            }
            _ => {
                frame.render_widget(Paragraph::new(home_score.to_string()), digit_split[4]);
            }
        }

        frame.render_widget(hyphen(), digit_split[6]);

        match away_score {
            x if x < 10 => frame.render_widget((away_score % 10).big_font(), digit_split[8]),
            x if x < 100 => {
                frame.render_widget((away_score % 100 / 10).big_font(), digit_split[8]);
                frame.render_widget((away_score % 10).big_font(), digit_split[10]);
            }
            x if x < 1000 => {
                frame.render_widget((away_score / 100).big_font(), digit_split[8]);
                frame.render_widget((away_score % 100 / 10).big_font(), digit_split[10]);
                frame.render_widget((away_score % 10).big_font(), digit_split[12]);
            }
            _ => {
                frame.render_widget(Paragraph::new(away_score.to_string()), digit_split[8]);
            }
        }
        Ok(())
    }

    fn build_pitch_panel(&self, frame: &mut Frame, world: &World, game: &Game, area: Rect) {
        let split = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(2),           // border
                Constraint::Length(PITCH_WIDTH), // pitch
                Constraint::Min(1),              // score
            ])
            .split(area);

        let action = if self.commentary_index == 0 {
            &game.action_results[game.action_results.len() - 1]
        } else {
            &game.action_results[self.action_results.len() - 1 - self.commentary_index]
        };
        let planet = world.get_planet_or_err(game.location).unwrap();
        let pitch_style = match planet.planet_type {
            PlanetType::Earth => PitchStyle::PitchBall,
            _ => PitchStyle::PitchClassic,
        };

        let mut pitch_image = pitch_style.image().unwrap();
        let max_index = self.action_results.len() - self.commentary_index;
        for result in game.action_results.iter().take(max_index).rev() {
            // add quarter filter here
            match self.pitch_view_filter {
                PitchViewFilter::All => {}
                PitchViewFilter::First => {
                    if result.start_at.period() != Period::Q1 {
                        continue;
                    }
                }
                PitchViewFilter::Second => {
                    if result.start_at.period() != Period::Q2 {
                        continue;
                    }
                }
                PitchViewFilter::Third => {
                    if result.start_at.period() != Period::Q3 {
                        continue;
                    }
                }
                PitchViewFilter::Fourth => {
                    if result.start_at.period() != Period::Q4 {
                        continue;
                    }
                }
            }
            if let Some(stats_map) = &result.attack_stats_update {
                pitch_image = set_shot_pixels(pitch_image, stats_map);
            }
        }

        let pitch = Paragraph::new(img_to_lines(&pitch_image)).alignment(Alignment::Center);
        frame.render_widget(pitch, split[1]);

        let home_dot = if action.possession == Possession::Home {
            "●"
        } else {
            " "
        };
        let away_dot = if action.possession == Possession::Away {
            "●"
        } else {
            " "
        };

        let mut lines: Vec<Line> = vec![
            Line::from(""),
            Line::from(format!(
                "{:1} {:>13} {:>3}-{:<3} {:<} {}",
                home_dot,
                game.home_team_in_game.name,
                action.home_score,
                action.away_score,
                game.away_team_in_game.name,
                away_dot
            )),
            Line::from(""),
            Line::from(format!(
                "{:>18}-->{:<}",
                game.home_team_in_game.offense_tactic, game.away_team_in_game.defense_tactic
            )),
            Line::from(format!(
                "{:>18}<--{:<}",
                game.home_team_in_game.defense_tactic, game.away_team_in_game.offense_tactic
            )),
        ];

        let mut timer_lines = self.build_timer_lines(world, game);
        lines.append(&mut timer_lines);
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("{:<16}", "██ made shot"),
            UiStyle::OWN_TEAM,
        )));
        lines.push(Line::from(Span::styled(
            format!("{:<16}", "██ missed shot"),
            UiStyle::ERROR,
        )));

        let score = Paragraph::new(lines).alignment(Alignment::Center);
        frame.render_widget(score, split[2]);
    }

    fn build_bottom_panel(&mut self, frame: &mut Frame, world: &World, area: Rect) {
        let split = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(8), Constraint::Length(73)])
            .split(area);
        self.build_commentary(frame, split[0]);
        let game = self.selected_game(world);
        if game.is_none() {
            return;
        }
        let game = game.unwrap();
        self.build_statbox(game, frame, split[1]);
    }

    fn format_commentary(
        &self,
        action_result: ActionOutput,
        timer: Timer,
        switch_possession: bool,
    ) -> Line {
        let arrow: Span<'_>;
        if switch_possession {
            arrow = SWITCH_ARROW_SPAN.clone();
        } else {
            arrow = match action_result.advantage {
                Advantage::Attack => UP_ARROW_SPAN.clone(),
                Advantage::Defense => DOWN_ARROW_SPAN.clone(),
                Advantage::Neutral => Span::raw(""),
            };
        }
        let timer = Span::styled(format!("[{}] ", timer.format()), UiStyle::HIGHLIGHT);
        let text = Span::from(format!("{} ", action_result.description.clone()));
        Line::from(vec![timer, text, arrow])
    }

    fn build_commentary(&mut self, frame: &mut Frame, area: Rect) {
        let mut commentary = vec![];
        let max_index = self.action_results.len() - self.commentary_index;

        for idx in 0..max_index {
            let result = self.action_results[idx].clone();
            let situation = result.situation.clone();
            let timer = self.action_results[idx].start_at;
            let switch_possession = if idx > 0 {
                result.possession != self.action_results[idx - 1].possession
            } else {
                false
            };
            commentary.push(self.format_commentary(result, timer, switch_possession));
            match situation {
                ActionSituation::BallInBackcourt
                | ActionSituation::AfterDefensiveRebound
                | ActionSituation::Turnover => {
                    commentary.push(Line::from(""));
                }
                _ => {}
            }
        }

        commentary.reverse();

        frame.render_widget(
            Paragraph::new(commentary).wrap(Wrap { trim: false }).block(
                Block::new()
                    .title(" Commentary ")
                    .title_alignment(Alignment::Left)
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            ),
            area,
        )
    }

    fn build_stat_table(&self, players_data: &GameStatsMap, players: Vec<&Player>) -> Table {
        let mut rows: Vec<Row<'_>> = vec![];

        let mut points_total = 0;
        let mut attempted_2pt_total = 0;
        let mut made_2pt_total = 0;
        let mut attempted_3pt_total = 0;
        let mut made_3pt_total = 0;
        let mut assists_total = 0;
        let mut turnovers_total = 0;
        let mut defensive_rebounds_total = 0;
        let mut offensive_rebounds_total = 0;
        let mut steals_total = 0;
        let mut blocks_total = 0;
        let mut fouls_total = 0;
        let mut plus_minus_total = 0;

        for player in players.iter() {
            let player_data = players_data[&player.id].clone();
            points_total += player_data.points as u16;
            attempted_2pt_total += player_data.attempted_2pt as u16;
            made_2pt_total += player_data.made_2pt as u16;
            attempted_3pt_total += player_data.attempted_3pt as u16;
            made_3pt_total += player_data.made_3pt as u16;
            assists_total += player_data.assists as u16;
            turnovers_total += player_data.turnovers as u16;
            defensive_rebounds_total += player_data.defensive_rebounds as u16;
            offensive_rebounds_total += player_data.offensive_rebounds as u16;
            steals_total += player_data.steals as u16;
            blocks_total += player_data.blocks as u16;
            fouls_total += player_data.fouls as u16;
            plus_minus_total += player_data.plus_minus as i16;

            let role = match player_data.position {
                Some(p) => (p as Position).as_str().to_string(),
                None => "".to_string(),
            };

            let name_span = if self.debug_mode {
                Span::raw(format!("Tds: {}", player_data.tiredness))
            } else {
                let style = match player_data.tiredness {
                    x if x < MAX_TIREDNESS / 4.0 => Style::default().fg(Color::White),
                    x if x < MAX_TIREDNESS / 2.0 => Style::default().fg(Color::Yellow),
                    x if x < MAX_TIREDNESS => Style::default().fg(Color::Red),
                    _ => Style::default().fg(Color::DarkGray),
                };
                Span::styled(
                    format!(
                        "{}.{}",
                        player.info.first_name.chars().next().unwrap_or_default(),
                        player.info.last_name,
                    ),
                    style,
                )
            };

            let cells = vec![
                Cell::from(format!("{:<2}", role,)),
                Cell::from(name_span),
                Cell::from(format!(
                    "{:^3}",
                    players_data[&player.id].seconds_played / 60
                )),
                Cell::from(format!("{:^3}", players_data[&player.id].points)),
                Cell::from(format!(
                    "{:>2}/{:<3}",
                    players_data[&player.id].made_2pt, players_data[&player.id].attempted_2pt
                )),
                Cell::from(format!(
                    "{:>2}/{:<2}",
                    players_data[&player.id].made_3pt, players_data[&player.id].attempted_3pt
                )),
                Cell::from(format!(
                    "{:>3}/{:<2}",
                    players_data[&player.id].assists, players_data[&player.id].turnovers
                )),
                Cell::from(format!(
                    "{:>3}/{:<3}",
                    players_data[&player.id].defensive_rebounds,
                    players_data[&player.id].offensive_rebounds
                )),
                Cell::from(format!("{:^3}", players_data[&player.id].steals)),
                Cell::from(format!("{:^3}", players_data[&player.id].blocks)),
                Cell::from(format!("{:>2}", players_data[&player.id].fouls)),
                Cell::from(format!("{:>+3}", players_data[&player.id].plus_minus)),
            ];
            rows.push(Row::new(cells).height(1));
        }

        let totals = vec![
            Cell::from(format!("")),
            Cell::from(format!("Total")),
            Cell::from(""),
            Cell::from(format!("{:^3}", points_total)),
            Cell::from(format!("{:>2}/{:<2}", made_2pt_total, attempted_2pt_total)),
            Cell::from(format!("{:>2}/{:<2}", made_3pt_total, attempted_3pt_total)),
            Cell::from(format!("{:>3}/{:<2}", assists_total, turnovers_total)),
            Cell::from(format!(
                "{:>3}/{:<3}",
                defensive_rebounds_total, offensive_rebounds_total
            )),
            Cell::from(format!("{:^3}", steals_total)),
            Cell::from(format!("{:^3}", blocks_total)),
            Cell::from(format!("{:>2}", fouls_total)),
            Cell::from(format!("{:>+3}", plus_minus_total / 5)),
        ];

        rows.push(Row::new(totals).cyan());

        Table::new(
            rows,
            [
                Constraint::Length(2),
                Constraint::Length(14),
                Constraint::Length(3),
                Constraint::Length(4),
                Constraint::Length(6),
                Constraint::Length(5),
                Constraint::Length(6),
                Constraint::Length(7),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(2),
                Constraint::Length(3),
            ],
        )
    }

    fn build_statbox(&self, game: &Game, frame: &mut Frame, area: Rect) {
        let header_cells_home = [
            "  ",
            game.home_team_in_game.name.as_str(),
            "Min",
            "Pts",
            " 2pt ",
            " 3pt ",
            "Ast/TO",
            "DRb/ORb",
            "Stl",
            "Blk",
            "PF",
            "+/-",
        ];

        let header_cells_away = [
            "  ",
            game.away_team_in_game.name.as_str(),
            "Min",
            "Pts",
            " 2pt ",
            " 3pt ",
            "Ast/TO",
            "DRb/ORb",
            "Stl",
            "Blk",
            "PF",
            "+/-",
        ];

        let home_players = game
            .home_team_in_game
            .initial_positions
            .iter()
            .map(|id| game.home_team_in_game.players.get(id).unwrap())
            .collect::<Vec<&Player>>();
        let away_players = game
            .away_team_in_game
            .initial_positions
            .iter()
            .map(|id| game.away_team_in_game.players.get(id).unwrap())
            .collect::<Vec<&Player>>();

        let constraint = &[
            Constraint::Length(2), //role
            Constraint::Min(16),   //player
            Constraint::Length(3), //minutes
            Constraint::Length(3), //points
            Constraint::Length(6), //2pt
            Constraint::Length(5), //3pt
            Constraint::Length(6), //assists/turnovers
            Constraint::Length(7), //defensive rebounds/offensive rebounds
            Constraint::Length(3), //steals
            Constraint::Length(3), //blocks
            Constraint::Length(2), //personal fouls
            Constraint::Length(3), //plus minus
        ];

        let home_table = self
            .build_stat_table(&game.home_team_in_game.stats, home_players)
            .header(Row::new(header_cells_home).style(UiStyle::HEADER).height(1))
            .widths(constraint);

        let away_table = self
            .build_stat_table(&game.away_team_in_game.stats, away_players)
            .header(Row::new(header_cells_away).style(UiStyle::HEADER).height(1))
            .widths(constraint);

        let box_area = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Max(game.home_team_in_game.players.len() as u16 + 2),
                Constraint::Max(1),
                Constraint::Max(game.away_team_in_game.players.len() as u16 + 2),
                Constraint::Min(0),
            ])
            .split(area.inner(&Margin {
                horizontal: 1,
                vertical: 1,
            }));

        frame.render_widget(home_table, box_area[0]);
        frame.render_widget(away_table, box_area[2]);
        frame.render_widget(default_block().title(" Stats "), area);
    }

    fn build_timer_lines(&self, world: &World, game: &Game) -> Vec<Line<'static>> {
        let timer = if self.commentary_index > 0 {
            self.action_results[self.action_results.len() - 1 - self.commentary_index].start_at
        } else {
            game.timer
        };
        let mut timer_lines: Vec<Line> = vec![];
        if !timer.has_started() {
            timer_lines.push(Line::from(
                Timer::from(timer.period().next().start()).format(),
            ));
            let starting_in_seconds = (game.starting_at - world.last_tick_short_interval) / 1000;
            timer_lines.push(Line::from(format!(
                "Starting in {:02}:{:02}",
                starting_in_seconds / 60,
                starting_in_seconds % 60
            )));
        } else if timer.has_ended() {
            timer_lines.push(Line::from(
                Timer::from(timer.period().next().start()).format(),
            ));
        } else if timer.is_break() {
            timer_lines.push(Line::from(
                Timer::from(timer.period().next().start()).format(),
            ));
            timer_lines.push(Line::from(format!(
                "Resuming in {:02}:{:02}",
                timer.minutes(),
                timer.seconds()
            )));
        } else {
            timer_lines.push(Line::from(timer.format()));
        }
        timer_lines
    }
}

impl Screen for GamePanel {
    fn name(&self) -> &str {
        "Game"
    }

    fn update(&mut self, world: &World) -> AppResult<()> {
        self.tick += 1;
        if world.dirty_ui || self.games.len() != world.games.len() {
            // Try to keep track of current game when other games finish
            let current_game_id = if let Some(current_game) = self.selected_game(world) {
                Some(current_game.id)
            } else {
                None
            };

            self.games = world
                .games
                .keys()
                .into_iter()
                .cloned()
                .sorted_by(|&a, &b| {
                    let game_a = world.get_game(a).unwrap();
                    let game_b = world.get_game(b).unwrap();
                    game_b.starting_at.cmp(&game_a.starting_at)
                })
                .collect();
            if current_game_id.is_some() {
                self.set_index(
                    self.games
                        .iter()
                        .position(|&id| id == current_game_id.unwrap())
                        .unwrap_or_default(),
                );
            }
        }

        if let Some(game) = self.selected_game(world) {
            if self.commentary_index == 0 {
                self.action_results = game.action_results.clone();
            }
        } else {
            self.set_index(0);
        }
        Ok(())
    }

    fn render(&mut self, frame: &mut Frame, world: &World, area: Rect) -> AppResult<()> {
        if self.games.len() == 0 {
            frame.render_widget(
                Paragraph::new(" No games today!"),
                area.inner(&Margin {
                    vertical: 1,
                    horizontal: 1,
                }),
            );
            return Ok(());
        }

        // Split into top and bottom panels
        let split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(PLAYER_IMAGE_HEIGHT as u16 / 2 - 1),
                Constraint::Min(4),
            ])
            .split(area);
        self.build_top_panel(frame, world, split[0])?;
        self.build_bottom_panel(frame, world, split[1]);
        Ok(())
    }

    fn handle_key_events(
        &mut self,
        key_event: crossterm::event::KeyEvent,
    ) -> Option<UiCallbackPreset> {
        match key_event.code {
            KeyCode::Up => self.next_index(),
            KeyCode::Down => self.previous_index(),
            KeyCode::Left => {
                if self.commentary_index > 0 {
                    self.commentary_index -= 1;
                }
            }
            KeyCode::Right => {
                // CHECK if this works
                if self.commentary_index < self.action_results.len() - 1 {
                    self.commentary_index += 1;
                }
            }
            KeyCode::Enter => self.commentary_index = 0,
            UiKey::PITCH_VIEW => {
                self.pitch_view = !self.pitch_view;
                // self.debug_mode = !self.debug_mode;
            }

            KeyCode::Char('0') => {
                self.pitch_view_filter = PitchViewFilter::All;
                // self.debug_mode = !self.debug_mode;
            }
            KeyCode::Char('1') => {
                self.pitch_view_filter = PitchViewFilter::First;
                // self.debug_mode = !self.debug_mode;
            }
            KeyCode::Char('2') => {
                self.pitch_view_filter = PitchViewFilter::Second;
                // self.debug_mode = !self.debug_mode;
            }
            KeyCode::Char('3') => {
                self.pitch_view_filter = PitchViewFilter::Third;
                // self.debug_mode = !self.debug_mode;
            }
            KeyCode::Char('4') => {
                self.pitch_view_filter = PitchViewFilter::Fourth;
                // self.debug_mode = !self.debug_mode;
            }
            _ => {}
        };
        None
    }

    fn footer_spans(&self) -> Vec<Span> {
        let next_view = if self.pitch_view { "Score" } else { "Pitch" };
        vec![
            Span::styled(
                " ↑/↓ ",
                Style::default().bg(Color::Gray).fg(Color::DarkGray),
            ),
            Span::styled(" Select game ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                " ←/→ ",
                Style::default().bg(Color::Gray).fg(Color::DarkGray),
            ),
            Span::styled(" Scroll commentary ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                " Enter ",
                Style::default().bg(Color::Gray).fg(Color::DarkGray),
            ),
            Span::styled(
                " Scroll commentary to top ",
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                format!(" {} ", UiKey::PITCH_VIEW.to_string()),
                Style::default().bg(Color::Gray).fg(Color::DarkGray),
            ),
            Span::styled(
                format!(" Change view: {} ", next_view),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                " 0-4 ",
                Style::default().bg(Color::Gray).fg(Color::DarkGray),
            ),
            Span::styled(
                format!(" Filter: {:<6} ", self.pitch_view_filter),
                Style::default().fg(Color::DarkGray),
            ),
        ]
    }
}

impl SplitPanel for GamePanel {
    fn index(&self) -> usize {
        self.index
    }

    fn max_index(&self) -> usize {
        self.games.len()
    }

    fn set_index(&mut self, index: usize) {
        self.index = index;
        self.commentary_index = 0;
    }
}
