use crate::{
    core::DAYS,
    network::types::{ChatHistoryEntry, NetworkTeam, PlayerRanking, TeamRanking},
    types::*,
};
use itertools::Itertools;
use libp2p::{Multiaddr, PeerId};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

const CHAT_RETENTION_DURATION: Tick = 7 * DAYS;
const PEERS_RETENTION_DURATION: Tick = 30 * DAYS;
const TOP_PLAYER_RANKING_LENGTH: usize = 20;
const TOP_TEAM_RANKING_LENGTH: usize = 10;
const RANDOM_PEER_ADDRESSES_LENGTH: usize = 10;
const MAX_CHAT_HISTORY: usize = 200;

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
pub struct NetworkStoreData {
    pub keypair: Option<Vec<u8>>, // Allows to re-establish the same PeerId across sessions.
    pub team_ranking: HashMap<TeamId, TeamRanking>,
    pub player_ranking: HashMap<PlayerId, PlayerRanking>,
    pub peer_addresses: HashMap<PeerId, Multiaddr>,
    pub peer_last_connection: HashMap<PeerId, Tick>,
    pub chat_history: HashSet<ChatHistoryEntry>,
}

impl NetworkStoreData {
    pub fn to_broadcast_snapshot(&self) -> Self {
        Self {
            team_ranking: self.get_top_team_ranking().into_iter().collect(),
            player_ranking: self.get_top_player_ranking().into_iter().collect(),
            peer_addresses: self.get_random_peer_addresses().into_iter().collect(),
            chat_history: self.get_recent_chat_history().into_iter().collect(),
            ..Default::default()
        }
    }

    pub fn update(&mut self, other: &Self) {
        for (id, team) in other.team_ranking.iter() {
            if !self.team_ranking.contains_key(id) {
                self.team_ranking.insert(*id, team.clone());
            }
        }

        for (id, player) in other.player_ranking.iter() {
            if !self.player_ranking.contains_key(id) {
                self.player_ranking.insert(*id, player.clone());
            }
        }

        for entry in other.chat_history.iter() {
            self.chat_history.insert(entry.clone());
        }

        // Peers are not updated here, as they are only added if the connection is succesfull
    }

    pub fn reset_peers(&mut self) {
        self.peer_addresses.clear();
        self.peer_last_connection.clear();
    }

    pub fn to_store(&self) -> Self {
        let mut self_store = self.clone();
        let now = Tick::now();
        self_store
            .chat_history
            .retain(|entry| now.saturating_sub(entry.timestamp) <= CHAT_RETENTION_DURATION);

        self_store.peer_addresses.retain(|peer_id, _| {
            now.saturating_sub(
                self_store
                    .peer_last_connection
                    .get(peer_id)
                    .copied()
                    .unwrap_or_default(),
            ) <= PEERS_RETENTION_DURATION
        });

        self_store
            .peer_last_connection
            .retain(|_, timestamp| now.saturating_sub(*timestamp) <= PEERS_RETENTION_DURATION);

        self_store
    }

    pub fn set_keypair(&mut self, keypair: Vec<u8>) {
        self.keypair = Some(keypair);
    }

    pub fn update_peer_addresses(&mut self, peer_id: PeerId, address: Multiaddr) {
        self.peer_addresses.insert(peer_id, address);
        self.peer_last_connection.insert(peer_id, Tick::now());
    }

    pub fn update_rankings(&mut self, timestamp: Tick, network_team: &NetworkTeam) {
        let ranking = TeamRanking::from_network_team(timestamp, network_team);

        // If the team is already stored, remove players from previous version.
        // This is to ensure that fired players are removed.
        if let Some(current_ranking) = self.team_ranking.get(&network_team.team.id) {
            for player_id in current_ranking.team.player_ids.iter() {
                self.player_ranking.remove(player_id);
            }
        }

        self.team_ranking
            .insert(network_team.team.id, ranking.clone());

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
    }

    pub fn get_top_player_ranking(&self) -> Vec<(PlayerId, PlayerRanking)> {
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

    pub fn get_random_peer_addresses(&self) -> Vec<(PeerId, Multiaddr)> {
        self.peer_addresses
            .iter()
            .sorted_by(|(a, _), (b, _)| {
                let a_timestamp = self.peer_last_connection.get(a).unwrap_or(&0);
                let b_timestamp = self.peer_last_connection.get(b).unwrap_or(&0);
                b_timestamp.cmp(a_timestamp)
            })
            .take(RANDOM_PEER_ADDRESSES_LENGTH)
            .map(|(k, v)| (*k, v.clone()))
            .collect_vec()
    }

    pub fn get_top_team_ranking(&self) -> Vec<(TeamId, TeamRanking)> {
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

    pub fn get_recent_chat_history(&self) -> Vec<ChatHistoryEntry> {
        self.chat_history
            .iter()
            .sorted_by_key(|e| e.timestamp)
            .rev()
            .take(MAX_CHAT_HISTORY)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .cloned()
            .collect()
    }
}
