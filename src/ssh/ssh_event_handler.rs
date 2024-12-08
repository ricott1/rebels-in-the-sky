use crate::ssh::utils::convert_data_to_crossterm_event;
use crate::tui::{EventHandler, TerminalEvent};
use crate::types::{SystemTimeTick, Tick};
use crate::world::constants::MILLISECONDS;
use crossterm::event::{Event as CrosstermEvent, KeyEventKind};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::{select, time};
use tokio_util::sync::CancellationToken;

/// Terminal event handler.
#[allow(dead_code)]
#[derive(Debug)]
pub struct SSHEventHandler {
    sender: mpsc::Sender<TerminalEvent>,
    receiver: mpsc::Receiver<TerminalEvent>,
    handler: tokio::task::JoinHandle<()>,
    fps: u8,
}

impl EventHandler for SSHEventHandler {
    async fn next(&mut self) -> TerminalEvent {
        self.receiver.recv().await.expect("Channel should be open")
    }

    fn simulation_update_interval(&self) -> Tick {
        500 * MILLISECONDS
    }
}

impl SSHEventHandler {
    pub const CMD_RESIZE: u8 = 0x04;
    const DEFAULT_FPS: u8 = 30;

    pub fn new(
        mut stdin: mpsc::Receiver<Vec<u8>>,
        app_shutdown: CancellationToken,
        server_shutdown: CancellationToken,
    ) -> Self {
        let fps = SSHEventHandler::DEFAULT_FPS;
        let time_step_millis: Tick = (1000.0 / fps as f32) as Tick;
        let time_step = Duration::from_millis(time_step_millis as u64);
        let mut ticker = time::interval(time_step);

        let (sender, receiver) = mpsc::channel(1);
        let handler = {
            let sender = sender.clone();
            tokio::task::spawn(async move {
                loop {
                    select! {
                        stdin = stdin.recv() => {
                            ticker.reset();
                            if let Some(event) = convert_data_to_crossterm_event(&stdin.expect("Data should be pushed")) {
                                match event {
                                    CrosstermEvent::Key(key) => {
                                        if key.kind == KeyEventKind::Press {
                                            sender.send(TerminalEvent::Key(key)).await
                                        }else{Ok(())}
                                    }
                                    CrosstermEvent::Mouse(e) =>  sender.send(TerminalEvent::Mouse(e)).await,
                                    CrosstermEvent::Resize(w, h) =>
                                        sender.send(TerminalEvent::Resize(w, h)).await,

                                    _ => unimplemented!()
                                }.expect("Cannot send over channel");
                            }
                        }
                        _ = app_shutdown.cancelled() => {
                                println!("Shutting down.");
                                break;
                        },

                        _ = server_shutdown.cancelled() => {
                            println!("Shutting down from server.");
                            app_shutdown.cancel();
                            sender.send(TerminalEvent::Quit).await.expect("Cannot send over channel");
                            break;
                    },
                        _ = ticker.tick() => {
                            let now = Tick::now();
                            if let Err(e) = sender.send(TerminalEvent::Tick { tick: now }).await {
                                log::error!("Failed to send tick event: {e}");
                                app_shutdown.cancel();
                                break;
                            }
                        }

                    }
                }
            })
        };
        Self {
            sender,
            receiver,
            handler,
            fps,
        }
    }
}
