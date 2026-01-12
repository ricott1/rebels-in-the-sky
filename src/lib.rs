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
pub mod space_adventure;
#[cfg(feature = "ssh")]
pub mod ssh;
pub mod store;
pub mod tick_event_handler;
pub mod tui;
pub mod types;
pub mod ui;

pub fn app_version() -> [usize; 3] {
    [
        env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap_or_default(),
        env!("CARGO_PKG_VERSION_MINOR").parse().unwrap_or_default(),
        env!("CARGO_PKG_VERSION_PATCH").parse().unwrap_or_default(),
    ]
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AudioPlayerState {
    Playing,
    Paused,
    Disabled,
}
