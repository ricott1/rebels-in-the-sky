use crate::tui::{EventHandler, TerminalEvent};
use crate::types::{SystemTimeTick, Tick};
use crate::world::constants::MILLISECONDS;
use crossterm::event::{self, Event as CrosstermEvent, KeyEventKind};
use std::time::{Duration, Instant};
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

    pub fn new(fps: Option<u8>, with_input_reader: bool) -> Self {
        let fps = fps.unwrap_or(CrosstermEventHandler::DEFAULT_FPS);
        let time_step: Duration = Duration::from_secs_f32(1.0 / fps as f32);
        let (sender, receiver) = mpsc::channel(100);
        let handler = {
            let sender = sender.clone();
            let mut last_tick = Instant::now();
            tokio::task::spawn(async move {
                loop {
                    if with_input_reader {
                        if event::poll(time_step).expect("no events available") {
                            let result = match event::read().expect("unable to read event") {
                                CrosstermEvent::Key(key) => {
                                    if key.kind == KeyEventKind::Press {
                                        sender.send(TerminalEvent::Key(key)).await
                                    } else {
                                        Ok(())
                                    }
                                }
                                CrosstermEvent::Mouse(e) => {
                                    sender.send(TerminalEvent::Mouse(e)).await
                                }
                                CrosstermEvent::Resize(w, h) => {
                                    sender.send(TerminalEvent::Resize(w, h)).await
                                }
                                _ => {
                                    log::info!("Crossterm event not implemented");
                                    Ok(())
                                }
                            };

                            if let Err(e) = result {
                                log::error!("Failed to send terminal event: {e}");
                                break;
                            }
                        }
                    }

                    if last_tick.elapsed() >= time_step {
                        if let Err(e) = sender.send(TerminalEvent::Tick { tick: Tick::now() }).await
                        {
                            log::error!("Failed to send tick event: {e}");
                            break;
                        }
                        last_tick = Instant::now();
                    }
                    if !with_input_reader {
                        tokio::time::sleep(time_step).await;
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
