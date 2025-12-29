use super::channel::AppChannel;
use crate::ssh::utils::{generate_user_id, Password, SessionAuth};
use crate::store::{load_data, save_data, save_game_exists};
use crate::types::AppResult;
use anyhow::anyhow;
use anyhow::Context;
use russh::{
    server::{self, *},
    ChannelId,
};
use russh::{Channel, Disconnect, Pty};
use std::collections::HashMap;
use tokio_util::sync::CancellationToken;

const MIN_USERNAME_LENGTH: usize = 3;
const MAX_USERNAME_LENGTH: usize = 16;

pub struct AppClient {
    network_port: Option<u16>,
    shutdown: CancellationToken,
    channels: HashMap<ChannelId, AppChannel>,
    session_auth: SessionAuth,
}

impl AppClient {
    pub fn new(network_port: Option<u16>, shutdown: CancellationToken) -> Self {
        AppClient {
            network_port,
            shutdown,
            channels: HashMap::new(),
            session_auth: SessionAuth::default(),
        }
    }

    fn channel_mut(&mut self, id: ChannelId) -> AppResult<&mut AppChannel> {
        self.channels
            .get_mut(&id)
            .with_context(|| format!("unknown channel: {id}"))
    }
}

impl server::Handler for AppClient {
    type Error = anyhow::Error;

    async fn auth_password(&mut self, user: &str, password: &str) -> Result<Auth, Self::Error> {
        println!("User {user} requested password authentication");
        let username = if !save_game_exists(user) && user.is_empty() {
            generate_user_id()
        } else {
            user.to_string()
        };

        // We defer checking username and password to channel_open_session so that it is possible
        // to send informative error messages to the user using session.write.
        self.session_auth = SessionAuth::new(username, password.to_string());

        Ok(Auth::Accept)
    }

    async fn auth_publickey(
        &mut self,
        user: &str,
        public_key: &russh::keys::PublicKey,
    ) -> Result<Auth, Self::Error> {
        println!("User {user} requested public key authentication");
        let username = if !save_game_exists(user) && user.is_empty() {
            generate_user_id()
        } else {
            user.to_string()
        };

        // We defer checking username and password to channel_open_session so that it is possible
        // to send informative error messages to the user using session.write.
        self.session_auth = SessionAuth::new(username, public_key.to_string());

        Ok(Auth::Accept)
    }

    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        session: &mut Session,
    ) -> AppResult<bool> {
        println!("User connected with {:?}", self.session_auth);

        // If a world exists in the store for the session_aut username, we check the password
        let store_prefix = &self.session_auth.username;
        let filename = format!("{store_prefix}.sshpwd");
        if save_game_exists(store_prefix) {
            println!("Found valid save file");
            // If the password exists, we check it
            if let Ok(persisted_password) = load_data(&filename) {
                let password: Password = persisted_password.try_into().unwrap_or_default();
                if !self.session_auth.check_password(password) {
                    let error_string = "\n\rWrong password.\n".to_string();
                    session.disconnect(Disconnect::ByApplication, error_string.as_str(), "")?;
                    session.close(channel.id())?;
                    return Ok(false);
                }
            } else {
                // Otherwise, we just accept the new password and we persist it.
                println!("Persisting sshpwd");
                if let Err(err) = save_data(&filename, &self.session_auth.hashed_password) {
                    let description = format!("\n\rError storing password: {err}\n");
                    session.disconnect(Disconnect::ByApplication, description.as_str(), "")?;
                    session.close(channel.id())?;
                    return Ok(false);
                }
            }
        }
        // Else, we check the username and persist the session auth
        else {
            if self.session_auth.username.len() < MIN_USERNAME_LENGTH
                || self.session_auth.username.len() > MAX_USERNAME_LENGTH
            {
                let error_string = format!(
                    "\n\rInvalid username. The username must have between {MIN_USERNAME_LENGTH} and {MAX_USERNAME_LENGTH} characters.\n"
                );
                session.disconnect(Disconnect::ByApplication, error_string.as_str(), "")?;
                session.close(channel.id())?;
                return Ok(false);
            }
            println!("No valid save file, starting from scratch.");

            println!("Persisting sshpwd");
            if let Err(e) = save_data(&filename, &self.session_auth.hashed_password) {
                session.disconnect(
                    Disconnect::ByApplication,
                    format!("\n\rError storing password {e}.\n").as_str(),
                    "",
                )?;
                session.close(channel.id())?;
                return Ok(false);
            }
        }

        self.session_auth.update_last_active_time();

        let app_channel = AppChannel::new(
            self.shutdown.clone(),
            self.network_port,
            self.session_auth.username.clone(),
        );

        let created = self.channels.insert(channel.id(), app_channel).is_none();

        if created {
            Ok(true)
        } else {
            Err(anyhow!(
                "channel `{}` has been already opened",
                channel.id()
            ))
        }
    }

    async fn channel_close(&mut self, channel: ChannelId, _: &mut Session) -> AppResult<()> {
        if self.channels.remove(&channel).is_some() {
            Ok(())
        } else {
            Err(anyhow!("channel `{channel}` has been already closed"))
        }
    }

    async fn data(&mut self, id: ChannelId, data: &[u8], _: &mut Session) -> AppResult<()> {
        self.channel_mut(id)?.data(data).await?;

        Ok(())
    }

    async fn pty_request(
        &mut self,
        id: ChannelId,
        _: &str,
        width: u32,
        height: u32,
        _: u32,
        _: u32,
        _: &[(Pty, u32)],
        session: &mut Session,
    ) -> AppResult<()> {
        self.channel_mut(id)?
            .pty_request(id, width, height, session)
            .await?;

        Ok(())
    }

    async fn window_change_request(
        &mut self,
        id: ChannelId,
        width: u32,
        height: u32,
        _: u32,
        _: u32,
        _: &mut Session,
    ) -> AppResult<()> {
        self.channel_mut(id)?
            .window_change_request(width, height)
            .await?;

        Ok(())
    }
}
