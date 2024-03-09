use super::button::{Button, RadioButton};
use super::clickable_list::ClickableListState;
use super::gif_map::GifMap;
use super::ui_callback::{CallbackRegistry, UiCallbackPreset};
use super::widgets::render_spaceship_description;
use super::{
    constants::{UiKey, UiStyle, LEFT_PANEL_WIDTH},
    traits::{Screen, SplitPanel},
    utils::img_to_lines,
    widgets::{default_block, selectable_list},
};
use crate::image::spaceship::{SPACESHIP_IMAGE_HEIGHT, SPACESHIP_IMAGE_WIDTH};
use crate::types::{AppResult, SystemTimeTick};
use crate::world::position::MAX_POSITION;
use crate::world::team::Team;
use crate::world::types::TeamLocation;
use crate::{
    image::pitch::floor_from_size,
    image::player::{PLAYER_IMAGE_HEIGHT, PLAYER_IMAGE_WIDTH},
    types::{PlayerId, TeamId},
    ui::constants::PrintableKeyCode,
    world::{
        position::{GamePosition, Position},
        skill::Rated,
        world::World,
    },
};
use core::fmt::Debug;
use crossterm::event::KeyCode;
use ratatui::layout::Margin;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    prelude::Rect,
    style::{Color, Style},
    text::Span,
    widgets::{Borders, Paragraph},
    Frame,
};
use std::vec;
use std::{sync::Arc, sync::Mutex};
use strum_macros::Display;

const IMG_FRAME_WIDTH: u16 = 80;

#[derive(Debug, Clone, Copy, Display, Default, PartialEq, Eq, Hash)]
pub enum TeamFilter {
    #[default]
    All,
    OpenToChallenge,
    Peers,
}

impl TeamFilter {
    fn next(&self) -> Self {
        match self {
            TeamFilter::All => TeamFilter::OpenToChallenge,
            TeamFilter::OpenToChallenge => TeamFilter::Peers,
            TeamFilter::Peers => TeamFilter::All,
        }
    }

    fn rule(&self, team: &Team, own_team: &Team) -> bool {
        match self {
            TeamFilter::All => true,
            TeamFilter::OpenToChallenge => team.can_challenge_team(own_team).is_ok(),
            TeamFilter::Peers => team.peer_id.is_some(),
        }
    }

    fn to_string(&self) -> String {
        match self {
            TeamFilter::All => "All".to_string(),
            TeamFilter::OpenToChallenge => "Open to challenge".to_string(),
            TeamFilter::Peers => "From swarm".to_string(),
        }
    }
}

#[derive(Debug, Default)]
pub struct TeamListPanel {
    pub index: usize,
    pub player_index: usize,
    pub selected_player_id: PlayerId,
    pub selected_team_id: TeamId,
    pub teams: Vec<TeamId>,
    pub all_teams: Vec<TeamId>,
    filter: TeamFilter,
    update_filter: bool,
    current_team_players_length: usize,
    tick: usize,
    callback_registry: Arc<Mutex<CallbackRegistry>>,
    gif_map: Arc<Mutex<GifMap>>,
}

impl TeamListPanel {
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

    fn next_player_index(&mut self) {
        if self.current_team_players_length == 0 {
            return;
        }
        let current_index = self.player_index;
        self.player_index = (current_index + 1) % self.current_team_players_length;
    }

    fn previous_player_index(&mut self) {
        if self.current_team_players_length == 0 {
            return;
        }
        let current_index = self.player_index;
        self.player_index = (current_index + self.current_team_players_length - 1)
            % self.current_team_players_length;
    }

    pub fn set_filter(&mut self, filter: TeamFilter) {
        self.filter = filter;
        self.update_filter = true;
    }

    pub fn reset_filter(&mut self) {
        self.set_filter(TeamFilter::All);
    }

    fn build_left_panel(&mut self, frame: &mut Frame, world: &World, area: Rect) {
        let split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Min(1),
            ])
            .split(area);

        let mut filter_all_button = Button::new(
            format!("Filter: {}", TeamFilter::All.to_string()),
            UiCallbackPreset::SetTeamPanelFilter {
                filter: TeamFilter::All,
            },
            Arc::clone(&self.callback_registry),
        );

        let mut filter_challenge_button = Button::new(
            format!("Filter: {}", TeamFilter::OpenToChallenge.to_string()),
            UiCallbackPreset::SetTeamPanelFilter {
                filter: TeamFilter::OpenToChallenge,
            },
            Arc::clone(&self.callback_registry),
        );

        let mut filter_peers_button = Button::new(
            format!("Filter: {}", TeamFilter::Peers.to_string()),
            UiCallbackPreset::SetTeamPanelFilter {
                filter: TeamFilter::Peers,
            },
            Arc::clone(&self.callback_registry),
        );
        match self.filter {
            TeamFilter::All => filter_all_button.disable(None),
            TeamFilter::OpenToChallenge => filter_challenge_button.disable(None),
            TeamFilter::Peers => filter_peers_button.disable(None),
        }

        frame.render_widget(filter_all_button, split[0]);
        frame.render_widget(filter_challenge_button, split[1]);
        frame.render_widget(filter_peers_button, split[2]);

        if self.teams.len() > 0 {
            let mut options = vec![];
            for &team_id in self.teams.iter() {
                let team = world.get_team(team_id);
                if team.is_none() {
                    continue;
                }
                let team = team.unwrap();
                let mut style = UiStyle::DEFAULT;
                if team.id == world.own_team_id {
                    style = UiStyle::OWN_TEAM;
                } else if team.peer_id.is_some() {
                    style = UiStyle::NETWORK;
                }
                let text = format!("{:<14} {}", team.name, world.team_rating(team.id).stars());
                options.push((text, style));
            }
            let list = selectable_list(options, &self.callback_registry);

            frame.render_stateful_widget(
                list.block(default_block().title("Teams ↓/↑")),
                split[3],
                &mut ClickableListState::default().with_selected(Some(self.index)),
            );
        } else {
            frame.render_widget(default_block().title("Teams"), split[3]);
        }
    }

    fn build_right_panel(&mut self, frame: &mut Frame, world: &World, area: Rect) -> AppResult<()> {
        if self.index >= self.teams.len() {
            return Ok(());
        }
        let team = world.get_team_or_err(self.teams[self.index]).unwrap();
        self.current_team_players_length = team.player_ids.len();
        let vertical_split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(PLAYER_IMAGE_HEIGHT as u16 / 2), //players
                Constraint::Length(1),                              //floor
                Constraint::Length(1),                              //name
                Constraint::Length(2),                              //rating
                Constraint::Min(1),                                 //bottom
            ])
            .split(area.inner(&Margin {
                horizontal: 1,
                vertical: 0,
            }));

        let floor = floor_from_size(area.width as u32, 2);
        frame.render_widget(
            Paragraph::new(img_to_lines(&floor)).alignment(Alignment::Center),
            vertical_split[1],
        );

        let side_length: u16;
        if area.width > (PLAYER_IMAGE_WIDTH as u16 + 2) * 5 {
            side_length = (area.width - (PLAYER_IMAGE_WIDTH as u16 + 2) * 5) / 2;
        } else {
            side_length = 0;
        }

        let constraints = [
            Constraint::Min(side_length),
            Constraint::Min(PLAYER_IMAGE_WIDTH as u16 + 2),
            Constraint::Min(PLAYER_IMAGE_WIDTH as u16 + 2),
            Constraint::Min(PLAYER_IMAGE_WIDTH as u16 + 2),
            Constraint::Min(PLAYER_IMAGE_WIDTH as u16 + 2),
            Constraint::Min(PLAYER_IMAGE_WIDTH as u16 + 2),
            Constraint::Min(side_length),
        ];

        let player_img_split = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(vertical_split[0]);

        let player_name_split = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(vertical_split[2]);

        let player_rating_split = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(vertical_split[3]);

        for i in 0..MAX_POSITION as usize {
            if i >= team.player_ids.len() {
                break;
            }

            // recalculate button area: to offset the missing box of the radiobutton
            // we add an extra row to top and bottom
            let button_area = Rect {
                x: player_img_split[i + 1].x,
                y: player_img_split[i + 1].y,
                width: player_img_split[i + 1].width,
                height: player_img_split[i + 1].height + 1,
            };

            let button = RadioButton::no_box(
                "".to_string(),
                UiCallbackPreset::GoToPlayer {
                    player_id: team.player_ids[i],
                },
                Arc::clone(&self.callback_registry),
                &mut self.player_index,
                i,
            );
            frame.render_widget(button, button_area);

            let player = world.get_player_or_err(team.player_ids[i])?;

            if let Ok(lines) = self
                .gif_map
                .lock()
                .unwrap()
                .player_frame_lines(player, self.tick)
            {
                frame.render_widget(
                    Paragraph::new(lines).alignment(Alignment::Center),
                    player_img_split[i + 1],
                );
            }

            let name = format!(
                "{}. {} #{} ",
                player.info.first_name.chars().next().unwrap_or_default(),
                player.info.last_name,
                player.jersey_number.unwrap()
            );
            frame.render_widget(
                Paragraph::new(name).alignment(Alignment::Center),
                player_name_split[i + 1],
            );

            frame.render_widget(
                Paragraph::new(format!(
                    "{} {}",
                    (i as Position).as_str(),
                    (i as Position)
                        .player_rating(player.current_skill_array())
                        .stars()
                ))
                .alignment(Alignment::Center),
                player_rating_split[i + 1],
            );
        }

        let bottom_split = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(44),
                Constraint::Length(SPACESHIP_IMAGE_WIDTH as u16 + 2 + 34),
            ])
            .split(vertical_split[4]);

        frame.render_widget(default_block().title("Bench"), bottom_split[0]);

        if team.player_ids.len() > 5 {
            let bench_row_split = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(4),
                    Constraint::Length(4),
                    Constraint::Length(4),
                    Constraint::Min(0),
                ])
                .split(bottom_split[0].inner(&Margin {
                    horizontal: 1,
                    vertical: 1,
                }));

            for (i, &player_id) in team.player_ids.iter().skip(5).enumerate() {
                if let Some(player) = world.get_player(player_id) {
                    let info = format!(
                        "{}. {} #{}\n",
                        player.info.first_name.chars().next().unwrap_or_default(),
                        player.info.last_name,
                        player.jersey_number.unwrap()
                    );
                    let skills = player.current_skill_array();
                    let best_role = Position::best(skills);

                    let role_info = format!(
                        "{:<2} {:<5}",
                        best_role.as_str(),
                        best_role.player_rating(skills).stars()
                    );
                    let button = RadioButton::new(
                        format!("{}{}", info, role_info),
                        UiCallbackPreset::GoToPlayer {
                            player_id: team.player_ids[i + 5],
                        },
                        Arc::clone(&self.callback_registry),
                        &mut self.player_index,
                        i + 5,
                    );
                    let row = i / 2;

                    let bench_column_split = Layout::default()
                        .direction(Direction::Horizontal)
                        .constraints([Constraint::Length(20), Constraint::Length(20)])
                        .split(bench_row_split[row]);
                    let column = i % 2;
                    frame.render_widget(button, bench_column_split[column]);
                }
            }
        }

        let ship_buttons_split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(SPACESHIP_IMAGE_HEIGHT as u16 / 2 + 2), // ship
                Constraint::Length(3),                                     //button
                Constraint::Min(0),
            ])
            .split(bottom_split[1]);

        let button_split = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
            .split(ship_buttons_split[1].inner(&Margin {
                horizontal: 1,
                vertical: 0,
            }));

        let own_team = world.get_own_team()?;
        if team.id != world.own_team_id {
            match team.current_location {
                TeamLocation::OnPlanet { planet_id } => {
                    let location = world.get_planet_or_err(planet_id)?;
                    let planet = world.get_planet_or_err(planet_id)?;

                    let travel_time = world.travel_time_to_planet(own_team.id, planet.id);
                    let can_travel = match travel_time {
                        Ok(time) => own_team.can_travel_to_planet(&planet, time),
                        Err(e) => Err(e),
                    };

                    let mut button = Button::new(
                        format!(
                            "{}: On planet {} ",
                            UiKey::GO_TO_PLANET.to_string(),
                            location.name
                        ),
                        UiCallbackPreset::GoToCurrentTeamPlanet { team_id: team.id },
                        Arc::clone(&self.callback_registry),
                    );
                    if can_travel.is_err() {
                        button.disable(Some(format!(
                            "{}: {}",
                            UiKey::GO_TO_PLANET.to_string(),
                            can_travel.unwrap_err().to_string()
                        )));
                    }

                    frame.render_widget(button, button_split[0]);
                }
                TeamLocation::Travelling {
                    from: _from,
                    to,
                    started,
                    duration,
                } => {
                    let to = world.get_planet_or_err(to)?.name.to_string();
                    let duration = if started + duration > world.last_tick_short_interval {
                        (started + duration - world.last_tick_short_interval).formatted()
                    } else {
                        "landing".into()
                    };

                    let mut button = Button::new(
                        format!("Travelling to {} {}", to, duration),
                        UiCallbackPreset::None,
                        Arc::clone(&self.callback_registry),
                    );
                    button.disable(None);
                    frame.render_widget(button, button_split[0]);
                }
            }

            let can_challenge = own_team.can_challenge_team(team);

            let mut button = Button::new(
                format!("{}: Challenge", UiKey::CHALLENGE_TEAM.to_string()),
                UiCallbackPreset::ChallengeTeam { team_id: team.id },
                Arc::clone(&self.callback_registry),
            );
            if can_challenge.is_err() {
                button.disable(Some(format!(
                    "{}: {}",
                    UiKey::CHALLENGE_TEAM.to_string(),
                    can_challenge.unwrap_err().to_string()
                )));
            } else {
                button = button.set_box_style(UiStyle::NETWORK);
            }

            frame.render_widget(button, button_split[1])
        }

        render_spaceship_description(
            &team,
            &self.gif_map,
            self.tick,
            world,
            frame,
            bottom_split[1],
        );

        let box_split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(PLAYER_IMAGE_HEIGHT as u16 / 2 + 1),
                Constraint::Min(0),
            ])
            .split(area);

        frame.render_widget(
            default_block()
                .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
                .title(format!(" {} ", team.name))
                .title_alignment(Alignment::Left),
            box_split[0],
        );

        Ok(())
    }
}

impl Screen for TeamListPanel {
    fn name(&self) -> &str {
        "Teams"
    }

    fn update(&mut self, world: &World) -> AppResult<()> {
        self.tick += 1;
        if world.dirty_ui || self.all_teams.len() != world.teams.len() {
            self.all_teams = world.teams.keys().into_iter().cloned().collect();
            self.all_teams.sort_by(|a, b| {
                let a = world.get_team_or_err(*a).unwrap();
                let b = world.get_team_or_err(*b).unwrap();
                world
                    .team_rating(b.id)
                    .partial_cmp(&world.team_rating(a.id))
                    .unwrap()
            });
            self.update_filter = true;
        }

        if self.update_filter {
            self.teams = self
                .all_teams
                .iter()
                .filter(|&&team_id| {
                    let team = world.get_team_or_err(team_id).unwrap();
                    self.filter.rule(team, world.get_own_team().unwrap())
                })
                .map(|&player_id| player_id)
                .collect();
            self.update_filter = false;
        }

        if self.index >= self.teams.len() && self.teams.len() > 0 {
            self.set_index(self.teams.len() - 1);
        }
        if self.index < self.teams.len() {
            self.selected_team_id = self.teams[self.index];
            let players = world
                .get_team_or_err(self.selected_team_id)?
                .player_ids
                .clone();
            if self.player_index < players.len() {
                self.selected_player_id = players[self.player_index];
            }
        }
        Ok(())
    }
    fn render(&mut self, frame: &mut Frame, world: &World, area: Rect) -> AppResult<()> {
        if self.all_teams.len() == 0 {
            frame.render_widget(
                Paragraph::new(" No team yet!"),
                area.inner(&Margin {
                    vertical: 1,
                    horizontal: 1,
                }),
            );
            return Ok(());
        }

        // Split into left and right panels
        let left_right_split = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(LEFT_PANEL_WIDTH),
                Constraint::Min(IMG_FRAME_WIDTH),
            ])
            .split(area);
        self.build_left_panel(frame, world, left_right_split[0]);
        self.build_right_panel(frame, world, left_right_split[1])?;
        Ok(())
    }

    fn handle_key_events(
        &mut self,
        key_event: crossterm::event::KeyEvent,
    ) -> Option<UiCallbackPreset> {
        match key_event.code {
            KeyCode::Up => self.next_index(),
            KeyCode::Down => self.previous_index(),
            KeyCode::Right => self.next_player_index(),
            KeyCode::Left => self.previous_player_index(),
            UiKey::CYCLE_FILTER => {
                self.set_filter(self.filter.next());
                self.set_index(0);
            }
            KeyCode::Enter => {
                let player_id = self.selected_player_id.clone();
                return Some(UiCallbackPreset::GoToPlayer { player_id });
            }
            UiKey::GO_TO_PLANET | KeyCode::Backspace => {
                let team_id = self.selected_team_id.clone();
                return Some(UiCallbackPreset::GoToCurrentTeamPlanet { team_id });
            }
            UiKey::CHALLENGE_TEAM => {
                let team_id: uuid::Uuid = self.selected_team_id.clone();
                return Some(UiCallbackPreset::ChallengeTeam { team_id });
            }
            _ => {}
        }
        None
    }

    fn footer_spans(&self) -> Vec<Span> {
        vec![
            Span::styled(
                format!(" {} ", UiKey::CYCLE_FILTER.to_string()),
                Style::default().bg(Color::Gray).fg(Color::DarkGray),
            ),
            Span::styled(
                format!(" Change filter: {:<12} ", self.filter.to_string()),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                " ←/→ ",
                Style::default().bg(Color::Gray).fg(Color::DarkGray),
            ),
            Span::styled(" Select player ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!(" {} ", KeyCode::Backspace.to_string()),
                Style::default().bg(Color::Gray).fg(Color::DarkGray),
            ),
            Span::styled(
                format!(" Go to planet "),
                Style::default().fg(Color::DarkGray),
            ),
        ]
    }
}

impl SplitPanel for TeamListPanel {
    fn index(&self) -> usize {
        self.index
    }

    fn max_index(&self) -> usize {
        self.teams.len()
    }

    fn set_index(&mut self, index: usize) {
        self.index = index;
    }
}
