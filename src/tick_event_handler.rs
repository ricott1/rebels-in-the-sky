use crate::app::AppEvent;
use crate::types::{SystemTimeTick, Tick};
use std::time::Duration;
use tokio::{select, sync::mpsc, task::JoinHandle, time};
use tokio_util::sync::CancellationToken;

pub const TICK_FPS: u8 = 40;

pub fn start_tick_event_loop(
    event_sender: mpsc::Sender<AppEvent>,
    cancellation_token: CancellationToken,
) -> JoinHandle<()> {
    let tick_interval_duration = Duration::from_secs_f32(1.0 / TICK_FPS as f32);
    let mut ticker = time::interval(tick_interval_duration);
    ticker.set_missed_tick_behavior(time::MissedTickBehavior::Delay);

    tokio::spawn(async move {
        loop {
            select! {
                _ = cancellation_token.cancelled() => {
                    log::info!("Tick loop shutting down.");
                    break;
                }

                _ = ticker.tick() => {
                    let tick = Tick::now();
                    if event_sender
                        .send(AppEvent::Tick(tick))
                        .await
                        .is_err()
                    {
                        log::warn!("App receiver dropped; stopping tick loop.");
                        break;
                    }
                }
            }
        }
    })
}
