use crate::app::AppEvent;
use crate::store::ASSETS_DIR;
use crate::types::AppResult;
use anyhow::anyhow;
use rodio::OutputStream;
use rodio::{OutputStreamHandle, Sink};
use serde::Deserialize;
use std::fmt::Debug;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use stream_download::storage::temp::TempStorageProvider;
use stream_download::{Settings, StreamDownload};
use tokio::sync::mpsc;
use url::Url;

const STREAMING_TIMEOUT_MILLIS: u64 = 2_000;

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

#[derive(Clone)]
struct MusicPlayerTask {
    event_sender: mpsc::Sender<AppEvent>,
    is_buffering: Arc<AtomicBool>,
}

pub struct MusicPlayer {
    _stream: OutputStream,
    _stream_handle: OutputStreamHandle,
    is_buffering: Arc<AtomicBool>,
    sink: Sink,
    streams: Vec<Stream>,
    index: usize,
    event_sender: mpsc::Sender<AppEvent>,
}

unsafe impl Send for MusicPlayer {}
unsafe impl Sync for MusicPlayer {}

impl Debug for MusicPlayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MusicPlayer")
            .field("is_buffering", &self.is_buffering)
            .field("sink length", &self.sink.len())
            .field("streams", &self.streams)
            .field("index", &self.index)
            .finish()
    }
}

impl MusicPlayer {
    fn current_url(&self) -> AppResult<Url> {
        self.streams
            .get(self.index)
            .ok_or(anyhow!("No streams available"))?
            .url()
    }

    fn task_handle(&self) -> MusicPlayerTask {
        MusicPlayerTask {
            event_sender: self.event_sender.clone(),
            is_buffering: self.is_buffering.clone(),
        }
    }

    fn start_streaming(&self, url: Url) -> AppResult<()> {
        let task = self.task_handle();

        tokio::spawn(async move {
            task.is_buffering.store(true, Ordering::Relaxed);

            let result =
                tokio::time::timeout(Duration::from_millis(STREAMING_TIMEOUT_MILLIS), async {
                    match StreamDownload::new_http(
                        url,
                        TempStorageProvider::default(),
                        Settings::default(),
                    )
                    .await
                    {
                        Ok(data) => {
                            if let Err(err) =
                                task.event_sender.send(AppEvent::AudioEvent(data)).await
                            {
                                log::error!("Audio event receiver dropped: {err}");
                            }
                        }
                        Err(err) => {
                            log::error!("Unable to start audio stream: {err}");
                        }
                    }
                })
                .await;

            if result.is_err() {
                log::error!("Audio streaming timed out");
            }

            task.is_buffering.store(false, Ordering::Relaxed);
        });

        Ok(())
    }

    pub fn new(event_sender: mpsc::Sender<AppEvent>) -> AppResult<MusicPlayer> {
        let (_stream, _stream_handle) = OutputStream::try_default()?;
        let sink = rodio::Sink::try_new(&_stream_handle)?;
        sink.pause();

        let file = ASSETS_DIR
            .get_file("data/stream_data.json")
            .expect("Could not find stream_data.json");
        let data = file
            .contents_utf8()
            .expect("Could not read stream_data.json");
        let streams = serde_json::from_str(data)?;

        Ok(MusicPlayer {
            _stream,
            _stream_handle,
            is_buffering: Arc::new(AtomicBool::new(false)),
            sink,
            streams,
            index: 0,
            event_sender,
        })
    }

    pub fn is_playing(&self) -> bool {
        !self.sink.is_paused()
    }

    pub fn previous_radio_stream(&mut self) -> AppResult<()> {
        if self.streams.is_empty() {
            return Err(anyhow!("No streams available"));
        }
        self.index = (self.index + self.streams.len() - 1) % self.streams.len();
        if self.is_playing() {
            self.sink.clear();
            self.toggle_state()?;
        } else {
            self.sink.clear();
        }
        Ok(())
    }

    pub fn next_radio_stream(&mut self) -> AppResult<()> {
        if self.streams.is_empty() {
            return Err(anyhow!("No streams available"));
        }
        self.index = (self.index + 1) % self.streams.len();
        if self.is_playing() {
            self.sink.clear();
            self.toggle_state()?;
        } else {
            self.sink.clear();
        }
        Ok(())
    }

    pub fn toggle_state(&mut self) -> AppResult<()> {
        if self.is_playing() {
            log::info!("Pausing playback");
            self.sink.pause();
        } else if self.sink.empty() {
            if !self.is_buffering.load(Ordering::Relaxed) {
                let url = self.current_url()?;
                self.start_streaming(url)?;
            }
        } else {
            log::info!("Resuming playback");
            self.sink.play();
        }

        Ok(())
    }

    pub fn handle_streaming_ready(
        &mut self,
        data: StreamDownload<TempStorageProvider>,
    ) -> AppResult<()> {
        self.sink.append(rodio::Decoder::new(data)?);
        self.sink.play();
        Ok(())
    }

    pub fn currently_playing(&self) -> Option<String> {
        Some(self.streams[self.index].name.clone())
    }
}
