use super::challenge::Challenge;
use super::trade::Trade;
use super::types::{NetworkData, NetworkGame, NetworkRequestState, NetworkTeam, SeedInfo};
use crate::app_version;
use crate::core::constants::NETWORK_GAME_START_DELAY;
use crate::core::MAX_AVG_TIREDNESS_PER_AUTO_GAME;
use crate::game_engine::types::TeamInGame;
use crate::store::deserialize;
use crate::types::{AppResult, SystemTimeTick, TeamId, Tick};
use crate::ui::popup_message::PopupMessage;
use crate::{app::App, types::AppCallback};
use anyhow::anyhow;
use libp2p::gossipsub::TopicHash;
use libp2p::{gossipsub::Message, Multiaddr, PeerId};
use rand::seq::SliceRandom;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

#[derive(Debug, Clone)]
pub enum NetworkCallback {
    PushSwarmPanelChat {
        timestamp: Tick,
        peer_id: PeerId,
        text: String,
    },
    PushSwarmPanelLog {
        timestamp: Tick,
        peer_id: Option<PeerId>,
        text: String,
        level: log::Level,
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
    HandleMessage {
        message: Message,
    },
}
impl NetworkCallback {
    fn push_swarm_panel_message(timestamp: Tick, peer_id: PeerId, text: String) -> AppCallback {
        Box::new(move |app: &mut App| {
            app.ui
                .push_chat_event(timestamp, Some(peer_id), text.clone());
            Ok(None)
        })
    }

    fn bind_address(address: Multiaddr) -> AppCallback {
        Box::new(move |app: &mut App| {
            app.ui.push_log_event(
                Tick::now(),
                None,
                format!("Bound to {address}"),
                log::Level::Debug,
            );

            app.network_handler.dial_seed()?;

            Ok(None)
        })
    }

    fn subscribe(topic: TopicHash) -> AppCallback {
        Box::new(move |app: &mut App| {
            app.ui.push_log_event(
                Tick::now(),
                None,
                format!("Subscribed to topic: {topic}"),
                log::Level::Debug,
            );
            app.world.dirty_network = true;
            Ok(None)
        })
    }

    fn unsubscribe(peer_id: PeerId, topic: TopicHash) -> AppCallback {
        Box::new(move |app: &mut App| {
            app.ui.push_log_event(
                Tick::now(),
                Some(peer_id),
                format!("Peer {peer_id} unsubscribed from topic: {topic}"),
                log::Level::Debug,
            );
            app.world.filter_peer_data(Some(peer_id))?;
            app.ui.swarm_panel.remove_peer_id(&peer_id);
            Ok(None)
        })
    }

    fn close_connection(peer_id: PeerId) -> AppCallback {
        Box::new(move |app: &mut App| {
            app.ui.push_log_event(
                Tick::now(),
                Some(peer_id),
                format!("Closing connection: {peer_id}"),
                log::Level::Debug,
            );
            // FIXME: read connection protocol and understand when this is called.
            //        For example, we could check that num_established >0 or that cause = None

            Ok(None)
        })
    }

    fn handle_team_topic(
        peer_id: Option<PeerId>,
        timestamp: Tick,
        network_team: NetworkTeam,
    ) -> AppCallback {
        Box::new(move |app: &mut App| {
            app.world.add_network_team(network_team.clone())?;

            if let Some(id) = peer_id {
                app.ui.swarm_panel.add_peer_id(id, network_team.team.id);
            }
            app.ui.push_log_event(
                timestamp,
                peer_id,
                format!(
                    "Received team {} (version {})",
                    network_team.team.name, network_team.team.version
                ),
                log::Level::Info,
            );

            Ok(None)
        })
    }

    fn handle_message_topic(peer_id: Option<PeerId>, timestamp: Tick, text: String) -> AppCallback {
        Box::new(move |app: &mut App| {
            app.ui.push_chat_event(timestamp, peer_id, text.clone());
            Ok(None)
        })
    }

    fn handle_relayer_message_to_team_topic(
        timestamp: Tick,
        message: String,
        team_id: TeamId,
    ) -> AppCallback {
        Box::new(move |app: &mut App| {
            if app.world.own_team_id == team_id {
                app.ui.push_popup(PopupMessage::Ok {
                    message: message.clone(),
                    is_skippable: false,
                    tick: timestamp,
                });
            }

            Ok(None)
        })
    }

    fn handle_game_topic(
        peer_id: Option<PeerId>,
        timestamp: Tick,
        game: NetworkGame,
    ) -> AppCallback {
        Box::new(move |app: &mut App| {
            app.ui.push_log_event(
                timestamp,
                peer_id,
                format!(
                    "Deserialized game {}:  {} vs {}",
                    game.id, game.home_team_in_game.name, game.away_team_in_game.name
                ),
                log::Level::Info,
            );
            app.world.add_network_game(game.clone())?;
            Ok(None)
        })
    }

    fn handle_seed_topic(
        peer_id: Option<PeerId>,
        timestamp: Tick,
        seed_info: SeedInfo,
    ) -> AppCallback {
        Box::new(move |app: &mut App| {
            log::info!("Got seed info");
            app.ui.push_log_event(
                timestamp,
                peer_id,
                format!("Total peers: {}", seed_info.connected_peers_count),
                log::Level::Info,
            );

            if let Some(message) = seed_info.message.clone() {
                app.ui.push_popup(PopupMessage::Ok {
                    message,
                    is_skippable: false,
                    tick: timestamp,
                });
            }

            // Notify about new version (only once).
            app.notify_seed_version(seed_info.version)?;

            app.ui
                .swarm_panel
                .update_team_ranking(&seed_info.team_ranking);

            app.ui
                .swarm_panel
                .update_player_ranking(&seed_info.player_ranking);

            app.world.dirty_network = true;
            Ok(None)
        })
    }

    fn handle_trade_topic(peer_id: Option<PeerId>, timestamp: Tick, trade: Trade) -> AppCallback {
        Box::new(move |app: &mut App| {
            app.ui.push_log_event(
                timestamp,
                peer_id,
                format!("Deserialized trade: {}", trade.format()),
                log::Level::Info,
            );

            let self_peer_id = app.network_handler.own_peer_id();
            match &trade.state {
                NetworkRequestState::Syn => {
                    if trade.proposer_peer_id == *self_peer_id {
                        return Err(anyhow!("Team is trade sender (should be receiver)"));
                    }

                    if trade.target_peer_id != *self_peer_id {
                        return Err(anyhow!("Team is not trade receiver"));
                    }

                    let own_team = app.world.get_own_team_mut()?;
                    own_team.add_received_trade(trade.clone());

                    return Ok(Some(
                        "Trade offer received.\nCheck the swarm panel".to_string(),
                    ));
                }
                NetworkRequestState::SynAck => {
                    if trade.target_peer_id == *self_peer_id {
                        return Err(anyhow!(
                            "Invalid trade: team is trade receiver (should be sender)"
                        ));
                    }

                    if trade.proposer_peer_id != *self_peer_id {
                        return Err(anyhow!("Invalid trade: team is not trade sender"));
                    }

                    let mut handle_syn_ack = || -> AppResult<()> {
                        let mut trade = trade.clone();
                        let proposer_player = app
                            .world
                            .get_player_or_err(&trade.proposer_player.id)?
                            .clone();
                        trade.proposer_player = proposer_player;

                        // Check if trade is still valid.
                        // Note: here there are no consistency problems, as the swap is performed before the other team.
                        //       We are the proposer team, so we should have the last updated version of proposer_player
                        //       and we do not update it. We do update target_player using the version from the trade.
                        app.world
                            .players
                            .insert(trade.target_player.id, trade.target_player.clone());

                        let own_team = app.world.get_own_team()?;
                        let target_team = app.world.get_team_or_err(
                            &trade
                                .target_player
                                .team
                                .ok_or(anyhow!("Player in trade should have a team"))?,
                        )?;

                        own_team.can_trade_players(
                            &trade.proposer_player,
                            &trade.target_player,
                            target_team,
                        )?;

                        app.world
                            .swap_players_team(trade.proposer_player.id, trade.target_player.id)?;

                        let own_team = app.world.get_own_team_mut()?;
                        own_team.remove_trade(trade.proposer_player.id, trade.target_player.id);

                        app.ui.push_log_event(
                            timestamp,
                            peer_id,
                            "Trade accepted, players swapped".to_string(),
                            log::Level::Info,
                        );

                        app.ui.push_popup(PopupMessage::Ok {
                            message: "Trade accepted, players swapped.".to_string(),
                            is_skippable: false,
                            tick: Tick::now(),
                        });
                        trade.state = NetworkRequestState::Ack;
                        app.network_handler.send_trade(trade)?;
                        Ok(())
                    };

                    if let Err(err) = handle_syn_ack() {
                        let mut trade = trade.clone();
                        trade.state = NetworkRequestState::Failed {
                            error_message: err.to_string(),
                        };
                        let own_team = app.world.get_own_team_mut()?;
                        own_team.remove_trade(trade.proposer_player.id, trade.target_player.id);
                        app.network_handler.send_trade(trade)?;

                        return Err(anyhow!(err.to_string()));
                    }
                }
                NetworkRequestState::Ack => {
                    // Not team trade, we do nothing.
                    if trade.proposer_peer_id != *self_peer_id
                        && trade.target_peer_id != *self_peer_id
                    {
                        app.ui.push_log_event(
                            timestamp,
                            peer_id,
                            "A trade is happening in the network".to_string(),
                            log::Level::Info,
                        );

                        return Ok(None);
                    }

                    if trade.proposer_peer_id == *self_peer_id {
                        return Err(anyhow!("Team is trade sender (should be receiver)"));
                    }

                    // The following check sometimes fails if the peer_id changed after the challenge has been sent.
                    if trade.target_peer_id != *self_peer_id {
                        return Err(anyhow!("Team is not trade receiver"));
                    }

                    let mut handle_ack = || -> AppResult<()> {
                        // Check if trade is still valid.
                        // Note: here there could be consistency problems, as the trade has been done by the other team
                        //       which could have updated the target_player by sending their team over the network
                        //       Because of this, we need to insert both players from the trade to ensure we use the trade version.
                        //       Still, we do not have the correct version of the proposer team. It's a race condition: it could
                        //       not include the proposer player anymore. In this case, releasing could not work.
                        // To avoid these sort of problems, receiving a team over the network gives an error if the incoming team
                        // contains a player which is currently part of the own team.

                        app.world
                            .players
                            .insert(trade.proposer_player.id, trade.proposer_player.clone());

                        let own_team = app.world.get_own_team()?;
                        let proposer_team = app.world.get_team_or_err(
                            &trade
                                .proposer_player
                                .team
                                .ok_or(anyhow!("Player in trade should have a team"))?,
                        )?;

                        proposer_team.can_trade_players(
                            &trade.proposer_player,
                            &trade.target_player,
                            own_team,
                        )?;

                        app.world
                            .swap_players_team(trade.proposer_player.id, trade.target_player.id)?;

                        let own_team = app.world.get_own_team_mut()?;
                        own_team.remove_trade(trade.proposer_player.id, trade.target_player.id);

                        app.ui.push_log_event(
                            timestamp,
                            peer_id,
                            "Trade accepted, players swapped".to_string(),
                            log::Level::Info,
                        );

                        app.ui.push_popup(PopupMessage::Ok {
                            message: "Trade accepted, players swapped.".to_string(),
                            is_skippable: false,
                            tick: Tick::now(),
                        });
                        Ok(())
                    };

                    if let Err(err) = handle_ack() {
                        let mut trade = trade.clone();
                        trade.state = NetworkRequestState::Failed {
                            error_message: err.to_string(),
                        };
                        app.network_handler.send_trade(trade)?;
                        return Err(anyhow!(err.to_string()));
                    }
                }

                NetworkRequestState::Failed { error_message } => {
                    if trade.proposer_peer_id != *self_peer_id
                        && trade.target_peer_id != *self_peer_id
                    {
                        return Err(anyhow!("Trade failed, but it's not our trade."));
                    }

                    let own_team = app.world.get_own_team_mut()?;
                    own_team.remove_trade(trade.proposer_player.id, trade.target_player.id);

                    app.ui.push_popup(PopupMessage::Error {
                        message: format!("Trade failed: {error_message}"),
                        tick: Tick::now(),
                    });

                    return Err(anyhow!(format!("Trade failed: {error_message}")))?;
                }
            }

            Ok(None)
        })
    }

    fn handle_challenge_topic(
        peer_id: Option<PeerId>,
        timestamp: Tick,
        challenge: Challenge,
    ) -> AppCallback {
        Box::new(move |app: &mut App| {
            app.ui.push_log_event(
                timestamp,
                peer_id,
                format!("Challenge: {}", challenge.format()),
                log::Level::Info,
            );

            let self_peer_id = app.network_handler.own_peer_id();
            match &challenge.state {
                NetworkRequestState::Syn => {
                    if challenge.proposer_peer_id == *self_peer_id {
                        return Err(anyhow!("Team is challenge sender (should be receiver)"));
                    }

                    if challenge.target_peer_id != *self_peer_id {
                        return Err(anyhow!("Team is not challenge receiver"));
                    }

                    let [own_major_version, own_minor_version, own_patch_version] = app_version();
                    let [challenge_major_version, challenge_minor_version, challenge_patch_version] =
                        challenge.app_version;
                    if challenge_major_version != own_major_version
                        || challenge_minor_version != own_minor_version
                    {
                        return Err(anyhow!(
                            "App versions do not match: Proposer version {challenge_major_version}.{challenge_minor_version}.{challenge_patch_version} - Target version {own_major_version}.{own_minor_version}.{own_patch_version}"
                        ));
                    }

                    let own_team = app.world.get_own_team()?;
                    let average_tiredness = own_team.average_tiredness(&app.world);

                    let own_team = app.world.get_own_team_mut()?;

                    if own_team.current_game.is_none()
                        && own_team.autonomous_strategy.challenge_network
                        && average_tiredness <= MAX_AVG_TIREDNESS_PER_AUTO_GAME
                    {
                        let rng = &mut ChaCha8Rng::from_os_rng();
                        own_team.player_ids.shuffle(rng);
                        app.network_handler
                            .accept_challenge(&app.world, challenge.clone())?;
                        return Ok(Some("Challenge received.\nAuto accepted".to_string()));
                    }

                    own_team.add_received_challenge(challenge.clone());

                    return Ok(Some(
                        "Challenge received.\nCheck the swarm panel".to_string(),
                    ));
                }

                NetworkRequestState::SynAck => {
                    if challenge.target_peer_id == *self_peer_id {
                        return Err(anyhow!(
                            "Invalid challenge: team is challenge receiver (should be sender)"
                        ));
                    }

                    if challenge.proposer_peer_id != *self_peer_id {
                        return Err(anyhow!("Invalid challenge: team is not challenge sender"));
                    }

                    let [own_major_version, own_minor_version, own_patch_version] = app_version();
                    let [challenge_major_version, challenge_minor_version, challenge_patch_version] =
                        challenge.app_version;
                    if challenge_major_version != own_major_version
                        || challenge_minor_version != own_minor_version
                    {
                        return Err(anyhow!(
                            "App versions do not match: Proposer version {challenge_major_version}.{challenge_minor_version}.{challenge_patch_version} - Target version {own_major_version}.{own_minor_version}.{own_patch_version}"
                        ));
                    }

                    let mut handle_syn_ack = || -> AppResult<()> {
                        let mut home_team_in_game = TeamInGame::from_team_id(
                            &app.world.own_team_id,
                            &app.world.teams,
                            &app.world.players,
                        )?;

                        home_team_in_game.peer_id = Some(*app.network_handler.own_peer_id());

                        let mut challenge = challenge.clone();
                        challenge.home_team_in_game = home_team_in_game;
                        let starting_at = Tick::now() + NETWORK_GAME_START_DELAY;
                        challenge.state = NetworkRequestState::Ack;
                        challenge.starting_at = Some(starting_at);

                        app.ui.push_log_event(
                            timestamp,
                            peer_id,
                            "Challenge accepted, generating game".to_string(),
                            log::Level::Info,
                        );

                        let own_team = app.world.get_own_team_mut()?;
                        own_team.remove_challenge(
                            challenge.home_team_in_game.team_id,
                            challenge.away_team_in_game.team_id,
                        );

                        if let Err(err) = app.world.generate_network_game(
                            challenge.home_team_in_game.clone(),
                            challenge.away_team_in_game.clone(),
                            starting_at,
                        ) {
                            challenge.state = NetworkRequestState::Failed {
                                error_message: err.to_string(),
                            };
                            app.network_handler.send_challenge(challenge)?;

                            return Err(anyhow!(err.to_string()));
                        }

                        app.ui.push_popup(PopupMessage::Ok {
                            message: "Challenge accepted, game is starting.".to_string(),
                            is_skippable: false,
                            tick: Tick::now(),
                        });

                        app.network_handler.send_challenge(challenge)?;
                        Ok(())
                    };

                    if let Err(err) = handle_syn_ack() {
                        let mut challenge = challenge.clone();

                        challenge.state = NetworkRequestState::Failed {
                            error_message: err.to_string(),
                        };
                        app.network_handler.send_challenge(challenge)?;
                        return Err(anyhow!(err.to_string()));
                    }
                }

                NetworkRequestState::Ack => {
                    // Not team challenge, we just generate game to display it in UI.
                    if challenge.proposer_peer_id != *self_peer_id
                        && challenge.target_peer_id != *self_peer_id
                    {
                        app.ui.push_log_event(
                            timestamp,
                            peer_id,
                            "Adding challenge from network".to_string(),
                            log::Level::Info,
                        );

                        if let Some(starting_at) = challenge.starting_at {
                            app.world.generate_network_game(
                                challenge.home_team_in_game.clone(),
                                challenge.away_team_in_game.clone(),
                                starting_at,
                            )?;
                        } else {
                            return Err(anyhow!("Cannot generate game, starting_at not set"));
                        }
                        return Ok(None);
                    }

                    if challenge.proposer_peer_id == *self_peer_id {
                        return Err(anyhow!("Team is challenge sender (should be receiver)"));
                    }

                    // The following check sometimes fails if the peer_id changed after the challenge has been sent.
                    if challenge.target_peer_id != *self_peer_id {
                        return Err(anyhow!("Team is not challenge receiver"));
                    }

                    let mut handle_ack = || -> AppResult<()> {
                        app.ui.push_log_event(
                            timestamp,
                            peer_id,
                            "Challenge accepted, generating game".to_string(),
                            log::Level::Info,
                        );

                        if let Some(starting_at) = challenge.starting_at {
                            // In generate_game we check again if the challenge is valid.
                            // Note: there could be a race condition where we receive a team over the network right after
                            //       accepting the challenge but before the challenge has been finalized on our side.
                            //       In this case, the received team would have current_game set to some (set to the challenge game
                            //       they just started) and the challenge would fail on our hand since the challenge team must have no game.
                            //       Because of this, we accept the challenge by running a special set of checks.
                            app.world.generate_network_game(
                                challenge.home_team_in_game.clone(),
                                challenge.away_team_in_game.clone(),
                                starting_at,
                            )?;
                        } else {
                            return Err(anyhow!("Cannot generate game, starting_at not set"));
                        }

                        app.ui.push_popup(PopupMessage::Ok {
                            message: "Challenge accepted, game is starting.".to_string(),
                            is_skippable: false,
                            tick: Tick::now(),
                        });
                        Ok(())
                    };

                    if let Err(err) = handle_ack() {
                        let mut challenge = challenge.clone();
                        challenge.state = NetworkRequestState::Failed {
                            error_message: err.to_string(),
                        };
                        app.network_handler.send_challenge(challenge)?;
                        app.ui.push_popup(PopupMessage::Error {
                            message: format!("Challenge failed: {err}"),
                            tick: Tick::now(),
                        });

                        return Err(anyhow!(err.to_string()));
                    }
                }

                NetworkRequestState::Failed { error_message } => {
                    if challenge.proposer_peer_id != *self_peer_id
                        && challenge.target_peer_id != *self_peer_id
                    {
                        return Err(anyhow!("Challenge failed, but it's not our challenge."));
                    }

                    let own_team = app.world.get_own_team_mut()?;
                    own_team.remove_challenge(
                        challenge.home_team_in_game.team_id,
                        challenge.away_team_in_game.team_id,
                    );

                    app.ui.push_popup(PopupMessage::Error {
                        message: format!("Challenge failed: {error_message}"),
                        tick: Tick::now(),
                    });

                    return Err(anyhow!("Challenge failed: {error_message}"))?;
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
            } => Self::push_swarm_panel_message(*timestamp, *peer_id, text.clone())(app),
            Self::PushSwarmPanelLog {
                timestamp,
                peer_id,
                text,
                level,
            } => {
                app.ui
                    .push_log_event(*timestamp, *peer_id, text.clone(), *level);
                Ok(None)
            }
            Self::BindAddress { address } => Self::bind_address(address.clone())(app),
            Self::Subscribe { peer_id: _, topic } => Self::subscribe(topic.clone())(app),
            Self::Unsubscribe { peer_id, topic } => Self::unsubscribe(*peer_id, topic.clone())(app),
            Self::CloseConnection { peer_id } => Self::close_connection(*peer_id)(app),
            Self::HandleConnectionEstablished { peer_id } => {
                app.network_handler.send_own_team(&app.world)?;

                app.ui.push_log_event(
                    Tick::now(),
                    Some(*peer_id),
                    format!("Connected to peer: {peer_id}"),
                    log::Level::Debug,
                );
                Ok(None)
            }
            Self::HandleMessage { message } => {
                let peer_id = message.source;

                let network_data = deserialize::<NetworkData>(&message.data)?;
                match network_data {
                    NetworkData::Team(timestamp, team) => {
                        Self::handle_team_topic(peer_id, timestamp, team)(app)
                    }
                    NetworkData::Message(timestamp, text) => {
                        Self::handle_message_topic(peer_id, timestamp, text)(app)
                    }
                    NetworkData::Challenge(timestamp, challenge) => {
                        Self::handle_challenge_topic(peer_id, timestamp, challenge)(app)
                    }
                    NetworkData::Trade(timestamp, trade) => {
                        Self::handle_trade_topic(peer_id, timestamp, trade)(app)
                    }
                    NetworkData::Game(timestamp, game) => {
                        Self::handle_game_topic(peer_id, timestamp, game)(app)
                    }
                    NetworkData::SeedInfo(timestamp, seed_info) => {
                        Self::handle_seed_topic(peer_id, timestamp, seed_info)(app)
                    }
                    NetworkData::RelayerMessageToTeam(timestamp, message, team_id) => {
                        Self::handle_relayer_message_to_team_topic(timestamp, message, team_id)(app)
                    }
                }
            }
        }
    }
}
