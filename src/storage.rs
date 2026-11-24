use crate::models::{MetricsData, Playlist, Track};
use std::fs;
use std::path::{Path, PathBuf};

const DATA_DIR: &str = "data";
const COVERS_DIR: &str = "data/covers";
const LIBRARY_FILE: &str = "data/library_cache.json";
const PLAYLISTS_FILE: &str = "data/playlists.json";
const METRICS_FILE: &str = "data/metrics.json";

fn ensure_dirs() {
    let _ = fs::create_dir_all(DATA_DIR);
    let _ = fs::create_dir_all(COVERS_DIR);
}

pub fn load_library() -> Vec<Track> {
    ensure_dirs();
    let Ok(raw) = fs::read_to_string(LIBRARY_FILE) else {
        return Vec::new();
    };
    let mut tracks: Vec<Track> = serde_json::from_str(&raw).unwrap_or_default();
    let before = tracks.len();
    tracks.retain(|t| t.path.exists());
    if tracks.len() != before {
        save_library(&tracks);
    }
    tracks
}

pub fn save_library(tracks: &[Track]) {
    ensure_dirs();
    if let Ok(json) = serde_json::to_string_pretty(tracks) {
        let _ = fs::write(LIBRARY_FILE, json);
    }
}

pub fn load_playlists() -> Vec<Playlist> {
    ensure_dirs();
    let Ok(raw) = fs::read_to_string(PLAYLISTS_FILE) else {
        return Vec::new();
    };
    let mut playlists: Vec<Playlist> = serde_json::from_str(&raw).unwrap_or_default();
    let mut dirty = false;

    for pl in &mut playlists {
        let before = pl.track_paths.len();
        pl.track_paths.retain(|p| p.exists());
        if pl.track_paths.len() != before {
            dirty = true;
        }
        if let Some(cover) = &pl.cover_path {
            if !cover.exists() {
                pl.cover_path = None;
                dirty = true;
            }
        }
    }

    if dirty {
        save_playlists(&playlists);
    }
    playlists
}

pub fn save_playlists(playlists: &[Playlist]) {
    ensure_dirs();
    if let Ok(json) = serde_json::to_string_pretty(playlists) {
        let _ = fs::write(PLAYLISTS_FILE, json);
    }
}

pub fn import_playlist_cover(source_path: &Path) -> Result<PathBuf, String> {
    ensure_dirs();
    let ext = source_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png");
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let dest = PathBuf::from(COVERS_DIR).join(format!("cover_{stamp}.{ext}"));
    fs::copy(source_path, &dest).map_err(|e| format!("Failed to copy cover image: {e}"))?;
    Ok(dest)
}

pub fn load_metrics() -> MetricsData {
    ensure_dirs();
    let Ok(raw) = fs::read_to_string(METRICS_FILE) else {
        return MetricsData::default();
    };
    let mut metrics: MetricsData = serde_json::from_str(&raw).unwrap_or_default();
    let before = metrics.per_track.len();
    metrics.per_track.retain(|path, _| path.exists());
    if metrics.per_track.len() != before {
        save_metrics(&metrics);
    }
    metrics
}

pub fn save_metrics(metrics: &MetricsData) {
    ensure_dirs();
    if let Ok(json) = serde_json::to_string_pretty(metrics) {
        let _ = fs::write(METRICS_FILE, json);
    }
}

#[cfg(target_os = "windows")]
pub fn current_process_ram_mb() -> f32 {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> isize;
    }

    #[link(name = "psapi")]
    unsafe extern "system" {
        fn K32QueryWorkingSet(
            hprocess: isize,
            pv: *mut usize,
            cb: u32,
        ) -> i32;
    }

    unsafe {
        let proc = GetCurrentProcess();
        // First call with a small buffer to read the required entry count in buf[0]
        let mut count_buf = [0usize; 2];
        let _ = K32QueryWorkingSet(
            proc,
            count_buf.as_mut_ptr(),
            std::mem::size_of_val(&count_buf) as u32,
        );

        let total_entries = count_buf[0].saturating_add(256);
        let mut ws_info = vec![0usize; total_entries + 1];
        let byte_size = (ws_info.len() * std::mem::size_of::<usize>()) as u32;

        if K32QueryWorkingSet(proc, ws_info.as_mut_ptr(), byte_size) != 0 {
            let entry_count = ws_info[0].min(ws_info.len() - 1);
            // Bit 8 (0x100) in each PSAPI_WORKING_SET_BLOCK is the Shared flag
            let private_pages = ws_info[1..=entry_count]
                .iter()
                .filter(|&&block| (block & 0x100) == 0)
                .count();
            // Each standard x64 Windows page is 4 KB (4096 bytes)
            return (private_pages * 4096) as f32 / (1024.0 * 1024.0);
        }
    }
    0.0
}

#[cfg(target_os = "windows")]
pub fn trim_working_set() {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> isize;
    }
    #[link(name = "psapi")]
    unsafe extern "system" {
        fn K32EmptyWorkingSet(process: isize) -> i32;
    }
    unsafe {
        let _ = K32EmptyWorkingSet(GetCurrentProcess());
    }
}


