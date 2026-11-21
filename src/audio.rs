use crate::models::PlaybackState;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::time::Duration;

pub struct AudioEngine {
    _stream: OutputStream,
    _stream_handle: OutputStreamHandle,
    sink: Sink,
    pub state: PlaybackState,
    pub volume: f32,
}

impl AudioEngine {
    pub fn new() -> Result<Self, String> {
        let (stream, stream_handle) =
            OutputStream::try_default().map_err(|e| format!("Audio output error: {e}"))?;
        let sink =
            Sink::try_new(&stream_handle).map_err(|e| format!("Audio sink error: {e}"))?;
        sink.set_volume(0.8);

        Ok(Self {
            _stream: stream,
            _stream_handle: stream_handle,
            sink,
            state: PlaybackState::Stopped,
            volume: 0.8,
        })
    }

    pub fn play_file(&mut self, path: &Path) -> Result<(), String> {
        self.sink.stop();
        let file = File::open(path).map_err(|e| format!("Cannot open file: {e}"))?;
        let source =
            Decoder::new(BufReader::new(file)).map_err(|e| format!("Decode error: {e}"))?;

        self.sink.append(source);
        self.sink.set_volume(self.volume);
        self.sink.play();
        self.state = PlaybackState::Playing;
        Ok(())
    }

    pub fn toggle_play_pause(&mut self) {
        match self.state {
            PlaybackState::Playing => {
                self.sink.pause();
                self.state = PlaybackState::Paused;
            }
            PlaybackState::Paused => {
                self.sink.play();
                self.state = PlaybackState::Playing;
            }
            PlaybackState::Stopped => {}
        }
    }

    pub fn stop(&mut self) {
        self.sink.stop();
        self.state = PlaybackState::Stopped;
    }

    pub fn set_volume(&mut self, vol: f32) {
        self.volume = vol.clamp(0.0, 1.0);
        self.sink.set_volume(self.volume);
    }

    pub fn seek(&mut self, position: Duration) {
        let _ = self.sink.try_seek(position);
    }

    pub fn position(&self) -> Duration {
        self.sink.get_pos()
    }

    pub fn is_finished(&self) -> bool {
        self.state == PlaybackState::Playing && self.sink.empty()
    }
}