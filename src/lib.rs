pub mod app;
pub mod args;
#[cfg(feature = "audio")]
pub mod audio;
pub mod core;
pub mod crossterm_event_handler;
pub mod game_engine;
pub mod image;
pub mod network;
#[cfg(feature = "relayer")]
pub mod relayer;
#[cfg(feature = "ssh")]
pub mod session_auth;
pub mod space_adventure;
#[cfg(feature = "ssh")]
pub mod ssh_game;
pub mod store;
pub mod tick_event_handler;
pub mod tui;
pub mod types;
pub mod ui;
use std::sync::OnceLock;

use update_informer::Check;

static LATEST_VERSION: OnceLock<Option<String>> = OnceLock::new();

pub fn app_version() -> [usize; 3] {
    [
        env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap_or_default(),
        env!("CARGO_PKG_VERSION_MINOR").parse().unwrap_or_default(),
        env!("CARGO_PKG_VERSION_PATCH").parse().unwrap_or_default(),
    ]
}

pub fn spawn_update_check() {
    std::thread::spawn(|| {
        let latest = update_informer::new(
            update_informer::registry::Crates,
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION"),
        )
        .interval(std::time::Duration::from_secs(60 * 60 * 24))
        .check_version()
        .ok()
        .flatten()
        .map(|v| v.to_string());
        let _ = LATEST_VERSION.set(latest);
    });
}

pub fn update_available() -> Option<&'static str> {
    LATEST_VERSION.get()?.as_deref()
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AudioPlayerState {
    Playing,
    Paused,
    Disabled,
}
