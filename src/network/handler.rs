use super::challenge::Challenge;
use super::constants::*;
use super::network_callback::NetworkCallback;
use super::trade::Trade;
use super::types::{NetworkData, NetworkGame, NetworkRequestState, NetworkTeam, SeedInfo};
use crate::game_engine::types::TeamInGame;
use crate::store::serialize;
use crate::types::{AppResult, GameId};
use crate::types::{PlayerId, TeamId};
use crate::types::{SystemTimeTick, Tick};
use crate::world::world::World;
use anyhow::anyhow;
use libp2p::gossipsub::{self, IdentTopic, MessageId};
use libp2p::swarm::SwarmEvent;
use libp2p::{identity, noise, tcp, yamux, PeerId};
use libp2p::{Multiaddr, Swarm};
use log::{error, info};
use std::collections::hash_map::DefaultHasher;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use std::time::Duration;

pub struct NetworkHandler {
    pub swarm: Swarm<gossipsub::Behaviour>,
    pub address: Multiaddr,
    pub seed_addresses: Vec<Multiaddr>,
}

impl Debug for NetworkHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NetworkHandler")
            .field("address", &self.address)
            .finish()
    }
}

impl NetworkHandler {
    pub fn new(seed_ip: Option<String>, tcp_port: u16) -> AppResult<Self> {
        let local_key = identity::Keypair::generate_ed25519();

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
            gossipsub::MessageAuthenticity::Signed(local_key),
            gossipsub_config,
        )
        .expect("Correct configuration");

        gossipsub.subscribe(&IdentTopic::new(TOPIC))?;

        let mut swarm = libp2p::SwarmBuilder::with_new_identity()
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
        if let Err(e) = swarm.listen_on(format!("/ip4/0.0.0.0/tcp/{tcp_port}").parse()?) {
            log::error!("Could not listen on ip4: {}", e);
        } else {
            succesful_listen_on = true;
        }
        if let Err(e) = swarm.listen_on(format!("/ip6/::/tcp/{tcp_port}").parse()?) {
            log::error!("Could not listen on ip6: {}", e);
        } else {
            succesful_listen_on = true;
        };

        if !succesful_listen_on {
            return Err(anyhow!("Swarm could not start listening."));
        }

        let mut seed_addresses = vec![
            format!("/dns4/{DEFAULT_SEED_URL}/tcp/{DEFAULT_SEED_PORT}")
                .parse()
                .expect("Invalid default seed address."),
            format!("/ip6/{DEFAULT_SEED_IPV6}/tcp/{DEFAULT_SEED_PORT}")
                .parse()
                .expect("Invalid provided seed ip."),
        ];

        if let Some(ip) = seed_ip {
            if let Ok(address) = format!("/ip4/{ip}/tcp/{DEFAULT_SEED_PORT}").parse() {
                seed_addresses.push(address);
            } else if let Ok(address) = format!("/ip6/{ip}/tcp/{DEFAULT_SEED_PORT}").parse() {
                seed_addresses.push(address);
            }
        }

        info!(
            "Network handler started on port {} with {} seed addresses.",
            tcp_port,
            seed_addresses.len()
        );

        Ok(Self {
            swarm,
            address: Multiaddr::empty(),
            seed_addresses,
        })
    }

    fn _send(&mut self, data: NetworkData) -> AppResult<MessageId> {
        let data = serialize(&data)?;
        let msg_id = self
            .swarm
            .behaviour_mut()
            .publish(IdentTopic::new(TOPIC), data)?;
        Ok(msg_id)
    }

    pub fn dial_seed(&mut self) -> AppResult<()> {
        for address in self.seed_addresses.iter() {
            if *address != self.address {
                if let Err(e) = self.swarm.dial(address.clone()) {
                    log::error!("Dial error: {}", e);
                }
            }
        }
        Ok(())
    }

    pub fn send_msg(&mut self, msg: String) -> AppResult<MessageId> {
        self._send(NetworkData::Message(Tick::now(), msg))
    }

    pub fn send_seed_info(&mut self, seed_info: SeedInfo) -> AppResult<MessageId> {
        self._send(NetworkData::SeedInfo(Tick::now(), seed_info))
    }

    pub fn send_own_team(&mut self, world: &World) -> AppResult<MessageId> {
        let message_id = if world.has_own_team() {
            self.send_team(world, world.own_team_id)?
        } else {
            return Err(anyhow!("No own team"));
        };

        // If own team is playing with network peer, send the game.
        if let Some(game_id) = world.get_own_team()?.current_game {
            let game = world.get_game_or_err(&game_id)?;
            // FIX BUG?? Send game even if we are playing with local team.
            // return self.send_game(world, game_id);

            if game.home_team_in_game.peer_id.is_some() || game.away_team_in_game.peer_id.is_some()
            {
                return self.send_game(world, &game_id);
            }
        }

        Ok(message_id)
    }

    fn send_game(&mut self, world: &World, game_id: &GameId) -> AppResult<MessageId> {
        let network_game = NetworkGame::from_game_id(&world, game_id)?;
        self._send(NetworkData::Game(Tick::now(), network_game))
    }

    fn send_team(&mut self, world: &World, team_id: TeamId) -> AppResult<MessageId> {
        let network_team =
            NetworkTeam::from_team_id(world, &team_id, self.swarm.local_peer_id().clone())?;

        self._send(NetworkData::Team(Tick::now(), network_team))
    }

    pub fn send_challenge(&mut self, challenge: Challenge) -> AppResult<MessageId> {
        self._send(NetworkData::Challenge(Tick::now(), challenge))
    }

    pub fn send_trade(&mut self, trade: Trade) -> AppResult<MessageId> {
        self._send(NetworkData::Trade(Tick::now(), trade))
    }

    pub fn send_new_challenge(
        &mut self,
        world: &World,
        peer_id: PeerId,
        team_id: TeamId,
    ) -> AppResult<Challenge> {
        self.send_own_team(world)?;
        let mut home_team_in_game =
            TeamInGame::from_team_id(world.own_team_id, &world.teams, &world.players)
                .ok_or(anyhow!("Cannot generate team in game"))?;
        home_team_in_game.peer_id = Some(self.swarm.local_peer_id().clone());

        let away_team_in_game = TeamInGame::from_team_id(team_id, &world.teams, &world.players)
            .ok_or(anyhow!("Cannot generate team in game"))?;

        let challenge = Challenge::new(
            self.swarm.local_peer_id().clone(),
            peer_id,
            home_team_in_game,
            away_team_in_game,
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

        let proposer_player = world.get_player_or_err(&proposer_player_id)?.clone();
        let target_player = world.get_player_or_err(&target_player_id)?.clone();

        let trade = Trade::new(
            self.swarm.local_peer_id().clone(),
            target_peer_id,
            proposer_player,
            target_player,
            0,
        );

        self.send_trade(trade.clone())?;
        Ok(trade)
    }

    pub fn quit(&mut self) {
        if let Err(e) = self
            .swarm
            .behaviour_mut()
            .unsubscribe(&IdentTopic::new(TOPIC))
        {
            error!("Error unsubscribing from events: {e}");
        }

        let peers = self
            .swarm
            .connected_peers()
            .map(|id| id.clone())
            .collect::<Vec<PeerId>>();

        for peer_id in peers {
            if self.swarm.is_connected(&peer_id) {
                let _ = self
                    .swarm
                    .disconnect_peer_id(peer_id)
                    .map_err(|e| error!("Error disconnecting peer id {}: {:?}", peer_id, e));
            }
        }

        let external_addresses = self
            .swarm
            .external_addresses()
            .map(|addr| addr.clone())
            .collect::<Vec<Multiaddr>>();

        for addr in external_addresses {
            self.swarm.remove_external_address(&addr);
        }
    }

    pub fn accept_challenge(&mut self, world: &World, challenge: Challenge) -> AppResult<()> {
        self.send_own_team(world)?;
        let mut handle_syn = || -> AppResult<()> {
            let home_team = world.get_team_or_err(&challenge.home_team_in_game.team_id)?;
            let away_team = world.get_team_or_err(&challenge.away_team_in_game.team_id)?;
            home_team.can_challenge_team(away_team)?;

            let mut away_team_in_game =
                TeamInGame::from_team_id(world.own_team_id, &world.teams, &world.players)
                    .ok_or(anyhow!("Cannot generate team in game"))?;

            away_team_in_game.peer_id = Some(self.swarm.local_peer_id().clone());

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

    pub fn decline_challenge(&mut self, challenge: Challenge) -> AppResult<()> {
        let mut challenge = challenge.clone();
        challenge.state = NetworkRequestState::Failed {
            error_message: "Challenge declined".to_string(),
        };
        self.send_challenge(challenge)?;
        Ok(())
    }

    pub fn accept_trade(&mut self, world: &World, trade: Trade) -> AppResult<()> {
        let mut handle_syn = || -> AppResult<()> {
            let own_team = world.get_own_team()?;
            let proposer_team = if let Some(proposer_team_id) = trade.proposer_player.team {
                world.get_team_or_err(&proposer_team_id)?
            } else {
                return Err(anyhow!("Trade target player has no team"));
            };

            // Note: we do not apply immediately the trade at this point,
            // because it could take a long time to accept a trade
            // and the status of the proposer could have changed considerably
            // possibly making the trade invalid.
            let mut trade = trade.clone();
            let target_player = world.get_player_or_err(&trade.target_player.id)?.clone();
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
                text: format!("Expired listen address: {}", address),
            }),
            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                Some(NetworkCallback::HandleConnectionEstablished { peer_id })
            }
            SwarmEvent::ConnectionClosed { peer_id, .. } => {
                Some(NetworkCallback::CloseConnection { peer_id })
            }
            _ => Some(NetworkCallback::PushSwarmPanelLog {
                timestamp: Tick::now(),
                text: format!("Event: {:?}", event),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        network::types::{NetworkData, NetworkTeam},
        store::{deserialize, serialize},
        types::{AppResult, SystemTimeTick, Tick},
        world::world::World,
    };
    use anyhow::anyhow;
    use libp2p::PeerId;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    #[test]
    fn test_send_own_team() -> AppResult<()> {
        let mut world = World::new(None);
        let rng = &mut ChaCha8Rng::from_entropy();
        let home_planet = world.planets.keys().next().unwrap().clone();
        let team_name = "Testen".to_string();
        let ship_name = "Tosten".to_string();
        let own_team_id = world.generate_random_team(rng, home_planet, team_name, ship_name);
        let network_team =
            NetworkTeam::from_team_id(&world, &own_team_id.unwrap(), PeerId::random()).unwrap();

        let timestamp = Tick::now();
        let serialized_network_data =
            serialize(&NetworkData::Team(timestamp, network_team.clone()))?;

        let deserialized_network_data = deserialize::<NetworkData>(&serialized_network_data)?;

        match deserialized_network_data {
            NetworkData::Team(deserialized_timestamp, deserialized_team) => {
                assert!(deserialized_timestamp == timestamp);

                assert_eq!(deserialized_team.team, network_team.team);
                assert_eq!(deserialized_team.players.len(), network_team.players.len());

                let network_player = network_team.players[0].clone();
                let deserialized_player = deserialized_team.players[0].clone();
                assert_eq!(deserialized_player.id, network_player.id);
                assert_eq!(deserialized_player.mental, network_player.mental);
            }
            _ => return Err(anyhow!("Invalid NetworkData deserialization")),
        }

        Ok(())
    }
}
