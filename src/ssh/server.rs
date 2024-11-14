use super::client::AppClient;
use crate::network::constants::DEFAULT_PORT;
use crate::types::AppResult;
use itertools::Either;
use russh::server::{Config, Server};
use std::fs::File;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::pin::pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::{select, time};
use tokio_util::sync::CancellationToken;

const SERVER_SSH_PORT: u16 = 3788;
const MAX_SSH_CLIENT_PORT: u16 = DEFAULT_PORT + 32;

fn save_keys(signing_key: &ed25519_dalek::SigningKey) -> AppResult<()> {
    let file = File::create::<&str>("./keys".into())?;
    assert!(file.metadata()?.is_file());
    let mut buffer = std::io::BufWriter::new(file);
    buffer.write(&signing_key.to_bytes())?;
    Ok(())
}

fn load_keys() -> AppResult<ed25519_dalek::SigningKey> {
    let file = File::open::<&str>("./keys".into())?;
    let mut buffer = std::io::BufReader::new(file);
    let mut buf: [u8; 32] = [0; 32];
    buffer.read(&mut buf)?;
    Ok(ed25519_dalek::SigningKey::from_bytes(&buf))
}

fn get_available_port() -> Option<u16> {
    (DEFAULT_PORT..MAX_SSH_CLIENT_PORT).find(|port| port_is_available(*port))
}

fn port_is_available(port: u16) -> bool {
    match TcpListener::bind(("127.0.0.1", port)) {
        Ok(_) => true,
        Err(_) => false,
    }
}

#[derive(Clone, Default)]
pub struct AppServer {
    shutdown: CancellationToken,
}

impl AppServer {
    pub fn new() -> Self {
        Self {
            shutdown: CancellationToken::new(),
        }
    }

    pub async fn run(&mut self) -> AppResult<()> {
        println!("Starting SSH server. Press Ctrl-C to exit.");

        let signing_key = load_keys().unwrap_or_else(|_| {
            let key_pair = russh_keys::key::KeyPair::generate_ed25519();
            let signing_key = match key_pair {
                russh_keys::key::KeyPair::Ed25519(signing_key) => signing_key,
                _ => panic!("SSH server: Invalid KeyPair"),
            };
            save_keys(&signing_key).expect("Failed to save SSH keys.");
            signing_key
        });
        let key_pair = russh::keys::key::KeyPair::Ed25519(signing_key);

        let config = Config {
            inactivity_timeout: Some(std::time::Duration::from_secs(3600)),
            auth_rejection_time: std::time::Duration::from_secs(3),
            auth_rejection_time_initial: Some(std::time::Duration::from_secs(0)),
            keys: vec![key_pair],
            ..Default::default()
        };

        let shutdown = self.shutdown.clone();
        let server = self.run_on_address(Arc::new(config), ("0.0.0.0", SERVER_SSH_PORT));

        let shutdown = shutdown.cancelled();
        let result = {
            let mut server = pin!(server);
            let mut shutdown = pin!(shutdown);

            select! {
                result = &mut server => Either::Left(result),
                _ = &mut shutdown => Either::Right(()),
            }
        };

        match result {
            Either::Left(result) => Ok(result?),
            Either::Right(_) => {
                println!("Shutting down");

                // TODO wait for clients to disconnect
                time::sleep(Duration::from_secs(1)).await;

                Ok(())
            }
        }
    }
}

impl Server for AppServer {
    type Handler = AppClient;
    fn new_client(&mut self, _: Option<std::net::SocketAddr>) -> AppClient {
        let network_port = if let Some(available_port) = get_available_port() {
            Some(available_port)
        } else {
            None
        };
        AppClient::new(network_port, self.shutdown.clone())
    }
}
