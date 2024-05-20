use super::constants::*;
use super::handler::NetworkHandler;
use super::types::{Challenge, NetworkGame, NetworkRequestState, NetworkTeam, SeedInfo};
use crate::types::{AppResult, SystemTimeTick, Tick, MINUTES};
use crate::types::{GameId, IdSystem};
use crate::ui::utils::SwarmPanelEvent;
use crate::{app::App, types::AppCallback};
use libp2p::gossipsub::{IdentTopic, TopicHash};
use libp2p::{gossipsub::Message, Multiaddr, PeerId};

#[derive(Debug, Clone)]
pub enum NetworkCallbackPreset {
    PushSwarmPanelChat {
        timestamp: Tick,
        peer_id: PeerId,
        text: String,
    },
    PushSwarmPanelLog {
        timestamp: Tick,
        text: String,
    },
    BindAddress {
        address: Multiaddr,
    },
    Subscribe {
        peer_id: PeerId,
        topic: TopicHash,
    },
    Unsubscribe {
        peer_id: PeerId,
        topic: TopicHash,
    },
    CloseConnection {
        peer_id: PeerId,
    },
    HandleConnectionEstablished {
        peer_id: PeerId,
    },
    HandleTeamTopic {
        message: Message,
    },
    HandleMsgTopic {
        message: Message,
    },
    HandleChallengeTopic {
        message: Message,
    },
    HandleGameTopic {
        message: Message,
    },
    HandleSeedTopic {
        message: Message,
    },
}
impl NetworkCallbackPreset {
    fn push_swarm_panel_message(timestamp: Tick, peer_id: PeerId, text: String) -> AppCallback {
        Box::new(move |app: &mut App| {
            let event = SwarmPanelEvent {
                timestamp,
                peer_id: Some(peer_id),
                text: text.clone(),
            };
            app.ui.swarm_panel.push_chat_event(event);
            Ok(None)
        })
    }

    fn push_swarm_panel_log(timestamp: Tick, text: String) -> AppCallback {
        Box::new(move |app: &mut App| {
            let event = SwarmPanelEvent {
                timestamp,
                peer_id: None,
                text: text.clone(),
            };
            app.ui.swarm_panel.push_log_event(event);
            Ok(None)
        })
    }

    fn bind_address(address: Multiaddr) -> AppCallback {
        Box::new(move |app: &mut App| {
            let event = SwarmPanelEvent {
                timestamp: Tick::now(),
                peer_id: None,
                text: format!("Bound to {}", address),
            };
            app.ui.swarm_panel.push_log_event(event);
            app.network_handler.as_mut().unwrap().address = address.clone();

            let multiaddr = app.network_handler.as_ref().unwrap().seed_address.clone();
            app.network_handler.as_mut().unwrap().dial(multiaddr)?;
            Ok(None)
        })
    }

    fn subscribe(topic: TopicHash) -> AppCallback {
        Box::new(move |app: &mut App| {
            let event = SwarmPanelEvent {
                timestamp: Tick::now(),
                peer_id: None,
                text: format!("Subscribed to topic: {}", topic),
            };
            app.ui.swarm_panel.push_log_event(event);

            if topic == IdentTopic::new(SubscriptionTopic::TEAM).hash() {
                if app.world.has_own_team() {
                    app.world.dirty_network = true;
                }
            }
            Ok(None)
        })
    }

    fn unsubscribe(peer_id: PeerId, topic: TopicHash) -> AppCallback {
        Box::new(move |app: &mut App| {
            let event = SwarmPanelEvent {
                timestamp: Tick::now(),
                peer_id: None,
                text: format!("Unsubscribed from topic: {}", topic),
            };
            app.ui.swarm_panel.push_log_event(event);
            app.world.filter_peer_data(Some(peer_id));
            app.ui.swarm_panel.remove_peer_id(&peer_id);
            Ok(None)
        })
    }

    fn close_connection(peer_id: PeerId) -> AppCallback {
        Box::new(move |app: &mut App| {
            if !app
                .network_handler
                .as_ref()
                .unwrap()
                .swarm
                .is_connected(&peer_id)
            {
                let event = SwarmPanelEvent {
                    timestamp: Tick::now(),
                    peer_id: None,
                    text: format!("Closing connection: {}", peer_id),
                };
                app.ui.swarm_panel.push_log_event(event);
                // app.ui.swarm_panel.remove_peer_id(&peer_id);
                app.world.filter_peer_data(Some(peer_id));
            }
            Ok(None)
        })
    }

    fn handle_team_topic(message: Message) -> AppCallback {
        Box::new(move |app: &mut App| {
            let (timestamp, data) = split_message(&message);
            let peer_id = message.source.clone();
            let event = SwarmPanelEvent {
                timestamp,
                peer_id,
                text: format!("Got a team from peer: {:?}", peer_id),
            };
            app.ui.swarm_panel.push_log_event(event);

            let try_deserialize_team = serde_json::from_slice::<NetworkTeam>(data);
            if let Ok(network_team) = try_deserialize_team {
                let event = SwarmPanelEvent {
                    timestamp,
                    peer_id,
                    text: format!(
                        "Deserialized team: {} {}",
                        network_team.team.name, network_team.team.version
                    ),
                };
                app.ui.swarm_panel.push_log_event(event);
                if let Some(id) = peer_id {
                    app.ui.swarm_panel.add_peer_id(id, network_team.team.id);
                }
                app.world.add_network_team(network_team)?;
            } else {
                let text = format!(
                    "Failed to deserialize network team {}",
                    try_deserialize_team.unwrap_err()
                );
                let event = SwarmPanelEvent {
                    timestamp,
                    peer_id,
                    text: text.clone(),
                };
                app.ui.swarm_panel.push_log_event(event);
                return Err(text)?;
            }
            Ok(None)
        })
    }

    fn handle_msg_topic(message: Message) -> AppCallback {
        Box::new(move |app: &mut App| {
            let (timestamp, data) = split_message(&message);
            let text = std::str::from_utf8(data)?.to_string();
            let event = SwarmPanelEvent {
                timestamp,
                peer_id: message.source,
                text: text.clone(),
            };
            app.ui.swarm_panel.push_chat_event(event);
            Ok(None)
        })
    }

    fn handle_game_topic(message: Message) -> AppCallback {
        Box::new(move |app: &mut App| {
            let (timestamp, data) = split_message(&message);
            let peer_id = message.source.clone();
            let event = SwarmPanelEvent {
                timestamp,
                peer_id,
                text: format!("Got a game from peer: {:?}", peer_id),
            };
            app.ui.swarm_panel.push_log_event(event);

            let try_deserialize_game = serde_json::from_slice::<NetworkGame>(data);
            if let Ok(game) = try_deserialize_game {
                let event = SwarmPanelEvent {
                    timestamp,
                    peer_id,
                    text: format!("Deserialized game: {}", game.id),
                };
                app.ui.swarm_panel.push_log_event(event);
                app.world.add_network_game(game)?;
            } else {
                let text = format!(
                    "Failed to deserialize game {}",
                    try_deserialize_game.unwrap_err()
                );
                let event = SwarmPanelEvent {
                    timestamp,
                    peer_id,
                    text: text.clone(),
                };
                app.ui.swarm_panel.push_log_event(event);
                return Err(text)?;
            }
            Ok(None)
        })
    }

    pub fn handle_seed_topic(message: Message) -> AppCallback {
        Box::new(move |app: &mut App| {
            let (timestamp, data) = split_message(&message);
            let info = serde_json::from_slice::<SeedInfo>(data)?;

            let event = SwarmPanelEvent {
                timestamp,
                peer_id: message.source,
                text: format!("Total peers: {}", info.connected_peers_count),
            };
            app.ui.swarm_panel.push_log_event(event);

            if info.message.is_some() {
                app.ui.set_popup(crate::ui::popup_message::PopupMessage::Ok(
                    info.message.unwrap(),
                    timestamp,
                ));
            }

            let own_version_major = env!("CARGO_PKG_VERSION_MAJOR").parse()?;
            let own_version_minor = env!("CARGO_PKG_VERSION_MINOR").parse()?;
            let own_version_patch = env!("CARGO_PKG_VERSION_PATCH").parse()?;

            if info.version_major > own_version_major
                || (info.version_major == own_version_major
                    && info.version_minor > own_version_minor)
                || (info.version_major == own_version_major
                    && info.version_minor == own_version_minor
                    && info.version_patch > own_version_patch)
            {
                let text = format!(
                    "New version {}.{}.{} available. Download at https://rebels.frittura.org",
                    info.version_major, info.version_minor, info.version_patch,
                );
                app.ui
                    .set_popup(crate::ui::popup_message::PopupMessage::Ok(text, timestamp));
            }
            Ok(None)
        })
    }

    pub fn handle_challenge_topic(message: Message) -> AppCallback {
        Box::new(move |app: &mut App| {
            let network_handler = app.network_handler.as_mut().unwrap();
            let self_peer_id = network_handler.swarm.local_peer_id().clone();
            let (timestamp, data) = split_message(&message);

            let mut challenge = serde_json::from_slice::<Challenge>(data)?;
            let event = SwarmPanelEvent {
                timestamp,
                peer_id: message.source,
                text: format!("\nChallenge: {}", challenge.format()),
            };
            app.ui.swarm_panel.push_log_event(event);

            match challenge.state {
                //FIXME: I think a single player is trying to handle more than one state. We should enforce the roles more clearly, i.e. who is the challenger who the challenged and only handle relevant states
                NetworkRequestState::Syn => {
                    if challenge.home_peer_id == self_peer_id {
                        return Err(format!("Team is challenge sender (should be receiver)").into());
                    }

                    if challenge.away_peer_id != self_peer_id {
                        return Err(format!("Team is not challenge receiver").into());
                    }

                    network_handler.add_challenge(challenge.clone());

                    app.ui
                        .swarm_panel
                        .add_challenge(challenge.home_peer_id.clone(), challenge.clone());

                    return Ok(Some(
                        "Challenge received.\nCheck the swarm panel".to_string(),
                    ));
                }

                NetworkRequestState::SynAck => {
                    if challenge.away_peer_id == self_peer_id {
                        return Err(format!("Team is challenge receiver (should be sender)").into());
                    }

                    if challenge.home_peer_id != self_peer_id {
                        return Err(format!("Team is not challenge sender").into());
                    }

                    let mut handle_syn_ack = || -> AppResult<()> {
                        NetworkHandler::can_handle_challenge(&app.world)?;
                        challenge.state = NetworkRequestState::Ack;
                        challenge.game_id = Some(GameId::new());
                        challenge.starting_at = Some(Tick::now() + 2 * MINUTES);

                        let event = SwarmPanelEvent {
                            timestamp,
                            peer_id: message.source,
                            text: format!("Challenge accepted, generating game"),
                        };
                        app.ui.swarm_panel.push_log_event(event);
                        challenge.generate_game(&mut app.world)?;
                        app.ui.set_popup(crate::ui::popup_message::PopupMessage::Ok(
                            format!("Challenge accepted, game is starting."),
                            Tick::now(),
                        ));

                        network_handler.send_challenge(&challenge)?;
                        Ok(())
                    };

                    if let Err(err) = handle_syn_ack() {
                        let mut challenge =
                            Challenge::new(challenge.home_peer_id, challenge.away_peer_id);
                        challenge.state = NetworkRequestState::Failed;
                        challenge.error_message = Some(err.to_string());
                        network_handler.send_challenge(&challenge)?;
                        return Err(err.to_string())?;
                    }
                }

                NetworkRequestState::Ack => {
                    // Not team challenge, we just generate game to display it in UI.
                    if challenge.home_peer_id != self_peer_id
                        && challenge.away_peer_id != self_peer_id
                    {
                        let event = SwarmPanelEvent {
                            timestamp,
                            peer_id: message.source,
                            text: format!("Adding challenge from network"),
                        };
                        app.ui.swarm_panel.push_log_event(event);
                        challenge.generate_game(&mut app.world)?;
                        return Ok(None);
                    }

                    if challenge.home_peer_id == self_peer_id {
                        return Err(format!("Team is challenge sender (should be receiver)").into());
                    }

                    if challenge.away_peer_id != self_peer_id {
                        return Err(format!("Team is not challenge receiver").into());
                    }

                    let mut handle_ack = || -> AppResult<()> {
                        NetworkHandler::can_handle_challenge(&app.world)?;
                        let event = SwarmPanelEvent {
                            timestamp,
                            peer_id: message.source,
                            text: format!("Challenge accepted, generating game"),
                        };
                        app.ui.swarm_panel.push_log_event(event);
                        challenge.generate_game(&mut app.world)?;
                        app.ui.set_popup(crate::ui::popup_message::PopupMessage::Ok(
                            format!("Challenge accepted, game is starting."),
                            Tick::now(),
                        ));
                        Ok(())
                    };

                    if let Err(err) = handle_ack() {
                        let mut challenge =
                            Challenge::new(challenge.home_peer_id, challenge.away_peer_id);
                        challenge.state = NetworkRequestState::Failed;
                        challenge.error_message = Some(err.to_string());
                        network_handler.send_challenge(&challenge)?;
                        return Err(err.to_string())?;
                    }
                }

                NetworkRequestState::Failed => {
                    assert!(challenge.error_message.is_some());
                    if challenge.home_peer_id != self_peer_id
                        && challenge.away_peer_id != self_peer_id
                    {
                        return Err("Challenge failed, but it's not our challenge.")?;
                    }
                    app.ui.swarm_panel.remove_challenge(&challenge.home_peer_id);
                    app.ui
                        .set_popup(crate::ui::popup_message::PopupMessage::Error(
                            format!(
                                "Challenge failed: {}",
                                challenge.error_message.clone().unwrap()
                            ),
                            Tick::now(),
                        ));

                    return Err(format!(
                        "Challenge failed. {}",
                        challenge.error_message.unwrap()
                    ))?;
                }
            }

            Ok(None)
        })
    }

    pub fn call(&self, app: &mut App) -> AppResult<Option<String>> {
        match self {
            Self::PushSwarmPanelChat {
                timestamp,
                peer_id,
                text,
            } => Self::push_swarm_panel_message(timestamp.clone(), peer_id.clone(), text.clone())(
                app,
            ),
            Self::PushSwarmPanelLog { timestamp, text } => {
                Self::push_swarm_panel_log(timestamp.clone(), text.clone())(app)
            }
            Self::BindAddress { address } => Self::bind_address(address.clone())(app),
            Self::Subscribe { peer_id: _, topic } => Self::subscribe(topic.clone())(app),
            Self::Unsubscribe { peer_id, topic } => {
                Self::unsubscribe(peer_id.clone(), topic.clone())(app)
            }
            Self::CloseConnection { peer_id } => Self::close_connection(peer_id.clone())(app),
            Self::HandleConnectionEstablished { peer_id } => {
                let network_handler = app.network_handler.as_mut().unwrap();
                network_handler.send_own_team(&app.world)?;
                let event = SwarmPanelEvent {
                    timestamp: Tick::now(),
                    peer_id: Some(peer_id.clone()),
                    text: format!("Connected to peer: {}", peer_id),
                };
                app.ui.swarm_panel.push_log_event(event);
                Ok(None)
            }
            Self::HandleTeamTopic { message } => Self::handle_team_topic(message.clone())(app),
            Self::HandleMsgTopic { message } => Self::handle_msg_topic(message.clone())(app),
            Self::HandleChallengeTopic { message } => {
                Self::handle_challenge_topic(message.clone())(app)
            }
            Self::HandleGameTopic { message } => Self::handle_game_topic(message.clone())(app),
            Self::HandleSeedTopic { message } => Self::handle_seed_topic(message.clone())(app),
        }
    }
}

pub fn split_message(message: &Message) -> (Tick, &[u8]) {
    // Deserialize the data as a team. first 16 bytes are the timestamp, so we slice them off
    let timestamp = Tick::from_le_bytes(message.data[..16].try_into().unwrap());
    (timestamp, &message.data[16..])
}
