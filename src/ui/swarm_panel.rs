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
use crate::core::constants::{MINUTES, MIN_PLAYERS_PER_GAME};
use crate::core::{skill::Rated, world::World};
use crate::network::types::{PlayerRanking, TeamRanking};
use crate::types::{AppResult, HashMapWithResult, PlayerId, SystemTimeTick, TeamId, Tick};
use crate::ui::clickable_list::{ClickableList, ClickableListItem};
use crate::ui::ui_key;
use crate::ui::utils::wrap_text;
use core::fmt::Debug;
use itertools::Itertools;
use libp2p::PeerId;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Margin;
use ratatui::style::Stylize;
use ratatui::{
    layout::{Constraint, Layout},
    prelude::Rect,
    text::{Line, Span},
    widgets::{List, ListItem},
};
use std::collections::HashMap;
use strum_macros::Display;
use tui_textarea::{CursorMove, TextArea};

const EVENT_DUPLICATE_DELAY: Tick = 2 * MINUTES;
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
    const fn next(&self) -> Self {
        match self {
            Self::Chat => Self::Requests,
            Self::Requests => Self::Log,
            Self::Log => Self::Ranking,
            Self::Ranking => Self::Chat,
        }
    }
}

#[derive(Debug, PartialEq)]
struct LogEvent {
    timestamp: Tick,
    peer_id: Option<PeerId>,
    text: String,
    level: log::Level,
}

impl LogEvent {
    pub const fn new(
        timestamp: Tick,
        peer_id: Option<PeerId>,
        text: String,
        level: log::Level,
    ) -> Self {
        Self {
            timestamp,
            peer_id,
            text,
            level,
        }
    }
}

#[derive(Debug, PartialEq)]
struct ChatEvent {
    timestamp: Tick,
    peer_id: PeerId,
    author: String,
    text: String,
}

impl ChatEvent {
    pub const fn new(timestamp: Tick, peer_id: PeerId, author: String, text: String) -> Self {
        Self {
            timestamp,
            peer_id,
            author,
            text,
        }
    }
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
    chat_events: Vec<ChatEvent>,
    log_events: Vec<LogEvent>,
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
    unread_chat_messages: usize,
    chat_message_index: Option<usize>,
    log_message_index: Option<usize>,
    chat_message_list: ClickableList<'static>,
    log_message_list: ClickableList<'static>,
    should_update_message_list: Option<SwarmView>,
    emojies_substutions: Vec<(&'static str, &'static str)>,
}

impl SwarmPanel {
    pub const fn unread_chat_messages(&self) -> usize {
        self.unread_chat_messages
    }

    pub fn remove_player_from_ranking(&mut self, player_id: PlayerId) {
        self.player_ranking.retain(|&(id, _)| id != player_id);
        if self.player_ranking.is_empty() {
            self.player_ranking_index = None;
        }
    }
    pub fn new() -> Self {
        let emojies_substutions = vec![
            (":fire:", "🔥"),
            (":heart:", "❤️"),
            (":thumbsup:", "👍"),
            (":thumbsdown:", "👎"),
            (":laugh:", "😂"),
            (":cry:", "😢"),
            (":star:", "⭐"),
            (":check:", "✅"),
            (":x:", "❌"),
            (":wave:", "👋"),
            (":clap:", "👏"),
            (":eyes:", "👀"),
            (":rocket:", "🚀"),
            (":trophy:", "🏆"),
            (":skull:", "💀"),
            (":100:", "💯"),
            (":gg:", "🤝"),
        ];

        Self {
            emojies_substutions,
            ..Default::default()
        }
    }

    pub fn update_team_ranking(&mut self, team_ranking: &[(TeamId, TeamRanking)]) {
        self.team_ranking = team_ranking
            .iter()
            .sorted_by(|(_, a), (_, b)| {
                b.team
                    .network_game_rating
                    .rating
                    .partial_cmp(&a.team.network_game_rating.rating)
                    .expect("Netowrk rating should be a number")
            })
            .cloned()
            .collect_vec();
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

    pub fn push_log_event(
        &mut self,
        timestamp: Tick,
        peer_id: Option<PeerId>,
        text: String,
        level: log::Level,
    ) {
        let event = LogEvent::new(timestamp, peer_id, text, level);

        if let Some(last_event) = self.log_events.last_mut() {
            // If we recently pushed the same event, don't push it again,
            // but update the timestamp
            if last_event.peer_id == event.peer_id
                && last_event.text == event.text
                && event.timestamp.saturating_sub(last_event.timestamp) <= EVENT_DUPLICATE_DELAY
            {
                last_event.timestamp = event.timestamp;
                return;
            }
        }

        let previous_length = self.log_events.len();
        self.log_events.push(event);
        self.log_events.sort_by_key(|a| a.timestamp);
        self.log_events.dedup();
        let new_length = self.log_events.len();

        if new_length > previous_length {
            self.log_message_index = self
                .log_message_index
                .map_or(Some(0), |index| Some(index + 1));
        }
        self.should_update_message_list = Some(SwarmView::Log);
    }

    pub fn push_chat_event(
        &mut self,
        timestamp: Tick,
        peer_id: PeerId,
        author: String,
        message: String,
    ) {
        let event = ChatEvent::new(timestamp, peer_id, author, message);

        let previous_length = self.chat_events.len();
        self.chat_events.push(event);
        self.chat_events.sort_by_key(|a| a.timestamp);
        // FIXME: this is very inefficient (N^2). We should find a better way to dedup.
        self.chat_events.dedup();
        let new_length = self.chat_events.len();

        if new_length > previous_length {
            self.unread_chat_messages += 1;
            self.chat_message_index = self
                .chat_message_index
                .map_or(Some(0), |index| Some(index + 1));
        }
        self.should_update_message_list = Some(SwarmView::Chat);
    }

    pub fn add_peer_id(&mut self, peer_id: PeerId, team_id: TeamId) {
        self.team_id_to_peer_id.insert(team_id, peer_id);
        self.peer_id_to_team_id.insert(peer_id, team_id);
        self.connected_peers.insert(peer_id, Tick::now());
        self.should_update_message_list = Some(SwarmView::Chat);
    }

    pub fn remove_peer_id(&mut self, peer_id: &PeerId) {
        self.connected_peers.remove(peer_id);
        self.should_update_message_list = Some(SwarmView::Chat);
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

    fn build_left_panel(&self, frame: &mut UiFrame, world: &World, area: Rect) {
        let split = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Fill(1),
            Constraint::Length(3),
        ])
        .split(area);

        let mut chat_button = Button::new(
            "Chat",
            UiCallback::SetSwarmPanelView {
                topic: SwarmView::Chat,
            },
        )
        .bold()
        .set_hotkey(ui_key::CYCLE_VIEW)
        .set_hover_text("View the chat. Just type and press Enter to message the network.");

        let mut requests_button = Button::new(
            "Requests",
            UiCallback::SetSwarmPanelView {
                topic: SwarmView::Requests,
            },
        )
        .bold()
        .set_hotkey(ui_key::CYCLE_VIEW)
        .set_hover_text("View challenges received from the network.");

        let mut log_button = Button::new(
            "Log",
            UiCallback::SetSwarmPanelView {
                topic: SwarmView::Log,
            },
        )
        .bold()
        .set_hotkey(ui_key::CYCLE_VIEW)
        .set_hover_text("View log and system info from the network.");

        let mut ranking_button = Button::new(
            "Ranking",
            UiCallback::SetSwarmPanelView {
                topic: SwarmView::Ranking,
            },
        )
        .bold()
        .set_hotkey(ui_key::CYCLE_VIEW)
        .set_hover_text("View ranking of best pirates and crews in the network.");

        match self.view {
            SwarmView::Chat => chat_button.select(),
            SwarmView::Requests => requests_button.select(),
            SwarmView::Log => log_button.select(),
            SwarmView::Ranking => ranking_button.select(),
        }

        frame.render_interactive_widget(chat_button, split[0]);
        frame.render_interactive_widget(requests_button, split[1]);
        frame.render_interactive_widget(log_button, split[2]);
        frame.render_interactive_widget(ranking_button, split[3]);

        let mut items: Vec<ListItem> = vec![];

        for (&team_id, peer_id) in self.team_id_to_peer_id.iter() {
            if let Ok(team) = world.teams.get_or_err(&team_id) {
                let style = if team_id == world.own_team_id {
                    UiStyle::OWN_TEAM
                } else if self.is_peer_connected(peer_id) {
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

        frame.render_interactive_widget(dial_button, split[5]);
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
        constraints.push(Constraint::Fill(1));
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
                Constraint::Fill(1),
            ])
            .split(split[idx]);

            let team = if is_sent {
                &challenge.away_team_in_game
            } else {
                &challenge.home_team_in_game
            };
            frame.render_interactive_widget(
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
                    accept_button = accept_button.set_hotkey(ui_key::YES_TO_DIALOG);
                }
                frame.render_interactive_widget(accept_button, line_split[1]);
                let mut decline_button = Button::new(
                    format!("{:6^}", UiText::NO),
                    UiCallback::DeclineChallenge {
                        challenge: challenge.clone(),
                    },
                )
                .block(default_block().border_style(UiStyle::ERROR))
                .set_hover_text(format!("Decline the challenge from {}.", team.name));
                if idx == 0 {
                    decline_button = decline_button.set_hotkey(ui_key::NO_TO_DIALOG);
                }
                frame.render_interactive_widget(decline_button, line_split[2]);
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
        constraints.push(Constraint::Fill(1));
        let split = Layout::vertical(constraints).split(area.inner(Margin {
            horizontal: 1,
            vertical: 1,
        }));

        for (idx, (_, trade)) in trades.iter().enumerate() {
            let line_split = Layout::horizontal([
                Constraint::Length(46),
                Constraint::Length(6),
                Constraint::Length(6),
                Constraint::Fill(1),
            ])
            .split(split[idx]);

            let proposer_player = &trade.proposer_player;
            let target_player = &trade.target_player;
            frame.render_interactive_widget(
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
                    accept_button = accept_button.set_hotkey(ui_key::YES_TO_DIALOG);
                }
                frame.render_interactive_widget(accept_button, line_split[1]);
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
                    decline_button = decline_button.set_hotkey(ui_key::NO_TO_DIALOG);
                }
                frame.render_interactive_widget(decline_button, line_split[2]);
            }
        }
        Ok(())
    }

    fn render_team_ranking(&mut self, frame: &mut UiFrame, world: &World, area: Rect) {
        let block_title = "Top 10 Crews by Elo";
        let h_split = Layout::horizontal([Constraint::Fill(1), Constraint::Length(80)]).split(area);
        if self.team_ranking.is_empty() {
            frame.render_widget(default_block().title(block_title), h_split[0]);
            frame.render_widget(default_block(), h_split[1]);
            return;
        }

        let team_ranking_index = if let Some(index) = self.team_ranking_index {
            index % self.team_ranking.len()
        } else {
            frame.render_widget(default_block().title(block_title), h_split[0]);
            frame.render_widget(default_block(), h_split[1]);
            return;
        };

        let (_, top_team) = &self.team_ranking[team_ranking_index];
        let team_rating = if world.teams.contains_key(&top_team.team.id) {
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
                    "{:>2}. {:<MAX_NAME_LENGTH$} {:5.0}   {}",
                    idx + 1,
                    &ranking.team.name,
                    ranking.team.network_game_rating.rating,
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

        frame.render_widget(default_block().title(block_title), h_split[0]);
        let list_split = Layout::vertical([Constraint::Length(1), Constraint::Fill(1)])
            .split(h_split[0].inner(Margin::new(1, 1)));

        frame.render_widget(
            Span::styled(
                format!(
                    "{:>2}   {:<MAX_NAME_LENGTH$}  {}    {}",
                    "", "Team", "Elo", "Reputation"
                ),
                UiStyle::HEADER,
            ),
            list_split[0],
        );

        let list = selectable_list(options);

        frame.render_stateful_interactive_widget(
            list,
            list_split[1],
            &mut ClickableListState::default().with_selected(Some(team_ranking_index)),
        );
    }

    fn render_player_ranking(&mut self, frame: &mut UiFrame, world: &World, area: Rect) {
        let block_title = "Top 20 Pirates by Reputation";
        let h_split = Layout::horizontal([Constraint::Fill(1), Constraint::Length(60)]).split(area);
        if self.player_ranking.is_empty() {
            frame.render_widget(default_block().title(block_title), h_split[0]);
            frame.render_widget(default_block(), h_split[1]);
            return;
        }

        let player_ranking_index = if let Some(index) = self.player_ranking_index {
            index % self.player_ranking.len()
        } else {
            frame.render_widget(default_block().title(block_title), h_split[0]);
            frame.render_widget(default_block(), h_split[1]);
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
                let player = if let Ok(p) = world.players.get_or_err(player_id) {
                    p
                } else {
                    &ranking.player
                };

                let text = format!(
                    "{:>2}. {:<name_length$} {:<MAX_NAME_LENGTH$} {}",
                    idx + 1,
                    player.info.full_name(),
                    ranking.team_name,
                    player.reputation.stars(),
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

        frame.render_widget(default_block().title(block_title), h_split[0]);
        let list_split = Layout::vertical([Constraint::Length(1), Constraint::Fill(1)])
            .split(h_split[0].inner(Margin::new(1, 1)));

        frame.render_widget(
            Span::styled(
                format!(
                    "{:>2}   {:<name_length$} {:<MAX_NAME_LENGTH$} {}",
                    "", "Player", "Team", "Reputation"
                ),
                UiStyle::HEADER,
            ),
            list_split[0],
        );

        let list = selectable_list(options);

        frame.render_stateful_interactive_widget(
            list,
            list_split[1],
            &mut ClickableListState::default().with_selected(Some(player_ranking_index)),
        );
    }

    fn build_right_panel(
        &mut self,
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        let split = Layout::vertical([Constraint::Fill(1), Constraint::Length(3)]).split(area);

        self.textarea.set_block(default_block());
        frame.render_widget(&self.textarea, split[1]);

        match self.view {
            SwarmView::Requests => {
                let h_split =
                    Layout::horizontal([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
                        .split(split[0]);
                let challenge_split =
                    Layout::vertical([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
                        .split(h_split[0]);
                self.build_challenge_list(false, frame, world, challenge_split[0])?;
                self.build_challenge_list(true, frame, world, challenge_split[1])?;
                let trade_split =
                    Layout::vertical([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
                        .split(h_split[1]);
                self.build_trade_list(false, frame, world, trade_split[0])?;
                self.build_trade_list(true, frame, world, trade_split[1])?;
            }
            SwarmView::Ranking => {
                let ranking_split =
                    Layout::vertical([Constraint::Length(24), Constraint::Fill(1)]).split(split[0]);
                if frame.is_hovering(ranking_split[0]) {
                    self.active_list = PanelList::Players;
                } else {
                    self.active_list = PanelList::Teams;
                }

                self.render_player_ranking(frame, world, ranking_split[0]);
                self.render_team_ranking(frame, world, ranking_split[1]);
            }

            SwarmView::Chat => self.render_event_messages(frame, SwarmView::Chat, split[0]),
            SwarmView::Log => self.render_event_messages(frame, SwarmView::Log, split[0]),
        }

        // Reset only if current index is equal to max index - 1
        if self.view == SwarmView::Chat
            && matches!(self.chat_message_index, Some(index) if index == self.max_index() - 1)
        {
            self.unread_chat_messages = 0;
        }

        Ok(())
    }

    fn update_chat_event_list(&mut self, world: &World) {
        let mut items = vec![];
        for event in self.chat_events.iter() {
            let timestamp_span = Span::styled(
                format!("[{}] ", event.timestamp.formatted_as_time()),
                UiStyle::HIGHLIGHT,
            );

            let author = if matches!(self.peer_id_to_team_id.get(&event.peer_id), Some(&id) if id == world.own_team_id)
            {
                "You"
            } else {
                event.author.as_str()
            };

            let style = if matches!(self.peer_id_to_team_id.get(&event.peer_id), Some(&id) if id == world.own_team_id)
            {
                UiStyle::OWN_TEAM
            } else if self.is_peer_connected(&event.peer_id) {
                UiStyle::NETWORK
            } else {
                UiStyle::DISCONNECTED
            };

            let author_span = Span::styled(format!("{}: ", author), style);

            let timestamp_length = timestamp_span.content.len();
            let incipit_length = timestamp_length + author_span.content.len();
            // FIXME: we should ideally use area.width and make this adaptive.
            let message_max_length =
                (UI_SCREEN_SIZE.0 as usize).saturating_sub(36 + 2 + incipit_length); // 36 is left panel width, extra 2 is because of Block outside.

            let text_lines = wrap_text(event.text.as_str(), message_max_length);
            let mut lines = vec![Line::from(vec![
                timestamp_span,
                author_span,
                Span::raw(text_lines[0].clone()),
            ])];

            for text in text_lines.iter().skip(1) {
                lines.push(Line::from(format!(
                    "{}{}",
                    " ".repeat(incipit_length),
                    text
                )));
            }

            items.push(ClickableListItem::new(lines));
        }

        self.chat_message_list = ClickableList::new(items).block(default_block().title("Chat"));
    }

    fn update_log_event_list(&mut self) {
        let mut items = vec![];
        for event in self.log_events.iter() {
            let timestamp_span = Span::styled(
                format!("[{}] ", event.timestamp.formatted_as_time()),
                UiStyle::HIGHLIGHT,
            );

            let style = match event.level {
                log::Level::Debug => UiStyle::NETWORK,
                log::Level::Info => UiStyle::OK,
                log::Level::Warn => UiStyle::WARNING,
                log::Level::Error => UiStyle::ERROR,
                log::Level::Trace => UiStyle::HEADER,
            };

            let author_span = Span::styled(format!("{:<5}: ", event.level), style);

            let timestamp_length = timestamp_span.content.len();
            let incipit_length = timestamp_length + author_span.content.len();
            // FIXME: we should ideally use area.width and make this adaptive.
            let message_max_length =
                (UI_SCREEN_SIZE.0 as usize).saturating_sub(36 + 2 + incipit_length); // 36 is left panel width, extra 2 is because of Block outside.

            let text_lines = wrap_text(event.text.as_str(), message_max_length);
            let mut lines = vec![Line::from(vec![
                timestamp_span,
                author_span,
                Span::raw(text_lines[0].clone()),
            ])];

            for text in text_lines.iter().skip(1) {
                lines.push(Line::from(format!(
                    "{}{}",
                    " ".repeat(incipit_length),
                    text
                )));
            }

            items.push(ClickableListItem::new(lines));
        }

        self.log_message_list = ClickableList::new(items).block(default_block().title("Log"));
    }

    fn render_event_messages(&self, frame: &mut UiFrame, swarm_view: SwarmView, area: Rect) {
        if swarm_view == SwarmView::Chat {
            frame.render_stateful_interactive_widget(
                &self.chat_message_list,
                area,
                &mut ClickableListState::default().with_selected(self.chat_message_index),
            );
        } else {
            frame.render_stateful_interactive_widget(
                &self.log_message_list,
                area,
                &mut ClickableListState::default().with_selected(self.log_message_index),
            )
        }
    }

    pub const fn set_view(&mut self, topic: SwarmView) {
        self.view = topic;
    }
}

impl Screen for SwarmPanel {
    fn update(&mut self, world: &World) -> AppResult<()> {
        self.tick += 1;

        if self.max_index() == 0 {
            match self.view {
                SwarmView::Chat => self.chat_message_index = None,
                SwarmView::Log => self.log_message_index = None,
                SwarmView::Ranking => match self.active_list {
                    PanelList::Players => self.player_ranking_index = None,
                    PanelList::Teams => self.team_ranking_index = None,
                },
                SwarmView::Requests => {}
            }
        }

        match self.should_update_message_list {
            Some(SwarmView::Chat) => {
                self.update_chat_event_list(world);
            }
            Some(SwarmView::Log) => {
                self.update_log_event_list();
            }
            _ => {}
        }
        self.should_update_message_list = None;

        Ok(())
    }

    fn render(
        &mut self,
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
        _debug_view: bool,
    ) -> AppResult<()> {
        let split = Layout::horizontal([Constraint::Length(LEFT_PANEL_WIDTH), Constraint::Fill(1)])
            .split(area);

        self.build_left_panel(frame, world, split[0]);
        self.build_right_panel(frame, world, split[1])?;
        Ok(())
    }

    fn handle_key_events(&mut self, key_event: KeyEvent, world: &World) -> Option<UiCallback> {
        match key_event.code {
            KeyCode::Up => self.next_index(),
            KeyCode::Down => self.previous_index(),
            ui_key::CYCLE_VIEW => {
                return Some(UiCallback::SetSwarmPanelView {
                    topic: self.view.next(),
                });
            }
            KeyCode::Enter => {
                // FIXME: if a message is selected, render this as a reply to that message.
                if self.max_index() > 0 {
                    self.set_index(self.max_index() - 1);
                }

                let lines: Vec<String> = self
                    .textarea
                    .lines()
                    .iter()
                    .map(|x| x.to_string())
                    .collect();

                let mut message = lines.iter().join("/n");

                if message.is_empty() {
                    // If no message, go to last message
                    self.chat_message_index = if self.chat_events.is_empty() {
                        None
                    } else {
                        Some(self.chat_events.len() - 1)
                    };
                    return None;
                }

                for (from, to) in self.emojies_substutions.iter() {
                    message = message.replace(from, to);
                }

                self.textarea.move_cursor(CursorMove::End);
                self.textarea.delete_line_by_head();

                let own_peer_id = self
                    .team_id_to_peer_id
                    .get(&world.own_team_id)
                    .copied()
                    .expect("There should be an own peer id.");
                let own_team = world.get_own_team().expect("There should be an own team.");
                let timestamp = Tick::now();
                self.push_chat_event(
                    timestamp,
                    own_peer_id,
                    own_team.name.clone(),
                    message.clone(),
                );
                return Some(UiCallback::SendMessage { timestamp, message });
            }
            _ => {
                self.textarea.input(input_from_key_event(key_event));
            }
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

impl SplitPanel for SwarmPanel {
    fn index(&self) -> Option<usize> {
        match self.view {
            SwarmView::Chat => self.chat_message_index,
            SwarmView::Log => self.log_message_index,
            SwarmView::Ranking => match self.active_list {
                PanelList::Players => self.player_ranking_index,
                PanelList::Teams => self.team_ranking_index,
            },
            SwarmView::Requests => None,
        }
    }

    fn max_index(&self) -> usize {
        match self.view {
            SwarmView::Chat => self.chat_events.len(),
            SwarmView::Log => self.log_events.len(),
            SwarmView::Ranking => match self.active_list {
                PanelList::Players => self.player_ranking.len(),
                PanelList::Teams => self.team_ranking.len(),
            },
            SwarmView::Requests => 0,
        }
    }

    fn set_index(&mut self, index: usize) {
        let index = if self.max_index() == 0 {
            None
        } else {
            Some(index % self.max_index())
        };

        match self.view {
            SwarmView::Chat => self.chat_message_index = index,
            SwarmView::Log => self.log_message_index = index,
            SwarmView::Ranking => match self.active_list {
                PanelList::Players => self.player_ranking_index = index,
                PanelList::Teams => self.team_ranking_index = index,
            },
            SwarmView::Requests => {}
        }
    }

    fn previous_index(&mut self) {
        if self.max_index() > 0 {
            if let Some(current_index) = self.index() {
                self.set_index((current_index + 1).min(self.max_index() - 1));
            }
        }
    }

    fn next_index(&mut self) {
        if self.max_index() > 0 {
            if let Some(current_index) = self.index() {
                self.set_index(current_index.saturating_sub(1));
            }
        }
    }
}
