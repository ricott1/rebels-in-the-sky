use crate::app::AppEvent;
use crate::types::{SystemTimeTick, Tick};
use std::time::Duration;
use tokio::{select, sync::mpsc, task::JoinHandle, time};
use tokio_util::sync::CancellationToken;

const SLOW_TICK_FPS: u8 = 10;
const FAST_TICK_FPS: u8 = 40;

pub fn start_tick_event_loop(
    event_sender: mpsc::Sender<AppEvent>,
    cancellation_token: CancellationToken,
) -> JoinHandle<()> {
    let mut slow_ticker = time::interval(Duration::from_secs_f32(1.0 / SLOW_TICK_FPS as f32));
    slow_ticker.set_missed_tick_behavior(time::MissedTickBehavior::Delay);

    let mut fast_ticker = time::interval(Duration::from_secs_f32(1.0 / FAST_TICK_FPS as f32));
    fast_ticker.set_missed_tick_behavior(time::MissedTickBehavior::Burst);

    tokio::spawn(async move {
        loop {
            select! {
                _ = cancellation_token.cancelled() => {
                    log::info!("Tick loop shutting down.");
                    break;
                }

                _ = slow_ticker.tick() => {
                    let tick = Tick::now();
                    if event_sender
                        .send(AppEvent::SlowTick(tick))
                        .await
                        .is_err()
                    {
                        log::warn!("App receiver dropped; stopping tick loop.");
                        break;
                    }
                }

                _ = fast_ticker.tick() => {
                    let tick = Tick::now();
                    if event_sender
                        .send(AppEvent::FastTick(tick))
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
