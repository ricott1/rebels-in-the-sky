use super::SSHEventHandler;
use crate::app::App;
use crate::tui::{Tui, WriterProxy};
use crate::types::AppResult;
use anyhow::{anyhow, Result};
use russh::server::{Handle, Session};
use russh::{ChannelId, CryptoVec};
use std::fmt::Debug;
use tokio::sync::mpsc;
use tokio::task;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct SSHWriterProxy {
    flushing: bool,
    channel_id: ChannelId,
    handle: Handle,
    // The sink collects the data which is finally flushed to the handle.
    sink: Vec<u8>,
}

impl Debug for SSHWriterProxy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SSHWriterProxy")
            .field("flushing", &self.flushing)
            .field("channel_id", &self.channel_id)
            .field("sink", &self.sink)
            .finish()
    }
}

impl SSHWriterProxy {
    pub fn new(channel_id: ChannelId, handle: Handle) -> Self {
        Self {
            flushing: false,
            channel_id,
            handle,
            sink: vec![],
        }
    }
}

impl WriterProxy for SSHWriterProxy {
    async fn send(&mut self) -> std::io::Result<usize> {
        if !self.flushing {
            return Ok(0);
        }

        let data: CryptoVec = self.sink.clone().into();
        let data_length = self.sink.len();
        if let Err(e) = self.handle.data(self.channel_id, data).await {
            log::error!("Flushing error: {:#?}", e.to_ascii_lowercase());
            let _ = self.handle.close(self.channel_id).await;
        }
        self.sink.clear();
        self.flushing = false;
        Ok(data_length)
    }
}

// The crossterm backend writes to the terminal handle.
impl std::io::Write for SSHWriterProxy {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.sink.extend(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.flushing = true;
        Ok(())
    }
}

#[derive(Debug)]
pub struct AppChannel {
    state: AppChannelState,
    network_port: Option<u16>,
    username: String,
}

#[derive(Debug)]
enum AppChannelState {
    AwaitingPty { shutdown: CancellationToken },
    Ready { stdin: mpsc::Sender<Vec<u8>> },
}

impl AppChannel {
    pub fn new(shutdown: CancellationToken, network_port: Option<u16>, username: String) -> Self {
        let state = AppChannelState::AwaitingPty { shutdown };

        println!("New AppChannel created for {}", username);

        Self {
            state,
            network_port,
            username,
        }
    }

    pub async fn data(&mut self, data: &[u8]) -> Result<()> {
        let AppChannelState::Ready { stdin } = &mut self.state else {
            return Err(anyhow!("pty hasn't been allocated yet"));
        };

        stdin
            .send(data.to_vec())
            .await
            .map_err(|_| anyhow!("lost ui"))?;

        Ok(())
    }

    pub async fn pty_request(
        &mut self,
        id: ChannelId,
        _width: u32,
        _height: u32,
        session: &mut Session,
    ) -> AppResult<()> {
        // FIXME: this is the server shitdown token, we should use it to stop the app (which stops everything else).
        let AppChannelState::AwaitingPty { shutdown } = &mut self.state else {
            return Err(anyhow!("pty has been already allocated"));
        };

        let (stdin_tx, stdin_rx) = mpsc::channel(1);
        let app_shutdown = CancellationToken::new();
        let events = SSHEventHandler::new(stdin_rx, app_shutdown.clone(), shutdown.clone());
        let writer = SSHWriterProxy::new(id, session.handle());
        let tui = Tui::new_ssh(writer, events)?;

        let username = self.username.clone();
        let network_port = self.network_port.clone();
        self.state = AppChannelState::Ready { stdin: stdin_tx };

        // Main loop to run the update, including updating and drawing.
        task::spawn(async move {
            let store_prefix = Some(username.as_str());
            if let Err(e) = App::new(
                None,
                false,
                true,
                true,
                false,
                None,
                network_port,
                store_prefix,
            )
            .run(tui)
            .await
            {
                log::error!("Error running app: {e}")
            };
            // App has closed.
            app_shutdown.cancel();
        });

        Ok(())
    }

    pub async fn window_change_request(&mut self, width: u32, height: u32) -> Result<()> {
        let AppChannelState::Ready { stdin } = &mut self.state else {
            return Err(anyhow!("pty hasn't been allocated yet"));
        };

        let width = width.min(255);
        let height = height.min(255);

        stdin
            .send(vec![SSHEventHandler::CMD_RESIZE, width as u8, height as u8])
            .await
            .map_err(|_| anyhow!("lost ui"))?;

        Ok(())
    }
}
