use super::button::Button;
use super::clickable_list::ClickableListState;
use super::constants::*;
use super::gif_map::GifMap;

use super::ui_callback::{CallbackRegistry, UiCallbackPreset};
use super::utils::hover_text_target;
use super::{
    constants::{UiKey, IMG_FRAME_WIDTH, LEFT_PANEL_WIDTH},
    traits::{Screen, SplitPanel},
    widgets::{default_block, render_player_description, selectable_list},
};
use crate::types::AppResult;
use crate::world::constants::CURRENCY_SYMBOL;
use crate::world::team::Team;
use crate::world::types::PlayerLocation;
use crate::{
    types::{PlayerId, TeamId},
    world::{player::Player, skill::Rated, world::World},
};
use core::fmt::Debug;
use crossterm::event::KeyCode;
use ratatui::layout::Margin;
use ratatui::{
    layout::{Constraint, Layout},
    prelude::Rect,
    widgets::Paragraph,
    Frame,
};
use std::vec;
use std::{sync::Arc, sync::Mutex};
use strum_macros::Display;

#[derive(Debug, Clone, Copy, Display, Default, PartialEq, Eq, Hash)]
pub enum PlayerView {
    #[default]
    All,
    FreeAgents,
    OwnTeam,
}

impl PlayerView {
    fn next(&self) -> Self {
        match self {
            PlayerView::All => PlayerView::FreeAgents,
            PlayerView::FreeAgents => PlayerView::OwnTeam,
            PlayerView::OwnTeam => PlayerView::All,
        }
    }

    fn rule(&self, player: &Player, own_team: &Team) -> bool {
        match self {
            PlayerView::All => true,
            PlayerView::FreeAgents => {
                player.team.is_none() && own_team.can_hire_player(player).is_ok()
            }
            PlayerView::OwnTeam => player.team.is_some() && player.team.unwrap() == own_team.id,
        }
    }

    fn to_string(&self) -> String {
        match self {
            PlayerView::All => "All".to_string(),
            PlayerView::FreeAgents => "Hirable Free agents".to_string(),
            PlayerView::OwnTeam => "Own team".to_string(),
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
    view: PlayerView,
    update_view: bool,
    tick: usize,
    callback_registry: Arc<Mutex<CallbackRegistry>>,
    gif_map: Arc<Mutex<GifMap>>,
}

impl PlayerListPanel {
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

    fn build_left_panel(&mut self, frame: &mut Frame, world: &World, area: Rect) {
        let split = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(1),
        ])
        .split(area);

        let mut filter_all_button = Button::new(
            format!("View: {}", PlayerView::All.to_string()),
            UiCallbackPreset::SetPlayerPanelView {
                view: PlayerView::All,
            },
            Arc::clone(&self.callback_registry),
        )
        .set_hotkey(UiKey::CYCLE_VIEW);

        let mut filter_free_agents_button = Button::new(
            format!("View: {}", PlayerView::FreeAgents.to_string()),
            UiCallbackPreset::SetPlayerPanelView {
                view: PlayerView::FreeAgents,
            },
            Arc::clone(&self.callback_registry),
        )
        .set_hotkey(UiKey::CYCLE_VIEW);

        let mut filter_own_team_button = Button::new(
            format!("View: {}", PlayerView::OwnTeam.to_string()),
            UiCallbackPreset::SetPlayerPanelView {
                view: PlayerView::OwnTeam,
            },
            Arc::clone(&self.callback_registry),
        )
        .set_hotkey(UiKey::CYCLE_VIEW);
        match self.view {
            PlayerView::All => filter_all_button.disable(None),
            PlayerView::FreeAgents => filter_free_agents_button.disable(None),
            PlayerView::OwnTeam => filter_own_team_button.disable(None),
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

    fn build_right_panel(&self, frame: &mut Frame, world: &World, area: Rect) -> AppResult<()> {
        let v_split = Layout::vertical([Constraint::Length(24), Constraint::Min(1)]).split(area);

        let h_split = Layout::horizontal([
            Constraint::Length(60),
            Constraint::Length(60),
            Constraint::Min(1),
        ])
        .split(v_split[0]);

        let button_split = Layout::horizontal([
            Constraint::Length(60),
            Constraint::Length(60),
            Constraint::Min(1),
        ])
        .split(v_split[1]);

        if let Some(player) = world.get_player(self.selected_player_id) {
            // NOTE: here it is okay if the search fails. This is a hack to handle
            //       the FA refresh that could happen while the player panel is locked.
            render_player_description(
                player,
                &self.gif_map,
                &self.callback_registry,
                self.tick,
                frame,
                world,
                h_split[0],
            );
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
                    &self.callback_registry,
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
        frame: &mut Frame,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        let own_team = world.get_own_team()?;

        let buttons_split = Layout::vertical([
            Constraint::Length(3), //location
            Constraint::Length(3), //hire/release button
            Constraint::Length(3), //refresh info for FA
            Constraint::Min(1),
        ])
        .split(area);

        let hover_text_target = hover_text_target(frame);

        match player.current_location {
            PlayerLocation::OnPlanet { planet_id } => {
                let planet = world.get_planet_or_err(planet_id)?;
                let button = Button::new(
                    format!("FA on planet {}", planet.name),
                    UiCallbackPreset::GoToPlanetZoomIn { planet_id },
                    Arc::clone(&self.callback_registry),
                )
                .set_hover_text(
                    format!("Go to {}, the free agent's current location", planet.name),
                    hover_text_target,
                )
                .set_hotkey(UiKey::GO_TO_PLANET);
                frame.render_widget(button, buttons_split[0]);
            }
            PlayerLocation::WithTeam => {
                let team = world.get_team_or_err(player.team.unwrap())?;
                let button = Button::new(
                    format!("team {}", team.name),
                    UiCallbackPreset::GoToPlayerTeam {
                        player_id: player.id,
                    },
                    Arc::clone(&self.callback_registry),
                )
                .set_hover_text(format!("Go to team {}", team.name), hover_text_target)
                .set_hotkey(UiKey::GO_TO_TEAM_ALTERNATIVE);
                frame.render_widget(button, buttons_split[0]);
            }
        }
        let lock_button =
            if self.locked_player_id.is_some() && self.locked_player_id.unwrap() == player.id {
                Button::new(
                    "Unlock".into(),
                    UiCallbackPreset::LockPlayerPanel {
                        player_id: self.selected_player_id,
                    },
                    Arc::clone(&self.callback_registry),
                )
                .set_hover_text(
                    format!("Unlock the player panel to allow browsing other players"),
                    hover_text_target,
                )
                .set_hotkey(UiKey::UNLOCK_PLAYER)
            } else {
                Button::new(
                    "Lock".into(),
                    UiCallbackPreset::LockPlayerPanel {
                        player_id: self.selected_player_id,
                    },
                    Arc::clone(&self.callback_registry),
                )
                .set_hover_text(
                    format!("Lock the player panel to keep the info while browsing"),
                    hover_text_target,
                )
                .set_hotkey(UiKey::LOCK_PLAYER)
            };
        frame.render_widget(lock_button, buttons_split[1]);

        // Add hire button for free agents
        if player.team.is_none() {
            let can_hire = own_team.can_hire_player(&player);
            let hire_cost = player.hire_cost(own_team.reputation);

            let mut button = Button::new(
                format!("Hire -{} {}", hire_cost, CURRENCY_SYMBOL),
                UiCallbackPreset::HirePlayer {
                    player_id: player.id,
                },
                Arc::clone(&self.callback_registry),
            )
            .set_hover_text(
                format!("Hire the free agent for {} {}", hire_cost, CURRENCY_SYMBOL),
                hover_text_target,
            )
            .set_hotkey(UiKey::HIRE);
            if can_hire.is_err() {
                button.disable(Some(format!("{}", can_hire.unwrap_err().to_string())));
            }

            frame.render_widget(button, buttons_split[2]);
        }
        // Add release button for own players
        //FIXME: decide if we want to allow this.
        // else if player.team.is_some() && player.team.unwrap() == world.own_team_id {
        //     let can_release = own_team.can_release_player(&player);

        //     let mut button = Button::new(
        //         "Release".into(),
        //         UiCallbackPreset::ReleasePlayer {
        //             player_id: player.id,
        //         },
        //         Arc::clone(&self.callback_registry),
        //     )
        //     .set_hover_text(format!("Fire the player from the team"), hover_text_target)
        //     .set_hotkey(UiKey::FIRE);
        //     if can_release.is_err() {
        //         button.disable(Some(format!("{}", can_release.unwrap_err().to_string())));
        //     }

        //     frame.render_widget(button, buttons_split[2]);
        // }
        Ok(())
    }

    pub fn set_view(&mut self, filter: PlayerView) {
        self.view = filter;
        self.update_view = true;
    }

    pub fn reset_view(&mut self) {
        self.set_view(PlayerView::All);
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
            self.update_view = true;
        }
        if self.update_view {
            let own_team = world.get_own_team()?;
            self.players = self
                .all_players
                .iter()
                .filter(|&&player_id| {
                    let player = world.get_player(player_id).unwrap();
                    self.view.rule(player, own_team)
                })
                .map(|&player_id| player_id)
                .collect();
            self.update_view = false;
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
    fn render(&mut self, frame: &mut Frame, world: &World, area: Rect) -> AppResult<()> {
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

        self.callback_registry
            .lock()
            .unwrap()
            .register_mouse_callback(
                crossterm::event::MouseEventKind::ScrollDown,
                None,
                UiCallbackPreset::NextPanelIndex,
            );

        self.callback_registry
            .lock()
            .unwrap()
            .register_mouse_callback(
                crossterm::event::MouseEventKind::ScrollUp,
                None,
                UiCallbackPreset::PreviousPanelIndex,
            );

        // Split into left and right panels
        let left_right_split = Layout::horizontal([
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
        _world: &World,
    ) -> Option<UiCallbackPreset> {
        match key_event.code {
            KeyCode::Up => self.next_index(),
            KeyCode::Down => self.previous_index(),
            UiKey::GO_TO_TEAM => {
                if let Some(_) = self.selected_team_id.clone() {
                    return Some(UiCallbackPreset::GoToPlayerTeam {
                        player_id: self.selected_player_id,
                    });
                }
            }
            UiKey::CYCLE_VIEW => {
                return Some(UiCallbackPreset::SetPlayerPanelView {
                    view: self.view.next(),
                });
            }

            _ => {}
        }
        None
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
