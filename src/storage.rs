use crate::models::{Playlist, Track};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const DATA_DIR: &str = "data";
const COVERS_DIR: &str = "data/covers";
const LIBRARY_FILE: &str = "data/library_cache.json";
const PLAYLISTS_FILE: &str = "data/playlists.json";

fn ensure_dirs() {
    let _ = fs::create_dir_all(COVERS_DIR);
}

pub fn load_library() -> Vec<Track> {
    fs::read_to_string(LIBRARY_FILE)
        .ok()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

pub fn save_library(tracks: &[Track]) {
    ensure_dirs();
    if let Ok(json) = serde_json::to_string_pretty(tracks) {
        let _ = fs::write(LIBRARY_FILE, json);
    }
}

pub fn load_playlists() -> Vec<Playlist> {
    fs::read_to_string(PLAYLISTS_FILE)
        .ok()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

pub fn save_playlists(playlists: &[Playlist]) {
    ensure_dirs();
    if let Ok(json) = serde_json::to_string_pretty(playlists) {
        let _ = fs::write(PLAYLISTS_FILE, json);
    }
}

/// Copies a chosen PNG/JPEG into `data/covers/` so the playlist cover remains valid even if the source moves.
pub fn import_playlist_cover(source_image: &Path) -> Result<PathBuf, String> {
    ensure_dirs();
    let ext = source_image
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png");
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let dest = Path::new(DATA_DIR)
        .join("covers")
        .join(format!("cover_{stamp}.{ext}"));

    fs::copy(source_image, &dest).map_err(|e| format!("Failed to copy cover image: {e}"))?;
    Ok(dest)
}