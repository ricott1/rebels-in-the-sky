use crate::store::ASSETS_DIR;
use crate::types::AppResult;
use anyhow::anyhow;
use rodio::OutputStream;
use rodio::{OutputStreamHandle, Sink};
use serde::Deserialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Duration;
use stream_download::storage::temp::TempStorageProvider;
use stream_download::{Settings, StreamDownload};
use url::Url;

const STREAMING_TIMEOUT_MILLIS: u64 = 2_000;

#[derive(Deserialize)]
struct Stream {
    name: String,
    url_string: String,
}

impl Stream {
    pub fn url(&self) -> AppResult<Url> {
        Ok(self.url_string.parse::<Url>()?)
    }
}

pub struct MusicPlayer {
    _stream: OutputStream,
    _stream_handle: OutputStreamHandle,
    is_buffering: Arc<AtomicBool>,
    sink: Sink,
    sender: mpsc::Sender<StreamDownload<TempStorageProvider>>,
    receiver: mpsc::Receiver<StreamDownload<TempStorageProvider>>,
    streams: Vec<Stream>,
    index: usize,
}

unsafe impl Send for MusicPlayer {}
unsafe impl Sync for MusicPlayer {}

impl MusicPlayer {
    fn current_url(&self) -> AppResult<Url> {
        Ok(self.streams[self.index].url()?)
    }

    pub fn new() -> AppResult<MusicPlayer> {
        let (_stream, _stream_handle) = OutputStream::try_default()?;
        let sink = rodio::Sink::try_new(&_stream_handle)?;
        sink.pause();

        let (sender, receiver) = mpsc::channel();

        let file = ASSETS_DIR
            .get_file("data/stream_data.json")
            .expect("Could not find stream_data.json");
        let data = file
            .contents_utf8()
            .expect("Could not read stream_data.json");
        let streams = serde_json::from_str(&data).unwrap_or_else(|e| {
            panic!("Could not parse stream_data.json: {}", e);
        });

        Ok(MusicPlayer {
            _stream,
            _stream_handle,
            is_buffering: Arc::new(AtomicBool::new(false)),
            sink,
            sender,
            receiver,
            streams,
            index: 0,
        })
    }

    pub fn is_playing(&self) -> bool {
        !self.sink.is_paused()
    }

    pub fn next_audio_sample(&mut self) -> AppResult<()> {
        self.index = (self.index + 1) % self.streams.len();
        if self.is_playing() {
            self.sink.clear();
            self.toggle()?;
        } else {
            self.sink.clear();
        }
        Ok(())
    }

    pub fn toggle(&mut self) -> AppResult<()> {
        if self.is_playing() {
            self.sink.pause();
        } else {
            if self.sink.empty() {
                let is_buffering = self.is_buffering.clone();
                if !is_buffering.load(Ordering::Relaxed) {
                    let url = self.current_url()?.clone();
                    let sender = self.sender.clone();
                    is_buffering.store(true, Ordering::Relaxed);

                    tokio::spawn(tokio::time::timeout(
                        Duration::from_millis(STREAMING_TIMEOUT_MILLIS),
                        async move {
                            if let Ok(data) = StreamDownload::new_http(
                                url,
                                TempStorageProvider::default(),
                                Settings::default(),
                            )
                            .await
                            {
                                sender.send(data)?;
                                is_buffering.store(false, Ordering::Relaxed);
                                return Ok(());
                            } else {
                                log::error!("Unable to play stream");
                                is_buffering.store(false, Ordering::Relaxed);
                                return Err(anyhow!("Unable to start stream"));
                            }
                        },
                    ));
                }
            } else {
                self.sink.play();
            }
        }
        Ok(())
    }

    pub fn next_streaming_event(&self) -> AppResult<StreamDownload<TempStorageProvider>> {
        Ok(self.receiver.recv_timeout(Duration::from_millis(10))?)
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
