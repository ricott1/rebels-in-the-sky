use std::collections::HashMap;

use crate::network::constants::{DEFAULT_SEED_PORT, TOPIC};
use crate::network::types::{NetworkData, PlayerRanking, TeamRanking};
use crate::network::{handler::NetworkHandler, types::SeedInfo};
use crate::store::{
    deserialize, load_player_ranking, load_team_ranking, save_player_ranking, save_team_ranking,
};
use crate::types::{AppResult, PlayerId, SystemTimeTick, TeamId, Tick};
use crate::world::constants::*;
use futures::StreamExt;
use itertools::Itertools;
use libp2p::gossipsub::IdentTopic;
use libp2p::{gossipsub, swarm::SwarmEvent};
use tokio::select;

const SEED_INFO_INTERVAL_MILLIS: Tick = 60 * SECONDS;
const TOP_PLAYER_RANKING_LENGTH: usize = 20;
const TOP_TEAM_RANKING_LENGTH: usize = 10;

pub struct Relayer {
    pub running: bool,
    network_handler: NetworkHandler,
    last_seed_info_tick: Tick,
    team_ranking: HashMap<TeamId, TeamRanking>,
    top_team_ranking: Vec<(TeamId, TeamRanking)>,
    player_ranking: HashMap<PlayerId, PlayerRanking>,
    top_player_ranking: Vec<(PlayerId, PlayerRanking)>,
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
            .map(|(id, ranking)| (id.clone(), ranking.clone()))
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
            .map(|(id, ranking)| (id.clone(), ranking.clone()))
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
            .map(|(id, ranking)| (id.clone(), ranking.clone()))
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
            .map(|(id, ranking)| (id.clone(), ranking.clone()))
            .collect();

        Self {
            running: true,
            network_handler: NetworkHandler::new(None, DEFAULT_SEED_PORT)
                .expect("Failed to initialize network handler"),
            last_seed_info_tick: Tick::now(),
            team_ranking,
            top_team_ranking,
            player_ranking,
            top_player_ranking,
        }
    }

    pub async fn run(&mut self) -> AppResult<()> {
        println!("Starting relayer. Press Ctrl-C to exit.");
        while self.running {
            select! {
                    swarm_event = self.network_handler.swarm.select_next_some() =>  {
                        let result = self.handle_network_events(swarm_event);
                        if result.is_err() {
                            log::error!("Error handling network event: {:?}", result);
                        }
                }
            }

            let now = Tick::now();
            if now - self.last_seed_info_tick > SEED_INFO_INTERVAL_MILLIS {
                self.network_handler.send_seed_info(SeedInfo::new(
                    self.network_handler.swarm.connected_peers().count(),
                    None,
                    self.top_team_ranking.clone(),
                    self.top_player_ranking.clone(),
                )?)?;
                self.last_seed_info_tick = now;
            }
        }
        Ok(())
    }

    pub fn handle_network_events(
        &mut self,
        network_event: SwarmEvent<gossipsub::Event>,
    ) -> AppResult<()> {
        println!("Received network event: {:?}", network_event);
        match network_event {
            SwarmEvent::Behaviour(gossipsub::Event::Subscribed { peer_id, topic }) => {
                if topic == IdentTopic::new(TOPIC).hash() {
                    println!("Sending info to {}", peer_id);

                    self.network_handler.send_seed_info(SeedInfo::new(
                        self.network_handler.swarm.connected_peers().count(),
                        None,
                        self.top_team_ranking.clone(),
                        self.top_player_ranking.clone(),
                    )?)?;
                }
            }

            SwarmEvent::Behaviour(gossipsub::Event::Message { message, .. }) => {
                assert!(message.topic == IdentTopic::new(TOPIC).hash());
                let network_data = deserialize::<NetworkData>(&message.data)?;
                match network_data {
                    NetworkData::Team(timestamp, network_team) => {
                        if let Some(current_ranking) = self.team_ranking.get(&network_team.team.id)
                        {
                            if current_ranking.timestamp >= timestamp {
                                return Ok(());
                            }
                        }

                        let ranking = TeamRanking::from_network_team(timestamp, &network_team);

                        // If the team is already stored, remove players from previous version.
                        // This is to ensure that fired players are removed.
                        if let Some(current_ranking) = self.team_ranking.get(&network_team.team.id)
                        {
                            for player_id in current_ranking.team.player_ids.iter() {
                                self.player_ranking.remove(player_id);
                            }
                        }

                        self.team_ranking
                            .insert(network_team.team.id, ranking.clone());

                        if let Err(err) = save_team_ranking(&self.team_ranking, true) {
                            println!("Error while saving team ranking: {err}");
                        }

                        for player in network_team.players.iter() {
                            let ranking = PlayerRanking::new(timestamp, player.clone());
                            self.player_ranking.insert(player.id, ranking.clone());
                        }

                        if let Err(err) = save_player_ranking(&self.player_ranking, true) {
                            println!("Error while saving player ranking: {err}");
                        }

                        self.top_team_ranking = self.get_top_team_ranking();
                        self.top_player_ranking = self.get_top_player_ranking();
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        Ok(())
    }
}
