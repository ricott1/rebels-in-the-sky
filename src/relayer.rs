use crate::app::AppEvent;
use crate::core::DAYS;
use crate::network::constants::{DEFAULT_SEED_PORT, TOPIC};
use crate::network::types::{ChatHistoryEntry, NetworkData, PlayerRanking, TeamRanking};
use crate::network::{handler::NetworkHandler, types::SeedInfo};
use crate::store::*;
use crate::types::{AppResult, PlayerId, SystemTimeTick, TeamId, Tick};
use itertools::Itertools;
use libp2p::gossipsub::IdentTopic;
use libp2p::{gossipsub, swarm::SwarmEvent};
use std::collections::HashMap;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

const TOP_PLAYER_RANKING_LENGTH: usize = 20;
const TOP_TEAM_RANKING_LENGTH: usize = 10;
const CHAT_RETENTION_DURATION: Tick = 5 * DAYS;

pub struct Relayer {
    running: bool,
    network_handler: NetworkHandler,
    team_ranking: HashMap<TeamId, TeamRanking>,
    top_team_ranking: Vec<(TeamId, TeamRanking)>,
    player_ranking: HashMap<PlayerId, PlayerRanking>,
    top_player_ranking: Vec<(PlayerId, PlayerRanking)>,
    relayer_messages: Vec<String>,
    chat_history: Vec<ChatHistoryEntry>,
    last_message_sent_to_team: HashMap<TeamId, usize>,
}

impl Default for Relayer {
    fn default() -> Self {
        Self::new()
    }
}

impl Relayer {
    fn get_top_player_ranking(&self) -> Vec<(PlayerId, PlayerRanking)> {
        self.player_ranking
            .iter()
            .sorted_by(|(_, a), (_, b)| {
                b.player
                    .reputation
                    .partial_cmp(&a.player.reputation)
                    .expect("Reputation should exist")
            })
            .take(TOP_PLAYER_RANKING_LENGTH)
            .map(|(id, ranking)| (*id, ranking.clone()))
            .collect()
    }

    fn get_top_team_ranking(&self) -> Vec<(TeamId, TeamRanking)> {
        self.team_ranking
            .iter()
            .sorted_by(|(_, a), (_, b)| {
                b.team
                    .reputation
                    .partial_cmp(&a.team.reputation)
                    .expect("Reputation should exist")
            })
            .take(TOP_TEAM_RANKING_LENGTH)
            .map(|(id, ranking)| (*id, ranking.clone()))
            .collect()
    }

    pub fn new() -> Self {
        let team_ranking = match load_team_ranking() {
            Ok(team_ranking) => team_ranking,
            Err(err) => {
                println!("Error while loading team ranking: {err}");
                HashMap::new()
            }
        };

        println!("Team ranking has {} entries.", team_ranking.len());

        let top_team_ranking = team_ranking
            .iter()
            .sorted_by(|(_, a), (_, b)| {
                b.team
                    .reputation
                    .partial_cmp(&a.team.reputation)
                    .expect("Reputation should exist")
            })
            .take(TOP_TEAM_RANKING_LENGTH)
            .map(|(id, ranking)| (*id, ranking.clone()))
            .collect();

        let player_ranking = match load_player_ranking() {
            Ok(player_ranking) => player_ranking,
            Err(err) => {
                println!("Error while loading player ranking: {err}");
                HashMap::new()
            }
        };

        println!("Player ranking has {} entries.", player_ranking.len());

        let top_player_ranking = player_ranking
            .iter()
            .sorted_by(|(_, a), (_, b)| {
                b.player
                    .reputation
                    .partial_cmp(&a.player.reputation)
                    .expect("Reputation should exist")
            })
            .take(TOP_PLAYER_RANKING_LENGTH)
            .map(|(id, ranking)| (*id, ranking.clone()))
            .collect();

        let chat_history = match load_chat_history() {
            Ok(chat_history) => chat_history,
            Err(err) => {
                println!("Error while loading chat history: {err}");
                Vec::new()
            }
        };

        println!("Chat history has {} entries.", chat_history.len());

        Self {
            running: true,
            network_handler: NetworkHandler::new(None)
                .expect("Failed to initialize network handler"),
            team_ranking,
            top_team_ranking,
            player_ranking,
            top_player_ranking,
            relayer_messages: Vec::new(),
            chat_history,
            last_message_sent_to_team: HashMap::new(),
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
        while self.running {
            if let Some(AppEvent::NetworkEvent(swarm_event)) = event_receiver.recv().await {
                let result = self.handle_network_events(swarm_event);
                if result.is_err() {
                    println!("Error handling network event: {result:?}");
                }
            }
        }

        if let Err(err) = save_chat_history(&self.chat_history) {
            println!("Error while saving chat history: {err}");
        }
        cancellation_token.cancel();
        Ok(())
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

                    let now = Tick::now();
                    self.chat_history.retain(|entry| {
                        now.saturating_sub(entry.timestamp) <= CHAT_RETENTION_DURATION
                    });

                    const MAX_CHAT_HISTORY: usize = 200;
                    let chat_history: Vec<ChatHistoryEntry> = self
                        .chat_history
                        .iter()
                        .sorted_by_key(|e| e.timestamp)
                        .rev()
                        .take(MAX_CHAT_HISTORY)
                        .collect::<Vec<_>>()
                        .into_iter()
                        .rev()
                        .cloned()
                        .collect();

                    println!(
                        "Bundling {} chat history entries into SeedInfo for {}",
                        chat_history.len(),
                        peer_id
                    );

                    if let Err(err) = save_chat_history(&self.chat_history) {
                        println!("Error while saving chat history: {err}");
                    }

                    self.network_handler.send_seed_info(SeedInfo::new(
                        self.network_handler.connected_peers_count,
                        None,
                        self.top_team_ranking.clone(),
                        self.top_player_ranking.clone(),
                        chat_history,
                    )?)?;
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
                    if let Some(current_ranking) = self.team_ranking.get(&network_team.team.id) {
                        if current_ranking.timestamp >= timestamp {
                            return Ok(());
                        }
                    } else {
                        self.network_handler.send_seed_info(SeedInfo::new(
                            self.network_handler.connected_peers_count,
                            Some(format!(
                                "A new crew has started roaming the galaxy: {}",
                                network_team.team.name
                            )),
                            self.top_team_ranking.clone(),
                            self.top_player_ranking.clone(),
                            vec![],
                        )?)?;
                    }

                    let ranking = TeamRanking::from_network_team(timestamp, &network_team);

                    // If the team is already stored, remove players from previous version.
                    // This is to ensure that fired players are removed.
                    if let Some(current_ranking) = self.team_ranking.get(&network_team.team.id) {
                        for player_id in current_ranking.team.player_ids.iter() {
                            self.player_ranking.remove(player_id);
                        }
                    }

                    self.team_ranking
                        .insert(network_team.team.id, ranking.clone());

                    if let Err(err) = save_team_ranking(&self.team_ranking, true) {
                        println!("Error while saving team ranking: {err}");
                    }

                    for player in network_team.players.values() {
                        let team_name = if let Some(team_id) = player.team.as_ref() {
                            if let Some(team_ranking) = self.team_ranking.get(team_id) {
                                team_ranking.team.name.clone()
                            } else {
                                "Unknown".to_string()
                            }
                        } else {
                            "Free pirate".to_string()
                        };

                        let ranking = PlayerRanking::new(timestamp, player.clone(), team_name);
                        self.player_ranking.insert(player.id, ranking.clone());
                    }

                    if let Err(err) = save_player_ranking(&self.player_ranking, true) {
                        println!("Error while saving player ranking: {err}");
                    }

                    self.top_team_ranking = self.get_top_team_ranking();
                    self.top_player_ranking = self.get_top_player_ranking();

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

                        self.network_handler
                            .send_relayer_message_to_team(message.clone(), network_team.team.id)?;
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
                    self.chat_history.push(entry);
                }
            }
            _ => {}
        }
        Ok(())
    }
}
