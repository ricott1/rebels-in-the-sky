use crate::app::App;
use crate::event::TerminalEvent;
use crate::network::constants::DEFAULT_PORT;
use crate::store::world_exists;
use crate::types::{AppResult, SystemTimeTick, Tick, SECONDS};
use anyhow::anyhow;
use async_trait::async_trait;
use crossterm::event::KeyModifiers;
use rand::Rng;
use rand_distr::Alphanumeric;
use russh::keys::key::PublicKey;
use russh::{server::*, Channel, ChannelId, Disconnect, Pty};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use std::sync::Arc;
use tokio::select;
use tokio::sync::Mutex;

use super::client::{AppClient, SessionAuth};

const SERVER_SSH_PORT: u16 = 3788;
const MAX_SSH_CLIENT_PORT: u16 = DEFAULT_PORT + 32;

static AUTH_PASSWORD_SALT: &'static str = "agfg34g";
static AUTH_PUBLIC_KEY_SALT: &'static str = "1gfg22g";

const MIN_USERNAME_LENGTH: usize = 3;
const MAX_USERNAME_LENGTH: usize = 16;

const NETWORK_HANDLER_INIT_INTERVAL: u128 = 1 * SECONDS;

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

#[derive(Clone, Default)]
pub struct AppServer {
    clients: Arc<Mutex<HashMap<String, AppClient>>>,
    port: u16,
    session_auth: SessionAuth,
}

impl AppServer {
    fn generate_user_id() -> String {
        let buf_id = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(8)
            .collect::<Vec<u8>>()
            .to_ascii_lowercase();
        std::str::from_utf8(buf_id.as_slice())
            .expect("Failed to generate user id string")
            .to_string()
    }

    pub fn new() -> Self {
        Self {
            port: DEFAULT_PORT,
            ..Default::default()
        }
    }

    pub async fn run(&mut self) -> AppResult<()> {
        println!("Starting SSH server. Press Ctrl-C to exit.");
        let clients = self.clients.clone();
        tokio::spawn(async move {
            let mut last_network_handler_init = 0;
            loop {
                tokio::time::sleep(tokio::time::Duration::from_millis(25)).await;
                // let tick = Tick::now();

                let mut to_remove = Vec::new();
                let mut clients = clients.lock().await;

                for (username, app_client) in clients.iter_mut() {
                    if !app_client.app.running {
                        app_client.tui.exit().await.unwrap_or_else(|e| {
                            log::error!("Failed to exit terminal: {}", e);
                        });
                        to_remove.push(username.clone());
                        continue;
                    }

                    if app_client.app.network_handler.is_none()
                        && app_client.app.world.has_own_team()
                        && Tick::now() - last_network_handler_init > NETWORK_HANDLER_INIT_INTERVAL
                    {
                        println!("Initializing network handler for '{}'...", username);
                        if let Err(e) = app_client.app.initialize_network_handler() {
                            println!("Could not initialize network handler: {}", e);
                            // setting last_network_handler_init delays setting the handler for other clients.
                            // A better solution would be to store this on a per client basis
                            last_network_handler_init = Tick::now();
                        } else {
                            println!("Done");
                        }
                    }

                    select! {
                        Some(swarm_event) = App::conditional_network_event(&mut app_client.app.network_handler) =>  app_client.app.handle_network_events(swarm_event).unwrap_or_else(
                            |e| println!("Failed to handle network event: {}", e)
                        ),
                        app_event = app_client.tui.events.next().unwrap() => match app_event{
                            TerminalEvent::Tick {tick} => {
                                app_client.app.handle_tick_events(tick).unwrap_or_else(|e| {
                                    log::error!("Failed to handle tick event for client: {}", e);
                                    to_remove.push(username.clone());
                                });
                                app_client.tui.draw(&mut app_client.app.ui, &app_client.app.world).unwrap_or_else(|e| {
                                    log::error!("Failed to draw tui for client: {}", e);
                                    to_remove.push(username.clone());
                                });
                            }
                            _ => panic!("Should not receive any TerminalEvent apart from Tick over SSH handler.")
                        }
                    }
                }

                for username in to_remove {
                    clients.remove(&username);
                }
            }
        });

        let signing_key = load_keys().unwrap_or_else(|_| {
            let key_pair =
                russh_keys::key::KeyPair::generate_ed25519().expect("Failed to generate key pair");
            let signing_key = match key_pair {
                russh_keys::key::KeyPair::Ed25519(signing_key) => signing_key,
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

        self.run_on_address(Arc::new(config), ("0.0.0.0", SERVER_SSH_PORT))
            .await?;
        Ok(())
    }
}

impl Server for AppServer {
    type Handler = Self;
    fn new_client(&mut self, _: Option<std::net::SocketAddr>) -> Self {
        let s = self.clone();
        self.port += 1;
        if self.port > MAX_SSH_CLIENT_PORT {
            self.port = DEFAULT_PORT; //FIXME: we are hoping that the port is now free. We should instead keep track of this
        }
        s
    }
}

#[async_trait]
impl Handler for AppServer {
    type Error = anyhow::Error;

    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        session: &mut Session,
    ) -> Result<bool, Self::Error> {
        println!("User connected with {:?}", self.session_auth);

        // If a world exists in the store for the session_aut username, we check the password
        if world_exists(&self.session_auth.username) {
            if self
                .session_auth
                .check_password(self.session_auth.hashed_password)
                == false
            {
                let error_string = format!("\n\rWrong password.\n");
                session.disconnect(Disconnect::ByApplication, error_string.as_str(), "");
                session.close(channel.id());
                return Ok(false);
            }
            println!("Found valid save file");
        }
        // Else, we check the username and persist it
        else {
            if self.session_auth.username.len() < MIN_USERNAME_LENGTH
                || self.session_auth.username.len() > MAX_USERNAME_LENGTH
            {
                let error_string = format!(
                    "\n\rInvalid username. The username must have between {} and {} characters.\n",
                    MIN_USERNAME_LENGTH, MAX_USERNAME_LENGTH
                );
                session.disconnect(Disconnect::ByApplication, error_string.as_str(), "");
                session.close(channel.id());
                return Ok(false);
            }
            println!("No valid save file, starting from scratch.");
        }

        self.session_auth.update_last_active_time();
        let mut clients = self.clients.lock().await;

        let app_client = AppClient::new(
            session,
            channel.id(),
            Some(self.port),
            self.session_auth.username.as_str(),
        )
        .map_err(|e| anyhow::anyhow!("Create AppClient error: {}", e))?;

        let username = self.session_auth.username.clone();
        clients.insert(username, app_client);

        Ok(true)
    }

    async fn auth_password(&mut self, user: &str, password: &str) -> Result<Auth, Self::Error> {
        println!("User {} requested password authentication", user);
        let username = if !world_exists(user) && user.len() == 0 {
            Self::generate_user_id()
        } else {
            user.to_string()
        };

        let mut hasher = Sha256::new();
        let salted_password = format!("{}{}", password, AUTH_PASSWORD_SALT);
        hasher.update(salted_password);
        let hashed_password = hasher.finalize().to_vec()[..].try_into()?;

        // We defer checking username and password to channel_open_session so that it is possible
        // to send informative error messages to the user using session.write.
        self.session_auth = SessionAuth::new(username, hashed_password);

        Ok(Auth::Accept)
    }

    async fn auth_publickey(
        &mut self,
        user: &str,
        public_key: &PublicKey,
    ) -> Result<Auth, Self::Error> {
        println!("User {} requested public key authentication", user);
        let username = if !world_exists(user) && user.len() == 0 {
            Self::generate_user_id()
        } else {
            user.to_string()
        };

        let mut hasher = Sha256::new();
        let salted_password = format!("{}{}", public_key.fingerprint(), AUTH_PUBLIC_KEY_SALT);
        hasher.update(salted_password);
        let hashed_password = hasher.finalize().to_vec()[..].try_into()?;

        // We defer checking username and password to channel_open_session so that it is possible
        // to send informative error messages to the user using session.write.
        self.session_auth = SessionAuth::new(username, hashed_password);

        Ok(Auth::Accept)
    }

    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        let mut clients = self.clients.lock().await;
        if let Some(app_client) = clients.get_mut(&self.session_auth.username) {
            let event = convert_data_to_crossterm_event(data);
            match event {
                Some(crossterm::event::Event::Mouse(mouse_event)) => {
                    app_client
                        .app
                        .handle_mouse_events(mouse_event)
                        .map_err(|e| anyhow::anyhow!("Error: {}", e))?;
                }
                Some(crossterm::event::Event::Key(key_event)) => match key_event.code {
                    crossterm::event::KeyCode::Esc => {
                        app_client
                            .app
                            .quit()
                            .unwrap_or_else(|e| println!("Error quitting app: {}", e));
                        app_client
                            .tui
                            .reset()
                            .unwrap_or_else(|e| println!("Error resetting tui: {}", e));
                    }
                    _ => {
                        app_client
                            .app
                            .handle_key_events(key_event)
                            .map_err(|e| anyhow::anyhow!("Error: {}", e))?;
                    }
                },
                _ => {}
            }
        } else {
            session.disconnect(Disconnect::ByApplication, "Game quit", "");
            session.close(channel);
        }

        Ok(())
    }

    async fn pty_request(
        &mut self,
        channel: ChannelId,
        _: &str,
        col_width: u32,
        row_height: u32,
        pix_width: u32,
        pix_height: u32,
        _: &[(Pty, u32)],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.window_change_request(
            channel, col_width, row_height, pix_width, pix_height, session,
        )
        .await
    }

    async fn window_change_request(
        &mut self,
        _: ChannelId,
        col_width: u32,
        row_height: u32,
        _: u32,
        _: u32,
        _: &mut Session,
    ) -> Result<(), Self::Error> {
        let mut clients = self.clients.lock().await;
        if let Some(client) = clients.get_mut(&self.session_auth.username) {
            client
                .tui
                .resize(col_width as u16, row_height as u16)
                .map_err(|e| anyhow::anyhow!("Resize error: {}", e))?;
        }
        Ok(())
    }
}

fn convert_data_to_key_event(data: &[u8]) -> Option<crossterm::event::KeyEvent> {
    let key = match data {
        b"\x1b\x5b\x41" => crossterm::event::KeyCode::Up,
        b"\x1b\x5b\x42" => crossterm::event::KeyCode::Down,
        b"\x1b\x5b\x43" => crossterm::event::KeyCode::Right,
        b"\x1b\x5b\x44" => crossterm::event::KeyCode::Left,
        b"\x03" | b"\x1b" => crossterm::event::KeyCode::Esc, // Ctrl-C is also sent as Esc
        b"\x0d" => crossterm::event::KeyCode::Enter,
        b"\x7f" => crossterm::event::KeyCode::Backspace,
        b"\x1b[3~" => crossterm::event::KeyCode::Delete,
        b"\x09" => crossterm::event::KeyCode::Tab,
        x if x.len() == 1 => crossterm::event::KeyCode::Char(data[0] as char),
        _ => {
            return None;
        }
    };
    let event = crossterm::event::KeyEvent::new(key, crossterm::event::KeyModifiers::empty());

    Some(event)
}

fn decode_sgr_mouse_input(ansi_code: Vec<u8>) -> AppResult<(u8, u16, u16)> {
    // Convert u8 vector to a String
    let ansi_str =
        String::from_utf8(ansi_code.clone()).map_err(|_| anyhow!("Invalid UTF-8 sequence"))?;

    // Check the prefix
    if !ansi_str.starts_with("\x1b[<") {
        return Err(anyhow!("Invalid SGR ANSI mouse code"));
    }

    let cb_mod = if ansi_str.ends_with('M') {
        0
    } else if ansi_str.ends_with('m') {
        3
    } else {
        return Err(anyhow!("Invalid SGR ANSI mouse code"));
    };

    // Remove the prefix '\x1b[<' and trailing 'M'
    let code_body = &ansi_str[3..ansi_str.len() - 1];

    // Split the components
    let components: Vec<&str> = code_body.split(';').collect();

    if components.len() != 3 {
        return Err(anyhow!("Invalid SGR ANSI mouse code format"));
    }

    // Parse the components
    let cb = cb_mod
        + components[0]
            .parse::<u8>()
            .map_err(|_| anyhow!("Failed to parse Cb"))?;
    let cx = components[1]
        .parse::<u16>()
        .map_err(|_| anyhow!("Failed to parse Cx"))?
        - 1;
    let cy = components[2]
        .parse::<u16>()
        .map_err(|_| anyhow!("Failed to parse Cy"))?
        - 1;

    Ok((cb, cx, cy))
}

fn convert_data_to_mouse_event(data: &[u8]) -> Option<crossterm::event::MouseEvent> {
    let (cb, column, row) = decode_sgr_mouse_input(data.to_vec()).ok()?;
    let kind = match cb {
        0 => crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
        1 => crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Middle),
        2 => crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Right),
        3 => crossterm::event::MouseEventKind::Up(crossterm::event::MouseButton::Left),
        32 => crossterm::event::MouseEventKind::Drag(crossterm::event::MouseButton::Left),
        33 => crossterm::event::MouseEventKind::Drag(crossterm::event::MouseButton::Middle),
        34 => crossterm::event::MouseEventKind::Drag(crossterm::event::MouseButton::Right),
        35 => crossterm::event::MouseEventKind::Moved,
        64 => crossterm::event::MouseEventKind::ScrollUp,
        65 => crossterm::event::MouseEventKind::ScrollDown,
        96..=255 => {
            return None;
        }
        _ => return None,
    };

    let event = crossterm::event::MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::empty(),
    };

    Some(event)
}

fn convert_data_to_crossterm_event(data: &[u8]) -> Option<crossterm::event::Event> {
    if data.starts_with(&[27, 91, 60]) {
        if let Some(event) = convert_data_to_mouse_event(data) {
            return Some(crossterm::event::Event::Mouse(event));
        }
    } else {
        if let Some(event) = convert_data_to_key_event(data) {
            return Some(crossterm::event::Event::Key(event));
        }
    }

    None
}
