use super::button::Button;
use super::clickable_list::ClickableListState;
use super::constants::*;
use super::gif_map::GifMap;
use super::ui_callback::UiCallback;
use super::ui_frame::UiFrame;
use super::utils::format_satoshi;
use super::widgets::PlayerWidgetView;
use super::{
    constants::{IMG_FRAME_WIDTH, LEFT_PANEL_WIDTH},
    traits::{Screen, SplitPanel},
    widgets::{default_block, render_player_description, selectable_list},
};

use crate::network::trade::Trade;
use crate::types::{AppResult, HashMapWithResult};
use crate::ui::ui_key;
use crate::{
    core::*,
    types::{PlayerId, TeamId},
};
use core::fmt::Debug;
use ratatui::crossterm;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Margin;
use ratatui::style::Stylize;
use ratatui::{
    layout::{Constraint, Layout},
    prelude::Rect,
    widgets::Paragraph,
};
use std::fmt::Display;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum PlayerView {
    #[default]
    All,
    FreePirates,
    Tradable,
    OwnTeam,
}

impl PlayerView {
    const fn next(&self) -> Self {
        match self {
            Self::All => Self::FreePirates,
            Self::FreePirates => Self::Tradable,
            Self::Tradable => Self::OwnTeam,
            Self::OwnTeam => Self::All,
        }
    }

    fn rule(&self, player: &Player, world: &World) -> bool {
        let own_team = if let Ok(team) = world.get_own_team() {
            team
        } else {
            return false;
        };

        match self {
            Self::All => true,
            Self::FreePirates => {
                if player.team.is_some() {
                    return false;
                }

                let player_planet_id = match player.current_location {
                    PlayerLocation::OnPlanet { planet_id } => planet_id,
                    _ => panic!("Free pirate must be PlayerLocation::OnPlanet"),
                };

                let own_team_planet_id = match own_team.current_location {
                    TeamLocation::OnPlanet { planet_id } => planet_id,
                    TeamLocation::Travelling { to, .. } => to,
                    TeamLocation::Exploring { around, .. } => around,
                    TeamLocation::OnSpaceAdventure { around, .. } => around,
                };

                player_planet_id == own_team_planet_id
            }
            Self::Tradable => {
                let own_team_planet_id = match own_team.current_location {
                    TeamLocation::OnPlanet { planet_id } => planet_id,
                    _ => return false,
                };

                if player.team.is_none() {
                    return false;
                }

                if player.team.unwrap() == own_team.id {
                    return false;
                }

                let try_player_team = world.teams.get_or_err(&player.team.unwrap());
                if try_player_team.is_err() {
                    return false;
                }

                let player_team = try_player_team.unwrap();
                let player_team_planet_id = match player_team.current_location {
                    TeamLocation::OnPlanet { planet_id } => planet_id,
                    _ => return false,
                };

                player_team_planet_id == own_team_planet_id
            }
            Self::OwnTeam => player.team.is_some() && player.team.unwrap() == own_team.id,
        }
    }
}

impl Display for PlayerView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::All => write!(f, "All"),
            Self::FreePirates => write!(f, "Free pirates"),
            Self::Tradable => write!(f, "Open for trade"),
            Self::OwnTeam => write!(f, "Own team"),
        }
    }
}

#[derive(Debug, Default)]
pub struct PlayerListPanel {
    pub index: Option<usize>,
    pub locked_player_id: Option<PlayerId>,
    pub selected_player_id: PlayerId,
    player_widget_view: PlayerWidgetView,
    pub selected_team_id: Option<TeamId>,
    pub all_players: Vec<PlayerId>,
    pub players: Vec<PlayerId>,
    view: PlayerView,
    update_view: bool,
    tick: usize,
    gif_map: GifMap,
}

impl PlayerListPanel {
    pub fn new() -> Self {
        Self::default()
    }

    fn build_left_panel(&self, frame: &mut UiFrame, world: &World, area: Rect) {
        let split = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(1),
        ])
        .split(area);

        let mut filter_all_button = Button::new(
            PlayerView::All.to_string(),
            UiCallback::SetPlayerPanelView {
                view: PlayerView::All,
            },
        )
        .bold()
        .set_hotkey(ui_key::CYCLE_VIEW)
        .set_hover_text("View all pirates.");

        let mut filter_free_pirates_button = Button::new(
            PlayerView::FreePirates.to_string(),
            UiCallback::SetPlayerPanelView {
                view: PlayerView::FreePirates,
            },
        )
        .bold()
        .set_hotkey(ui_key::CYCLE_VIEW)
        .set_hover_text("View free pirates.");

        let mut filter_tradable_button = Button::new(
            PlayerView::Tradable.to_string(),
            UiCallback::SetPlayerPanelView {
                view: PlayerView::Tradable,
            },
        )
        .bold()
        .set_hotkey(ui_key::CYCLE_VIEW)
        .set_hover_text("View pirates open for trade.");

        let mut filter_own_team_button = Button::new(
            PlayerView::OwnTeam.to_string(),
            UiCallback::SetPlayerPanelView {
                view: PlayerView::OwnTeam,
            },
        )
        .bold()
        .set_hotkey(ui_key::CYCLE_VIEW)
        .set_hover_text("View your pirates from your crew.");
        match self.view {
            PlayerView::All => filter_all_button.select(),
            PlayerView::FreePirates => filter_free_pirates_button.select(),
            PlayerView::Tradable => filter_tradable_button.select(),
            PlayerView::OwnTeam => filter_own_team_button.select(),
        }

        frame.render_interactive_widget(filter_all_button, split[0]);
        frame.render_interactive_widget(filter_free_pirates_button, split[1]);
        frame.render_interactive_widget(filter_tradable_button, split[2]);
        frame.render_interactive_widget(filter_own_team_button, split[3]);

        if !self.players.is_empty() {
            let mut options = vec![];

            let name_length = 2 * MAX_NAME_LENGTH + 2;
            for player_id in self.players.iter() {
                let player = if let Some(p) = world.players.get(player_id) {
                    p
                } else {
                    continue;
                };
                let mut style = UiStyle::DEFAULT;
                if matches!(player.team, Some(id) if id== world.own_team_id) {
                    style = UiStyle::OWN_TEAM;
                } else if player.peer_id.is_some() {
                    style = UiStyle::NETWORK;
                }
                let full_name = player.info.full_name();
                let name = if full_name.len() <= name_length {
                    full_name
                } else {
                    player.info.short_name()
                };

                let text = format!("{:<name_length$} {}", name, player.stars());
                options.push((text, style));
            }
            let list = selectable_list(options);
            frame.render_stateful_interactive_widget(
                list.block(default_block().title("Pirates ↓/↑")),
                split[4],
                &mut ClickableListState::default().with_selected(self.index),
            );
        } else {
            frame.render_widget(default_block().title("Pirates"), split[4]);
        }
    }

    fn build_right_panel(
        &mut self,
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
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

        let player = world.players.get_or_err(&self.selected_player_id)?;
        let own_team = world.get_own_team()?;

        // Display open trade if the selected and lock player are the two being traded.
        let mut open_trade = None;

        if let Some(locked_player_id) = self.locked_player_id {
            // First option: selected player is in own_team and locked player has a team
            // and this team has sent an offer containing exactly these players.
            if own_team.player_ids.contains(&player.id) {
                if let Some(trade) = own_team.received_trades.get(&(locked_player_id, player.id)) {
                    open_trade = Some(trade);
                }
            }
            // Second option: locked player is in own_team and selected player has a team
            // and this team has sent an offer containing exactly these players.
            if own_team.player_ids.contains(&locked_player_id) {
                if let Some(trade) = own_team.received_trades.get(&(player.id, locked_player_id)) {
                    open_trade = Some(trade);
                }
            }
        }

        render_player_description(
            player,
            self.player_widget_view,
            &mut self.gif_map,
            self.tick,
            world,
            frame,
            h_split[0],
        );
        self.render_buttons(player, open_trade, frame, world, button_split[0])?;

        // If there is an open trade for the locked and selected players,
        // display a button to accept

        if let Some(locked_player_id) = self.locked_player_id {
            let locked_player = world.players.get_or_err(&locked_player_id)?;
            render_player_description(
                locked_player,
                self.player_widget_view,
                &mut self.gif_map,
                self.tick,
                world,
                frame,
                h_split[1],
            );
            self.render_buttons(locked_player, open_trade, frame, world, button_split[1])?;
        }

        Ok(())
    }

    fn render_buttons(
        &self,
        player: &Player,
        open_trade: Option<&Trade>,
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        let own_team = world.get_own_team()?;

        let buttons_split = Layout::vertical([
            Constraint::Length(3), //team
            Constraint::Length(3), //Lock/Unlock
            Constraint::Length(3), //skills/stats
            Constraint::Length(3), //hire info for FA or optionally trade
            Constraint::Min(0),
        ])
        .split(area);

        match player.current_location {
            PlayerLocation::OnPlanet { planet_id } => {
                let planet = world.planets.get_or_err(&planet_id)?;
                let button = Button::new(
                    format!("Free pirate - On planet {}", planet.name),
                    UiCallback::GoToPlanetZoomIn { planet_id },
                )
                .set_hover_text(format!(
                    "Go to {}, this free pirate's current location",
                    planet.name
                ))
                .set_hotkey(ui_key::ON_PLANET);
                frame.render_interactive_widget(button, buttons_split[0]);
            }
            PlayerLocation::WithTeam => {
                let team = world.teams.get_or_err(&player.team.unwrap())?;
                let button = Button::new(
                    format!("team {}", team.name),
                    UiCallback::GoToPlayerTeam {
                        player_id: player.id,
                    },
                )
                .set_hover_text(format!("Go to team {}", team.name))
                .set_hotkey(ui_key::GO_TO_TEAM_ALT);
                frame.render_interactive_widget(button, buttons_split[0]);
            }
        }

        let player_widget_view_button = Button::new(
            format!(
                "View {}",
                if self.player_widget_view == PlayerWidgetView::Skills {
                    PlayerWidgetView::Stats.to_string().to_lowercase()
                } else {
                    PlayerWidgetView::Skills.to_string().to_lowercase()
                }
            ),
            UiCallback::TogglePlayerWidgetView,
        )
        .set_hover_text(format!(
            "View player's {}",
            self.player_widget_view.to_string().to_lowercase()
        ))
        .set_hotkey(ui_key::player::PLAYER_STATUS_VIEW);
        frame.render_interactive_widget(player_widget_view_button, buttons_split[1]);

        let lock_button = if self.locked_player_id.is_some()
            && self.locked_player_id.unwrap() == player.id
        {
            Button::new(
                "Unlock",
                UiCallback::LockPlayerPanel {
                    player_id: player.id,
                },
            )
            .set_hover_text("Unlock the player panel to allow browsing other players".to_string())
            .set_hotkey(ui_key::player::UNLOCK_PLAYER)
            .selected()
        } else {
            Button::new(
                "Lock",
                UiCallback::LockPlayerPanel {
                    player_id: self.selected_player_id,
                },
            )
            .set_hover_text("Lock the player panel to keep the info while browsing".to_string())
            .set_hotkey(ui_key::player::LOCK_PLAYER)
        };
        frame.render_interactive_widget(lock_button, buttons_split[2]);

        // Add hire button for free pirates
        if player.team.is_none() {
            let hire_cost = player.hire_cost(own_team.reputation);
            let mut button = Button::new(
                format!("Hire (-{})", format_satoshi(hire_cost)),
                UiCallback::HirePlayer {
                    player_id: player.id,
                },
            )
            .set_hover_text(format!(
                "Hire this free pirate for {}",
                format_satoshi(hire_cost)
            ))
            .set_hotkey(ui_key::player::HIRE);
            if let Err(err) = own_team.can_hire_player(player) {
                button.disable(Some(err.to_string()));
            }

            frame.render_interactive_widget(button, buttons_split[3]);
        }
        // or if a trade exists and player is part of it, add trade buttons
        else if let Some(trade) = open_trade {
            let proposer_player = &trade.proposer_player;
            let target_player = &trade.target_player;
            if player.id == self.selected_player_id {
                let proposer_team = world
                    .teams
                    .get_or_err(&proposer_player.team.expect("Player should have a team"))?;
                let mut button = Button::new(
                    "Accept trade",
                    UiCallback::AcceptTrade {
                        trade: trade.clone(),
                    },
                )
                .set_hover_text(format!(
                    "Accept to trade {} for {}",
                    target_player.info.short_name(),
                    proposer_player.info.short_name(),
                ))
                .block(default_block().border_style(UiStyle::OK))
                .set_hotkey(ui_key::ACCEPT_TRADE);

                let can_trade =
                    proposer_team.can_trade_players(proposer_player, target_player, own_team);

                if let Err(err) = can_trade {
                    button.disable(Some(err.to_string()));
                }
                frame.render_interactive_widget(button, buttons_split[3]);
            } else if player.id == self.locked_player_id.expect("One player should be locked") {
                let button = Button::new(
                    "Decline trade",
                    UiCallback::DeclineTrade {
                        trade: trade.clone(),
                    },
                )
                .set_hover_text(format!(
                    "Decline to trade {} for {}",
                    target_player.info.short_name(),
                    proposer_player.info.short_name(),
                ))
                .block(default_block().border_style(UiStyle::ERROR))
                .set_hotkey(ui_key::DECLINE_TRADE);

                frame.render_interactive_widget(button, buttons_split[3]);
            };
        }
        // or finally if either the selected or locked player are part of own_team (but not both)
        // add button to propose a trade.
        else if let Some(locked_player_id) = self.locked_player_id {
            //If player is selected and part of own team
            if own_team.player_ids.contains(&player.id) && player.id == self.selected_player_id {
                let proposer_player = world.players.get_or_err(&player.id)?;
                let target_player = world.players.get_or_err(&locked_player_id)?;
                if let Some(target_team_id) = target_player.team {
                    let target_team = world.teams.get_or_err(&target_team_id)?;
                    if own_team
                        .can_trade_players(proposer_player, target_player, target_team)
                        .is_ok()
                    {
                        let mut trade_button = Button::new(
                            "Propose trade",
                            UiCallback::CreateTradeProposal {
                                proposer_player_id: proposer_player.id,
                                target_player_id: target_player.id,
                            },
                        )
                        .set_hover_text(format!(
                            "Propose to trade {} for {}",
                            proposer_player.info.short_name(),
                            target_player.info.short_name(),
                        ))
                        .set_hotkey(ui_key::CREATE_TRADE);

                        if own_team
                            .sent_trades
                            .contains_key(&(proposer_player.id, target_player.id))
                        {
                            trade_button.disable(Some("Trade already proposed"));
                        }

                        frame.render_interactive_widget(trade_button, buttons_split[3]);
                    }
                }
            }
        }

        Ok(())
    }

    pub const fn set_view(&mut self, filter: PlayerView) {
        self.view = filter;
        self.update_view = true;
    }

    pub const fn reset_view(&mut self) {
        self.set_view(PlayerView::All);
    }

    pub const fn toggle_player_widget_view(&mut self) {
        match self.player_widget_view {
            PlayerWidgetView::Skills => self.player_widget_view = PlayerWidgetView::Stats,
            PlayerWidgetView::Stats => self.player_widget_view = PlayerWidgetView::Skills,
        }
    }
}

impl Screen for PlayerListPanel {
    fn update(&mut self, world: &World) -> AppResult<()> {
        self.tick += 1;
        if world.dirty_ui || self.all_players.len() != world.players.len() {
            self.all_players = world.players.keys().copied().collect();
            self.all_players.sort_by(|a, b| {
                let a = world.players.get(a).unwrap();
                let b = world.players.get(b).unwrap();
                if a.rating() == b.rating() {
                    b.average_skill()
                        .partial_cmp(&a.average_skill())
                        .expect("Skill value should exist.")
                } else {
                    b.rating()
                        .partial_cmp(&a.rating())
                        .expect("Skill should exist")
                }
            });
            self.update_view = true;
        }
        if self.update_view {
            self.players = self
                .all_players
                .iter()
                .filter(|&&player_id| {
                    let player = world.players.get(&player_id).unwrap();
                    self.view.rule(player, world)
                })
                .copied()
                .collect();
            self.update_view = false;
        }

        if let Some(index) = self.index {
            if self.players.is_empty() {
                self.index = None;
            } else if index >= self.players.len() && !self.players.is_empty() {
                self.set_index(self.players.len() - 1);
            }

            if index < self.players.len() && !self.players.is_empty() {
                self.selected_player_id = self.players[index];
                self.selected_team_id = world.players.get_or_err(&self.selected_player_id)?.team;
            }
        } else if !self.players.is_empty() {
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
        // Split into left and right panels
        let left_right_split = Layout::horizontal([
            Constraint::Length(LEFT_PANEL_WIDTH),
            Constraint::Min(IMG_FRAME_WIDTH),
        ])
        .split(area);
        self.build_left_panel(frame, world, left_right_split[0]);

        if self.all_players.is_empty() {
            frame.render_widget(
                Paragraph::new(" No pirates yet!"),
                left_right_split[1].inner(Margin::new(1, 1)),
            );
            return Ok(());
        }
        self.build_right_panel(frame, world, left_right_split[1])?;
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
            ui_key::GO_TO_TEAM => {
                if self.selected_team_id.is_some() {
                    return Some(UiCallback::GoToPlayerTeam {
                        player_id: self.selected_player_id,
                    });
                }
            }
            ui_key::CYCLE_VIEW => {
                return Some(UiCallback::SetPlayerPanelView {
                    view: self.view.next(),
                });
            }

            _ => {}
        }
        None
    }

    fn footer_spans(&self) -> Vec<String> {
        vec![
            format!(" {} ", ui_key::CYCLE_VIEW.to_string()),
            " Next tab ".to_string(),
        ]
    }
}

impl SplitPanel for PlayerListPanel {
    fn index(&self) -> Option<usize> {
        self.index
    }

    fn max_index(&self) -> usize {
        self.players.len()
    }

    fn set_index(&mut self, index: usize) {
        self.index = Some(index);
    }
}
