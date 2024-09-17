use crate::network::constants::{DEFAULT_SEED_PORT, TOPIC};
use crate::network::{handler::NetworkHandler, types::SeedInfo};
use crate::types::AppResult;
use futures::StreamExt;
use libp2p::gossipsub::IdentTopic;
use libp2p::{gossipsub, swarm::SwarmEvent};
use tokio::select;
use void::Void;

pub struct Relayer {
    pub running: bool,
    network_handler: NetworkHandler,
}

impl Relayer {
    pub fn new() -> Self {
        Self {
            running: true,
            network_handler: NetworkHandler::new(None, Some(DEFAULT_SEED_PORT))
                .expect("Failed to initialize network handler"),
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
                    )?)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}
