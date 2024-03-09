use super::constants::*;
use super::network_callback::NetworkCallbackPreset;
use super::types::{Challenge, ChallengeState, NetworkGame, NetworkTeam, SeedInfo};
use crate::engine::types::TeamInGame;
use crate::types::TeamId;
use crate::types::{AppResult, GameId};
use crate::types::{SystemTimeTick, Tick};
use crate::world::world::World;
use libp2p::core::upgrade::Version;
use libp2p::gossipsub::{self, IdentTopic, MessageId};
use libp2p::swarm::{Config, SwarmEvent};
use libp2p::{identity, noise, tcp, yamux, PeerId, Transport};
use libp2p::{Multiaddr, Swarm};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::error::Error;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use std::time::Duration;
use void::Void;

pub struct NetworkHandler {
    pub swarm: Swarm<gossipsub::Behaviour>,
    pub address: Multiaddr,
    challenges: HashMap<PeerId, Challenge>,
    pub seed_address: Multiaddr,
}

impl Debug for NetworkHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NetworkHandler")
            .field("address", &self.address)
            .field("challenges", &self.challenges)
            .finish()
    }
}

impl NetworkHandler {
    pub fn new(seed_ip: Option<String>) -> Result<Self, Box<dyn Error>> {
        let local_key = identity::Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(local_key.public());

        let tcp_transport = tcp::tokio::Transport::default()
            .upgrade(Version::V1Lazy)
            .authenticate(noise::Config::new(&local_key)?)
            .multiplex(yamux::Config::default())
            .timeout(std::time::Duration::from_secs(20))
            .boxed();

        // let quic_transport = quic::tokio::Transport::new(quic::Config::new(&local_key));
        // let transport = OrTransport::new(quic_transport, tcp_transport)
        //     .map(|either_output, _| match either_output {
        //         Either::Left((peer_id, muxer)) => (peer_id, StreamMuxerBox::new(muxer)),
        //         Either::Right((peer_id, muxer)) => (peer_id, StreamMuxerBox::new(muxer)),
        //     })
        //     .boxed();

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

        gossipsub.subscribe(&IdentTopic::new(SubscriptionTopic::SEED_INFO))?;
        gossipsub.subscribe(&IdentTopic::new(SubscriptionTopic::TEAM))?;
        gossipsub.subscribe(&IdentTopic::new(SubscriptionTopic::MSG))?;
        gossipsub.subscribe(&IdentTopic::new(SubscriptionTopic::GAME))?;
        gossipsub.subscribe(&IdentTopic::new(SubscriptionTopic::CHALLENGE))?;

        let mut swarm = Swarm::new(
            tcp_transport,
            gossipsub,
            local_peer_id,
            Config::with_tokio_executor(),
        );

        // swarm.listen_on("/ip4/0.0.0.0/udp/0/quic-v1".parse()?)?;
        if swarm
            .listen_on(format!("/ip4/0.0.0.0/tcp/{DEFAULT_PORT}").parse()?)
            .is_err()
        {
            swarm.listen_on(format!("/ip4/0.0.0.0/tcp/{DEFAULT_PORT_BACKUP}").parse()?)?;
        }

        let seed_address = match seed_ip {
            Some(ip) => format!("/ip4/{ip}/tcp/{DEFAULT_PORT}")
                .parse()
                .expect("Invalid provided seed ip."),
            None => SEED_ADDRESS.parse()?,
        };

        Ok(Self {
            swarm,
            address: Multiaddr::empty(),
            challenges: HashMap::new(),
            seed_address,
        })
    }

    fn _send(&mut self, data: Vec<u8>, topic: &str) -> AppResult<MessageId> {
        let timestamp = Tick::now().to_le_bytes().to_vec();
        let msg_id = self
            .swarm
            .behaviour_mut()
            .publish(IdentTopic::new(topic), [timestamp, data].concat());
        Ok(msg_id?)
    }

    pub fn add_challenge(&mut self, challenge: Challenge) {
        self.challenges
            .insert(challenge.home_peer_id.clone(), challenge);
    }

    pub fn dial(&mut self, address: Multiaddr) -> AppResult<()> {
        if address != self.address {
            self.swarm.dial(address)?;
        }
        Ok(())
    }

    pub fn send_msg(&mut self, msg: String) -> AppResult<MessageId> {
        self._send(msg.as_bytes().to_vec(), SubscriptionTopic::MSG)
    }

    pub fn send_seed_info(&mut self, info: SeedInfo) -> AppResult<MessageId> {
        let serialized_info = serde_json::to_string(&info)?.as_bytes().to_vec();
        self._send(serialized_info, SubscriptionTopic::SEED_INFO)
    }

    pub fn send_own_team(&mut self, world: &World) -> AppResult<MessageId> {
        let message_id = if world.has_own_team() {
            self.send_team(world, world.own_team_id)?
        } else {
            return Err("No own team".into());
        };

        //If own team is playing with network peer, send the game.
        if let Some(game_id) = world.get_own_team()?.current_game {
            let game = world.get_game_or_err(game_id)?;
            if game.home_team_in_game.peer_id.is_some() && game.away_team_in_game.peer_id.is_some()
            {
                return self.send_game(world, game_id);
            }
        }

        Ok(message_id)
    }

    fn send_game(&mut self, world: &World, game_id: GameId) -> AppResult<MessageId> {
        let network_game = NetworkGame::from_game_id(&world, game_id)?;
        let serialized_game = serde_json::to_string(&network_game)?.as_bytes().to_vec();
        self._send(serialized_game, SubscriptionTopic::GAME)
    }

    fn send_team(&mut self, world: &World, team_id: TeamId) -> AppResult<MessageId> {
        let mut network_team = NetworkTeam::from_team_id(world, &team_id)?;
        // Set the peer_id for team we are sending out
        // This means that the team can be challenged online and it will not be stored.
        network_team.set_peer_id(self.swarm.local_peer_id().clone());

        let serialized_team = serde_json::to_string(&network_team)?.as_bytes().to_vec();
        self._send(serialized_team, SubscriptionTopic::TEAM)
    }

    pub fn send_challenge(&mut self, challenge: &Challenge) -> AppResult<MessageId> {
        let serialized_challenge = serde_json::to_vec(challenge)?;
        self._send(serialized_challenge, SubscriptionTopic::CHALLENGE)
    }

    pub fn can_handle_challenge(world: &World) -> AppResult<()> {
        if !world.has_own_team() {
            return Err(format!("No own team, declining challenge").into());
        }

        let own_team = world.get_own_team()?;

        if own_team.current_game.is_some() {
            return Err(format!("Already in a game, declining challenge").into());
        }

        Ok(())
    }

    pub fn send_new_challenge(&mut self, world: &World, peer_id: PeerId) -> AppResult<()> {
        self.send_own_team(world)?;

        let mut challenge = Challenge::new(self.swarm.local_peer_id().clone(), peer_id);
        let mut home_team_in_game =
            TeamInGame::from_team_id(world.own_team_id, &world.teams, &world.players)
                .ok_or("Cannot generate team in game")?;
        home_team_in_game.peer_id = Some(self.swarm.local_peer_id().clone());
        challenge.home_team = Some(home_team_in_game);

        self.send_challenge(&challenge)?;
        Ok(())
    }

    pub fn accept_challenge(&mut self, world: &World, challenge: Challenge) -> AppResult<()> {
        let mut handle_syn = || -> AppResult<()> {
            Self::can_handle_challenge(world)?;

            let away_team = world.get_own_team()?;
            if away_team.current_game.is_some() {
                return Err("Cannot accept challenge, already in a game".into());
            }

            let try_away_team_in_game =
                TeamInGame::from_team_id(world.own_team_id, &world.teams, &world.players);

            if try_away_team_in_game.is_none() {
                return Err("Cannot generate team in game for challenge")?;
            }

            let mut away_team_in_game = try_away_team_in_game.unwrap();
            away_team_in_game.peer_id = Some(self.swarm.local_peer_id().clone());

            let mut challenge = challenge.clone();
            challenge.away_team = Some(away_team_in_game);
            challenge.state = ChallengeState::SynAck;
            self.send_challenge(&challenge)?;
            Ok(())
        };

        if let Err(err) = handle_syn() {
            let mut challenge = Challenge::new(challenge.home_peer_id, challenge.away_peer_id);
            challenge.state = ChallengeState::Failed;
            challenge.error_message = Some(err.to_string());
            self.send_challenge(&challenge)?;
            return Err(err.to_string())?;
        }
        Ok(())
    }

    pub fn decline_challenge(&mut self, challenge: Challenge) -> AppResult<()> {
        let mut challenge = challenge.clone();
        challenge.state = ChallengeState::Failed;
        challenge.error_message = Some("Declined".to_string());
        self.send_challenge(&challenge)?;
        self.challenges.remove(&challenge.home_peer_id);
        Ok(())
    }

    pub fn handle_network_events(
        &mut self,
        event: SwarmEvent<gossipsub::Event, Void>,
    ) -> Option<NetworkCallbackPreset> {
        // self.swarm.connected_peers()
        match event {
            SwarmEvent::NewListenAddr { address, .. } => {
                Some(NetworkCallbackPreset::BindAddress { address })
            }
            SwarmEvent::Behaviour(gossipsub::Event::Message {
                propagation_source: _,
                message_id: _,
                message,
            }) => match message.topic.clone() {
                x if x == IdentTopic::new(SubscriptionTopic::TEAM).hash() => {
                    Some(NetworkCallbackPreset::HandleTeamTopic { message })
                }
                x if x == IdentTopic::new(SubscriptionTopic::MSG).hash() => {
                    Some(NetworkCallbackPreset::HandleMsgTopic { message })
                }
                x if x == IdentTopic::new(SubscriptionTopic::CHALLENGE).hash() => {
                    Some(NetworkCallbackPreset::HandleChallengeTopic { message })
                }
                x if x == IdentTopic::new(SubscriptionTopic::GAME).hash() => {
                    Some(NetworkCallbackPreset::HandleGameTopic { message })
                }
                x if x == IdentTopic::new(SubscriptionTopic::SEED_INFO).hash() => {
                    Some(NetworkCallbackPreset::HandleSeedTopic { message })
                }
                _ => None,
            },
            SwarmEvent::Behaviour(gossipsub::Event::Subscribed { peer_id, topic }) => {
                Some(NetworkCallbackPreset::Subscribe { peer_id, topic })
            }

            SwarmEvent::Behaviour(gossipsub::Event::Unsubscribed { peer_id, topic }) => {
                Some(NetworkCallbackPreset::Unsubscribe { peer_id, topic })
            }
            SwarmEvent::ExpiredListenAddr {
                listener_id: _,
                address,
            } => Some(NetworkCallbackPreset::PushSwarmPanelLog {
                timestamp: Tick::now(),
                text: format!("Expired listen address: {}", address),
            }),
            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                Some(NetworkCallbackPreset::HandleConnectionEstablished { peer_id })
            }
            SwarmEvent::ConnectionClosed { peer_id, .. } => {
                Some(NetworkCallbackPreset::CloseConnection { peer_id })
            }
            _ => Some(NetworkCallbackPreset::PushSwarmPanelLog {
                timestamp: Tick::now(),
                text: format!("Event: {:?}", event),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        network::types::NetworkTeam,
        types::{SystemTimeTick, Tick},
        world::world::World,
    };
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    #[test]
    fn test_send_own_team() {
        let mut world = World::new(None);
        let rng = &mut ChaCha8Rng::from_entropy();
        let home_planet = world.planets.keys().next().unwrap().clone();
        let team_name = "Testen".to_string();
        let ship_name = "Tosten".to_string();
        let own_team_id = world.generate_random_team(rng, home_planet, team_name, ship_name);
        let network_team = NetworkTeam::from_team_id(&world, &own_team_id.unwrap()).unwrap();

        let timestamp = Tick::now().as_secs().to_le_bytes().to_vec();
        let serialized_team = serde_json::to_string(&network_team)
            .unwrap()
            .as_bytes()
            .to_vec();
        let data = [timestamp.clone(), serialized_team].concat();

        let deserialize_timestamp = u128::from_le_bytes(data[..16].try_into().unwrap());
        let old_timestamp: u128 = u128::from_le_bytes(timestamp.as_slice().try_into().unwrap());
        assert!(old_timestamp == deserialize_timestamp);
        let try_deserialize_team = serde_json::from_slice::<NetworkTeam>(&data[16..]);
        assert_eq!(try_deserialize_team.is_ok(), true);
        let deserialized_team = try_deserialize_team.unwrap();
        assert_eq!(deserialized_team.team, network_team.team);
        assert_eq!(deserialized_team.players.len(), network_team.players.len());
        assert_eq!(deserialized_team.players[0], network_team.players[0]);
    }
}
