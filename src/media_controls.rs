use crate::models::{PlaybackState, Track};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

pub enum HardwareMediaCmd {
    Play,
    Pause,
    Toggle,
    Next,
    Previous,
    Stop,
}

pub struct OsMediaBridge {
    controls: MediaControls,
    rx: Receiver<HardwareMediaCmd>,
}

impl OsMediaBridge {
    pub fn new(frame: &eframe::CreationContext<'_>) -> Option<Self> {
        let handle = frame.window_handle().ok()?;
        let hwnd = match handle.as_raw() {
            RawWindowHandle::Win32(h) => Some(h.hwnd.get() as *mut std::ffi::c_void),
            _ => None,
        };

        let config = PlatformConfig {
            dbus_name: "rustify",
            display_name: "Rustify",
            hwnd,
        };

        let mut controls = MediaControls::new(config).ok()?;
        let (tx, rx) = mpsc::channel();

        controls
            .attach(move |event: MediaControlEvent| {
                let cmd = match event {
                    MediaControlEvent::Play => Some(HardwareMediaCmd::Play),
                    MediaControlEvent::Pause => Some(HardwareMediaCmd::Pause),
                    MediaControlEvent::Toggle => Some(HardwareMediaCmd::Toggle),
                    MediaControlEvent::Next => Some(HardwareMediaCmd::Next),
                    MediaControlEvent::Previous => Some(HardwareMediaCmd::Previous),
                    MediaControlEvent::Stop => Some(HardwareMediaCmd::Stop),
                    _ => None,
                };
                if let Some(c) = cmd {
                    let _ = tx.send(c);
                }
            })
            .ok()?;

        Some(Self { controls, rx })
    }

    pub fn poll(&self) -> Vec<HardwareMediaCmd> {
        let mut cmds = Vec::new();
        while let Ok(cmd) = self.rx.try_recv() {
            cmds.push(cmd);
        }
        cmds
    }

    pub fn set_metadata(&mut self, track: &Track) {
        let _ = self.controls.set_metadata(MediaMetadata {
            title: Some(&track.title),
            artist: Some(&track.artist),
            album: Some(&track.album),
            duration: Some(Duration::from_secs(track.duration_secs)),
            cover_url: None,
        });
    }

    pub fn set_playback(&mut self, state: PlaybackState, elapsed: Duration) {
        let progress = Some(MediaPosition(elapsed));
        let playback = match state {
            PlaybackState::Playing => MediaPlayback::Playing { progress },
            PlaybackState::Paused => MediaPlayback::Paused { progress },
            PlaybackState::Stopped => MediaPlayback::Stopped,
        };
        let _ = self.controls.set_playback(playback);
    }
}