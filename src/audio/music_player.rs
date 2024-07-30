use crate::types::AppResult;
use anyhow::anyhow;
use rodio::OutputStream;
use rodio::{OutputStreamHandle, Sink};
use stream_download::http::reqwest::Client;
use stream_download::http::HttpStream;
use stream_download::source::SourceStream;
use stream_download::storage::temp::TempStorageProvider;
use stream_download::{Settings, StreamDownload};
use url::Url;

const DEFAULT_RADIO_URL: &'static str = "https://radio.frittura.org/rebels.ogg";

pub struct MusicPlayer {
    _stream: OutputStream,
    _stream_handle: OutputStreamHandle,
    sink: Sink,
    pub is_playing: bool,
    currently_playing: Option<Url>,
}

unsafe impl Send for MusicPlayer {}
unsafe impl Sync for MusicPlayer {}

impl MusicPlayer {
    async fn load_stream(&mut self, url: Url) -> AppResult<()> {
        let stream = HttpStream::<Client>::create(url.clone()).await?;

        log::info!("content type={:?}", stream.content_type());

        let reader =
            StreamDownload::from_stream(stream, TempStorageProvider::new(), Settings::default())
                .await?;

        self.sink.append(rodio::Decoder::new(reader)?);
        self.currently_playing = Some(url);

        Ok(())
    }

    async fn play(&mut self) -> AppResult<()> {
        if self.sink.empty() {
            let url: Url = DEFAULT_RADIO_URL.parse()?;
            if let Err(e) = self.load_stream(url.clone()).await {
                return Err(anyhow!("Error loading stream from {url}: {e}"));
            }
        }
        self.sink.play();
        self.is_playing = true;
        Ok(())
    }

    fn pause(&mut self) {
        self.sink.pause();
        self.is_playing = false;
    }

    pub async fn new() -> AppResult<MusicPlayer> {
        let (_stream, _stream_handle) = OutputStream::try_default()?;
        let sink = rodio::Sink::try_new(&_stream_handle)?;

        let mut player = MusicPlayer {
            sink,
            _stream,
            _stream_handle,
            is_playing: true,
            currently_playing: None,
        };

        // Start in paused state.
        player.pause();

        let url: Url = DEFAULT_RADIO_URL.parse()?;
        player
            .load_stream(url.clone())
            .await
            .unwrap_or_else(|e| log::error!("Error loading stream from {url}: {e}"));

        Ok(player)
    }

    pub async fn toggle(&mut self) -> AppResult<()> {
        if self.is_playing {
            self.pause();
        } else {
            self.play().await?;
        }
        Ok(())
    }

    pub fn currently_playing(&self) -> Option<String> {
        match self.currently_playing.as_ref() {
            Some(url) => Some(url.as_str().replace("https://", "").replace("http://", "")),
            None => None,
        }
    }
}
