use crate::app::{App, AppEvent};
use crate::args::AppArgs;
use crate::network::constants::DEFAULT_NETWORK_PORT;
use crate::session_auth::{generate_user_id, Password, SessionAuth};
use crate::store::{load_data, save_data, save_game_exists};
use crate::tui::{TerminalEvent, Tui};
use anyhow::anyhow;
use frittura_ssh_core::{
    spawn_event_converter, Credential, SshGame, SshSession, TerminalEvent as FtTerminalEvent,
};
use std::net::TcpListener;
use std::sync::Arc;
use std::time::Duration;

const MIN_USERNAME_LENGTH: usize = 3;
const MAX_USERNAME_LENGTH: usize = 16;
const CHANNEL_DISCONNECTION_TIME_IN_SECONDS: u64 = 120;
const MAX_LIBP2P_CLIENT_PORT: u16 = DEFAULT_NETWORK_PORT + 32;

fn libp2p_port_available(port: u16) -> bool {
    TcpListener::bind(("127.0.0.1", port)).is_ok()
}

fn get_available_libp2p_port() -> Option<u16> {
    (DEFAULT_NETWORK_PORT..MAX_LIBP2P_CLIENT_PORT).find(|p| libp2p_port_available(*p))
}

pub struct RebelsGame;

impl RebelsGame {
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}

impl SshGame for RebelsGame {
    type Auth = SessionAuth;
    const SCREEN_SIZE: (u16, u16) = crate::ui::UI_SCREEN_SIZE;
    const TITLE: &'static str = "Rebels in the sky";
    const SERVER_INACTIVITY: Duration = Duration::from_secs(3600);

    async fn authenticate(
        &self,
        username: &str,
        credential: Credential,
    ) -> anyhow::Result<SessionAuth> {
        let cred_str = match credential {
            Credential::Password(p) => p,
            Credential::PublicKey(pk) => pk.to_string(),
        };

        let username = if save_game_exists(username) {
            username.to_string()
        } else if username.is_empty() {
            generate_user_id()
        } else if username.len() < MIN_USERNAME_LENGTH || username.len() > MAX_USERNAME_LENGTH {
            return Err(anyhow!(
                "invalid username (must be {MIN_USERNAME_LENGTH}-{MAX_USERNAME_LENGTH} chars)"
            ));
        } else {
            username.to_string()
        };

        let session_auth = SessionAuth::new(username.clone(), cred_str);
        let filename = format!("{username}.sshpwd");

        if save_game_exists(&username) {
            match load_data(&filename) {
                Ok(persisted) => {
                    let saved: Password = persisted
                        .try_into()
                        .map_err(|_| anyhow!("malformed sshpwd for {username}"))?;
                    if !session_auth.check_password(saved) {
                        return Err(anyhow!("invalid credential for {username}"));
                    }
                }
                Err(_) => {
                    save_data(&filename, &session_auth.hashed_password)?;
                }
            }
        } else {
            save_data(&filename, &session_auth.hashed_password)?;
        }

        Ok(session_auth)
    }

    async fn on_session(self: Arc<Self>, session: SshSession<SessionAuth>) {
        let SshSession {
            auth,
            writer,
            data_rx,
            resize_rx,
            initial_size,
            ..
        } = session;
        let username = auth.username.clone();

        let libp2p_port = tokio::task::spawn_blocking(get_available_libp2p_port)
            .await
            .ok()
            .flatten();

        let mut app = match App::new(AppArgs::ssh_client(
            Some(username.clone()),
            libp2p_port,
            Some(CHANNEL_DISCONNECTION_TIME_IN_SECONDS),
        )) {
            Ok(a) => a,
            Err(e) => {
                log::error!("App init failed for {username}: {e}");
                return;
            }
        };

        let tui = match Tui::new_ssh(writer) {
            Ok(t) => t,
            Err(e) => {
                log::error!("Tui init failed for {username}: {e}");
                return;
            }
        };

        let app_event_sender = app.get_event_sender();

        let (init_w, init_h) = initial_size;
        let _ = app_event_sender
            .send(AppEvent::TerminalEvent(TerminalEvent::Resize(
                init_w.min(u16::MAX as u32) as u16,
                init_h.min(u16::MAX as u32) as u16,
            )))
            .await;

        let mut events = spawn_event_converter(data_rx, resize_rx, None, None);
        let sender = app_event_sender.clone();
        let forwarder = tokio::spawn(async move {
            while let Some(ev) = events.recv().await {
                let local = match ev {
                    FtTerminalEvent::Key(k) => TerminalEvent::Key(k),
                    FtTerminalEvent::Mouse(m) => TerminalEvent::Mouse(m),
                    FtTerminalEvent::Resize(w, h) => TerminalEvent::Resize(w, h),
                    FtTerminalEvent::Quit => TerminalEvent::Quit,
                    FtTerminalEvent::IdleWarning(_) => continue,
                };
                if sender.send(AppEvent::TerminalEvent(local)).await.is_err() {
                    break;
                }
            }
        });

        if let Err(e) = app.run(tui).await {
            log::error!("App run failed for {username}: {e}");
        }

        forwarder.abort();
    }
}
