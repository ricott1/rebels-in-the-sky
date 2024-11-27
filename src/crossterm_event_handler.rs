use crate::tui::{EventHandler, TerminalEvent};
use crate::types::{SystemTimeTick, Tick};
use crate::world::constants::MILLISECONDS;
use crossterm::event::{self, Event as CrosstermEvent, KeyEventKind};
use log::error;
use std::time::Duration;
use tokio::sync::mpsc;

/// Terminal event handler.
#[allow(dead_code)]
#[derive(Debug)]
pub struct CrosstermEventHandler {
    sender: mpsc::Sender<TerminalEvent>,
    receiver: mpsc::Receiver<TerminalEvent>,
    handler: tokio::task::JoinHandle<()>,
    fps: u8,
}

impl EventHandler for CrosstermEventHandler {
    async fn next(&mut self) -> TerminalEvent {
        self.receiver.recv().await.expect("Channel should be open")
    }

    fn simulation_update_interval(&self) -> Tick {
        250 * MILLISECONDS
    }
}

impl CrosstermEventHandler {
    const DEFAULT_FPS: u8 = 30;

    pub fn new(fps: Option<u8>) -> Self {
        let fps = fps.unwrap_or(CrosstermEventHandler::DEFAULT_FPS);
        let time_step_millis: Tick = (1000.0 / fps as f32) as Tick;
        let time_step: Duration = Duration::from_millis(time_step_millis as u64);
        let (sender, receiver) = mpsc::channel(1);
        let handler = {
            let sender = sender.clone();
            let mut last_tick = Tick::now();
            tokio::task::spawn(async move {
                loop {
                    if event::poll(time_step).expect("no events available") {
                        let result = match event::read().expect("unable to read event") {
                            CrosstermEvent::Key(key) => {
                                if key.kind == KeyEventKind::Press {
                                    sender.send(TerminalEvent::Key(key)).await
                                } else {
                                    Ok(())
                                }
                            }
                            CrosstermEvent::Mouse(e) => sender.send(TerminalEvent::Mouse(e)).await,
                            CrosstermEvent::Resize(w, h) => {
                                sender.send(TerminalEvent::Resize(w, h)).await
                            }
                            _ => {
                                log::info!("Crossterm event not implemented");
                                Ok(())
                            }
                        };

                        if let Err(e) = result {
                            error!("Failed to send terminal event: {e}");
                            break;
                        }
                    }

                    let now = Tick::now();
                    if now - last_tick >= time_step_millis {
                        if let Err(e) = sender.send(TerminalEvent::Tick { tick: now }).await {
                            error!("Failed to send tick event: {e}");
                            break;
                        }
                        last_tick = now;
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
