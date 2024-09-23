use std::collections::HashMap;

use crate::network::constants::{DEFAULT_SEED_PORT, TOPIC};
use crate::network::types::{NetworkData, TeamRanking};
use crate::network::{handler::NetworkHandler, types::SeedInfo};
use crate::store::{load_team_ranking, save_team_ranking};
use crate::types::{AppResult, SystemTimeTick, TeamId, Tick};
use crate::world::constants::*;
use futures::StreamExt;
use libp2p::gossipsub::IdentTopic;
use libp2p::{gossipsub, swarm::SwarmEvent};
use tokio::select;
use void::Void;

const SEED_INFO_INTERVAL_MILLIS: Tick = 60 * SECONDS;

pub struct Relayer {
    pub running: bool,
    network_handler: NetworkHandler,
    last_seed_info_tick: Tick,
    team_ranking: HashMap<TeamId, TeamRanking>,
}

impl Relayer {
    pub fn new() -> Self {
        let team_ranking = match load_team_ranking() {
            Ok(team_ranking) => team_ranking,
            Err(err) => {
                println!("Error while loading team ranking: {err}");
                HashMap::new()
            }
        };
        Self {
            running: true,
            network_handler: NetworkHandler::new(None, Some(DEFAULT_SEED_PORT))
                .expect("Failed to initialize network handler"),
            last_seed_info_tick: Tick::now(),
            team_ranking,
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
                    self.team_ranking.clone(),
                )?)?;
                self.last_seed_info_tick = now;
            }
        }
        Ok(())
    }

    pub fn handle_network_events(
        &mut self,
        network_event: SwarmEvent<gossipsub::Event, Void>,
    ) -> AppResult<()> {
        println!("Received network event: {:?}", network_event);
        match network_event {
            SwarmEvent::Behaviour(gossipsub::Event::Subscribed { peer_id, topic }) => {
                if topic == IdentTopic::new(TOPIC).hash() {
                    println!("Sending info to {}", peer_id);
                    self.network_handler.send_seed_info(SeedInfo::new(
                        self.network_handler.swarm.connected_peers().count(),
                        None,
                        self.team_ranking.clone(),
                    )?)?;
                }
            }

            SwarmEvent::Behaviour(gossipsub::Event::Message { message, .. }) => {
                assert!(message.topic == IdentTopic::new(TOPIC).hash());
                let network_data = serde_json::from_slice::<NetworkData>(&message.data)?;
                match network_data {
                    NetworkData::Team(timestamp, network_team) => {
                        self.team_ranking.insert(
                            network_team.team.id,
                            TeamRanking::from_network_team(timestamp, &network_team),
                        );
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
