use super::challenge::Challenge;
use super::constants::*;
use super::network_callback::NetworkCallback;
use super::trade::Trade;
#[cfg(feature = "relayer")]
use super::types::SeedInfo;
use super::types::{NetworkData, NetworkGame, NetworkRequestState, NetworkTeam};
use crate::app::AppEvent;
use crate::core::world::World;
use crate::core::Team;
use crate::game_engine::types::TeamInGame;
use crate::game_engine::{Tournament, TournamentId};
use crate::network::types::TournamentRequestState;
use crate::store::serialize;
use crate::types::{AppResult, GameId, HashMapWithResult, PlayerMap};
use crate::types::{PlayerId, TeamId};
use crate::types::{SystemTimeTick, Tick};
use anyhow::anyhow;
use futures::StreamExt;
use itertools::Itertools;
use libp2p::gossipsub::{self, IdentTopic};
use libp2p::identity::Keypair;
use libp2p::swarm::{DialError, SwarmEvent};
use libp2p::{identity, noise, tcp, yamux, PeerId, TransportError};
use libp2p::{Multiaddr, Swarm};
use log::{error, info};
use std::collections::hash_map::DefaultHasher;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Default)]
enum SwarmStatus {
    #[default]
    Uninitialized,
    Ready {
        sender: mpsc::Sender<SwarmCommand>,
    },
}

#[derive(Debug, Clone)]
enum SwarmCommand {
    Dial { address: Multiaddr },
    Send { topic: IdentTopic, data: Vec<u8> },
}

#[derive(Debug)]
pub struct NetworkHandler {
    local_keypair: Keypair,
    pub connected_peers_count: usize,
    own_peer_id: PeerId,
    pub seed_addresses: Vec<Multiaddr>,
    swarm_status: SwarmStatus,
}

impl NetworkHandler {
    fn new_swarm(
        keypair: Keypair,
        tcp_port: u16,
        use_ipv4: bool,
        use_ipv6: bool,
    ) -> AppResult<Swarm<gossipsub::Behaviour>> {
        // To content-address message, we can take the hash of message and use it as an ID.
        let message_id_fn = |message: &gossipsub::Message| {
            let mut s = DefaultHasher::new();
            message.data.hash(&mut s);
            gossipsub::MessageId::from(s.finish().to_string())
        };

        // Set a custom gossipsub configuration
        let gossipsub_config = gossipsub::ConfigBuilder::default()
            .heartbeat_interval(Duration::from_secs(1)) // This is set to aid debugging by not cluttering the log space
            .validation_mode(gossipsub::ValidationMode::Strict) // This sets the kind of message validation. The default is Strict (enforce message signing)
            .message_id_fn(message_id_fn) // content-address messages. No two messages of the same content will be propagated.
            .build()
            .expect("Valid config");

        // build a gossipsub network behaviour
        let mut gossipsub = gossipsub::Behaviour::new(
            gossipsub::MessageAuthenticity::Signed(keypair.clone()),
            gossipsub_config,
        )
        .expect("Correct configuration");

        gossipsub.subscribe(&IdentTopic::new(TOPIC))?;
        let mut swarm = libp2p::SwarmBuilder::with_existing_identity(keypair)
            .with_tokio()
            .with_tcp(
                tcp::Config::default(),
                noise::Config::new,
                yamux::Config::default,
            )?
            .with_dns()?
            .with_behaviour(|_| gossipsub)?
            .with_swarm_config(|cfg| {
                cfg.with_idle_connection_timeout(Duration::from_secs(u64::MAX))
            })
            .build();

        let mut succesful_listen_on = false;

        if use_ipv6 {
            if let Err(e) = swarm.listen_on(format!("/ip6/::/tcp/{tcp_port}").parse()?) {
                log::error!("Could not listen on ip6: {e}");
            } else {
                succesful_listen_on = true;
            };
        }

        // Fallback to ipv4 if ipv6 gave an error.
        if use_ipv4 || !succesful_listen_on {
            if let Err(e) = swarm.listen_on(format!("/ip4/0.0.0.0/tcp/{tcp_port}").parse()?) {
                log::error!("Could not listen on ip4: {e}");
            } else {
                succesful_listen_on = true;
            }
        }

        if !succesful_listen_on {
            return Err(anyhow!("Swarm could not start listening."));
        }

        Ok(swarm)
    }

    pub fn test_default() -> Self {
        let local_keypair = identity::Keypair::generate_ed25519();
        let own_peer_id = PeerId::from_public_key(&local_keypair.public());
        Self {
            local_keypair,
            connected_peers_count: 0,
            own_peer_id,
            seed_addresses: vec![],
            swarm_status: SwarmStatus::Uninitialized,
        }
    }

    pub fn new(seed_node_ip: Option<&String>) -> AppResult<Self> {
        let local_keypair = identity::Keypair::generate_ed25519();
        let mut seed_addresses = vec![
            format!("/dns4/{DEFAULT_SEED_URL}/tcp/{DEFAULT_SEED_PORT}")
                .parse()
                .expect("Invalid default seed address."),
            format!("/ip6/{DEFAULT_SEED_IPV6}/tcp/{DEFAULT_SEED_PORT}")
                .parse()
                .expect("Invalid provided seed ip."),
        ];

        if let Some(ip) = seed_node_ip {
            if let Ok(address) = format!("/ip4/{ip}/tcp/{DEFAULT_SEED_PORT}").parse() {
                seed_addresses.push(address);
            } else if let Ok(address) = format!("/ip6/{ip}/tcp/{DEFAULT_SEED_PORT}").parse() {
                seed_addresses.push(address);
            }
        }

        info!(
            "Network handler created with {} seed addresses.",
            seed_addresses.len()
        );

        let own_peer_id = local_keypair.public().to_peer_id();

        Ok(Self {
            local_keypair,
            connected_peers_count: 0,
            own_peer_id,
            seed_addresses,
            swarm_status: SwarmStatus::Uninitialized,
        })
    }

    pub fn with_keypair(mut self, keypair: Keypair) -> Self {
        self.local_keypair = keypair;
        self.own_peer_id = self.local_keypair.public().to_peer_id();

        self
    }

    pub fn set_keypair(&mut self, keypair: Keypair) {
        self.local_keypair = keypair;
        self.own_peer_id = self.local_keypair.public().to_peer_id();
    }

    pub fn keypair_bytes(&self) -> AppResult<Vec<u8>> {
        let bytes = self.local_keypair.to_protobuf_encoding()?;
        Ok(bytes)
    }

    pub fn own_peer_id(&self) -> &PeerId {
        &self.own_peer_id
    }

    pub fn start_polling_events(
        &mut self,
        event_sender: mpsc::Sender<AppEvent>,
        cancellation_token: CancellationToken,
        tcp_port: u16,
        use_ipv4: bool,
        use_ipv6: bool,
    ) -> JoinHandle<()> {
        let local_keypair = self.local_keypair.clone();
        let own_peer_id = *self.own_peer_id();

        let (sender, mut receiver) = mpsc::channel(64);

        self.swarm_status = SwarmStatus::Ready { sender };
        let handle = tokio::spawn(async move {
            let mut swarm =
                if let Ok(swarm) = Self::new_swarm(local_keypair, tcp_port, use_ipv4, use_ipv6) {
                    swarm
                } else {
                    return;
                };

            assert_eq!(own_peer_id, *swarm.local_peer_id());

            loop {
                tokio::select! {
                    _ = cancellation_token.cancelled() => {
                        log::info!("NetworkHandler background task shutting down.");
                        if !swarm.behaviour_mut().unsubscribe(&IdentTopic::new(TOPIC)) {
                            error!("Cannot unsubscribe from events");
                        }

                        let connected_peers = swarm.connected_peers().cloned().collect_vec();
                        for peer_id in connected_peers {
                            let _ = swarm
                                .disconnect_peer_id(peer_id)
                                .map_err(|e| error!("Error disconnecting peer id {peer_id}: {e:?}"));
                        }

                        break;
                    }

                    event = swarm.select_next_some() => {
                        if event_sender.send(AppEvent::NetworkEvent(event)).await.is_err() {
                                log::warn!("App receiver dropped; stopping network loop");
                                break;
                            }
                    }

                    Some(command) = receiver.recv() => {
                        match command {
                            SwarmCommand::Send { topic, data } => {
                                if let Err(e) = swarm
                                    .behaviour_mut()
                                    .publish(topic, data) {
                                        error!("Swarm send error: {e}");
                                    }
                            }
                            SwarmCommand::Dial {address} => {
                                 if let Err(e) = swarm.dial(address){
                                        error!("Swarm dial error: {e}");
                                    }
                            }
                        }
                    }
                }
            }
        });

        handle
    }

    fn _send(&mut self, data: &NetworkData) -> AppResult<()> {
        match &self.swarm_status {
            SwarmStatus::Uninitialized => {}
            SwarmStatus::Ready { sender } => {
                let data = serialize(data)?;
                sender.try_send(SwarmCommand::Send {
                    topic: IdentTopic::new(TOPIC),
                    data,
                })?;
            }
        }
        Ok(())
    }

    pub fn dial_seed(&mut self) -> AppResult<()> {
        match &self.swarm_status {
            SwarmStatus::Uninitialized => {}
            SwarmStatus::Ready { sender } => {
                for address in self.seed_addresses.iter() {
                    sender.try_send(SwarmCommand::Dial {
                        address: address.clone(),
                    })?;
                }
            }
        }

        Ok(())
    }

    pub fn send_message(&mut self, message: String) -> AppResult<()> {
        self._send(&NetworkData::Message {
            timestamp: Tick::now(),
            from: *self.own_peer_id(),
            message,
        })
    }

    #[cfg(feature = "relayer")]
    pub fn resend_message(
        &mut self,
        timestamp: Tick,
        from: PeerId,
        message: String,
    ) -> AppResult<()> {
        self._send(&NetworkData::Message {
            timestamp,
            from,
            message,
        })
    }

    #[cfg(feature = "relayer")]
    pub fn send_relayer_message_to_team(
        &mut self,
        message: String,
        team_id: TeamId,
    ) -> AppResult<()> {
        self._send(&NetworkData::RelayerMessageToTeam {
            timestamp: Tick::now(),
            message,
            team_id,
        })
    }

    #[cfg(feature = "relayer")]
    pub fn send_seed_info(&mut self, seed_info: SeedInfo) -> AppResult<()> {
        self._send(&NetworkData::SeedInfo {
            timestamp: Tick::now(),
            seed_info,
        })
    }

    pub fn send_own_team(&mut self, world: &World) -> AppResult<()> {
        if world.has_own_team() {
            self.send_team(world, world.own_team_id)?
        } else {
            return Err(anyhow!("No own team"));
        };

        // If own team is playing with network peer, send the game.
        if let Some(game_id) = world.get_own_team()?.current_game {
            let game = world.games.get_or_err(&game_id)?;
            // FIXME: Send game even if we are playing with local team.
            if game.is_network() {
                return self.send_game(world, &game_id);
            }
        }

        Ok(())
    }

    pub fn resend_tournaments(&mut self, world: &World) -> AppResult<()> {
        for tournament in world.tournaments.values() {
            // Only organizer has most updated state of the tournament.
            if tournament.organizer_id == world.own_team_id || tournament.is_initialized() {
                self.send_tournament(tournament.clone())?;
            }
        }

        Ok(())
    }

    pub fn resend_open_trades(&mut self, world: &World) -> AppResult<()> {
        let own_team = world.get_own_team()?;
        for trade in own_team.sent_trades.values() {
            if trade.state == NetworkRequestState::Syn {
                self.send_trade(trade.clone())?;
            }
        }

        Ok(())
    }

    pub fn resend_open_challenges(&mut self, world: &World) -> AppResult<()> {
        let own_team = world.get_own_team()?;
        for challenge in own_team.sent_challenges.values() {
            if challenge.state == NetworkRequestState::Syn {
                self.send_challenge(challenge.clone())?;
            }
        }

        Ok(())
    }

    fn send_game(&mut self, world: &World, game_id: &GameId) -> AppResult<()> {
        let game = NetworkGame::from_game_id(world, game_id)?;
        self._send(&NetworkData::Game {
            timestamp: Tick::now(),
            game,
        })
    }

    fn send_team(&mut self, world: &World, team_id: TeamId) -> AppResult<()> {
        let team = NetworkTeam::from_team_id(world, &team_id, *self.own_peer_id())?;
        self._send(&NetworkData::Team {
            timestamp: Tick::now(),
            team,
        })
    }

    pub fn send_tournament(&mut self, tournament: Tournament) -> AppResult<()> {
        self._send(&NetworkData::Tournament {
            timestamp: Tick::now(),
            tournament,
        })
    }

    pub fn send_tournament_registration_request(
        &mut self,
        tournament_id: TournamentId,
        team_id: TeamId,
        team_data: Option<(Team, PlayerMap)>,
        request_state: TournamentRequestState,
    ) -> AppResult<()> {
        self._send(&NetworkData::TournamentRegistrationRequest {
            timestamp: Tick::now(),
            tournament_id,
            team_id,
            team_data,
            request_state,
        })
    }

    pub fn send_challenge(&mut self, challenge: Challenge) -> AppResult<()> {
        self._send(&NetworkData::Challenge {
            timestamp: Tick::now(),
            challenge,
        })
    }

    pub fn send_trade(&mut self, trade: Trade) -> AppResult<()> {
        self._send(&NetworkData::Trade {
            timestamp: Tick::now(),
            trade,
        })
    }

    pub fn send_new_challenge(
        &mut self,
        world: &World,
        peer_id: PeerId,
        team_id: TeamId,
        app_version: [usize; 3],
    ) -> AppResult<Challenge> {
        self.send_own_team(world)?;
        let mut home_team_in_game =
            TeamInGame::from_team_id(&world.own_team_id, &world.teams, &world.players)?;
        home_team_in_game.peer_id = Some(*self.own_peer_id());

        let away_team_in_game = TeamInGame::from_team_id(&team_id, &world.teams, &world.players)?;

        let challenge = Challenge::new(
            *self.own_peer_id(),
            peer_id,
            home_team_in_game,
            away_team_in_game,
            app_version,
        );

        self.send_challenge(challenge.clone())?;
        Ok(challenge)
    }

    pub fn send_new_trade(
        &mut self,
        world: &World,
        target_peer_id: PeerId,
        proposer_player_id: PlayerId,
        target_player_id: PlayerId,
    ) -> AppResult<Trade> {
        self.send_own_team(world)?;

        let proposer_player = world.players.get_or_err(&proposer_player_id)?.clone();
        let target_player = world.players.get_or_err(&target_player_id)?.clone();

        let trade = Trade::new(
            *self.own_peer_id(),
            target_peer_id,
            proposer_player,
            target_player,
            0,
        );

        self.send_trade(trade.clone())?;
        Ok(trade)
    }

    pub fn accept_challenge(&mut self, world: &World, challenge: Challenge) -> AppResult<()> {
        self.send_own_team(world)?;
        let mut handle_syn = || -> AppResult<()> {
            let home_team = world
                .teams
                .get_or_err(&challenge.home_team_in_game.team_id)?;
            let away_team = world
                .teams
                .get_or_err(&challenge.away_team_in_game.team_id)?;

            // Away team is our team.
            if away_team.current_game.is_some() {
                return Err(anyhow!("{} is already playing", away_team.name));
            }

            away_team.can_accept_network_challenge(home_team)?;

            let mut away_team_in_game =
                TeamInGame::from_team_id(&world.own_team_id, &world.teams, &world.players)?;

            away_team_in_game.peer_id = Some(*self.own_peer_id());

            // Note: we do not start immediately the game at this point,
            // because it could take a long time to accept a challenge
            // and the status of the challenger could have changed considerably
            // possibly making the challenge invalid.
            let mut challenge = challenge.clone();
            challenge.away_team_in_game = away_team_in_game;
            challenge.state = NetworkRequestState::SynAck;
            self.send_challenge(challenge)?;
            Ok(())
        };

        if let Err(err) = handle_syn() {
            let mut challenge = challenge.clone();
            challenge.state = NetworkRequestState::Failed {
                error_message: err.to_string(),
            };
            self.send_challenge(challenge)?;
            return Err(anyhow!(err.to_string()));
        }
        Ok(())
    }

    pub fn decline_challenge(&mut self, mut challenge: Challenge) -> AppResult<()> {
        challenge.state = NetworkRequestState::Failed {
            error_message: format!("{} declined", challenge.away_team_in_game.name),
        };
        self.send_challenge(challenge)?;
        Ok(())
    }

    pub fn accept_trade(&mut self, world: &World, trade: Trade) -> AppResult<()> {
        let mut handle_syn = || -> AppResult<()> {
            let own_team = world.get_own_team()?;
            let proposer_team = if let Some(proposer_team_id) = trade.proposer_player.team {
                world.teams.get_or_err(&proposer_team_id)?
            } else {
                return Err(anyhow!("Trade target player has no team"));
            };

            // Note: we do not apply immediately the trade at this point,
            // because it could take a long time to accept a trade
            // and the status of the proposer could have changed considerably
            // possibly making the trade invalid.
            let mut trade = trade.clone();
            let target_player = world.players.get_or_err(&trade.target_player.id)?.clone();
            trade.target_player = target_player;
            proposer_team.can_trade_players(
                &trade.proposer_player,
                &trade.target_player,
                own_team,
            )?;

            trade.state = NetworkRequestState::SynAck;
            self.send_trade(trade)?;
            Ok(())
        };

        if let Err(err) = handle_syn() {
            let mut trade = trade.clone();
            trade.state = NetworkRequestState::Failed {
                error_message: err.to_string(),
            };
            self.send_trade(trade)?;
            return Err(anyhow!(err.to_string()));
        }
        Ok(())
    }

    pub fn decline_trade(&mut self, trade: Trade) -> AppResult<()> {
        let mut trade = trade.clone();
        trade.state = NetworkRequestState::Failed {
            error_message: "Trade declined".to_string(),
        };
        self.send_trade(trade)?;
        Ok(())
    }

    pub fn handle_network_events(
        &mut self,
        event: SwarmEvent<gossipsub::Event>,
    ) -> Option<NetworkCallback> {
        match event {
            SwarmEvent::NewListenAddr { address, .. } => {
                Some(NetworkCallback::BindAddress { address })
            }
            SwarmEvent::Behaviour(gossipsub::Event::Message {
                propagation_source: _,
                message_id: _,
                message,
            }) => {
                assert!(message.topic == IdentTopic::new(TOPIC).hash());
                Some(NetworkCallback::HandleMessage { message })
            }
            SwarmEvent::Behaviour(gossipsub::Event::Subscribed { peer_id, topic }) => {
                assert!(topic == IdentTopic::new(TOPIC).hash());
                Some(NetworkCallback::Subscribe { peer_id, topic })
            }

            SwarmEvent::Behaviour(gossipsub::Event::Unsubscribed { peer_id, topic }) => {
                assert!(topic == IdentTopic::new(TOPIC).hash());
                Some(NetworkCallback::Unsubscribe { peer_id, topic })
            }
            SwarmEvent::ExpiredListenAddr {
                listener_id: _,
                address,
            } => Some(NetworkCallback::PushSwarmPanelLog {
                timestamp: Tick::now(),
                peer_id: None,
                text: format!("Expired listen address: {address}"),
                level: log::Level::Warn,
            }),
            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                self.connected_peers_count += 1;
                Some(NetworkCallback::HandleConnectionEstablished { peer_id })
            }
            SwarmEvent::ConnectionClosed { peer_id, .. } => {
                self.connected_peers_count -= 1;
                Some(NetworkCallback::CloseConnection { peer_id })
            }
            SwarmEvent::OutgoingConnectionError { error, .. } => {
                let text = match error {
                    // Special handling for DialError::Transport -> TransportError::Other as it is not displayed nicely :(
                    DialError::Transport(errors) => {
                        let error_text = errors
                            .iter()
                            .map(|(_, error)| match error {
                                TransportError::Other(err) => err.to_string(),
                                _ => error.to_string(),
                            })
                            .unique()
                            .join(" + ");
                        format!("Outgoing connection error: {error_text}")
                    }
                    _ => format!("Outgoing connection error: {error}"),
                };
                Some(NetworkCallback::PushSwarmPanelLog {
                    timestamp: Tick::now(),
                    peer_id: None,
                    text,
                    level: log::Level::Error,
                })
            }
            _ => Some(NetworkCallback::PushSwarmPanelLog {
                timestamp: Tick::now(),
                peer_id: None,
                text: format!("Unhandled event: {event:?}"),
                level: log::Level::Warn,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TOPIC;
    use crate::{
        app::App,
        core::{constants::NETWORK_GAME_START_DELAY, types::TeamLocation, world::World},
        network::{
            network_callback::NetworkCallback,
            types::{NetworkData, NetworkRequestState, NetworkTeam},
        },
        store::{deserialize, serialize},
        types::{AppResult, HashMapWithResult, SystemTimeTick, Tick},
        ui::UiCallback,
    };
    use anyhow::anyhow;
    use libp2p::{
        gossipsub::{IdentTopic, Message},
        PeerId,
    };
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    #[test]
    fn test_network_challenge_success() -> AppResult<()> {
        let topic = IdentTopic::new(TOPIC);

        let mut app1 = App::test_with_network_handler()?;
        let mut app2 = App::test_with_network_handler()?;

        let proposer_peer_id = app1.network_handler.own_peer_id().clone();
        let target_peer_id = app2.network_handler.own_peer_id().clone();

        // Add other team by hand
        let mut own_team1 = app1.world.get_own_team()?.clone();
        own_team1.peer_id = Some(proposer_peer_id);
        let planet_id = own_team1.home_planet_id;
        for player_id in own_team1.player_ids.iter() {
            let player = app1.world.players.get(&player_id).unwrap();
            app2.world.players.insert(player_id.clone(), player.clone());
        }
        app2.world.teams.insert(own_team1.id, own_team1);

        let mut own_team2 = app2.world.get_own_team()?.clone();
        own_team2.peer_id = Some(target_peer_id);
        // Override current location to ensure challenge is possible
        own_team2.current_location = TeamLocation::OnPlanet { planet_id };

        for player_id in own_team2.player_ids.iter() {
            let player = app2.world.players.get(&player_id).unwrap();
            app1.world.players.insert(player_id.clone(), player.clone());
        }
        app1.world.teams.insert(own_team2.id, own_team2.clone());
        app2.world.teams.insert(own_team2.id, own_team2);

        let cb = UiCallback::ChallengeTeam {
            team_id: app2.world.own_team_id,
        };

        if let Err(e) = cb.call(&mut app1) {
            return Err(e);
        }

        let own_team1 = app1.world.get_own_team()?;
        assert!(own_team1
            .sent_challenges
            .get(&app2.world.own_team_id)
            .is_some());
        assert!(own_team1.current_game.is_none());

        let syn_challenge = own_team1
            .sent_challenges
            .get(&app2.world.own_team_id)
            .unwrap()
            .clone();

        // Mock up send_challenge
        let network_data = NetworkData::Challenge {
            timestamp: Tick::now(),
            challenge: syn_challenge,
        };
        let data = serialize::<NetworkData>(&network_data)?;

        let message = Message {
            source: None,
            data,
            sequence_number: None,
            topic: topic.clone().into(),
        };
        let cb = NetworkCallback::HandleMessage { message };
        assert!(cb.call(&mut app2).is_ok());

        let own_team2 = app2.world.get_own_team()?.clone();
        assert!(own_team2.current_game.is_none());
        let received_challenge = own_team2.received_challenges.get(&app1.world.own_team_id);
        assert!(received_challenge.is_some());

        let cb = UiCallback::AcceptChallenge {
            challenge: received_challenge.unwrap().clone(),
        };

        // Still no game
        let own_team2 = app2.world.get_own_team()?.clone();
        assert!(own_team2.current_game.is_none());

        if let Err(e) = cb.call(&mut app2) {
            return Err(e);
        }

        // Get response challenges
        let mut syn_ack_challenge = received_challenge.unwrap().clone();
        syn_ack_challenge.state = NetworkRequestState::SynAck;
        let mut ack_challenge = received_challenge.unwrap().clone();
        let starting_at = Tick::now() + NETWORK_GAME_START_DELAY;
        ack_challenge.starting_at = Some(starting_at);
        ack_challenge.state = NetworkRequestState::Ack;

        let network_data = NetworkData::Challenge {
            timestamp: Tick::now(),
            challenge: syn_ack_challenge,
        };
        let data = serialize::<NetworkData>(&network_data)?;

        let message = Message {
            source: None,
            data,
            sequence_number: None,
            topic: topic.clone().into(),
        };

        // Check that challenge has been removed after accepting
        let own_team2 = app2.world.get_own_team()?.clone();
        let received_challenge = own_team2.received_challenges.get(&app1.world.own_team_id);
        assert!(received_challenge.is_none());

        let cb = NetworkCallback::HandleMessage { message };
        let own_team1 = app1.world.get_own_team()?.clone();
        assert!(own_team1.current_game.is_none());
        assert!(cb.call(&mut app1).is_ok());
        let own_team1 = app1.world.get_own_team()?.clone();
        assert!(own_team1.current_game.is_some());

        let game_id = own_team1.current_game.unwrap();
        let game = app1.world.games.get_or_err(&game_id)?;
        println!("{:?}, starting_at {}", game_id, game.starting_at);

        let network_data = NetworkData::Challenge {
            timestamp: Tick::now(),
            challenge: ack_challenge,
        };
        let data = serialize::<NetworkData>(&network_data)?;

        let message = Message {
            source: None,
            data,
            sequence_number: None,
            topic: topic.clone().into(),
        };

        let cb = NetworkCallback::HandleMessage { message };
        let own_team2 = app2.world.get_own_team()?.clone();
        assert!(own_team2.current_game.is_none());

        if let Err(e) = cb.call(&mut app2) {
            return Err(e);
        }

        let own_team2 = app2.world.get_own_team()?.clone();
        let game_id = own_team2.current_game.unwrap();
        let game = app2.world.games.get_or_err(&game_id)?;
        println!("{:?}, starting_at {}", game_id, game.starting_at);
        assert!(own_team2.current_game == Some(game_id));

        Ok(())
    }

    #[test]
    fn test_send_own_team() -> AppResult<()> {
        let mut world = World::new(None);
        let rng = &mut ChaCha8Rng::from_os_rng();
        let home_planet = world.planets.keys().next().unwrap().clone();
        let team_name = "Testen".to_string();
        let ship_name = "Tosten".to_string();
        let own_team_id = world.generate_random_team(rng, home_planet, team_name, ship_name, None);
        let network_team =
            NetworkTeam::from_team_id(&world, &own_team_id.unwrap(), PeerId::random()).unwrap();

        let timestamp = Tick::now();
        let serialized_network_data = serialize(&NetworkData::Team {
            timestamp,
            team: network_team.clone(),
        })?;

        let deserialized_network_data = deserialize::<NetworkData>(&serialized_network_data)?;

        match deserialized_network_data {
            NetworkData::Team {
                timestamp: deserialized_timestamp,
                team: deserialized_team,
            } => {
                assert!(deserialized_timestamp == timestamp);

                assert_eq!(deserialized_team.team, network_team.team);
                assert_eq!(deserialized_team.players.len(), network_team.players.len());

                let player_id = &network_team.team.player_ids[0];
                let network_player = network_team.players.get(player_id).unwrap().clone();
                let deserialized_player = deserialized_team.players.get(player_id).unwrap().clone();
                assert_eq!(deserialized_player.id, network_player.id);
                assert_eq!(deserialized_player.mental, network_player.mental);
            }
            _ => return Err(anyhow!("Invalid NetworkData deserialization")),
        }

        Ok(())
    }
}
