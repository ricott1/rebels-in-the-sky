use super::button::Button;
use super::clickable_list::ClickableListState;
use super::constants::*;
use super::gif_map::GifMap;

use super::ui_callback::{CallbackRegistry, UiCallbackPreset};
use super::{
    constants::{PrintableKeyCode, UiKey, IMG_FRAME_WIDTH, LEFT_PANEL_WIDTH},
    traits::{Screen, SplitPanel},
    widgets::{default_block, render_player_description, selectable_list},
};
use crate::types::AppResult;
use crate::world::constants::CURRENCY_SYMBOL;
use crate::world::types::PlayerLocation;
use crate::{
    types::{PlayerId, TeamId},
    world::{player::Player, skill::Rated, world::World},
};
use core::fmt::Debug;
use crossterm::event::KeyCode;
use ratatui::layout::Margin;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    prelude::{CrosstermBackend, Rect},
    style::{Color, Style},
    text::Span,
    widgets::Paragraph,
    Frame,
};
use std::vec;
use std::{cell::RefCell, rc::Rc};
use strum_macros::Display;

#[derive(Debug, Clone, Copy, Display, Default, PartialEq, Eq, Hash)]
pub enum PlayerFilter {
    #[default]
    All,
    FreeAgents,
    OwnTeam,
}

impl PlayerFilter {
    fn next(&self) -> Self {
        match self {
            PlayerFilter::All => PlayerFilter::FreeAgents,
            PlayerFilter::FreeAgents => PlayerFilter::OwnTeam,
            PlayerFilter::OwnTeam => PlayerFilter::All,
        }
    }

    fn rule(&self, player: &Player, own_team_id: TeamId) -> bool {
        match self {
            PlayerFilter::All => true,
            PlayerFilter::FreeAgents => player.team.is_none(),
            PlayerFilter::OwnTeam => player.team.is_some() && player.team.unwrap() == own_team_id,
        }
    }

    fn to_string(&self) -> String {
        match self {
            PlayerFilter::All => "All".to_string(),
            PlayerFilter::FreeAgents => "Free agents".to_string(),
            PlayerFilter::OwnTeam => "Own team".to_string(),
        }
    }
}

#[derive(Debug, Default)]
pub struct PlayerListPanel {
    pub index: usize,
    pub locked_player_id: Option<PlayerId>,
    pub selected_player_id: PlayerId,
    pub selected_team_id: Option<TeamId>,
    pub all_players: Vec<PlayerId>,
    pub players: Vec<PlayerId>,
    own_team_id: TeamId,
    filter: PlayerFilter,
    update_filter: bool,
    tick: usize,
    callback_registry: Rc<RefCell<CallbackRegistry>>,
    gif_map: Rc<RefCell<GifMap>>,
}

impl PlayerListPanel {
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

    fn build_left_panel(
        &mut self,
        frame: &mut Frame<CrosstermBackend<std::io::Stdout>>,
        world: &World,
        area: Rect,
    ) {
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
            "Filter: All".to_string(),
            UiCallbackPreset::SetPlayerPanelFilter {
                filter: PlayerFilter::All,
            },
            Rc::clone(&self.callback_registry),
        );

        let mut filter_free_agents_button = Button::new(
            "Filter: Free agents".to_string(),
            UiCallbackPreset::SetPlayerPanelFilter {
                filter: PlayerFilter::FreeAgents,
            },
            Rc::clone(&self.callback_registry),
        );

        let mut filter_own_team_button = Button::new(
            "Filter: Own team".to_string(),
            UiCallbackPreset::SetPlayerPanelFilter {
                filter: PlayerFilter::OwnTeam,
            },
            Rc::clone(&self.callback_registry),
        );
        match self.filter {
            PlayerFilter::All => filter_all_button.disable(None),
            PlayerFilter::FreeAgents => filter_free_agents_button.disable(None),
            PlayerFilter::OwnTeam => filter_own_team_button.disable(None),
        }

        frame.render_widget(filter_all_button, split[0]);
        frame.render_widget(filter_free_agents_button, split[1]);
        frame.render_widget(filter_own_team_button, split[2]);

        if self.players.len() > 0 {
            let mut options = vec![];
            for &player_id in self.players.iter() {
                let player = world.get_player(player_id);
                if player.is_none() {
                    continue;
                }
                let player = player.unwrap();
                let mut style = UiStyle::DEFAULT;
                if player.team.is_some() && player.team.unwrap() == world.own_team_id {
                    style = UiStyle::OK;
                } else if player.peer_id.is_some() {
                    style = UiStyle::NETWORK;
                }
                let text = format!(
                    "{:<26} {}",
                    format!("{} {}", player.info.first_name, player.info.last_name),
                    player.stars()
                );
                options.push((text, style));
            }
            let list = selectable_list(options, &self.callback_registry);
            frame.render_stateful_widget(
                list.block(default_block().title("Players ↓/↑")),
                split[3],
                &mut ClickableListState::default().with_selected(Some(self.index)),
            );
        } else {
            frame.render_widget(default_block().title("Players"), split[3]);
        }
    }

    fn build_right_panel(
        &self,
        frame: &mut Frame<CrosstermBackend<std::io::Stdout>>,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        let v_split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(24), Constraint::Min(1)])
            .split(area);

        let h_split = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(60),
                Constraint::Length(60),
                Constraint::Min(1),
            ])
            .split(v_split[0]);

        let button_split = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(60),
                Constraint::Length(60),
                Constraint::Min(1),
            ])
            .split(v_split[1]);

        if let Some(player) = world.get_player(self.selected_player_id) {
            // NOTE: here it is okay if the search fails. This is a hack to handle
            //       the FA refresh that could happen while the player panel is locked.
            render_player_description(player, &self.gif_map, self.tick, frame, world, h_split[0]);
            self.render_buttons(
                player,
                frame,
                world,
                button_split[0].inner(&Margin {
                    horizontal: 1,
                    vertical: 0,
                }),
            )?;
        }

        if let Some(locked_player_id) = self.locked_player_id {
            // NOTE: here it is okay if the search fails. This is a hack to handle
            //       the FA refresh that could happen while the player panel is locked.
            if let Some(locked_player) = world.get_player(locked_player_id) {
                render_player_description(
                    locked_player,
                    &self.gif_map,
                    self.tick,
                    frame,
                    world,
                    h_split[1],
                );
                self.render_buttons(
                    locked_player,
                    frame,
                    world,
                    button_split[1].inner(&Margin {
                        horizontal: 1,
                        vertical: 0,
                    }),
                )?;
            }
        }

        Ok(())
    }

    fn render_buttons(
        &self,
        player: &Player,
        frame: &mut Frame<CrosstermBackend<std::io::Stdout>>,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        let own_team = world.get_own_team()?;

        let buttons_split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), //location
                Constraint::Length(3), //hire/release button
                Constraint::Length(3), //refresh info for FA
                Constraint::Min(1),
            ])
            .split(area);

        match player.current_location {
            PlayerLocation::OnPlanet { planet_id } => {
                let planet = world.get_planet_or_err(planet_id).unwrap();
                let button = Button::new(
                    format!("{}: FA on {}", UiKey::GO_TO_PLANET.to_string(), planet.name),
                    UiCallbackPreset::GoToPlanetZoomIn { planet_id },
                    Rc::clone(&self.callback_registry),
                );
                frame.render_widget(button, buttons_split[0]);
            }
            PlayerLocation::WithTeam => {
                let team = world.get_team_or_err(player.team.unwrap())?;
                let button = Button::new(
                    format!(
                        "{}: #{} Team {}",
                        UiKey::GO_TO_TEAM_ALTERNATIVE.to_string(),
                        player.jersey_number.unwrap(),
                        team.name
                    ),
                    UiCallbackPreset::GoToPlayerTeam {
                        player_id: player.id,
                    },
                    Rc::clone(&self.callback_registry),
                );
                frame.render_widget(button, buttons_split[0]);
            }
        }
        let lock_text =
            if self.locked_player_id.is_some() && self.locked_player_id.unwrap() == player.id {
                format!("{}: Unlock", UiKey::UNLOCK_PLAYER.to_string())
            } else {
                format!("{}: Lock", UiKey::LOCK_PLAYER.to_string())
            };
        let lock_button = Button::new(
            lock_text,
            UiCallbackPreset::LockPlayerPanel {
                player_id: self.selected_player_id,
            },
            Rc::clone(&self.callback_registry),
        );
        frame.render_widget(lock_button, buttons_split[1]);

        // Add hire button for free agents
        if player.team.is_none() {
            let can_hire = own_team.can_hire_player(&player);
            let hire_cost = player.hire_cost(own_team.reputation);

            let mut button = Button::new(
                format!(
                    "{}: Hire -{} {}",
                    UiKey::HIRE_FIRE.to_string(),
                    hire_cost,
                    CURRENCY_SYMBOL
                ),
                UiCallbackPreset::HirePlayer {
                    player_id: player.id,
                },
                Rc::clone(&self.callback_registry),
            );
            if can_hire.is_err() {
                button.disable(Some(format!(
                    "{}: {}",
                    UiKey::HIRE_FIRE.to_string(),
                    can_hire.unwrap_err().to_string()
                )));
            }

            frame.render_widget(button, buttons_split[2]);
        }
        // Add release button for own players
        else if player.team.is_some() && player.team.unwrap() == world.own_team_id {
            let can_release = own_team.can_release_player(&player);

            let mut button = Button::new(
                format!("{}: Release", UiKey::HIRE_FIRE.to_string()),
                UiCallbackPreset::ReleasePlayer {
                    player_id: player.id,
                },
                Rc::clone(&self.callback_registry),
            );
            if can_release.is_err() {
                button.disable(Some(format!(
                    "{}: {}",
                    UiKey::HIRE_FIRE.to_string(),
                    can_release.unwrap_err().to_string()
                )));
            }

            frame.render_widget(button, buttons_split[2]);
        }
        Ok(())
    }

    pub fn set_filter(&mut self, filter: PlayerFilter) {
        self.filter = filter;
        self.update_filter = true;
    }

    pub fn reset_filter(&mut self) {
        self.set_filter(PlayerFilter::All);
    }
}

impl Screen for PlayerListPanel {
    fn name(&self) -> &str {
        "Players"
    }

    fn update(&mut self, world: &World) -> AppResult<()> {
        self.tick += 1;
        self.own_team_id = world.own_team_id;
        if world.dirty_ui || self.all_players.len() != world.players.len() {
            self.all_players = world.players.keys().into_iter().cloned().collect();
            self.all_players.sort_by(|a, b| {
                let a = world.get_player(*a).unwrap();
                let b = world.get_player(*b).unwrap();
                if a.rating() == b.rating() {
                    b.total_skills().cmp(&a.total_skills())
                } else {
                    b.rating().cmp(&a.rating())
                }
            });
            self.update_filter = true;
        }
        if self.update_filter {
            self.players = self
                .all_players
                .iter()
                .filter(|&&player_id| {
                    let player = world.get_player(player_id).unwrap();
                    self.filter.rule(player, self.own_team_id)
                })
                .map(|&player_id| player_id)
                .collect();
            self.update_filter = false;
        }

        if self.index >= self.players.len() && self.players.len() > 0 {
            self.set_index(self.players.len() - 1);
        }

        if self.index < self.players.len() {
            self.selected_player_id = self.players[self.index];
            self.selected_team_id = world.get_player(self.selected_player_id).unwrap().team;
        }
        Ok(())
    }
    fn render(
        &mut self,
        frame: &mut Frame<CrosstermBackend<std::io::Stdout>>,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        if self.all_players.len() == 0 {
            frame.render_widget(
                Paragraph::new(" No player yet!"),
                area.inner(&Margin {
                    vertical: 1,
                    horizontal: 1,
                }),
            );
            return Ok(());
        }

        self.callback_registry.borrow_mut().register_callback(
            crossterm::event::MouseEventKind::ScrollDown,
            None,
            UiCallbackPreset::NextPanelIndex,
        );

        self.callback_registry.borrow_mut().register_callback(
            crossterm::event::MouseEventKind::ScrollUp,
            None,
            UiCallbackPreset::PreviousPanelIndex,
        );

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
            UiKey::CYCLE_FILTER => {
                self.set_filter(self.filter.next());
                self.set_index(0);
            }
            UiKey::GO_TO_TEAM_ALTERNATIVE | UiKey::GO_TO_TEAM => {
                if let Some(_) = self.selected_team_id.clone() {
                    return Some(UiCallbackPreset::GoToPlayerTeam {
                        player_id: self.selected_player_id,
                    });
                }
            }

            UiKey::GO_TO_PLANET => {
                return Some(UiCallbackPreset::GoToCurrentPlayerPlanet {
                    player_id: self.selected_player_id,
                })
            }

            UiKey::HIRE_FIRE => {
                let team_id = self.selected_team_id.clone();
                let player_id = self.selected_player_id.clone();
                if team_id.is_none() {
                    // player is a free agent, hire
                    return Some(UiCallbackPreset::HirePlayer { player_id });
                } else if team_id.is_some() && team_id.unwrap() == self.own_team_id {
                    // player is on own team, release
                    return Some(UiCallbackPreset::ReleasePlayer { player_id });
                }
            }
            UiKey::LOCK_PLAYER => {
                if self.locked_player_id.is_none()
                    || self.locked_player_id.unwrap() != self.selected_player_id
                {
                    return Some(UiCallbackPreset::LockPlayerPanel {
                        player_id: self.selected_player_id,
                    });
                }
            }
            UiKey::UNLOCK_PLAYER => {
                if self.locked_player_id.is_some() {
                    return Some(UiCallbackPreset::LockPlayerPanel {
                        player_id: self.selected_player_id,
                    });
                }
            }
            _ => {}
        }
        None
    }

    fn footer_spans(&self) -> Vec<Span> {
        let spans = vec![
            Span::styled(
                format!(" {} ", UiKey::CYCLE_FILTER.to_string()),
                Style::default().bg(Color::Gray).fg(Color::DarkGray),
            ),
            Span::styled(
                format!(" Change filter: {:<12} ", self.filter.to_string()),
                Style::default().fg(Color::DarkGray),
            ),
        ];

        spans
    }
}

impl SplitPanel for PlayerListPanel {
    fn index(&self) -> usize {
        self.index
    }

    fn max_index(&self) -> usize {
        self.players.len()
    }

    fn set_index(&mut self, index: usize) {
        self.index = index;
    }
}
