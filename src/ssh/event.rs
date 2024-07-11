use crate::event::TerminalEvent;
use crate::types::{AppResult, SystemTimeTick, Tick};
use std::sync::mpsc;
use std::thread;

const TICK_RATE: f64 = 25.0; //ticks per milliseconds
const TIME_STEP_MILLIS: Tick = (1000.0 / TICK_RATE) as Tick;

/// SSH event handler.
#[allow(dead_code)]
#[derive(Debug)]
pub struct TickEventHandler {
    /// TerminalEvent sender channel.
    sender: mpsc::Sender<TerminalEvent>,
    /// TerminalEvent receiver channel.
    receiver: mpsc::Receiver<TerminalEvent>,
    /// TerminalEvent handler thread.
    handler: thread::JoinHandle<()>,
}

impl TickEventHandler {
    /// Constructs a new instance of [`TickEventHandler`].
    pub fn handler() -> Self {
        let (sender, receiver) = mpsc::channel();
        let handler = {
            let sender = sender.clone();
            let mut last_tick = Tick::now();
            thread::spawn(move || loop {
                let now = Tick::now();
                if now - last_tick >= TIME_STEP_MILLIS {
                    if let Err(_) = sender.send(TerminalEvent::Tick { tick: now }) {
                        // eprintln!("Failed to send tick event: {}", err);
                        break;
                    }
                    last_tick = now;
                }
            })
        };
        Self {
            sender,
            receiver,
            handler,
        }
    }

    /// Receive the next event from the handler thread.
    ///
    /// This function will always block the current thread if
    /// there is no data available and it's possible for more data to be sent.
    pub fn next(&self) -> AppResult<TerminalEvent> {
        Ok(self.receiver.recv()?)
    }
}
