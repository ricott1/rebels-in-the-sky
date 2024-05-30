use super::button::Button;
use super::constants::{UiStyle, UiText, LEFT_PANEL_WIDTH};
use super::ui_callback::{CallbackRegistry, UiCallbackPreset};
use super::utils::{hover_text_target, SwarmPanelEvent};
use super::{
    traits::{Screen, SplitPanel},
    utils::input_from_key_event,
    widgets::default_block,
};
use crate::network::types::Challenge;
use crate::types::{AppResult, SystemTimeTick, TeamId, Tick};
use crate::ui::constants::{PrintableKeyCode, UiKey};
use crate::world::skill::Rated;
use crate::world::world::World;
use core::fmt::Debug;
use crossterm::event::{KeyCode, KeyEvent};
use libp2p::PeerId;
use ratatui::layout::Margin;
use ratatui::style::{Color, Style};
use ratatui::{
    layout::{Constraint, Layout},
    prelude::Rect,
    text::{Line, Span},
    widgets::{List, ListItem, Paragraph, Wrap},
    Frame,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use strum_macros::Display;
use tui_textarea::{CursorMove, TextArea};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Display, Hash, Default)]
pub enum EventTopic {
    Log,
    Challenges,
    #[default]
    Chat,
}

impl EventTopic {
    fn next(&self) -> EventTopic {
        match self {
            EventTopic::Log => EventTopic::Chat,
            EventTopic::Challenges => EventTopic::Log,
            EventTopic::Chat => EventTopic::Challenges,
        }
    }
}

#[derive(Debug, Default)]
pub struct SwarmPanel {
    pub index: usize,
    events: HashMap<EventTopic, Vec<SwarmPanelEvent>>,
    current_topic: EventTopic,
    textarea: TextArea<'static>,
    connected_peers: Vec<PeerId>,
    team_id_to_peer_id: HashMap<TeamId, PeerId>,
    peer_id_to_team_id: HashMap<PeerId, TeamId>,
    peer_to_challenge: HashMap<PeerId, Challenge>,
    callback_registry: Arc<Mutex<CallbackRegistry>>,
}

impl SwarmPanel {
    pub fn new(callback_registry: Arc<Mutex<CallbackRegistry>>) -> Self {
        let mut events = HashMap::new();
        events.insert(EventTopic::Log, vec![]);
        events.insert(EventTopic::Challenges, vec![]);
        events.insert(EventTopic::Chat, vec![]);
        Self {
            callback_registry,
            events,
            ..Default::default()
        }
    }

    pub fn push_log_event(&mut self, event: SwarmPanelEvent) {
        self.events.get_mut(&EventTopic::Log).unwrap().push(event);
    }

    pub fn push_chat_event(&mut self, event: SwarmPanelEvent) {
        self.events.get_mut(&EventTopic::Chat).unwrap().push(event);
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
        self.remove_challenge(peer_id);
    }

    pub fn add_challenge(&mut self, peer_id: PeerId, challenge: Challenge) {
        self.peer_to_challenge.insert(peer_id, challenge);
    }

    pub fn remove_challenge(&mut self, peer_id: &PeerId) {
        self.peer_to_challenge.remove(peer_id);
    }

    pub fn remove_all_challenges(&mut self) {
        self.peer_to_challenge.clear();
    }

    fn build_left_panel(&mut self, frame: &mut Frame, world: &World, area: Rect) {
        let split = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(area);

        let hover_text_target = hover_text_target(frame);

        let mut chat_button = Button::new(
            "View:Chat".to_string(),
            UiCallbackPreset::SetSwarmPanelTopic {
                topic: EventTopic::Chat,
            },
            Arc::clone(&self.callback_registry),
        )
        .set_hotkey(UiKey::CYCLE_VIEW)
        .set_hover_text(
            "View the chat. Just type and press Enter to message the network.".into(),
            hover_text_target,
        );

        let mut challenges_button = Button::new(
            "View:Challenges".to_string(),
            UiCallbackPreset::SetSwarmPanelTopic {
                topic: EventTopic::Challenges,
            },
            Arc::clone(&self.callback_registry),
        )
        .set_hotkey(UiKey::CYCLE_VIEW)
        .set_hover_text(
            "View challenges received from the network.".into(),
            hover_text_target,
        );

        let mut log_button = Button::new(
            "View:Log".to_string(),
            UiCallbackPreset::SetSwarmPanelTopic {
                topic: EventTopic::Log,
            },
            Arc::clone(&self.callback_registry),
        )
        .set_hotkey(UiKey::CYCLE_VIEW)
        .set_hover_text(
            "View log and system info from the network.".into(),
            hover_text_target,
        );
        match self.current_topic {
            EventTopic::Log => {
                log_button.disable(None);
            }
            EventTopic::Challenges => {
                challenges_button.disable(None);
            }
            EventTopic::Chat => {
                chat_button.disable(None);
            }
        }

        frame.render_widget(chat_button, split[0]);
        frame.render_widget(challenges_button, split[1]);
        frame.render_widget(log_button, split[2]);

        let mut items: Vec<ListItem> = vec![];

        for (team_id, peer_id) in self.team_id_to_peer_id.iter() {
            let team = world.get_team_or_err(*team_id);
            if team.is_ok() {
                let style = if self.connected_peers.contains(peer_id) {
                    UiStyle::NETWORK
                } else {
                    UiStyle::DISCONNECTED
                };
                items.push(ListItem::new(Span::styled(
                    team.unwrap().name.clone(),
                    style,
                )));
            }
        }
        let list = List::new(items);
        frame.render_widget(list.block(default_block().title("Peers")), split[3]);

        let dial_button = Button::new(
            "Ping".to_string(),
            UiCallbackPreset::Dial {
                address: "seed".to_string(),
            },
            Arc::clone(&self.callback_registry),
        );
        frame.render_widget(dial_button, split[4]);
    }

    fn build_challenge_list(&mut self, frame: &mut Frame, world: &World, area: Rect) {
        frame.render_widget(default_block().title("Challenges"), area);

        let mut constraints = [Constraint::Length(3)].repeat(self.peer_to_challenge.len());
        constraints.push(Constraint::Min(0));
        let split = Layout::vertical(constraints).split(area.inner(&Margin {
            horizontal: 1,
            vertical: 1,
        }));

        for (idx, (peer_id, challenge)) in self.peer_to_challenge.iter().enumerate() {
            let line_split = Layout::horizontal([
                Constraint::Length(24),
                Constraint::Length(8),
                Constraint::Length(8),
                Constraint::Min(0),
            ])
            .split(split[idx]);

            let hover_text_target = hover_text_target(frame);

            if let Some(team) = challenge.home_team.clone() {
                frame.render_widget(
                    Button::new(
                        format!(
                            "{} {} ({})",
                            team.name,
                            world.team_rating(team.team_id).stars(),
                            peer_id.to_base58().chars().take(4).collect::<String>()
                        ),
                        UiCallbackPreset::GoToTeam {
                            team_id: team.team_id,
                        },
                        Arc::clone(&self.callback_registry),
                    ),
                    line_split[0],
                );
                let mut accept_button = Button::new(
                    format!("{:6^}", UiText::YES),
                    UiCallbackPreset::AcceptChallenge {
                        challenge: challenge.clone(),
                    },
                    Arc::clone(&self.callback_registry),
                )
                .set_box_style(UiStyle::OK)
                .set_hover_text(
                    format!("Accept the challenge from {} and start a game.", team.name),
                    hover_text_target,
                );
                if idx == 0 {
                    accept_button = accept_button.set_hotkey(UiKey::YES_TO_DIALOG);
                }
                frame.render_widget(accept_button, line_split[1]);
                let mut decline_button = Button::new(
                    format!("{:6^}", UiText::NO),
                    UiCallbackPreset::DeclineChallenge {
                        challenge: challenge.clone(),
                    },
                    Arc::clone(&self.callback_registry),
                )
                .set_box_style(UiStyle::ERROR)
                .set_hover_text(
                    format!("Decline the challenge from {}.", team.name),
                    hover_text_target,
                );
                if idx == 0 {
                    decline_button = decline_button.set_hotkey(UiKey::NO_TO_DIALOG);
                }
                frame.render_widget(decline_button, line_split[2]);
            }
        }
    }

    fn build_right_panel(&mut self, frame: &mut Frame, world: &World, area: Rect) {
        let split = Layout::vertical([Constraint::Min(1), Constraint::Length(3)]).split(area);

        self.textarea.set_block(default_block());
        frame.render_widget(self.textarea.widget(), split[1]);

        if self.current_topic == EventTopic::Challenges {
            self.build_challenge_list(frame, world, split[0]);
            return;
        }
        let mut items = vec![];
        for event in self.events.get(&self.current_topic).unwrap().iter().rev() {
            match event.peer_id {
                Some(peer_id) => {
                    let from = if let Some(team_id) = self.peer_id_to_team_id.get(&peer_id) {
                        let team = world.get_team_or_err(*team_id);
                        if team.is_ok() {
                            team.unwrap().name.clone()
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
                    let own_message = if self.current_topic == EventTopic::Log {
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
                .block(default_block().title(self.current_topic.to_string())),
            split[0],
        );
    }

    pub fn set_current_topic(&mut self, topic: EventTopic) {
        self.current_topic = topic;
    }
}

impl Screen for SwarmPanel {
    fn name(&self) -> &str {
        "Swarm"
    }

    fn update(&mut self, _world: &World) -> AppResult<()> {
        Ok(())
    }

    fn render(&mut self, frame: &mut Frame, world: &World, area: Rect) -> AppResult<()> {
        let split = Layout::horizontal([Constraint::Length(LEFT_PANEL_WIDTH), Constraint::Min(1)])
            .split(area);

        self.build_left_panel(frame, world, split[0]);
        self.build_right_panel(frame, world, split[1]);
        Ok(())
    }

    fn handle_key_events(
        &mut self,
        key_event: KeyEvent,
        _world: &World,
    ) -> Option<UiCallbackPreset> {
        match key_event.code {
            KeyCode::Up => self.previous_index(),
            KeyCode::Down => self.next_index(),
            UiKey::CYCLE_VIEW => {
                //FIXME: this means the chat can't use the capital V
                return Some(UiCallbackPreset::SetSwarmPanelTopic {
                    topic: self.current_topic.next(),
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
                        let next = split_input.skip(1).next();
                        let address = if next.is_some() {
                            next.unwrap().to_string()
                        } else {
                            "seed".to_string()
                        };

                        return Some(UiCallbackPreset::Dial { address });
                    }
                    "/sync" => {
                        return Some(UiCallbackPreset::Sync);
                    }
                    "/clear" => {
                        self.events.clear();
                    }

                    "/help" => {
                        // self.push_log_event(SwarmPanelEvent {
                        //     timestamp: Tick::now(),
                        //     peer_id: None,
                        //     text: "/dial <Option<ip_address>>".to_string(),
                        // });
                        // self.push_log_event(SwarmPanelEvent {
                        //     timestamp: Tick::now(),
                        //     peer_id: None,
                        //     text: "/sync".to_string(),
                        // });
                        // self.push_log_event(SwarmPanelEvent {
                        //     timestamp: Tick::now(),
                        //     peer_id: None,
                        //     text: "/clear".to_string(),
                        // });
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
                        return Some(UiCallbackPreset::SendMessage {
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

    fn footer_spans(&self) -> Vec<Span> {
        vec![
            Span::styled(
                format!(" {} ", UiKey::CYCLE_VIEW.to_string()),
                Style::default().bg(Color::Gray).fg(Color::DarkGray),
            ),
            Span::styled(" Cycle topic ", Style::default().fg(Color::DarkGray)),
        ]
    }
}

impl SplitPanel for SwarmPanel {
    fn index(&self) -> usize {
        self.index
    }

    fn max_index(&self) -> usize {
        self.peer_to_challenge.len()
    }

    fn set_index(&mut self, index: usize) {
        self.index = index;
    }
}
