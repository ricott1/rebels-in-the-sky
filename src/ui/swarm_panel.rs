use super::button::Button;
use super::clickable_list::ClickableListState;
use super::constants::*;
use super::gif_map::GifMap;
use super::ui_callback::UiCallback;
use super::ui_frame::UiFrame;
use super::widgets::{
    render_player_description, render_spaceship_description, selectable_list, PlayerWidgetView,
};
use super::{
    traits::{Screen, SplitPanel},
    utils::input_from_key_event,
    widgets::default_block,
};

use crate::network::types::{PlayerRanking, TeamRanking};
use crate::types::{AppResult, PlayerId, SystemTimeTick, TeamId, Tick};
use crate::ui::constants::UiKey;
use crate::world::constants::{MINUTES, MIN_PLAYERS_PER_GAME, SECONDS};
use crate::world::{skill::Rated, world::World};
use core::fmt::Debug;
use crossterm::event::{KeyCode, KeyEvent};
use itertools::Itertools;
use libp2p::PeerId;
use ratatui::layout::Margin;
use ratatui::{
    layout::{Constraint, Layout},
    prelude::Rect,
    text::{Line, Span},
    widgets::{List, ListItem, Paragraph, Wrap},
};
use std::collections::HashMap;
use strum_macros::Display;
use tui_textarea::{CursorMove, TextArea};

const EVENT_DUPLICATE_DELAY: Tick = 10 * SECONDS;
const PEER_DISCONNECTION_INTERVAL: Tick = 5 * MINUTES;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Display, Hash, Default)]
pub enum SwarmView {
    #[default]
    Chat,
    Requests,
    Log,
    Ranking,
}

impl SwarmView {
    fn next(&self) -> SwarmView {
        match self {
            SwarmView::Chat => SwarmView::Requests,
            SwarmView::Requests => SwarmView::Log,
            SwarmView::Log => SwarmView::Ranking,
            SwarmView::Ranking => SwarmView::Chat,
        }
    }
}

#[derive(Debug)]
pub struct SwarmPanelEvent {
    pub timestamp: Tick,
    pub peer_id: Option<PeerId>,
    pub text: String,
}

#[derive(Debug, Display, Default, PartialEq)]
enum PanelList {
    #[default]
    Players,
    Teams,
}

#[derive(Debug, Default)]
pub struct SwarmPanel {
    tick: usize,
    events: HashMap<SwarmView, Vec<SwarmPanelEvent>>,
    view: SwarmView,
    textarea: TextArea<'static>,
    connected_peers: HashMap<PeerId, Tick>,
    team_id_to_peer_id: HashMap<TeamId, PeerId>,
    peer_id_to_team_id: HashMap<PeerId, TeamId>,
    team_ranking: Vec<(TeamId, TeamRanking)>,
    team_ranking_index: Option<usize>,
    player_ranking: Vec<(PlayerId, PlayerRanking)>,
    player_ranking_index: Option<usize>,
    gif_map: GifMap,
    active_list: PanelList,
}

impl SwarmPanel {
    pub fn remove_player_from_ranking(&mut self, player_id: PlayerId) {
        self.player_ranking.retain(|&(id, _)| id != player_id);
        if self.player_ranking.is_empty() {
            self.player_ranking_index = None;
        }
    }
    pub fn new() -> Self {
        let mut events = HashMap::new();
        events.insert(SwarmView::Log, vec![]);
        events.insert(SwarmView::Requests, vec![]);
        events.insert(SwarmView::Chat, vec![]);
        Self {
            events,
            ..Default::default()
        }
    }

    pub fn update_team_ranking(&mut self, team_ranking: &[(TeamId, TeamRanking)]) {
        self.team_ranking = team_ranking.to_vec();
        if self.team_ranking_index.is_none() && !self.team_ranking.is_empty() {
            self.team_ranking_index = Some(0);
        }
    }

    pub fn update_player_ranking(&mut self, player_ranking: &[(PlayerId, PlayerRanking)]) {
        self.player_ranking = player_ranking.to_vec();
        if self.player_ranking_index.is_none() && !self.player_ranking.is_empty() {
            self.player_ranking_index = Some(0);
        }
    }

    pub fn push_log_event(&mut self, event: SwarmPanelEvent) {
        if let Some(last_event) = self
            .events
            .get(&SwarmView::Log)
            .expect("Should have Log events")
            .last()
        {
            // If we recently pushed the same event, don't push it again.
            if last_event.peer_id == event.peer_id
                && last_event.text == event.text
                && event.timestamp - last_event.timestamp <= EVENT_DUPLICATE_DELAY
            {
                return;
            }
        }

        self.events
            .get_mut(&SwarmView::Log)
            .expect("Should have Log events")
            .push(event);
    }

    pub fn push_chat_event(&mut self, event: SwarmPanelEvent) {
        self.events
            .get_mut(&SwarmView::Chat)
            .expect("Should have Chat events")
            .push(event);
    }

    pub fn add_peer_id(&mut self, peer_id: PeerId, team_id: TeamId) {
        self.team_id_to_peer_id.insert(team_id, peer_id);
        self.peer_id_to_team_id.insert(peer_id, team_id);
        self.connected_peers.insert(peer_id, Tick::now());
    }

    pub fn remove_peer_id(&mut self, peer_id: &PeerId) {
        self.connected_peers.remove(peer_id);
    }

    fn is_peer_connected(&self, peer_id: &PeerId) -> bool {
        if let Some(last_tick) = self.connected_peers.get(peer_id) {
            let now = Tick::now();
            if now.saturating_sub(*last_tick) < PEER_DISCONNECTION_INTERVAL {
                return true;
            }
        }
        false
    }

    fn build_left_panel(&mut self, frame: &mut UiFrame, world: &World, area: Rect) {
        let split = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Length(3),
        ])
        .split(area);

        let mut chat_button = Button::new(
            "Chat",
            UiCallback::SetSwarmPanelView {
                topic: SwarmView::Chat,
            },
        )
        .set_hotkey(UiKey::CYCLE_VIEW)
        .set_hover_text("View the chat. Just type and press Enter to message the network.");

        let mut requests_button = Button::new(
            "Requests",
            UiCallback::SetSwarmPanelView {
                topic: SwarmView::Requests,
            },
        )
        .set_hotkey(UiKey::CYCLE_VIEW)
        .set_hover_text("View challenges received from the network.");

        let mut log_button = Button::new(
            "Log",
            UiCallback::SetSwarmPanelView {
                topic: SwarmView::Log,
            },
        )
        .set_hotkey(UiKey::CYCLE_VIEW)
        .set_hover_text("View log and system info from the network.");

        let mut ranking_button = Button::new(
            "Ranking",
            UiCallback::SetSwarmPanelView {
                topic: SwarmView::Ranking,
            },
        )
        .set_hotkey(UiKey::CYCLE_VIEW)
        .set_hover_text("View ranking of best pirates and crews in the network.");

        match self.view {
            SwarmView::Chat => chat_button.select(),
            SwarmView::Requests => requests_button.select(),
            SwarmView::Log => log_button.select(),
            SwarmView::Ranking => ranking_button.select(),
        }

        frame.render_interactive(chat_button, split[0]);
        frame.render_interactive(requests_button, split[1]);
        frame.render_interactive(log_button, split[2]);
        frame.render_interactive(ranking_button, split[3]);

        let mut items: Vec<ListItem> = vec![];

        for (&team_id, peer_id) in self.team_id_to_peer_id.iter() {
            if let Ok(team) = world.get_team_or_err(&team_id) {
                let style = if self.is_peer_connected(peer_id) {
                    UiStyle::NETWORK
                } else {
                    UiStyle::DISCONNECTED
                };
                items.push(ListItem::new(Span::styled(
                    format!(
                        " {:MAX_NAME_LENGTH$} ({})",
                        team.name.clone(),
                        peer_id
                            .to_base58()
                            .chars()
                            .skip(8)
                            .take(8)
                            .collect::<String>()
                    ),
                    style,
                )));
            }
        }
        let list = List::new(items);

        let connected_peers_count = self
            .connected_peers
            .keys()
            .filter(|peer_id| self.is_peer_connected(peer_id))
            .count();
        frame.render_widget(
            list.block(default_block().title(format!("Peers ({connected_peers_count})"))),
            split[4],
        );

        let dial_button = Button::new("Ping", UiCallback::Ping);

        frame.render_interactive(dial_button, split[5]);
    }

    fn build_challenge_list(
        &self,
        is_sent: bool,
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        let title = if is_sent {
            "Challenges sent"
        } else {
            "Challenges received"
        };

        frame.render_widget(default_block().title(title), area);
        let own_team = world.get_own_team()?;
        let challenges = if is_sent {
            &own_team.sent_challenges
        } else {
            &own_team.received_challenges
        };

        let mut constraints = [Constraint::Length(3)].repeat(challenges.len());
        constraints.push(Constraint::Min(0));
        let split = Layout::vertical(constraints).split(area.inner(Margin {
            horizontal: 1,
            vertical: 1,
        }));

        for (idx, (team_id, challenge)) in challenges.iter().enumerate() {
            let peer_id = self.team_id_to_peer_id.get(team_id);
            if peer_id.is_none() {
                continue;
            }

            let peer_id = peer_id.unwrap();

            let line_split = Layout::horizontal([
                Constraint::Length(32),
                Constraint::Length(6),
                Constraint::Length(6),
                Constraint::Min(0),
            ])
            .split(split[idx]);

            let team = if is_sent {
                &challenge.away_team_in_game
            } else {
                &challenge.home_team_in_game
            };
            frame.render_interactive(
                Button::new(
                    format!(
                        "{} {} ({})",
                        team.name,
                        world.team_rating(&team.team_id).unwrap_or_default().stars(),
                        peer_id
                            .to_base58()
                            .chars()
                            .skip(8)
                            .take(8)
                            .collect::<String>()
                    ),
                    UiCallback::GoToTeam {
                        team_id: team.team_id,
                    },
                ),
                line_split[0],
            );

            if !is_sent {
                let mut accept_button = Button::new(
                    format!("{:6^}", UiText::YES),
                    UiCallback::AcceptChallenge {
                        challenge: challenge.clone(),
                    },
                )
                .block(default_block().border_style(UiStyle::OK))
                .set_hover_text(format!(
                    "Accept the challenge from {} and start a game.",
                    team.name
                ));
                if idx == 0 {
                    accept_button = accept_button.set_hotkey(UiKey::YES_TO_DIALOG);
                }
                frame.render_interactive(accept_button, line_split[1]);
                let mut decline_button = Button::new(
                    format!("{:6^}", UiText::NO),
                    UiCallback::DeclineChallenge {
                        challenge: challenge.clone(),
                    },
                )
                .block(default_block().border_style(UiStyle::ERROR))
                .set_hover_text(format!("Decline the challenge from {}.", team.name));
                if idx == 0 {
                    decline_button = decline_button.set_hotkey(UiKey::NO_TO_DIALOG);
                }
                frame.render_interactive(decline_button, line_split[2]);
            }
        }
        Ok(())
    }

    fn build_trade_list(
        &self,
        is_sent: bool,
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        let title = if is_sent {
            "Trade offers sent"
        } else {
            "Trade offers received"
        };

        frame.render_widget(default_block().title(title), area);
        let own_team = world.get_own_team()?;
        let trades = if is_sent {
            &own_team.sent_trades
        } else {
            &own_team.received_trades
        };

        let mut constraints = [Constraint::Length(3)].repeat(trades.len());
        constraints.push(Constraint::Min(0));
        let split = Layout::vertical(constraints).split(area.inner(Margin {
            horizontal: 1,
            vertical: 1,
        }));

        for (idx, (_, trade)) in trades.iter().enumerate() {
            let line_split = Layout::horizontal([
                Constraint::Length(46),
                Constraint::Length(6),
                Constraint::Length(6),
                Constraint::Min(0),
            ])
            .split(split[idx]);

            let proposer_player = &trade.proposer_player;
            let target_player = &trade.target_player;
            frame.render_interactive(
                Button::new(
                    format!(
                        "{} {} ⇄ {} {}",
                        target_player.info.short_name(),
                        target_player.stars(),
                        proposer_player.info.short_name(),
                        proposer_player.stars()
                    ),
                    UiCallback::GoToTrade {
                        trade: trade.clone(),
                    },
                ),
                line_split[0],
            );
            if !is_sent {
                let mut accept_button = Button::new(
                    format!("{:6^}", UiText::YES),
                    UiCallback::AcceptTrade {
                        trade: trade.clone(),
                    },
                )
                .block(default_block().border_style(UiStyle::OK))
                .set_hover_text(format!(
                    "Accept to trade {} for {}.",
                    target_player.info.short_name(),
                    proposer_player.info.short_name()
                ));
                if idx == 0 {
                    accept_button = accept_button.set_hotkey(UiKey::YES_TO_DIALOG);
                }
                frame.render_interactive(accept_button, line_split[1]);
                let mut decline_button = Button::new(
                    format!("{:6^}", UiText::NO),
                    UiCallback::DeclineTrade {
                        trade: trade.clone(),
                    },
                )
                .block(default_block().border_style(UiStyle::ERROR))
                .set_hover_text(format!(
                    "Decline to trade {} for {}.",
                    target_player.info.short_name(),
                    proposer_player.info.short_name()
                ));
                if idx == 0 {
                    decline_button = decline_button.set_hotkey(UiKey::NO_TO_DIALOG);
                }
                frame.render_interactive(decline_button, line_split[2]);
            }
        }
        Ok(())
    }

    fn render_team_ranking(&mut self, frame: &mut UiFrame, world: &World, area: Rect) {
        let h_split = Layout::horizontal([Constraint::Min(1), Constraint::Length(60)]).split(area);
        let team_ranking_index = if let Some(index) = self.team_ranking_index {
            index % self.team_ranking.len()
        } else {
            frame.render_widget(
                default_block().title("Top 10 Crews by Reputation"),
                h_split[0],
            );
            return;
        };

        let (_, top_team) = &self.team_ranking[team_ranking_index];
        let team_rating = if world.get_team(&top_team.team.id).is_some() {
            world.team_rating(&top_team.team.id).unwrap_or_default()
        } else {
            top_team.player_ratings.iter().sum::<f32>()
                / top_team.player_ratings.len().max(MIN_PLAYERS_PER_GAME) as f32
        };

        render_spaceship_description(
            &top_team.team,
            world,
            team_rating,
            false,
            false,
            &mut self.gif_map,
            self.tick,
            frame,
            h_split[1],
        );

        let options = self
            .team_ranking
            .iter()
            .enumerate()
            .map(|(idx, (_, ranking))| {
                let team_id = ranking.team.id;
                let text = format!(
                    "{:>2}. {:<MAX_NAME_LENGTH$} {}",
                    idx + 1,
                    &ranking.team.name,
                    ranking.team.reputation.stars()
                );

                let peer_id = self.team_id_to_peer_id.get(&team_id);
                let style = if team_id == world.own_team_id {
                    UiStyle::OWN_TEAM
                } else if peer_id.is_some() && self.is_peer_connected(peer_id.unwrap()) {
                    UiStyle::NETWORK
                } else {
                    UiStyle::DISCONNECTED
                };

                (text, style)
            })
            .collect_vec();

        let list = selectable_list(options);

        frame.render_stateful_interactive(
            list.block(default_block().title("Top 10 Crews by Reputation")),
            h_split[0],
            &mut ClickableListState::default().with_selected(Some(team_ranking_index)),
        );
    }

    fn render_player_ranking(&mut self, frame: &mut UiFrame, world: &World, area: Rect) {
        let h_split = Layout::horizontal([Constraint::Min(1), Constraint::Length(60)]).split(area);
        let player_ranking_index = if let Some(index) = self.player_ranking_index {
            index % self.player_ranking.len()
        } else {
            frame.render_widget(
                default_block().title("Top 20 Pirates by Reputation"),
                h_split[0],
            );
            return;
        };

        let (_, top_player) = &self.player_ranking[player_ranking_index];
        render_player_description(
            &top_player.player,
            PlayerWidgetView::Skills,
            &mut self.gif_map,
            self.tick,
            world,
            frame,
            h_split[1],
        );

        let name_length = 2 * MAX_NAME_LENGTH + 2;
        let options = self
            .player_ranking
            .iter()
            .enumerate()
            .map(|(idx, (player_id, ranking))| {
                let player = if let Ok(p) = world.get_player_or_err(player_id) {
                    p
                } else {
                    &ranking.player
                };

                let text = format!(
                    "{:>2}. {:<name_length$} {}",
                    idx + 1,
                    player.info.full_name(),
                    player.reputation.stars()
                );

                let mut style = UiStyle::DISCONNECTED;

                if let Some(team_id) = player.team {
                    let peer_id = self.team_id_to_peer_id.get(&team_id);
                    if team_id == world.own_team_id {
                        style = UiStyle::OWN_TEAM
                    } else if peer_id.is_some() && self.is_peer_connected(peer_id.unwrap()) {
                        style = UiStyle::NETWORK
                    }
                }

                (text, style)
            })
            .collect_vec();

        let list = selectable_list(options);

        frame.render_stateful_interactive(
            list.block(default_block().title("Top 20 Pirates by Reputation")),
            h_split[0],
            &mut ClickableListState::default().with_selected(Some(player_ranking_index)),
        );
    }

    fn build_right_panel(
        &mut self,
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        let split = Layout::vertical([Constraint::Min(1), Constraint::Length(3)]).split(area);

        self.textarea.set_block(default_block());
        frame.render_widget(&self.textarea, split[1]);

        if self.view == SwarmView::Requests {
            let h_split = Layout::horizontal([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
                .split(split[0]);
            let challenge_split =
                Layout::vertical([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
                    .split(h_split[0]);
            self.build_challenge_list(false, frame, world, challenge_split[0])?;
            self.build_challenge_list(true, frame, world, challenge_split[1])?;
            let trade_split = Layout::vertical([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
                .split(h_split[1]);
            self.build_trade_list(false, frame, world, trade_split[0])?;
            self.build_trade_list(true, frame, world, trade_split[1])?;
            return Ok(());
        }

        if self.view == SwarmView::Ranking {
            let ranking_split =
                Layout::vertical([Constraint::Length(24), Constraint::Min(1)]).split(split[0]);
            if frame.is_hovering(ranking_split[0]) {
                self.active_list = PanelList::Players;
            } else {
                self.active_list = PanelList::Teams;
            }

            self.render_player_ranking(frame, world, ranking_split[0]);
            self.render_team_ranking(frame, world, ranking_split[1]);
            return Ok(());
        }

        let mut items = vec![];
        for event in self
            .events
            .get(&self.view)
            .expect("Should have current topic events")
            .iter()
            .rev()
        {
            match event.peer_id {
                Some(peer_id) => {
                    let from = if let Some(team_id) = self.peer_id_to_team_id.get(&peer_id) {
                        if let Ok(team) = world.get_team_or_err(team_id) {
                            team.name.clone()
                        } else {
                            "Unknown".to_string()
                        }
                    } else {
                        "SEED".to_string()
                    };

                    items.push(Line::from(vec![
                        Span::styled(
                            format!("[{}] ", event.timestamp.formatted_as_time()),
                            UiStyle::HIGHLIGHT,
                        ),
                        Span::styled(format!("{from}: "), UiStyle::NETWORK),
                        Span::raw(event.text.clone()),
                    ]));
                }
                None => {
                    let own_message = if self.view == SwarmView::Log {
                        "System"
                    } else {
                        "You"
                    };
                    items.push(Line::from(vec![
                        Span::styled(
                            format!("[{}] ", event.timestamp.formatted_as_time()),
                            UiStyle::HIGHLIGHT,
                        ),
                        Span::styled(format!("{own_message}: "), UiStyle::OWN_TEAM),
                        Span::raw(event.text.clone()),
                    ]));
                }
            }
        }

        frame.render_widget(
            Paragraph::new(items)
                .wrap(Wrap { trim: true })
                .block(default_block().title(self.view.to_string())),
            split[0],
        );
        Ok(())
    }

    pub fn set_view(&mut self, topic: SwarmView) {
        self.view = topic;
    }
}

impl Screen for SwarmPanel {
    fn update(&mut self, _world: &World) -> AppResult<()> {
        self.tick += 1;
        Ok(())
    }

    fn render(
        &mut self,
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
        _debug_view: bool,
    ) -> AppResult<()> {
        let split = Layout::horizontal([Constraint::Length(LEFT_PANEL_WIDTH), Constraint::Min(1)])
            .split(area);

        self.build_left_panel(frame, world, split[0]);
        self.build_right_panel(frame, world, split[1])?;
        Ok(())
    }

    fn handle_key_events(&mut self, key_event: KeyEvent, _world: &World) -> Option<UiCallback> {
        match key_event.code {
            KeyCode::Up => self.next_index(),
            KeyCode::Down => self.previous_index(),
            UiKey::CYCLE_VIEW => {
                return Some(UiCallback::SetSwarmPanelView {
                    topic: self.view.next(),
                });
            }
            KeyCode::Enter => {
                let lines: Vec<String> = self
                    .textarea
                    .lines()
                    .iter()
                    .map(|x| x.to_string())
                    .collect();

                self.textarea.move_cursor(CursorMove::End);
                self.textarea.delete_line_by_head();
                let split_input = lines[0].split_whitespace();
                let command = split_input.clone().next()?;

                match command {
                    "/ping" => {
                        return Some(UiCallback::Ping);
                    }
                    "/sync" => {
                        return Some(UiCallback::Sync);
                    }
                    "/clear" => {
                        self.events.clear();
                    }

                    "/help" => {
                        self.push_log_event(SwarmPanelEvent {
                            timestamp: Tick::now(),
                            peer_id: None,
                            text: "/Commands:\n/dial <Option<ip_address>>\n/sync\n/clear"
                                .to_string(),
                        });
                    }
                    _ => {
                        self.push_chat_event(SwarmPanelEvent {
                            timestamp: Tick::now(),
                            peer_id: None,
                            text: lines[0].clone(),
                        });
                        return Some(UiCallback::SendMessage {
                            message: lines[0].clone(),
                        });
                    }
                }
            }
            _ => {
                self.textarea.input(input_from_key_event(key_event));
            }
        }
        None
    }

    fn footer_spans(&self) -> Vec<String> {
        vec![
            format!(" {} ", UiKey::CYCLE_VIEW.to_string()),
            " Next tab ".to_string(),
        ]
    }
}

impl SplitPanel for SwarmPanel {
    fn index(&self) -> usize {
        if self.active_list == PanelList::Players && self.view == SwarmView::Ranking {
            return self.player_ranking_index.unwrap_or_default();
        }

        self.team_ranking_index.unwrap_or_default()
    }

    fn max_index(&self) -> usize {
        if self.active_list == PanelList::Players && self.view == SwarmView::Ranking {
            return self.player_ranking.len();
        }

        self.team_ranking.len()
    }

    fn set_index(&mut self, index: usize) {
        if self.max_index() == 0 {
            if self.active_list == PanelList::Players && self.view == SwarmView::Ranking {
                self.player_ranking_index = None
            } else {
                self.team_ranking_index = None;
            }
        } else if self.active_list == PanelList::Players && self.view == SwarmView::Ranking {
            self.player_ranking_index = Some(index % self.max_index())
        } else {
            self.team_ranking_index = Some(index % self.max_index());
        }
    }
}
