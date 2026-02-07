use crate::app::{App, AppEvent};
use crate::args::AppArgs;
use crate::ssh::utils::{convert_data_to_app_event, CMD_RESIZE};
use crate::tui::{Tui, WriterProxy};
use crate::types::AppResult;
use anyhow::{anyhow, Result};
use russh::server::{Handle, Session};
use russh::{ChannelId, CryptoVec};
use std::fmt::Debug;
use tokio::sync::mpsc;
use tokio::task;
use tokio_util::sync::CancellationToken;

const CHANNEL_DISCONNECTION_TIME_IN_SECONDS: u64 = 120; // Auto-disconnect after 2 minutes with no input.

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
    AwaitingPty {
        _server_shutdown: CancellationToken,
    },
    Ready {
        app_event_sender: mpsc::Sender<AppEvent>,
    },
}

impl AppChannel {
    pub fn new(
        _server_shutdown: CancellationToken,
        network_port: Option<u16>,
        username: String,
    ) -> Self {
        let state = AppChannelState::AwaitingPty { _server_shutdown };

        println!("New AppChannel created for {username}");

        Self {
            state,
            network_port,
            username,
        }
    }

    pub async fn data(&mut self, data: &[u8]) -> Result<()> {
        let AppChannelState::Ready { app_event_sender } = &mut self.state else {
            return Err(anyhow!("pty hasn't been allocated yet"));
        };

        if let Some(app_event) = convert_data_to_app_event(data) {
            app_event_sender
                .send(app_event)
                .await
                .map_err(|_| anyhow!("lost ssh connection"))?;
        }

        Ok(())
    }

    pub async fn pty_request(
        &mut self,
        id: ChannelId,
        width: u32,
        height: u32,
        session: &mut Session,
    ) -> AppResult<()> {
        // FIXME: this is the server shutdown token, we should use it to stop the app (which stops everything else).
        let AppChannelState::AwaitingPty { .. } = &mut self.state else {
            return Err(anyhow!("pty has been already allocated"));
        };

        let handle = session.handle();
        let channel_id = id;
        let writer = SSHWriterProxy::new(id, handle.clone());

        let username = self.username.clone();
        let network_port = self.network_port;

        let store_prefix = Some(username);
        let mut app = App::new(AppArgs::ssh_client(
            store_prefix,
            network_port,
            Some(CHANNEL_DISCONNECTION_TIME_IN_SECONDS),
        ))?;

        let tui = Tui::new_ssh(writer)?;

        self.state = AppChannelState::Ready {
            app_event_sender: app.get_event_sender(),
        };

        task::spawn(async move {
            if let Err(e) = app.run(tui).await {
                log::error!("Error running app: {e}")
            };

            // Send EOF and close the channel so the client returns to its prompt.
            let _ = handle.eof(channel_id).await;
            let _ = handle.close(channel_id).await;
        });

        self.window_change_request(width, height).await?;

        Ok(())
    }

    pub async fn window_change_request(&mut self, width: u32, height: u32) -> Result<()> {
        let AppChannelState::Ready { app_event_sender } = &mut self.state else {
            return Err(anyhow!("pty hasn't been allocated yet"));
        };

        let width = width.min(255);
        let height = height.min(255);

        let data = vec![CMD_RESIZE, width as u8, height as u8];
        if let Some(app_event) = convert_data_to_app_event(&data) {
            app_event_sender
                .send(app_event)
                .await
                .map_err(|_| anyhow!("lost ssh connection"))?;
        }

        Ok(())
    }
}
