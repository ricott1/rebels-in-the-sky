use super::button::Button;
use super::clickable_list::ClickableListState;
use super::constants::UiStyle;
use super::gif_map::*;
use super::ui_callback::UiCallback;
use super::ui_frame::UiFrame;
use super::{
    big_numbers::{hyphen, BigNumberFont},
    constants::{IMG_FRAME_WIDTH, LEFT_PANEL_WIDTH},
    traits::{Screen, SplitPanel},
    utils::img_to_lines,
    widgets::{default_block, selectable_list, DOWN_ARROW_SPAN, SWITCH_ARROW_SPAN, UP_ARROW_SPAN},
};
use crate::game_engine::constants::MIN_TIREDNESS_FOR_ROLL_DECLINE;
use crate::types::{AppResult, SystemTimeTick, Tick};
use crate::world::constants::{MAX_PLAYERS_PER_GAME, MORALE_THRESHOLD_FOR_LEAVING};
use crate::world::skill::MAX_SKILL;
use crate::{
    game_engine::{
        action::{ActionOutput, ActionSituation, Advantage},
        game::Game,
        timer::{Period, Timer},
        types::{GameStatsMap, Possession},
    },
    image::game::{PitchImage, PITCH_HEIGHT},
    image::player::{PLAYER_IMAGE_HEIGHT, PLAYER_IMAGE_WIDTH},
    types::GameId,
    ui::constants::*,
    world::{
        planet::PlanetType,
        player::Player,
        position::{GamePosition, Position},
        world::World,
    },
};
use anyhow::anyhow;
use core::fmt::Debug;
use crossterm::event::KeyCode;
use itertools::Itertools;
use ratatui::layout::Margin;
use ratatui::style::Styled;
use ratatui::{
    layout::{Constraint, Layout},
    prelude::Rect,
    text::{Line, Span},
    widgets::{Cell, Paragraph, Row, Table, Wrap},
};
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct GamePanel {
    pub index: usize,
    game_ids: Vec<GameId>,
    pitch_view: bool,
    pitch_view_filter: Option<Period>,
    player_status_view: bool,
    commentary_index: usize,
    action_results: Vec<ActionOutput>,
    tick: usize,
    gif_map: GifMap,
}

impl GamePanel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn toggle_pitch_view(&mut self) {
        self.pitch_view = !self.pitch_view;
    }

    pub fn toggle_player_status_view(&mut self) {
        self.player_status_view = !self.player_status_view;
    }

    pub fn set_active_game(&mut self, game_id: GameId) -> AppResult<()> {
        let index = self
            .game_ids
            .iter()
            .position(|&x| x == game_id)
            .ok_or(anyhow!("Game {:?} not found", game_id))?;

        self.set_index(index);

        Ok(())
    }

    fn selected_game<'a>(&self, world: &'a World) -> Option<&'a Game> {
        if self.index >= self.game_ids.len() {
            return None;
        }

        world.get_game(&self.game_ids[self.index])
    }

    fn build_top_panel(&mut self, frame: &mut UiFrame, world: &World, area: Rect) -> AppResult<()> {
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

    fn build_game_list(&mut self, frame: &mut UiFrame, world: &World, area: Rect) {
        let options = self
            .game_ids
            .iter()
            .map(|id| {
                let game = world.get_game(id).expect("Game should be stored in games.");
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

        let list = selectable_list(options);

        frame.render_stateful_interactive(
            list.block(default_block().title("Games ↓/↑")),
            area,
            &mut ClickableListState::default().with_selected(Some(self.index)),
        );
    }

    fn build_game_buttons(&mut self, frame: &mut UiFrame, area: Rect) {
        let b_split =
            Layout::horizontal([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)]).split(area);
        let text = if self.pitch_view {
            "Commentary view"
        } else {
            "Game view"
        };
        let pitch_button = Button::new(text, UiCallback::TogglePitchView)
            .set_hover_text(format!(
                "Change to {} view",
                if self.pitch_view {
                    "commentary"
                } else {
                    "pitch"
                }
            ))
            .set_hotkey(UiKey::PITCH_VIEW);

        frame.render_interactive(pitch_button, b_split[0]);

        let text = if self.player_status_view {
            "Game stats"
        } else {
            "Player status"
        };
        let player_status_button = Button::new(text, UiCallback::TogglePlayerStatusView)
            .set_hover_text(format!(
                "Change to {} view",
                if self.player_status_view {
                    "game box"
                } else {
                    "player status"
                }
            ))
            .set_hotkey(UiKey::PLAYER_STATUS_VIEW);

        frame.render_interactive(player_status_button, b_split[1]);
    }

    fn build_score_panel(
        &mut self,
        frame: &mut UiFrame,
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
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(2),
            Constraint::Length(8),
            Constraint::Length(margin_height),
        ])
        .split(top_split[2]);

        frame.render_widget(
            Paragraph::new(format!(
                "Playing on {}",
                world.get_planet_or_err(&game.location)?.name
            ))
            .centered(),
            central_split[3],
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
        .split(central_split[4]);

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
            .player_frame_lines(&base_home_player, self.tick)
        {
            lines.remove(0);
            let paragraph = Paragraph::new(lines).centered();
            frame.render_widget(paragraph, top_split[1]);
        }
        if let Ok(mut lines) = self
            .gif_map
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
        let l = MAX_NAME_LENGTH + 2;
        frame.render_widget(
            Paragraph::new(Line::from(format!(
                "{:>l$} vs {:<l$}",
                format!("{} {}", home_dot, game.home_team_in_game.name),
                format!("{} {}", game.away_team_in_game.name, away_dot),
            )))
            .centered(),
            central_split[1],
        );

        let timer_lines = self.build_timer_lines(world, game);
        frame.render_widget(Paragraph::new(timer_lines).centered(), central_split[5]);
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
        frame: &mut UiFrame,
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

        let planet = world.get_planet_or_err(&game.location)?;
        let pitch_style = match planet.planet_type {
            PlanetType::Earth => PitchImage::PitchFancy,
            PlanetType::Ring | PlanetType::Gas => PitchImage::PitchPlanet,
            PlanetType::Rocky => PitchImage::PitchBall,
            _ => PitchImage::PitchClassic,
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
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        let split = Layout::horizontal([Constraint::Min(8), Constraint::Length(73)]).split(area);
        if let Some(game) = self.selected_game(world) {
            let mut shot_img = None;
            // Display shot gif if the last action was a made 3 or if it was a substitution and the second last was a made 3.
            if let Some(last_action) = game.action_results.last() {
                let mut should_display_shot_gif_for = None;

                if last_action.score_change == 3 {
                    should_display_shot_gif_for = Some(last_action.possession);
                } else if last_action.situation == ActionSituation::AfterSubstitution
                    && game.action_results.len() > 1
                {
                    let second_last_action = &game.action_results[game.action_results.len() - 2];
                    if second_last_action.score_change == 3 {
                        should_display_shot_gif_for = Some(second_last_action.possession);
                    }
                }

                if let Some(side) = should_display_shot_gif_for {
                    let shot_tick = game.starting_at + last_action.start_at.as_tick();
                    let now = Tick::now();
                    let shot_frame = now.saturating_sub(shot_tick) as usize / 140;
                    if shot_frame < RIGHT_SHOT_GIF.len() {
                        // After scoring the possesion is flipped, so the opposite team scored.
                        if side == Possession::Home {
                            shot_img = Some(&RIGHT_SHOT_GIF[shot_frame]);
                        } else {
                            shot_img = Some(&LEFT_SHOT_GIF[shot_frame]);
                        }
                    }
                }
            }

            if let Some(img) = shot_img {
                frame.render_widget(Paragraph::new(img.clone()).centered(), split[0]);
            } else if self.pitch_view {
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
        action_result: &ActionOutput,
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
        let text = Span::from(format!("{} ", action_result.description));
        Line::from(vec![timer, text, arrow])
    }

    fn build_commentary(&mut self, frame: &mut UiFrame, area: Rect) {
        let mut commentary = vec![];
        let max_index = self.action_results.len() - self.commentary_index;

        for idx in 0..max_index {
            let result = &self.action_results[idx];
            let situation = &result.situation;
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
        let mut brawls_total = 0;
        let mut plus_minus_total = 0;

        for player in players.iter() {
            let player_data = &players_data[&player.id];
            points_total += player_data.points;
            attempted_2pt_total += player_data.attempted_2pt;
            made_2pt_total += player_data.made_2pt;
            attempted_3pt_total += player_data.attempted_3pt;
            made_3pt_total += player_data.made_3pt;
            assists_total += player_data.assists;
            turnovers_total += player_data.turnovers;
            defensive_rebounds_total += player_data.defensive_rebounds;
            offensive_rebounds_total += player_data.offensive_rebounds;
            steals_total += player_data.steals;
            blocks_total += player_data.blocks;
            brawls_total += player_data.brawls.iter().sum::<u16>();
            plus_minus_total += player_data.plus_minus as i16;

            let role = match player_data.position {
                Some(p) => (p as Position).as_str().to_string(),
                None => "".to_string(),
            };

            let name_span = {
                let style = match player.tiredness {
                    x if x < MIN_TIREDNESS_FOR_ROLL_DECLINE * 0.75 => UiStyle::DEFAULT,
                    x if x < MIN_TIREDNESS_FOR_ROLL_DECLINE * 1.5 => UiStyle::WARNING,
                    x if x < MAX_SKILL => UiStyle::ERROR,
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
                Cell::from(format!(
                    "{:^3}",
                    players_data[&player.id].brawls.iter().sum::<u16>()
                )),
                Cell::from(format!("{:>+3}", players_data[&player.id].plus_minus)),
            ];
            rows.push(Row::new(cells).height(1));
        }

        // We want the totals to be always at the bottom, exactly as the (MAX_PLAYERS_PER_GAME + 3)-th row
        while rows.len() < MAX_PLAYERS_PER_GAME + 1 {
            rows.push(Row::default().height(1));
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
            Cell::from(format!("{:^3}", brawls_total)),
            Cell::from(format!("{:>+3}", plus_minus_total / 5)),
        ];

        rows.push(Row::new(totals).set_style(UiStyle::HIGHLIGHT));

        Table::new(
            rows,
            [
                Constraint::Length(2),
                Constraint::Length(MAX_NAME_LENGTH as u16 + 2),
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
            let player_data = &players_data[&player.id];

            let role = match player_data.position {
                Some(p) => (p as Position).as_str().to_string(),
                None => "".to_string(),
            };

            let name_span = {
                let style = match player.tiredness {
                    x if x < MIN_TIREDNESS_FOR_ROLL_DECLINE * 0.75 => UiStyle::DEFAULT,
                    x if x < MIN_TIREDNESS_FOR_ROLL_DECLINE * 1.5 => UiStyle::WARNING,
                    x if x < MAX_SKILL => UiStyle::ERROR,
                    _ => UiStyle::UNSELECTABLE,
                };

                Span::styled(player.info.shortened_name(), style)
            };

            let morale_length = (player.morale / MAX_SKILL * bars_length as f32).round() as usize;
            let morale_string = format!(
                "{}{}",
                "▰".repeat(morale_length),
                "▱".repeat(bars_length - morale_length),
            );
            let morale_style = match player.morale {
                x if x > 5.0 * MORALE_THRESHOLD_FOR_LEAVING => UiStyle::OK,
                x if x > MORALE_THRESHOLD_FOR_LEAVING => UiStyle::WARNING,
                x if x > 0.0 => UiStyle::ERROR,
                _ => UiStyle::UNSELECTABLE,
            };
            let morale_span = Span::styled(morale_string, morale_style);

            let tiredness_length =
                (player.tiredness / MAX_SKILL * bars_length as f32).round() as usize;
            let energy_string = format!(
                "{}{}",
                "▰".repeat(bars_length - tiredness_length),
                "▱".repeat(tiredness_length),
            );
            let energy_style = match player.tiredness {
                x if x < MIN_TIREDNESS_FOR_ROLL_DECLINE * 0.75 => UiStyle::OK,
                x if x < MIN_TIREDNESS_FOR_ROLL_DECLINE * 1.5 => UiStyle::WARNING,
                x if x < MAX_SKILL => UiStyle::ERROR,
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
                Constraint::Length(MAX_NAME_LENGTH as u16 + 2),
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
            timer_lines.push(Line::from(format!(
                "Starting in {}",
                game.starting_at
                    .saturating_sub(world.last_tick_short_interval)
                    .formatted()
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

    fn build_status_box(game: &Game, frame: &mut UiFrame, area: Rect) {
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
            Constraint::Length(2),                          //role
            Constraint::Length(MAX_NAME_LENGTH as u16 + 2), //player
            Constraint::Ratio(1, 2),                        //morale
            Constraint::Ratio(1, 2),                        //tiredness
        ];

        let home_table =
            Self::build_player_status_table(&game.home_team_in_game.stats, home_players)
                .header(
                    Row::new([
                        "  ",
                        game.home_team_in_game.name.as_str(),
                        "Morale",
                        "Tiredness",
                    ])
                    .style(UiStyle::HEADER)
                    .height(1),
                )
                .widths(constraint);

        let away_table =
            Self::build_player_status_table(&game.away_team_in_game.stats, away_players)
                .header(
                    Row::new([
                        "  ",
                        game.away_team_in_game.name.as_str(),
                        "Morale",
                        "Tiredness",
                    ])
                    .style(UiStyle::HEADER)
                    .height(1),
                )
                .widths(constraint);

        let box_area = Layout::vertical([
            Constraint::Ratio(1, 2),
            Constraint::Ratio(1, 2),
            Constraint::Min(0),
        ])
        .split(area.inner(Margin {
            horizontal: 1,
            vertical: 0,
        }));

        let home_box_split = Layout::vertical([Constraint::Min(0), Constraint::Length(1)])
            .split(box_area[0].inner(Margin::new(1, 1)));
        let away_box_split = Layout::vertical([Constraint::Min(0), Constraint::Length(1)])
            .split(box_area[1].inner(Margin::new(1, 1)));

        frame.render_widget(default_block(), box_area[0]);
        frame.render_widget(home_table, home_box_split[0]);
        frame.render_widget(
            Span::styled(
                format!("   Tactic: {}", game.home_team_in_game.tactic),
                UiStyle::HIGHLIGHT,
            ),
            home_box_split[1],
        );
        frame.render_widget(default_block(), box_area[1]);
        frame.render_widget(away_table, away_box_split[0]);
        frame.render_widget(
            Span::styled(
                format!("   Tactic: {}", game.away_team_in_game.tactic),
                UiStyle::HIGHLIGHT,
            ),
            away_box_split[1],
        );
    }

    fn build_stats_box(game: &Game, frame: &mut UiFrame, area: Rect) {
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
            "Brw",
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
            "Brw",
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
            Constraint::Length(2),                          //role
            Constraint::Length(MAX_NAME_LENGTH as u16 + 2), //player
            Constraint::Length(3),                          //minutes
            Constraint::Length(3),                          //points
            Constraint::Length(6),                          //2pt
            Constraint::Length(5),                          //3pt
            Constraint::Length(6),                          //assists/turnovers
            Constraint::Length(7),                          //defensive rebounds/offensive rebounds
            Constraint::Length(3),                          //steals
            Constraint::Length(3),                          //blocks
            Constraint::Length(3),                          //brawls
            Constraint::Length(3),                          //plus minus
            Constraint::Min(0),
        ];

        let home_table = Self::build_stats_table(&game.home_team_in_game.stats, home_players)
            .header(Row::new(header_cells_home).style(UiStyle::HEADER).height(1))
            .widths(constraint);

        let away_table = Self::build_stats_table(&game.away_team_in_game.stats, away_players)
            .header(Row::new(header_cells_away).style(UiStyle::HEADER).height(1))
            .widths(constraint);

        let box_area = Layout::vertical([
            Constraint::Ratio(1, 2),
            Constraint::Ratio(1, 2),
            Constraint::Min(0),
        ])
        .split(area.inner(Margin {
            horizontal: 1,
            vertical: 0,
        }));

        frame.render_widget(home_table.block(default_block()), box_area[0]);
        frame.render_widget(away_table.block(default_block()), box_area[1]);
    }
}

impl Screen for GamePanel {
    fn update(&mut self, world: &World) -> AppResult<()> {
        self.tick += 1;

        self.game_ids = world
            .games
            .iter()
            .sorted_by(|&(_, a), &(_, b)| a.starting_at.cmp(&b.starting_at))
            .map(|(k, _)| k.clone())
            .collect_vec();

        if world.dirty_ui {
            // Try to keep track of current game when other games finish
            if let Some(game) = self.selected_game(world) {
                self.set_index(
                    self.game_ids
                        .iter()
                        .position(|&id| id == game.id)
                        .unwrap_or_default(),
                );
            }
        }

        if let Some(game) = self.selected_game(world) {
            if self.commentary_index == 0 {
                // FIXME: we dont need to clone, remove self.action_results completely
                self.action_results = game.action_results.clone();
            }
        } else {
            self.set_index(0);
        }
        Ok(())
    }

    fn render(
        &mut self,
        frame: &mut UiFrame,
        world: &World,
        area: Rect,

        _debug_view: bool,
    ) -> AppResult<()> {
        if self.game_ids.len() == 0 {
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
    ) -> Option<UiCallback> {
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

    fn footer_spans(&self) -> Vec<String> {
        let mut v = vec![];

        if self.pitch_view {
            v.append(&mut vec![
                " 0-4 ".to_string(),
                format!(
                    " Filter: {:<6} ",
                    if let Some(period) = self.pitch_view_filter {
                        period.to_string()
                    } else {
                        "Full game".to_string()
                    }
                ),
            ])
        } else {
            v.append(&mut vec![
                format!(
                    " {}/{} ",
                    UiKey::PREVIOUS_SELECTION.to_string(),
                    UiKey::NEXT_SELECTION.to_string()
                ),
                " Scroll commentary ".to_string(),
                " Enter ".to_string(),
                " Scroll commentary to top ".to_string(),
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
        self.game_ids.len()
    }

    fn set_index(&mut self, index: usize) {
        self.index = index;
        self.commentary_index = 0;
    }
}
