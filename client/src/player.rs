use rodio::{Decoder, OutputStream, Sink, Source};
use ssmgr_shared::PlaybackMode;
use std::io::Cursor;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Mutex;
use tracing::info;

pub struct AudioPlayer {
    _stream: OutputStream,
    stream_handle: rodio::OutputStreamHandle,
    sink: Arc<Mutex<Option<Sink>>>,
    playback_mode: PlaybackMode,
    currently_playing: Option<String>,
    is_playing: Arc<AtomicBool>,
}

impl AudioPlayer {
    pub fn new() -> Result<Self, String> {
        let (stream, stream_handle) =
            OutputStream::try_default().map_err(|e| format!("No audio output: {}", e))?;

        Ok(Self {
            _stream: stream,
            stream_handle,
            sink: Arc::new(Mutex::new(None)),
            playback_mode: PlaybackMode::Once,
            currently_playing: None,
            is_playing: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn set_playback_mode(&mut self, mode: PlaybackMode) {
        self.playback_mode = mode;
    }

    pub fn get_playback_mode(&self) -> &PlaybackMode {
        &self.playback_mode
    }

    pub fn is_playing(&self) -> bool {
        self.is_playing.load(Ordering::SeqCst)
    }

    pub fn currently_playing(&self) -> Option<&str> {
        self.currently_playing.as_deref()
    }

    pub async fn play_bytes(&mut self, data: Vec<u8>, sample_path: &str) -> Result<(), String> {
        self.stop().await;

        let cursor = Cursor::new(data);
        let source = Decoder::new(cursor).map_err(|e| format!("Decode error: {}", e))?;

        let sink = Sink::try_new(&self.stream_handle)
            .map_err(|e| format!("Sink error: {}", e))?;

        match self.playback_mode {
            PlaybackMode::Loop => {
                sink.append(source.repeat_infinite());
            }
            PlaybackMode::Once => {
                sink.append(source);
            }
        }

        self.currently_playing = Some(sample_path.to_string());
        self.is_playing.store(true, Ordering::SeqCst);

        {
            let mut sink_guard = self.sink.lock().await;
            *sink_guard = Some(sink);
        }

        let sink_ref = self.sink.clone();
        let is_playing = self.is_playing.clone();
        tokio::spawn(async move {
            loop {
                let sink_guard = sink_ref.lock().await;
                if let Some(s) = sink_guard.as_ref() {
                    if s.empty() {
                        break;
                    }
                } else {
                    break;
                }
                drop(sink_guard);
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
            is_playing.store(false, Ordering::SeqCst);
        });

        info!("Playing: {}", sample_path);
        Ok(())
    }

    pub async fn stop(&mut self) {
        let mut sink_guard = self.sink.lock().await;
        if let Some(sink) = sink_guard.take() {
            sink.stop();
        }
        self.currently_playing = None;
        self.is_playing.store(false, Ordering::SeqCst);
    }

    pub async fn toggle_loop(&mut self) {
        self.playback_mode = match self.playback_mode {
            PlaybackMode::Once => PlaybackMode::Loop,
            PlaybackMode::Loop => PlaybackMode::Once,
        };
    }
}
