use crate::app::AppEvent;
use crate::network::constants::{DEFAULT_SEED_PORT, TOPIC};
use crate::network::handler::NetworkHandler;
use crate::network::network_store_data::NetworkStoreData;
use crate::network::types::{ChatHistoryEntry, NetworkData};
use crate::store::*;
use crate::types::{AppResult, TeamId};
use libp2p::gossipsub::IdentTopic;
use libp2p::{gossipsub, swarm::SwarmEvent};
use std::collections::HashMap;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub struct Relayer {
    network_handler: NetworkHandler,
    relayer_messages: Vec<String>,
    last_message_sent_to_team: HashMap<TeamId, usize>,
    network_store_data: NetworkStoreData,
}

impl Default for Relayer {
    fn default() -> Self {
        Self::new()
    }
}

impl Relayer {
    pub fn new() -> Self {
        let network_store_data = if let Ok(data) = load_relayer_network_store_data() {
            data
        } else {
            NetworkStoreData::default()
        };

        Self {
            network_handler: NetworkHandler::new(None)
                .expect("Failed to initialize network handler"),
            relayer_messages: Vec::new(),
            last_message_sent_to_team: HashMap::new(),
            network_store_data,
        }
    }

    pub async fn run(&mut self) -> AppResult<()> {
        println!("Starting relayer. Press Ctrl-C to exit.");
        let (event_sender, mut event_receiver) = mpsc::channel(256);

        let cancellation_token = CancellationToken::new();
        self.network_handler.start_polling_events(
            event_sender,
            cancellation_token.clone(),
            DEFAULT_SEED_PORT,
            true,
            true,
        );
        loop {
            if let Some(AppEvent::NetworkEvent(swarm_event)) = event_receiver.recv().await {
                let result = self.handle_network_events(swarm_event);
                if result.is_err() {
                    println!("Error handling network event: {result:?}");
                }
            }
        }
    }

    pub fn handle_network_events(
        &mut self,
        network_event: SwarmEvent<gossipsub::Event>,
    ) -> AppResult<()> {
        println!("Received network event: {network_event:?}");
        match network_event {
            SwarmEvent::Behaviour(gossipsub::Event::Subscribed { peer_id, topic }) => {
                if topic == IdentTopic::new(TOPIC).hash() {
                    println!("Sending info to {peer_id}");
                    self.network_handler.send_seed_info(
                        self.network_store_data.get_top_team_ranking(),
                        self.network_store_data.get_top_player_ranking(),
                        self.network_store_data.get_random_peer_addresses(),
                        &self.network_store_data.chat_history,
                    )?;
                }
            }

            SwarmEvent::Behaviour(gossipsub::Event::Message { message, .. }) => {
                assert!(message.topic == IdentTopic::new(TOPIC).hash());
                let network_data = deserialize::<NetworkData>(&message.data)?;
                if let NetworkData::Team {
                    timestamp,
                    team: network_team,
                } = network_data
                {
                    if let Some(current_ranking) = self
                        .network_store_data
                        .team_ranking
                        .get(&network_team.team.id)
                    {
                        if current_ranking.timestamp >= timestamp {
                            return Ok(());
                        }
                    } else {
                        self.network_handler.send_seed_info(
                            self.network_store_data.get_top_team_ranking(),
                            self.network_store_data.get_top_player_ranking(),
                            self.network_store_data.get_random_peer_addresses(),
                            &self.network_store_data.chat_history,
                        )?;
                        self.network_handler.send_relayer_message_to_team(
                            format!(
                                "A new crew has started roaming the galaxy: {}",
                                network_team.team.name
                            ),
                            None,
                        )?;
                    }

                    self.network_store_data
                        .update_rankings(timestamp, &network_team);

                    save_relayer_network_store_data(&self.network_store_data, false)?;

                    // Check if there are new messages to send and append them to self.messages.
                    self.relayer_messages.extend(load_relayer_messages()?);

                    // Send messages starting from last sent message.
                    let last_message_sent = self
                        .last_message_sent_to_team
                        .get(&network_team.team.id)
                        .unwrap_or(&0);

                    for (index, message) in self.relayer_messages.iter().enumerate() {
                        if index < *last_message_sent {
                            continue;
                        }

                        self.network_handler.send_relayer_message_to_team(
                            message.clone(),
                            Some(network_team.team.id),
                        )?;
                    }

                    self.last_message_sent_to_team
                        .insert(network_team.team.id, self.relayer_messages.len());
                } else if let NetworkData::Message {
                    timestamp,
                    from_peer_id,
                    author,
                    message,
                } = network_data
                {
                    let entry = ChatHistoryEntry {
                        timestamp,
                        from_peer_id,
                        author,
                        message,
                    };
                    println!("Chat message stored: {entry:#?}");
                    self.network_store_data.chat_history.push(entry);
                } else if let NetworkData::SyncRequest = network_data {
                    self.network_handler.send_seed_info(
                        self.network_store_data.get_top_team_ranking(),
                        self.network_store_data.get_top_player_ranking(),
                        self.network_store_data.get_random_peer_addresses(),
                        &self.network_store_data.chat_history,
                    )?;
                } else if let NetworkData::PortInfo { port } = network_data {
                    if let Some(peer_id) = message.source {
                        self.network_store_data
                            .build_peer_address_from_port(peer_id, port);
                        save_relayer_network_store_data(&self.network_store_data, false)?;
                    }
                }
            }
            SwarmEvent::ConnectionEstablished {
                peer_id, endpoint, ..
            } => {
                self.network_store_data
                    .extract_and_store_peer_ip(peer_id, endpoint.get_remote_address());
            }
            _ => {}
        }
        Ok(())
    }
}
