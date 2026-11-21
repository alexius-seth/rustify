use crate::models::Track;
use lofty::config::ParseOptions;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::picture::PictureType;
use lofty::probe::Probe;
use lofty::tag::Accessor;
use std::path::Path;
use walkdir::WalkDir;

const SUPPORTED_EXTENSIONS: &[&str] = &["mp3", "flac", "wav", "ogg", "m4a"];

pub fn scan_folder(folder: &Path) -> Vec<Track> {
    let mut tracks = Vec::new();

    for entry in WalkDir::new(folder).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let is_supported = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| SUPPORTED_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
            .unwrap_or(false);

        if is_supported {
            tracks.push(read_track_metadata(path));
        }
    }

    tracks.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    tracks
}

pub fn read_track_metadata(path: &Path) -> Track {
    let fallback_title = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown Title")
        .to_string();

    let Ok(tagged_file) = Probe::open(path)
        .and_then(|p| p.options(ParseOptions::new()).read())
    else {
        return Track {
            path: path.to_path_buf(),
            title: fallback_title,
            artist: "Unknown Artist".to_string(),
            album: "Unknown Album".to_string(),
            year: None,
            duration_secs: 0,
            bitrate_kbps: None,
            sample_rate_hz: None,
        };
    };

    let properties = tagged_file.properties();
    let duration_secs = properties.duration().as_secs();
    let bitrate_kbps = properties.audio_bitrate();
    let sample_rate_hz = properties.sample_rate();

    if let Some(tag) = tagged_file.primary_tag().or_else(|| tagged_file.first_tag()) {
        let title = tag
            .title()
            .map(|s| s.to_string())
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(fallback_title);

        let artist = tag
            .artist()
            .map(|s| s.to_string())
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "Unknown Artist".to_string());

        let album = tag
            .album()
            .map(|s| s.to_string())
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "Unknown Album".to_string());

        let year = tag.year();

        Track {
            path: path.to_path_buf(),
            title,
            artist,
            album,
            year,
            duration_secs,
            bitrate_kbps,
            sample_rate_hz,
        }
    } else {
        Track {
            path: path.to_path_buf(),
            title: fallback_title,
            artist: "Unknown Artist".to_string(),
            album: "Unknown Album".to_string(),
            year: None,
            duration_secs,
            bitrate_kbps,
            sample_rate_hz,
        }
    }
}

/// Extracts front cover bytes (or the first embedded picture) from an audio file.
pub fn extract_cover_bytes(path: &Path) -> Option<Vec<u8>> {
    let tagged_file = Probe::open(path).ok()?.read().ok()?;
    let tag = tagged_file.primary_tag().or_else(|| tagged_file.first_tag())?;

    let picture = tag
        .pictures()
        .iter()
        .find(|p| p.pic_type() == PictureType::CoverFront)
        .or_else(|| tag.pictures().first())?;

    Some(picture.data().to_vec())
}