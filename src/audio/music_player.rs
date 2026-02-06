use crate::app::AppEvent;
use crate::store::ASSETS_DIR;
use crate::types::AppResult;
use anyhow::anyhow;
use rodio::{Decoder, OutputStream, Sink};
use serde::Deserialize;
use std::fmt::Debug;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{
    mpsc::{self, Receiver, Sender},
    Arc,
};
use std::thread;
use std::time::Duration;
use stream_download::storage::temp::TempStorageProvider;
use stream_download::{Settings, StreamDownload};
use tokio_util::sync::CancellationToken;
use url::Url;

const STREAMING_TIMEOUT_MILLIS: u64 = 2_000;

#[derive(Debug)]
pub enum MusicPlayerEvent {
    StreamOk,
    StreamErr { error_message: String },
}

#[derive(Debug, Deserialize)]
struct Stream {
    name: String,
    url_string: String,
}

impl Stream {
    pub fn url(&self) -> AppResult<Url> {
        Ok(self.url_string.parse::<Url>()?)
    }
}

enum AudioCommand {
    Append(StreamDownload<TempStorageProvider>),
    Clear,
    Play,
    Pause,
}

#[derive(Debug, Default)]
enum StreamStatus {
    #[default]
    Uninitialized,
    Ready {
        sender: mpsc::Sender<AudioCommand>,
    },
}

#[derive(Debug, Default)]
pub struct MusicPlayer {
    stream_status: StreamStatus,
    is_buffering: Arc<AtomicBool>,
    has_buffer: Arc<AtomicBool>,
    is_playing: Arc<AtomicBool>,
    streams: Vec<Stream>,
    index: usize,
}

impl MusicPlayer {
    fn current_url(&self) -> AppResult<Url> {
        self.streams
            .get(self.index)
            .ok_or_else(|| anyhow!("No streams available"))?
            .url()
    }

    fn start_streaming(&self, url: Url, app_sender: tokio::sync::mpsc::Sender<AppEvent>) {
        let sender = match &self.stream_status {
            StreamStatus::Uninitialized => unreachable!("Stream should have been initialized."),
            StreamStatus::Ready { sender } => sender.clone(),
        };

        let is_buffering_clone = self.is_buffering.clone();

        // spawn a tokio task for the HTTP streaming (non-blocking)
        tokio::spawn(async move {
            is_buffering_clone.store(true, Ordering::Relaxed);

            let result =
                tokio::time::timeout(Duration::from_millis(STREAMING_TIMEOUT_MILLIS), async {
                    StreamDownload::new_http(
                        url,
                        TempStorageProvider::default(),
                        Settings::default(),
                    )
                    .await
                })
                .await;

            match result {
                Ok(Ok(data)) => {
                    // send the StreamDownload to the audio thread
                    if let Err(send_err) = sender.send(AudioCommand::Append(data)) {
                        let error_message = format!("Audio thread receiver dropped: {send_err:?}");
                        log::error!("{error_message}");
                        let _ = app_sender
                            .send(AppEvent::AudioEvent(MusicPlayerEvent::StreamErr {
                                error_message,
                            }))
                            .await;
                    }
                }
                Ok(Err(err)) => {
                    let error_message = format!("Unable to start audio stream: {err}");
                    log::error!("{error_message}");
                    let _ = app_sender
                        .send(AppEvent::AudioEvent(MusicPlayerEvent::StreamErr {
                            error_message,
                        }))
                        .await;
                }
                Err(_) => {
                    let error_message = "Audio streaming timed out".to_string();
                    log::error!("{error_message}");
                    let _ = app_sender
                        .send(AppEvent::AudioEvent(MusicPlayerEvent::StreamErr {
                            error_message,
                        }))
                        .await;
                }
            }

            is_buffering_clone.store(false, Ordering::Relaxed);
        });
    }

    pub fn new() -> AppResult<MusicPlayer> {
        let file = ASSETS_DIR
            .get_file("data/stream_data.json")
            .expect("Could not find stream_data.json");
        let data = file
            .contents_utf8()
            .expect("Could not read stream_data.json");
        let streams: Vec<Stream> = serde_json::from_str(data)?;

        Ok(MusicPlayer {
            streams,
            ..Default::default()
        })
    }

    pub fn start_audio_event_loop(
        &mut self,
        cancellation_token: CancellationToken,
    ) -> AppResult<()> {
        let is_buffering_clone = self.is_buffering.clone();
        let has_buffer_clone = self.has_buffer.clone();
        let is_playing_clone = self.is_playing.clone();

        let (sender, receiver): (Sender<AudioCommand>, Receiver<AudioCommand>) = mpsc::channel();
        self.stream_status = StreamStatus::Ready { sender };

        thread::Builder::new()
            .name("audio-thread".into())
            .spawn(move || {
                let (_stream, stream_handle) = match OutputStream::try_default() {
                    Ok(v) => v,
                    Err(e) => {
                        log::error!("Failed to create audio output stream: {e}");
                        return;
                    }
                };

                let sink = match Sink::try_new(&stream_handle) {
                    Ok(s) => s,
                    Err(e) => {
                        log::error!("Failed to create rodio Sink: {e}");
                        return;
                    }
                };
                sink.pause();

                while let Ok(cmd) = receiver.recv() {
                    if cancellation_token.is_cancelled() {
                        log::info!("Music player loop shutting down.");
                        break;
                    }

                    match cmd {
                        AudioCommand::Append(data) => {
                            is_buffering_clone.store(true, Ordering::Relaxed);
                            match Decoder::new(data) {
                                Ok(dec) => {
                                    sink.append(dec);
                                    has_buffer_clone.store(true, Ordering::Relaxed);
                                    sink.play();
                                    is_playing_clone.store(true, Ordering::Relaxed);
                                }
                                Err(e) => {
                                    log::error!("Failed to create decoder on audio thread: {e}");
                                }
                            }
                            is_buffering_clone.store(false, Ordering::Relaxed);
                        }

                        AudioCommand::Play => {
                            sink.play();
                            is_playing_clone.store(true, Ordering::Relaxed);
                        }
                        AudioCommand::Pause => {
                            sink.pause();
                            is_playing_clone.store(false, Ordering::Relaxed);
                        }

                        AudioCommand::Clear => {
                            sink.clear();
                            has_buffer_clone.store(false, Ordering::Relaxed);
                            is_playing_clone.store(false, Ordering::Relaxed);
                            // is_buffering_clone.store(false, Ordering::Relaxed);
                        }
                    }

                    if sink.empty() {
                        has_buffer_clone.store(false, Ordering::Relaxed);
                        is_playing_clone.store(false, Ordering::Relaxed);
                    }
                }
            })?;

        Ok(())
    }

    pub fn is_buffering(&self) -> bool {
        self.is_buffering.load(Ordering::Relaxed)
    }

    pub fn is_playing(&self) -> bool {
        self.is_playing.load(Ordering::Relaxed)
    }

    pub fn previous_radio_stream(
        &mut self,
        app_sender: tokio::sync::mpsc::Sender<AppEvent>,
    ) -> AppResult<()> {
        let sender = match &self.stream_status {
            StreamStatus::Uninitialized => return Err(anyhow!("Stream is not initialized.")),
            StreamStatus::Ready { sender } => sender,
        };

        if self.streams.is_empty() {
            return Err(anyhow!("No streams available"));
        }
        self.index = (self.index + self.streams.len() - 1) % self.streams.len();
        if self.is_playing() {
            let _ = sender.send(AudioCommand::Clear);
            let url = self.current_url()?;
            self.start_streaming(url, app_sender);
        } else {
            let _ = sender.send(AudioCommand::Clear);
        }
        Ok(())
    }

    pub fn next_radio_stream(
        &mut self,
        app_sender: tokio::sync::mpsc::Sender<AppEvent>,
    ) -> AppResult<()> {
        let sender = match &self.stream_status {
            StreamStatus::Uninitialized => return Err(anyhow!("Stream is not initialized.")),
            StreamStatus::Ready { sender } => sender,
        };

        if self.streams.is_empty() {
            return Err(anyhow!("No streams available"));
        }
        self.index = (self.index + 1) % self.streams.len();
        if self.is_playing() {
            let _ = sender.send(AudioCommand::Clear);
            let url = self.current_url()?;
            self.start_streaming(url, app_sender);
        } else {
            let _ = sender.send(AudioCommand::Clear);
        }
        Ok(())
    }

    pub fn toggle_state(
        &mut self,
        app_sender: tokio::sync::mpsc::Sender<AppEvent>,
    ) -> AppResult<()> {
        let sender = match &self.stream_status {
            StreamStatus::Uninitialized => return Err(anyhow!("Stream is not initialized.")),
            StreamStatus::Ready { sender } => sender,
        };

        if self.is_playing() {
            let _ = sender.send(AudioCommand::Pause);
        } else if !self.has_buffer.load(Ordering::Relaxed) {
            if !self.is_buffering() {
                let url = self.current_url()?;
                self.start_streaming(url, app_sender);
            }
        } else {
            let _ = sender.send(AudioCommand::Play);
        }

        Ok(())
    }

    pub fn currently_playing(&self) -> Option<String> {
        Some(self.streams.get(self.index)?.name.clone())
    }
}
