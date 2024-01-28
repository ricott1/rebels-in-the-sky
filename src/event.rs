use crate::types::{AppResult, SystemTimeTick, Tick};
use crossterm::event::{self, Event as CrosstermEvent, KeyEvent, MouseEvent};
use futures::Future;
use std::pin::Pin;
use std::sync::mpsc;
use std::task::{Context, Poll};
use std::thread;
use std::time::Duration;

const TICK_RATE: f64 = 25.0; //ticks per milliseconds
const TIME_STEP: Duration = Duration::from_millis((1000.0 / TICK_RATE) as u64);
const TIME_STEP_MILLIS: Tick = (1000.0 / TICK_RATE) as Tick;

/// Terminal events.
#[derive(Clone, Copy, Debug)]
pub enum TerminalEvent {
    Tick { tick: Tick },
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize(u16, u16),
}

impl Future for TerminalEvent {
    type Output = Self;
    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Ready(*self)
    }
}

/// Terminal event handler.
#[allow(dead_code)]
#[derive(Debug)]
pub struct EventHandler {
    /// TerminalEvent sender channel.
    sender: mpsc::Sender<TerminalEvent>,
    /// TerminalEvent receiver channel.
    receiver: mpsc::Receiver<TerminalEvent>,
    /// TerminalEvent handler thread.
    handler: thread::JoinHandle<()>,
}

impl EventHandler {
    /// Constructs a new instance of [`EventHandler`].
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        let handler = {
            let sender = sender.clone();
            let mut last_tick = Tick::now();
            thread::spawn(move || loop {
                if event::poll(TIME_STEP).expect("no events available") {
                    match event::read().expect("unable to read event") {
                        CrosstermEvent::Key(e) => sender.send(TerminalEvent::Key(e)),
                        CrosstermEvent::Mouse(e) => sender.send(TerminalEvent::Mouse(e)),
                        CrosstermEvent::Resize(w, h) => sender.send(TerminalEvent::Resize(w, h)),
                        _ => unimplemented!(),
                    }
                    .expect("failed to send terminal event")
                }

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
