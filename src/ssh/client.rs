use crate::app::App;
use crate::types::AppResult;
use russh::{server::*, ChannelId, CryptoVec, Disconnect};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::time::SystemTime;

use super::backend::SSHBackend;
use super::event::TickEventHandler;
use super::tui::SSHTui;

#[derive(Clone)]
pub struct TerminalHandle {
    handle: Handle,
    // The sink collects the data which is finally flushed to the handle.
    sink: Vec<u8>,
    channel_id: ChannelId,
}

impl TerminalHandle {
    pub async fn close(&self) -> AppResult<()> {
        self.handle
            .close(self.channel_id)
            .await
            .map_err(|_| anyhow::anyhow!("Close terminal error"))?;
        self.handle
            .disconnect(Disconnect::ByApplication, "Game quit".into(), "".into())
            .await?;
        Ok(())
    }

    async fn _flush(&self) -> std::io::Result<usize> {
        let handle = self.handle.clone();
        let channel_id = self.channel_id.clone();
        let data: CryptoVec = self.sink.clone().into();
        let data_length = data.len();
        let result = handle.data(channel_id, data).await;
        if result.is_err() {
            log::error!("Failed to send data: {:?}", result);
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Failed to send data",
            ));
        }
        log::debug!(
            "Sent {} bytes of data to channel {}",
            data_length,
            channel_id
        );
        Ok(data_length)
    }
}

// The crossterm backend writes to the terminal handle.
impl std::io::Write for TerminalHandle {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.sink.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        futures::executor::block_on(self._flush())?;
        self.sink.clear();
        Ok(())
    }
}

pub struct AppClient {
    pub tui: SSHTui,
    pub app: App,
}

impl AppClient {
    pub fn new(
        session: &Session,
        channel_id: ChannelId,
        network_port: Option<u16>,
        username: &str,
    ) -> AppResult<Self> {
        let terminal_handle = TerminalHandle {
            handle: session.handle(),
            sink: Vec::new(),
            channel_id,
        };

        let backend = SSHBackend::new(terminal_handle, (160, 48));
        let events = TickEventHandler::handler();
        let mut tui = SSHTui::new(backend, events).map_err(|e| {
            log::error!("Failed to create terminal interface: {}", e);
            anyhow::anyhow!("Failed to create terminal interface: {}", e)
        })?;
        tui.terminal.clear().map_err(|e| {
            log::error!("Failed to clear terminal: {}", e);
            anyhow::anyhow!("Failed to clear terminal: {}", e)
        })?;

        let app = App::new(
            None,
            false,
            true,
            true,
            false,
            None,
            network_port,
            Some(username),
        );
        Ok(AppClient { tui, app })
    }
}

pub type Password = [u8; 32];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionAuth {
    pub username: String,
    pub hashed_password: Password,
    pub last_active_time: SystemTime,
}

impl Default for SessionAuth {
    fn default() -> Self {
        Self {
            username: "".to_string(),
            hashed_password: [0; 32],
            last_active_time: SystemTime::now(),
        }
    }
}

impl SessionAuth {
    pub fn new(username: String, hashed_password: Password) -> Self {
        Self {
            username,
            hashed_password,
            last_active_time: SystemTime::now(),
        }
    }

    pub fn update_last_active_time(&mut self) {
        self.last_active_time = SystemTime::now();
    }

    pub fn check_password(&self, password: Password) -> bool {
        self.hashed_password == password
    }
}
