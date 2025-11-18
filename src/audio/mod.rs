#[cfg(feature = "audio")]
pub mod music_player;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AudioPlayerState {
    Playing,
    Paused,
    Disabled,
}
