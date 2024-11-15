use std::collections::HashMap;

use crate::network::constants::{DEFAULT_SEED_PORT, TOPIC};
use crate::network::types::{NetworkData, TeamRanking};
use crate::network::{handler::NetworkHandler, types::SeedInfo};
use crate::store::{deserialize, load_team_ranking, save_team_ranking};
use crate::types::{AppResult, SystemTimeTick, TeamId, Tick};
use crate::world::constants::*;
use futures::StreamExt;
use itertools::Itertools;
use libp2p::gossipsub::IdentTopic;
use libp2p::{gossipsub, swarm::SwarmEvent};
use tokio::select;

const SEED_INFO_INTERVAL_MILLIS: Tick = 60 * SECONDS;

pub struct Relayer {
    pub running: bool,
    network_handler: NetworkHandler,
    last_seed_info_tick: Tick,
    team_ranking: HashMap<TeamId, TeamRanking>,
    top_team_ranking: Vec<(TeamId, TeamRanking)>,
}

impl Relayer {
    fn update_top_team_ranking(
        top_team_ranking: &mut Vec<(TeamId, TeamRanking)>,
        team_id: TeamId,
        team_ranking: TeamRanking,
    ) {
        let mut insertion_index = 0;
        for (_, top_ranking) in top_team_ranking.iter() {
            if team_ranking.reputation > top_ranking.reputation {
                break;
            }
            insertion_index += 1;
        }
        top_team_ranking.insert(insertion_index, (team_id, team_ranking));

        let mut unique_team_ids = vec![];
        let mut to_remove = vec![];

        for (index, (id, _)) in top_team_ranking.iter().enumerate() {
            if unique_team_ids.contains(id) {
                to_remove.push(index);
            } else {
                unique_team_ids.push(*id);
            }
        }

        for index in to_remove {
            top_team_ranking.remove(index);
        }

        if top_team_ranking.len() > 10 {
            top_team_ranking.pop();
        }
    }

    pub fn new() -> Self {
        let team_ranking = match load_team_ranking() {
            Ok(team_ranking) => team_ranking,
            Err(err) => {
                println!("Error while loading team ranking: {err}");
                HashMap::new()
            }
        };

        let top_team_ranking = team_ranking
            .iter()
            .sorted_by(|(_, a), (_, b)| {
                b.reputation
                    .partial_cmp(&a.reputation)
                    .expect("Reputation should exist")
            })
            .take(10)
            .map(|(id, ranking)| (id.clone(), ranking.clone()))
            .collect();

        Self {
            running: true,
            network_handler: NetworkHandler::new(None, DEFAULT_SEED_PORT)
                .expect("Failed to initialize network handler"),
            last_seed_info_tick: Tick::now(),
            team_ranking,
            top_team_ranking,
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
                    )?)?;
                }
            }

            SwarmEvent::Behaviour(gossipsub::Event::Message { message, .. }) => {
                assert!(message.topic == IdentTopic::new(TOPIC).hash());
                let network_data = deserialize::<NetworkData>(&message.data)?;
                match network_data {
                    NetworkData::Team(timestamp, network_team) => {
                        let team_ranking = TeamRanking::from_network_team(timestamp, &network_team);
                        self.team_ranking
                            .insert(network_team.team.id, team_ranking.clone());
                        Self::update_top_team_ranking(
                            &mut self.top_team_ranking,
                            network_team.team.id,
                            team_ranking,
                        );

                        // self.top_team_ranking = self
                        //     .team_ranking
                        //     .iter()
                        //     .sorted_by(|(_, a), (_, b)| {
                        //         b.reputation
                        //             .partial_cmp(&a.reputation)
                        //             .expect("Reputation should exist")
                        //     })
                        //     .take(10)
                        //     .map(|(id, ranking)| (id.clone(), ranking.clone()))
                        //     .collect();

                        if let Err(err) = save_team_ranking(&self.team_ranking, true) {
                            println!("Error while saving team ranking: {err}");
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        network::types::{NetworkTeam, TeamRanking},
        relayer::Relayer,
        types::{PlanetId, TeamId},
        world::team::Team,
    };
    use std::collections::HashMap;

    #[test]
    fn test_top_team_ranking() {
        let mut team_ranking = HashMap::new();
        let mut top_team_ranking: Vec<(TeamId, TeamRanking)> = vec![];

        for idx in 0..20 {
            let mut team = Team::random(TeamId::new_v4(), PlanetId::new_v4(), "name", "ship_name");
            team.reputation = idx as f32;
            let network_team = NetworkTeam::new(team, vec![], vec![]);
            let new_team_ranking = TeamRanking::from_network_team(0, &network_team);

            team_ranking.insert(network_team.team.id, new_team_ranking.clone());

            Relayer::update_top_team_ranking(
                &mut top_team_ranking,
                network_team.team.id,
                new_team_ranking,
            );
        }

        for idx in 0..top_team_ranking.len() - 1 {
            let (_, ranking) = &top_team_ranking[idx];
            let (_, next_ranking) = &top_team_ranking[idx + 1];
            assert!(ranking.reputation >= next_ranking.reputation)
        }
    }
}
