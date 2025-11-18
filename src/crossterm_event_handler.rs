use crate::app::AppEvent;
use crate::tui::TerminalEvent;
use crossterm::event::{self, Event as CrosstermEvent, KeyEventKind};
use tokio::{select, sync::mpsc, task::JoinHandle, time};
use tokio_util::sync::CancellationToken;

pub fn start_event_handler(
    event_sender: mpsc::Sender<AppEvent>,
    cancellation_token: CancellationToken,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let poll_interval = time::Duration::from_millis(10); // 100Hz polling
        loop {
            select! {
                _ = cancellation_token.cancelled() => {
                    log::info!("Terminal event handler shutting down.");
                    break;
                }

                _ = time::sleep(poll_interval) => {
                    if let Ok(true) = event::poll(std::time::Duration::from_millis(0)) {
                        match event::read() {
                            Ok(CrosstermEvent::Key(key)) => {
                                if key.kind == KeyEventKind::Press {
                                    let _ = event_sender
                                        .send(AppEvent::TerminalEvent(TerminalEvent::Key(key)))
                                        .await;
                                }
                            }
                            Ok(CrosstermEvent::Mouse(mouse)) => {
                                let _ = event_sender
                                    .send(AppEvent::TerminalEvent(TerminalEvent::Mouse(mouse)))
                                    .await;
                            }
                            Ok(CrosstermEvent::Resize(w, h)) => {
                                let _ = event_sender
                                    .send(AppEvent::TerminalEvent(TerminalEvent::Resize(w, h)))
                                    .await;
                            }
                            Ok(_) => {}
                            Err(e) => {
                                log::error!("Failed to read crossterm event: {e}");
                                break;
                            }
                        }
                    }
                }
            }
        }
    })
}
