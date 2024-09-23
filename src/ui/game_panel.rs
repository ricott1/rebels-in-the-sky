use super::button::Button;
use super::clickable_list::ClickableListState;
use super::constants::UiStyle;
use super::gif_map::GifMap;
use super::ui_callback::{CallbackRegistry, UiCallbackPreset};
use super::utils::hover_text_target;
use super::{
    big_numbers::{hyphen, BigNumberFont},
    constants::{IMG_FRAME_WIDTH, LEFT_PANEL_WIDTH},
    traits::{Screen, SplitPanel},
    utils::img_to_lines,
    widgets::{default_block, selectable_list, DOWN_ARROW_SPAN, SWITCH_ARROW_SPAN, UP_ARROW_SPAN},
};
use crate::engine::constants::MIN_TIREDNESS_FOR_ROLL_DECLINE;
use crate::types::AppResult;
use crate::world::constants::{MAX_MORALE, MORALE_THRESHOLD_FOR_LEAVING};
use crate::{
    engine::{
        action::{ActionOutput, ActionSituation, Advantage},
        game::Game,
        timer::{Period, Timer},
        types::{GameStatsMap, Possession},
    },
    image::pitch::{PitchStyle, PITCH_HEIGHT},
    image::player::{PLAYER_IMAGE_HEIGHT, PLAYER_IMAGE_WIDTH},
    types::GameId,
    ui::constants::UiKey,
    world::{
        constants::MAX_TIREDNESS,
        planet::PlanetType,
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
    layout::{Constraint, Layout},
    prelude::Rect,
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Cell, Paragraph, Row, Table, Wrap},
    Frame,
};
use std::collections::HashMap;
use std::{sync::Arc, sync::Mutex};

#[derive(Debug, Default)]
pub struct GamePanel {
    pub index: usize,
    pub games: Vec<GameId>,
    pitch_view: bool,
    pitch_view_filter: Option<Period>,
    player_status_view: bool,
    commentary_index: usize,
    action_results: Vec<ActionOutput>,
    tick: usize,
    callback_registry: Arc<Mutex<CallbackRegistry>>,
    gif_map: Arc<Mutex<GifMap>>,
}

impl GamePanel {
    pub fn new(
        callback_registry: Arc<Mutex<CallbackRegistry>>,
        gif_map: Arc<Mutex<GifMap>>,
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
        let split = Layout::horizontal([
            Constraint::Length(LEFT_PANEL_WIDTH),
            Constraint::Min(IMG_FRAME_WIDTH),
        ])
        .split(area);

        let game_button_split =
            Layout::vertical([Constraint::Min(3), Constraint::Length(3)]).split(split[0]);
        self.build_game_list(frame, world, game_button_split[0]);
        self.build_game_buttons(frame, game_button_split[1]);

        if let Some(game) = self.selected_game(world) {
            self.build_score_panel(frame, world, game, split[1])?;
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

    fn build_game_buttons(&mut self, frame: &mut Frame, area: Rect) {
        let b_split =
            Layout::horizontal([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)]).split(area);
        let hover_text_target = hover_text_target(frame);
        let text = if self.pitch_view {
            "Commentary view"
        } else {
            "Game view"
        };
        let pitch_button = Button::new(
            text.into(),
            UiCallbackPreset::TogglePitchView,
            Arc::clone(&self.callback_registry),
        )
        .set_hover_text(
            format!(
                "Change to {} view",
                if self.pitch_view {
                    "commentary"
                } else {
                    "pitch"
                }
            ),
            hover_text_target,
        )
        .set_hotkey(UiKey::PITCH_VIEW);

        frame.render_widget(pitch_button, b_split[0]);

        let text = if self.player_status_view {
            "Game stats"
        } else {
            "Player status"
        };
        let player_status_button = Button::new(
            text.into(),
            UiCallbackPreset::TogglePlayerStatusView,
            Arc::clone(&self.callback_registry),
        )
        .set_hover_text(
            format!(
                "Change to {} view",
                if self.player_status_view {
                    "game box"
                } else {
                    "player status"
                }
            ),
            hover_text_target,
        )
        .set_hotkey(UiKey::PLAYER_STATUS_VIEW);

        frame.render_widget(player_status_button, b_split[1]);
    }

    fn build_score_panel(
        &self,
        frame: &mut Frame,
        world: &World,
        game: &Game,
        area: Rect,
    ) -> AppResult<()> {
        let split = Layout::vertical([
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
        let top_split = Layout::horizontal([
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
        let central_split = Layout::vertical([
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
            .centered(),
            central_split[2],
        );

        let digit_split = Layout::horizontal([
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
            .max_by(|&a, &b| {
                b.average_skill()
                    .partial_cmp(&a.average_skill())
                    .expect("Skill value should exist")
            })
            .unwrap();
        let base_away_player = away_players
            .iter()
            .max_by(|&a, &b| {
                b.average_skill()
                    .partial_cmp(&a.average_skill())
                    .expect("Skill value should exist")
            })
            .unwrap();

        if let Ok(mut lines) = self
            .gif_map
            .lock()
            .unwrap()
            .player_frame_lines(&base_home_player, self.tick)
        {
            lines.remove(0);
            let paragraph = Paragraph::new(lines).centered();
            frame.render_widget(paragraph, top_split[1]);
        }
        if let Ok(mut lines) = self
            .gif_map
            .lock()
            .unwrap()
            .player_frame_lines(&base_away_player, self.tick)
        {
            lines.remove(0);
            let paragraph = Paragraph::new(lines).centered();
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
            .centered(),
            central_split[1],
        );

        let timer_lines = self.build_timer_lines(world, game);
        frame.render_widget(Paragraph::new(timer_lines).centered(), central_split[4]);
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

    fn build_pitch_panel(
        &self,
        frame: &mut Frame,
        world: &World,
        game: &Game,
        area: Rect,
    ) -> AppResult<()> {
        frame.render_widget(default_block().title("Shots map"), area);
        let split = Layout::vertical([
            Constraint::Length(PITCH_HEIGHT / 2 + 8), // pitch
            Constraint::Min(1),                       // score
        ])
        .split(area.inner(Margin {
            horizontal: 1,
            vertical: 1,
        }));

        let planet = world.get_planet_or_err(game.location)?;
        let pitch_style = match planet.planet_type {
            PlanetType::Earth => PitchStyle::PitchBall,
            _ => PitchStyle::PitchClassic,
        };

        let max_index = self.action_results.len() - self.commentary_index;

        // These map will contain every shot up to the max_index action.
        let mut shots_map: HashMap<(u32, u32), (u8, u8)> = HashMap::new();
        let mut last_shot = None;
        for result in game.action_results.iter().take(max_index) {
            match self.pitch_view_filter {
                Some(period) => {
                    if result.start_at.period() == period.next() {
                        last_shot = None;
                        break;
                    }

                    if result.start_at.period() != period {
                        continue;
                    }
                }
                None => {}
            }

            // Data about the shots (missed/made/position) is stored in the attack_stats_update.
            if let Some(stats_map) = &result.attack_stats_update {
                // Loop over players stats.
                for player_stats in stats_map.values() {
                    if let Some(shot) = player_stats.last_action_shot {
                        let x = shot.0 as u32;
                        let y = shot.1 as u32;
                        if let Some(count) = shots_map.get(&(x, y)) {
                            let new_count = if shot.2 {
                                (count.0, count.1 + 1)
                            } else {
                                (count.0 + 1, count.1)
                            };
                            shots_map.insert((x, y), new_count);
                        } else {
                            let new_count = if shot.2 { (0, 1) } else { (1, 0) };
                            shots_map.insert((x, y), new_count);
                        }
                        last_shot = Some(shot);
                    }
                }
            }
        }

        let pitch_image = pitch_style.image_with_shot_pixels(shots_map, last_shot, self.tick)?;

        frame.render_widget(
            Paragraph::new(img_to_lines(&pitch_image)).centered(),
            split[0],
        );

        let quarter = match self.pitch_view_filter {
            Some(Period::Q1) => "1st Quarter",
            Some(Period::Q2) => "2nd Quarter",
            Some(Period::Q3) => "3rd Quarter",
            Some(Period::Q4) => "4th Quarter",
            None => "Full game",
            _ => "Invalid filter",
        };

        let line = Line::from(vec![
            Span::raw(format!("{:<16}", quarter)),
            Span::styled(format!("{:<16}", "██ made shot"), UiStyle::OWN_TEAM),
            Span::styled(format!("{:<16}", "██ missed shot"), UiStyle::ERROR),
        ]);

        frame.render_widget(Paragraph::new(line).centered(), split[1]);

        Ok(())
    }

    fn build_bottom_panel(
        &mut self,
        frame: &mut Frame,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        let split = Layout::horizontal([Constraint::Min(8), Constraint::Length(73)]).split(area);

        if let Some(game) = self.selected_game(world) {
            if self.pitch_view {
                self.build_pitch_panel(frame, world, game, split[0])?;
            } else {
                self.build_commentary(frame, split[0]);
            }
        }
        if let Some(game) = self.selected_game(world) {
            if self.player_status_view {
                Self::build_status_box(game, frame, split[1]);
            } else {
                Self::build_stats_box(game, frame, split[1]);
            }
        }

        Ok(())
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
            Paragraph::new(commentary)
                .wrap(Wrap { trim: false })
                .block(default_block().title("Commentary")),
            area,
        )
    }

    fn build_stats_table<'a>(players_data: &'a GameStatsMap, players: Vec<&Player>) -> Table<'a> {
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

            let name_span = {
                let style = match player.tiredness {
                    x if x < MIN_TIREDNESS_FOR_ROLL_DECLINE * 0.75 => UiStyle::DEFAULT,
                    x if x < MIN_TIREDNESS_FOR_ROLL_DECLINE * 1.5 => UiStyle::WARNING,
                    x if x < MAX_TIREDNESS => UiStyle::ERROR,
                    _ => UiStyle::UNSELECTABLE,
                };

                Span::styled(player.info.shortened_name(), style)
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

    fn build_player_status_table<'a>(
        players_data: &'a GameStatsMap,
        players: Vec<&Player>,
    ) -> Table<'a> {
        let mut rows: Vec<Row<'_>> = vec![];
        let bars_length = 25;

        for player in players.iter() {
            let player_data = players_data[&player.id].clone();

            let role = match player_data.position {
                Some(p) => (p as Position).as_str().to_string(),
                None => "".to_string(),
            };

            let name_span = {
                let style = match player.tiredness {
                    x if x < MIN_TIREDNESS_FOR_ROLL_DECLINE * 0.75 => UiStyle::DEFAULT,
                    x if x < MIN_TIREDNESS_FOR_ROLL_DECLINE * 1.5 => UiStyle::WARNING,
                    x if x < MAX_TIREDNESS => UiStyle::ERROR,
                    _ => UiStyle::UNSELECTABLE,
                };

                Span::styled(player.info.shortened_name(), style)
            };

            let morale_length = (player.morale / MAX_MORALE * bars_length as f32).round() as usize;
            let morale_string = format!(
                "{}{}",
                "▰".repeat(morale_length),
                "▱".repeat(bars_length - morale_length),
            );
            let morale_style = match player.morale {
                x if x > 1.75 * MORALE_THRESHOLD_FOR_LEAVING => UiStyle::OK,
                x if x > MORALE_THRESHOLD_FOR_LEAVING => UiStyle::WARNING,
                x if x > 0.0 => UiStyle::ERROR,
                _ => UiStyle::UNSELECTABLE,
            };
            let morale_span = Span::styled(morale_string, morale_style);

            let tiredness_length =
                (player.tiredness / MAX_TIREDNESS * bars_length as f32).round() as usize;
            let energy_string = format!(
                "{}{}",
                "▰".repeat(bars_length - tiredness_length),
                "▱".repeat(tiredness_length),
            );
            let energy_style = match player.tiredness {
                x if x < MIN_TIREDNESS_FOR_ROLL_DECLINE * 0.75 => UiStyle::OK,
                x if x < MIN_TIREDNESS_FOR_ROLL_DECLINE * 1.5 => UiStyle::WARNING,
                x if x < MAX_TIREDNESS => UiStyle::ERROR,
                _ => UiStyle::UNSELECTABLE,
            };
            let energy_span = Span::styled(energy_string, energy_style);

            let cells = vec![
                Cell::from(format!("{:<2}", role,)),
                Cell::from(name_span),
                Cell::from(morale_span),
                Cell::from(energy_span),
            ];
            rows.push(Row::new(cells).height(1));
        }

        Table::new(
            rows,
            [
                Constraint::Length(2),
                Constraint::Length(14),
                Constraint::Length(bars_length as u16),
                Constraint::Length(bars_length as u16),
            ],
        )
    }

    fn build_timer_lines(&self, world: &World, game: &Game) -> Vec<Line<'static>> {
        let timer = if self.commentary_index > 0 {
            self.action_results[self.action_results.len() - 1 - self.commentary_index].start_at
        } else {
            game.timer
        };
        let mut timer_lines: Vec<Line> = vec![];
        if !timer.has_started() {
            timer_lines.push(Line::from(Timer::from(timer.period().start()).format()));
            let starting_in_seconds = (game.starting_at - world.last_tick_short_interval) / 1000;
            timer_lines.push(Line::from(format!(
                "Starting in {:02}:{:02}",
                starting_in_seconds / 60,
                starting_in_seconds % 60
            )));
        } else if timer.has_ended() {
            timer_lines.push(Line::from(Timer::from(timer.period().end()).format()));
        } else if timer.is_break() {
            timer_lines.push(Line::from(Timer::from(timer.period().end()).format()));
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

    fn build_status_box(game: &Game, frame: &mut Frame, area: Rect) {
        let header_cells_home = [
            "  ",
            game.home_team_in_game.name.as_str(),
            "Morale",
            "Tiredness",
        ];

        let header_cells_away = [
            "  ",
            game.away_team_in_game.name.as_str(),
            "Morale",
            "Tiredness",
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
            Constraint::Length(2),   //role
            Constraint::Length(16),  //player
            Constraint::Ratio(1, 2), //morale
            Constraint::Ratio(1, 2), //tiredness
        ];

        let home_table =
            Self::build_player_status_table(&game.home_team_in_game.stats, home_players)
                .header(Row::new(header_cells_home).style(UiStyle::HEADER).height(1))
                .widths(constraint);

        let away_table =
            Self::build_player_status_table(&game.away_team_in_game.stats, away_players)
                .header(Row::new(header_cells_away).style(UiStyle::HEADER).height(1))
                .widths(constraint);

        let box_area = Layout::vertical([
            Constraint::Length(game.home_team_in_game.players.len() as u16 + 2),
            Constraint::Max(1),
            Constraint::Length(game.away_team_in_game.players.len() as u16 + 2),
            Constraint::Min(0),
        ])
        .split(area.inner(Margin {
            horizontal: 1,
            vertical: 0,
        }));

        frame.render_widget(home_table, box_area[0]);
        frame.render_widget(away_table, box_area[2]);
    }

    fn build_stats_box(game: &Game, frame: &mut Frame, area: Rect) {
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

        let home_table = Self::build_stats_table(&game.home_team_in_game.stats, home_players)
            .header(Row::new(header_cells_home).style(UiStyle::HEADER).height(1))
            .widths(constraint);

        let away_table = Self::build_stats_table(&game.away_team_in_game.stats, away_players)
            .header(Row::new(header_cells_away).style(UiStyle::HEADER).height(1))
            .widths(constraint);

        let box_area = Layout::vertical([
            Constraint::Length(game.home_team_in_game.players.len() as u16 + 2),
            Constraint::Max(1),
            Constraint::Length(game.away_team_in_game.players.len() as u16 + 2),
            Constraint::Min(0),
        ])
        .split(area.inner(Margin {
            horizontal: 1,
            vertical: 0,
        }));

        frame.render_widget(home_table, box_area[0]);
        frame.render_widget(away_table, box_area[2]);
    }

    pub fn toggle_pitch_view(&mut self) {
        self.pitch_view = !self.pitch_view;
    }

    pub fn toggle_player_status_view(&mut self) {
        self.player_status_view = !self.player_status_view;
    }
}

impl Screen for GamePanel {
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
                Paragraph::new(" No games at the moment!").block(default_block()),
                area,
            );
            return Ok(());
        }

        // Split into top and bottom panels
        let split = Layout::vertical([
            Constraint::Length(PLAYER_IMAGE_HEIGHT as u16 / 2 - 1),
            Constraint::Min(4),
        ])
        .split(area);
        self.build_top_panel(frame, world, split[0])?;
        self.build_bottom_panel(frame, world, split[1])?;

        Ok(())
    }

    fn handle_key_events(
        &mut self,
        key_event: crossterm::event::KeyEvent,
        _world: &World,
    ) -> Option<UiCallbackPreset> {
        match key_event.code {
            KeyCode::Up => self.next_index(),
            KeyCode::Down => self.previous_index(),
            UiKey::PREVIOUS_SELECTION => {
                if self.commentary_index > 0 {
                    self.commentary_index -= 1;
                }
            }
            UiKey::NEXT_SELECTION => {
                if self.commentary_index < self.action_results.len() - 1 {
                    self.commentary_index += 1;
                }
            }
            KeyCode::Enter => self.commentary_index = 0,

            KeyCode::Char('0') => {
                self.pitch_view_filter = None;
            }
            KeyCode::Char('1') => {
                self.pitch_view_filter = Some(Period::Q1);
            }
            KeyCode::Char('2') => {
                self.pitch_view_filter = Some(Period::Q2);
            }
            KeyCode::Char('3') => {
                self.pitch_view_filter = Some(Period::Q3);
            }
            KeyCode::Char('4') => {
                self.pitch_view_filter = Some(Period::Q4);
            }
            _ => {}
        };
        None
    }

    fn footer_spans(&self) -> Vec<Span> {
        let mut v = vec![];

        if self.pitch_view {
            v.append(&mut vec![
                Span::styled(
                    " 0-4 ",
                    Style::default().bg(Color::Gray).fg(Color::DarkGray),
                ),
                Span::styled(
                    format!(
                        " Filter: {:<6} ",
                        if let Some(period) = self.pitch_view_filter {
                            period.to_string()
                        } else {
                            "Full game".to_string()
                        }
                    ),
                    Style::default().fg(Color::DarkGray),
                ),
            ])
        } else {
            v.append(&mut vec![
                Span::styled(
                    format!(
                        " {}/{} ",
                        UiKey::PREVIOUS_SELECTION.to_string(),
                        UiKey::NEXT_SELECTION.to_string()
                    ),
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
            ])
        };
        v
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

// Add test for timer formatting only

#[cfg(test)]
mod tests {
    use crate::{
        engine::timer::Timer,
        types::{SystemTimeTick, Tick},
        world::constants::*,
    };
    use std::{io::Write, thread, time::Duration};

    #[ignore]
    #[test]
    fn test_timer_formatting() {
        let mut stdout = std::io::stdout();
        let mut timer = Timer::new();

        let mut current_time = Tick::now();
        let starting_at = current_time + 5 * SECONDS;

        loop {
            current_time = Tick::now();
            if current_time > starting_at {
                timer.tick();
            }

            if !timer.has_started() {
                let starting_in_seconds = (starting_at - current_time) / SECONDS;
                print!(
                    "{} -- Starting in {:02}:{:02}\r",
                    Timer::from(timer.period().start()).format(),
                    starting_in_seconds / 60,
                    starting_in_seconds % 60
                );
            } else if timer.has_ended() {
                print!("{}\r", Timer::from(timer.period().next().start()).format());
            } else if timer.is_break() {
                print!(
                    "{} -- Resuming in {:02}:{:02}\r",
                    Timer::from(timer.period().next().start()).format(),
                    timer.minutes(),
                    timer.seconds()
                );
            } else {
                print!("{}                       \r", timer.format());
            }
            stdout.flush().unwrap();
            thread::sleep(Duration::from_millis(100));

            if timer.has_ended() {
                break;
            }
        }
    }
}
