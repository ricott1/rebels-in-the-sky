use crate::{
    core::DAYS,
    network::types::{ChatHistoryEntry, NetworkTeam, PlayerRanking, TeamRanking},
    types::*,
};
use itertools::Itertools;
use libp2p::{multiaddr::Protocol, Multiaddr, PeerId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::IpAddr;

const CHAT_RETENTION_DURATION: Tick = 7 * DAYS;
const PEERS_RETENTION_DURATION: Tick = 30 * DAYS;
const TOP_PLAYER_RANKING_LENGTH: usize = 20;
const TOP_TEAM_RANKING_LENGTH: usize = 10;
const RANDOM_PEER_ADDRESSES_LENGTH: usize = 10;

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
pub struct NetworkStoreData {
    pub keypair: Option<Vec<u8>>, // Allows to re-establish the same PeerId across sessions.
    pub team_ranking: HashMap<TeamId, TeamRanking>,
    pub player_ranking: HashMap<PlayerId, PlayerRanking>,
    pub peer_addresses: HashMap<PeerId, Multiaddr>,
    pub peer_last_connection: HashMap<PeerId, Tick>,
    #[serde(default)]
    pub peer_ip: HashMap<PeerId, IpAddr>,
    pub chat_history: Vec<ChatHistoryEntry>,
}

impl NetworkStoreData {
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

    pub fn extract_and_store_peer_ip(&mut self, peer_id: PeerId, address: &Multiaddr) {
        for proto in address.iter() {
            match proto {
                Protocol::Ip4(ip) => {
                    self.peer_ip.insert(peer_id, IpAddr::V4(ip));
                    return;
                }
                Protocol::Ip6(ip) => {
                    self.peer_ip.insert(peer_id, IpAddr::V6(ip));
                    return;
                }
                _ => {}
            }
        }
    }

    pub fn build_peer_address_from_port(&mut self, peer_id: PeerId, port: u16) {
        if let Some(ip) = self.peer_ip.get(&peer_id) {
            let address: Multiaddr = match ip {
                IpAddr::V4(ip) => format!("/ip4/{ip}/tcp/{port}"),
                IpAddr::V6(ip) => format!("/ip6/{ip}/tcp/{port}"),
            }
            .parse()
            .expect("Valid multiaddr");
            self.update_peer_addresses(peer_id, address);
        }
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
            .take(RANDOM_PEER_ADDRESSES_LENGTH)
            .map(|(k, v)| (k.clone(), v.clone()))
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
}
