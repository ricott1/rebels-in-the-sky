use super::button::Button;
use super::constants::*;
use super::ui_callback::UiCallback;
use super::ui_frame::UiFrame;
use super::utils::SwarmPanelEvent;
use super::{
    traits::{Screen, SplitPanel},
    utils::input_from_key_event,
    widgets::default_block,
};
use crate::network::types::TeamRanking;
use crate::types::{AppResult, SystemTimeTick, TeamId, Tick};
use crate::ui::constants::UiKey;
use crate::world::constants::{MIN_PLAYERS_PER_GAME, SECONDS};
use crate::world::{skill::Rated, world::World};
use core::fmt::Debug;
use crossterm::event::{KeyCode, KeyEvent};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Display, Hash, Default)]
pub enum SwarmView {
    #[default]
    Chat,
    Requests,
    Log,
    TeamRanking,
}

impl SwarmView {
    fn next(&self) -> SwarmView {
        match self {
            SwarmView::Chat => SwarmView::Requests,
            SwarmView::Requests => SwarmView::Log,
            SwarmView::Log => SwarmView::TeamRanking,
            SwarmView::TeamRanking => SwarmView::Chat,
        }
    }
}

#[derive(Debug, Default)]
pub struct SwarmPanel {
    pub index: usize,
    events: HashMap<SwarmView, Vec<SwarmPanelEvent>>,
    view: SwarmView,
    textarea: TextArea<'static>,
    connected_peers: Vec<PeerId>,
    team_id_to_peer_id: HashMap<TeamId, PeerId>,
    peer_id_to_team_id: HashMap<PeerId, TeamId>,
    team_ranking: Vec<(TeamId, TeamRanking)>,
}

impl SwarmPanel {
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

    pub fn update_team_ranking(&mut self, team_ranking: &Vec<(TeamId, TeamRanking)>) {
        self.team_ranking = team_ranking.clone();
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
        // If team id is already in the list, remove the previous entry
        if let Some(previous_peer_id) = self.team_id_to_peer_id.get(&team_id) {
            self.connected_peers.retain(|id| id != previous_peer_id);
            // We do not remove from peer_id_to_team_id to retain info about past messages
        }
        self.team_id_to_peer_id.insert(team_id, peer_id);
        self.peer_id_to_team_id.insert(peer_id, team_id);
        self.connected_peers.push(peer_id);
    }

    pub fn remove_peer_id(&mut self, peer_id: &PeerId) {
        self.connected_peers.retain(|id| id != peer_id);
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
            "View: Chat",
            UiCallback::SetSwarmPanelView {
                topic: SwarmView::Chat,
            },
        )
        .set_hotkey(UiKey::CYCLE_VIEW)
        .set_hover_text("View the chat. Just type and press Enter to message the network.");

        let mut requests_button = Button::new(
            "View: Requests",
            UiCallback::SetSwarmPanelView {
                topic: SwarmView::Requests,
            },
        )
        .set_hotkey(UiKey::CYCLE_VIEW)
        .set_hover_text("View challenges received from the network.");

        let mut log_button = Button::new(
            "View: Log",
            UiCallback::SetSwarmPanelView {
                topic: SwarmView::Log,
            },
        )
        .set_hotkey(UiKey::CYCLE_VIEW)
        .set_hover_text("View log and system info from the network.");

        let mut team_ranking_button = Button::new(
            "View: Ranking",
            UiCallback::SetSwarmPanelView {
                topic: SwarmView::TeamRanking,
            },
        )
        .set_hotkey(UiKey::CYCLE_VIEW)
        .set_hover_text("View ranking of best teams in the network.");

        match self.view {
            SwarmView::Chat => chat_button.select(),
            SwarmView::Requests => requests_button.select(),
            SwarmView::Log => log_button.select(),
            SwarmView::TeamRanking => team_ranking_button.select(),
        }

        frame.render_hoverable(chat_button, split[0]);
        frame.render_hoverable(requests_button, split[1]);
        frame.render_hoverable(log_button, split[2]);
        frame.render_hoverable(team_ranking_button, split[3]);

        let mut items: Vec<ListItem> = vec![];

        for (&team_id, peer_id) in self.team_id_to_peer_id.iter() {
            if let Ok(team) = world.get_team_or_err(&team_id) {
                let style = if self.connected_peers.contains(peer_id) {
                    UiStyle::NETWORK
                } else {
                    UiStyle::DISCONNECTED
                };
                items.push(ListItem::new(Span::styled(
                    format!(
                        "{} ({})",
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
        frame.render_widget(list.block(default_block().title("Peers")), split[4]);

        let dial_button = Button::new("Ping", UiCallback::DialSeed);

        frame.render_hoverable(dial_button, split[5]);
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
            frame.render_hoverable(
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
                frame.render_hoverable(accept_button, line_split[1]);
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
                frame.render_hoverable(decline_button, line_split[2]);
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
            frame.render_hoverable(
                Button::new(
                    format!(
                        "{} {} ⇄ {} {}",
                        target_player.info.shortened_name(),
                        target_player.stars(),
                        proposer_player.info.shortened_name(),
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
                    target_player.info.shortened_name(),
                    proposer_player.info.shortened_name()
                ));
                if idx == 0 {
                    accept_button = accept_button.set_hotkey(UiKey::YES_TO_DIALOG);
                }
                frame.render_hoverable(accept_button, line_split[1]);
                let mut decline_button = Button::new(
                    format!("{:6^}", UiText::NO),
                    UiCallback::DeclineTrade {
                        trade: trade.clone(),
                    },
                )
                .block(default_block().border_style(UiStyle::ERROR))
                .set_hover_text(format!(
                    "Decline to trade {} for {}.",
                    target_player.info.shortened_name(),
                    proposer_player.info.shortened_name()
                ));
                if idx == 0 {
                    decline_button = decline_button.set_hotkey(UiKey::NO_TO_DIALOG);
                }
                frame.render_hoverable(decline_button, line_split[2]);
            }
        }
        Ok(())
    }

    fn render_team_ranking(&self, frame: &mut UiFrame, world: &World, area: Rect) {
        frame.render_widget(default_block().title("Top 10 Pirate Crews"), area);
        let mut constraints = [Constraint::Length(1)].repeat(self.team_ranking.len());
        constraints.push(Constraint::Min(0));
        let split = Layout::vertical(constraints).split(area.inner(Margin {
            horizontal: 1,
            vertical: 1,
        }));
        for (idx, (team_id, ranking)) in self.team_ranking.iter().enumerate() {
            let mut rating = ranking
                .player_ratings
                .iter()
                .take(MIN_PLAYERS_PER_GAME)
                .sum::<f32>()
                / MIN_PLAYERS_PER_GAME as f32;

            if let Ok(r) = world.team_rating(team_id) {
                rating = r;
            }

            let text = format!(
                " {:<MAX_NAME_LENGTH$}  Reputation {:5}  Ranking {:5}  {:12} ({})",
                ranking.name.clone(),
                ranking.reputation.stars(),
                rating.stars(),
                format!(
                    "W{}/L{}/D{}",
                    ranking.record[0], ranking.record[1], ranking.record[2]
                ),
                ranking.timestamp.formatted_as_date()
            );
            if world.get_team(team_id).is_some() {
                frame.render_hoverable(
                    Button::no_box(
                        Span::styled(text, UiStyle::NETWORK).into_left_aligned_line(),
                        UiCallback::GoToTeam { team_id: *team_id },
                    )
                    .set_hover_text(format!(
                        "Go to team {} (Reputation {:.2})",
                        ranking.name, ranking.reputation
                    )),
                    split[idx],
                );
            } else {
                frame.render_widget(Span::styled(text, UiStyle::DISCONNECTED), split[idx]);
            };
        }
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

        if self.view == SwarmView::TeamRanking {
            self.render_team_ranking(frame, world, split[0]);
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
                        Span::styled(format!("{}: ", from), UiStyle::NETWORK),
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
            KeyCode::Up => self.previous_index(),
            KeyCode::Down => self.next_index(),
            UiKey::CYCLE_VIEW => {
                //FIXME: this means the chat can't use the capital V
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
                    "/dial" => {
                        return Some(UiCallback::DialSeed);
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
}

impl SplitPanel for SwarmPanel {
    fn index(&self) -> usize {
        self.index
    }

    fn max_index(&self) -> usize {
        self.peer_id_to_team_id.len()
    }

    fn set_index(&mut self, index: usize) {
        self.index = index;
    }
}
