use crate::network::handler::NetworkHandler;
use crate::types::AppResult;
use futures::StreamExt;
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
            network_handler: NetworkHandler::new(None)
                .expect("Failed to initialize network handler"),
        }
    }

    pub async fn run(&mut self) -> AppResult<()> {
        println!("Starting relayer. Press Ctrl-C to exit.");
        while self.running {
            select! {
                swarm_event = self.network_handler.swarm.select_next_some() =>  self.handle_network_events(swarm_event)?,
            }
        }
        Ok(())
    }

    pub fn handle_network_events(
        &mut self,
        network_event: SwarmEvent<gossipsub::Event, Void>,
    ) -> AppResult<()> {
        println!("Received network event: {:?}", network_event);
        Ok(())
    }
}
