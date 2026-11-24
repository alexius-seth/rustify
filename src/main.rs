#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod library;
mod media_controls;
mod models;
mod storage;
mod ui;

use audio::AudioEngine;
use eframe::egui::{
    self, Color32, ColorImage, Key, Rect, RichText, Sense, Stroke, TextureHandle, TextureOptions,
    Vec2,
};
use egui_extras::{Column, TableBuilder};
use media_controls::{HardwareMediaCmd, OsMediaBridge};
use models::{MetricsData, PlaybackState, Playlist, RepeatMode, Track};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;
use ui::{
    BEVEL_LIGHT, CHARCOAL_BG, PHOSPHOR_GREEN, SLATE_DARK, SLATE_PANEL, SPOTIFY_EMERALD,
    TagEditorState, WINAMP_CYAN,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActiveView {
    AllSongs,
    Stats,
    Playlist(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SortColumn {
    DefaultOrder,
    Title,
    Artist,
    Album,
    Duration,
}

fn load_window_icon() -> Option<egui::IconData> {
    let bytes = include_bytes!("../assets/rustify.ico");
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (width, height) = img.dimensions();
    Some(egui::IconData {
        rgba: img.into_raw(),
        width,
        height,
    })
}

fn main() -> eframe::Result<()> {
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("RUSTIFY // Lightweight Audio Engine")
        .with_inner_size([1120.0, 740.0])
        .with_min_inner_size([900.0, 600.0]);

    if let Some(icon) = load_window_icon() {
        viewport = viewport.with_icon(std::sync::Arc::new(icon));
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "Rustify",
        options,
        Box::new(|cc| {
            apply_hybrid_theme(&cc.egui_ctx);
            let mut app = RustifyApp::new();
            app.media_bridge = OsMediaBridge::new(cc);
            Ok(Box::new(app))
        }),
    )
}

fn apply_hybrid_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = CHARCOAL_BG;
    visuals.window_fill = SLATE_PANEL;
    visuals.extreme_bg_color = Color32::from_rgb(10, 10, 14);

    visuals.widgets.inactive.bg_fill = Color32::from_rgb(30, 32, 46);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, BEVEL_LIGHT);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(46, 50, 72);
    visuals.widgets.active.bg_fill = Color32::from_rgb(36, 46, 42);

    visuals.selection.bg_fill = Color32::from_rgb(32, 46, 40);
    visuals.selection.stroke = Stroke::new(1.0_f32, SPOTIFY_EMERALD);

    ctx.set_visuals(visuals);
}

fn format_secs(total_secs: u64) -> String {
    format!("{:02}:{:02}", total_secs / 60, total_secs % 60)
}

fn load_texture_from_bytes(ctx: &egui::Context, name: &str, bytes: &[u8]) -> Option<TextureHandle> {
    let dyn_img = image::load_from_memory(bytes).ok()?;
    let resized = if dyn_img.width() > 160 || dyn_img.height() > 160 {
        dyn_img.thumbnail(160, 160)
    } else {
        dyn_img
    };
    let rgba = resized.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let color_image = ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
    Some(ctx.load_texture(name, color_image, TextureOptions::LINEAR))
}

struct RustifyApp {
    audio: Option<AudioEngine>,
    media_bridge: Option<OsMediaBridge>,
    status_msg: String,
    tracks: Vec<Track>,
    playlists: Vec<Playlist>,
    active_view: ActiveView,
    creating_playlist: bool,
    new_playlist_name: String,
    renaming_playlist: bool,
    rename_playlist_buf: String,
    selected_track_path: Option<PathBuf>,
    cover_texture: Option<TextureHandle>,
    playlist_tex_cache: HashMap<String, TextureHandle>,
    search_query: String,
    sort_col: SortColumn,
    sort_asc: bool,
    tag_drawer_open: bool,
    tag_editor: TagEditorState,
    muted: bool,
    pre_mute_volume: f32,
    add_songs_modal_open: bool,
    add_songs_filter: String,
    shuffle: bool,
    repeat_mode: RepeatMode,
    focus_search_next_frame: bool,
    metrics: MetricsData,
    session_seconds_listened: u64,
    last_counted_play_path: Option<PathBuf>,
    last_tick_second: u64,
    cached_ram_mb: f32,
}

impl RustifyApp {
    fn new() -> Self {
        let tracks = storage::load_library();
        let playlists = storage::load_playlists();
        let metrics = storage::load_metrics();
        let (audio, status_msg) = match AudioEngine::new() {
            Ok(engine) => (Some(engine), "Audio engine ready".to_string()),
            Err(err) => (None, err),
        };
        storage::trim_working_set();
        let cached_ram_mb = storage::current_process_ram_mb();

        Self {
            audio,
            media_bridge: None,
            status_msg,
            tracks,
            playlists,
            active_view: ActiveView::AllSongs,
            creating_playlist: false,
            new_playlist_name: String::new(),
            renaming_playlist: false,
            rename_playlist_buf: String::new(),
            selected_track_path: None,
            cover_texture: None,
            playlist_tex_cache: HashMap::new(),
            search_query: String::new(),
            sort_col: SortColumn::DefaultOrder,
            sort_asc: true,
            tag_drawer_open: false,
            tag_editor: TagEditorState::default(),
            muted: false,
            pre_mute_volume: 0.8,
            add_songs_modal_open: false,
            add_songs_filter: String::new(),
            shuffle: false,
            repeat_mode: RepeatMode::Off,
            focus_search_next_frame: false,
            metrics,
            session_seconds_listened: 0,
            last_counted_play_path: None,
            last_tick_second: 0,
            cached_ram_mb,
        }
    }

    fn reload_cover_texture(&mut self, ctx: &egui::Context, path: &Path) {
        self.cover_texture = None;
        if let Some(bytes) = library::extract_cover_bytes(path) {
            self.cover_texture = load_texture_from_bytes(ctx, "active_cover", &bytes);
        }
    }

    fn select_track_only(&mut self, ctx: &egui::Context, path: &Path) {
        if let Some(track) = self.tracks.iter().find(|t| t.path == path).cloned() {
            self.selected_track_path = Some(track.path.clone());
            self.tag_editor.load_from_track(&track);
            self.reload_cover_texture(ctx, &track.path);
            storage::trim_working_set();
        }
    }

    fn play_track_by_path(&mut self, ctx: &egui::Context, path: &Path) {
        let Some(track) = self.tracks.iter().find(|t| t.path == path).cloned() else {
            return;
        };

        self.selected_track_path = Some(track.path.clone());
        self.last_counted_play_path = None;
        self.tag_editor.load_from_track(&track);
        self.reload_cover_texture(ctx, &track.path);
        storage::trim_working_set();

        if let Some(bridge) = &mut self.media_bridge {
            bridge.set_metadata(&track);
        }

        if let Some(audio) = &mut self.audio {
            match audio.play_file(&track.path) {
                Ok(()) => self.status_msg = format!("Playing: {}", track.title),
                Err(e) => self.status_msg = e,
            }
        }
    }

    fn visible_track_paths(&self) -> Vec<PathBuf> {
        let raw_paths: Vec<PathBuf> = match self.active_view {
            ActiveView::AllSongs | ActiveView::Stats => {
                self.tracks.iter().map(|t| t.path.clone()).collect()
            }
            ActiveView::Playlist(p_idx) => self
                .playlists
                .get(p_idx)
                .map(|p| p.track_paths.clone())
                .unwrap_or_default(),
        };

        if self.sort_col == SortColumn::DefaultOrder {
            return raw_paths;
        }

        let mut resolved: Vec<Track> = raw_paths
            .iter()
            .filter_map(|p| self.tracks.iter().find(|t| &t.path == p).cloned())
            .collect();

        resolved.sort_by(|a, b| {
            let ord = match self.sort_col {
                SortColumn::DefaultOrder => std::cmp::Ordering::Equal,
                SortColumn::Title => a.title.to_lowercase().cmp(&b.title.to_lowercase()),
                SortColumn::Artist => a.artist.to_lowercase().cmp(&b.artist.to_lowercase()),
                SortColumn::Album => a.album.to_lowercase().cmp(&b.album.to_lowercase()),
                SortColumn::Duration => a.duration_secs.cmp(&b.duration_secs),
            };
            if self.sort_asc { ord } else { ord.reverse() }
        });

        resolved.into_iter().map(|t| t.path).collect()
    }

    fn toggle_sort(&mut self, col: SortColumn) {
        if self.sort_col == col {
            if self.sort_asc {
                self.sort_asc = false;
            } else {
                self.sort_col = SortColumn::DefaultOrder;
                self.sort_asc = true;
            }
        } else {
            self.sort_col = col;
            self.sort_asc = true;
        }
    }

    fn sort_header_label(&self, title: &str, col: SortColumn) -> String {
        if self.sort_col == col {
            if self.sort_asc {
                format!("{title} ^")
            } else {
                format!("{title} v")
            }
        } else {
            title.to_string()
        }
    }

    fn pick_random_index(&self, len: usize, exclude: Option<usize>) -> usize {
        if len <= 1 {
            return 0;
        }
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as usize)
            .unwrap_or(0);
        let mut idx = nanos % len;
        if Some(idx) == exclude {
            idx = (idx + 1) % len;
        }
        idx
    }

    fn advance_track(&mut self, ctx: &egui::Context, auto_advance: bool) {
        let queue = self.visible_track_paths();
        if queue.is_empty() {
            return;
        }

        if auto_advance && self.repeat_mode == RepeatMode::One {
            if let Some(current) = self.selected_track_path.clone() {
                self.play_track_by_path(ctx, &current);
                return;
            }
        }

        let current_pos = self
            .selected_track_path
            .as_ref()
            .and_then(|p| queue.iter().position(|q| q == p));

        if self.shuffle {
            let next_idx = self.pick_random_index(queue.len(), current_pos);
            self.play_track_by_path(ctx, &queue[next_idx]);
            return;
        }

        match current_pos {
            Some(pos) if pos + 1 < queue.len() => {
                self.play_track_by_path(ctx, &queue[pos + 1]);
            }
            Some(_) | None => {
                if self.repeat_mode == RepeatMode::All || !auto_advance {
                    self.play_track_by_path(ctx, &queue[0]);
                } else if let Some(audio) = &mut self.audio {
                    audio.stop();
                }
            }
        }
    }

    fn play_previous(&mut self, ctx: &egui::Context) {
        let queue = self.visible_track_paths();
        if queue.is_empty() {
            return;
        }

        let elapsed_secs = self.audio.as_ref().map(|a| a.position().as_secs()).unwrap_or(0);
        if elapsed_secs > 3 {
            if let Some(audio) = &mut self.audio {
                audio.seek(Duration::ZERO);
            }
            return;
        }

        let current_pos = self
            .selected_track_path
            .as_ref()
            .and_then(|p| queue.iter().position(|q| q == p));

        if self.shuffle {
            let prev_idx = self.pick_random_index(queue.len(), current_pos);
            self.play_track_by_path(ctx, &queue[prev_idx]);
            return;
        }

        let prev_idx = match current_pos {
            Some(0) => {
                if self.repeat_mode == RepeatMode::All {
                    queue.len() - 1
                } else {
                    0
                }
            }
            Some(pos) => pos - 1,
            None => 0,
        };
        self.play_track_by_path(ctx, &queue[prev_idx]);
    }

    fn import_audio_files(&mut self, paths: Vec<PathBuf>, target_playlist: Option<usize>) {
        let mut added_to_lib = 0;
        let mut added_to_pl = 0;

        for path in paths {
            if !self.tracks.iter().any(|t| t.path == path) {
                let track = library::read_track_metadata(&path);
                self.tracks.push(track);
                added_to_lib += 1;
            }

            if let Some(p_idx) = target_playlist {
                if let Some(pl) = self.playlists.get_mut(p_idx) {
                    if !pl.track_paths.contains(&path) {
                        pl.track_paths.push(path);
                        added_to_pl += 1;
                    }
                }
            }
        }

        if added_to_lib > 0 {
            storage::save_library(&self.tracks);
        }
        if added_to_pl > 0 {
            storage::save_playlists(&self.playlists);
        }
        if added_to_lib > 0 || added_to_pl > 0 {
            self.playlist_tex_cache.clear();
            storage::trim_working_set();
            self.status_msg = match target_playlist {
                Some(p_idx) => {
                    let name = self
                        .playlists
                        .get(p_idx)
                        .map(|p| p.name.as_str())
                        .unwrap_or("playlist");
                    format!("Added {} track(s) to '{}'", added_to_pl, name)
                }
                None => format!("Imported {} track(s)", added_to_lib),
            };
        }
    }

    fn remove_track_from_library(&mut self, path: &Path) {
        if self.selected_track_path.as_deref() == Some(path) {
            if let Some(audio) = &mut self.audio {
                audio.stop();
            }
            self.selected_track_path = None;
            self.cover_texture = None;
        }

        self.tracks.retain(|t| t.path != path);
        storage::save_library(&self.tracks);

        let mut playlists_changed = false;
        for pl in &mut self.playlists {
            let before = pl.track_paths.len();
            pl.track_paths.retain(|p| p != path);
            if pl.track_paths.len() != before {
                playlists_changed = true;
            }
        }
        if playlists_changed {
            storage::save_playlists(&self.playlists);
            self.playlist_tex_cache.clear();
        }
        storage::trim_working_set();
        self.status_msg = "Removed track from library (file kept on disk)".to_string();
    }

    fn save_edited_tags(&mut self, ctx: &egui::Context) {
        let Some(target_path) = self.selected_track_path.clone() else {
            return;
        };
        let Some(idx) = self.tracks.iter().position(|t| t.path == target_path) else {
            return;
        };

        let parsed_year = self.tag_editor.year.trim().parse::<u32>().ok();

        let was_playing = self
            .audio
            .as_ref()
            .map(|a| a.state == PlaybackState::Playing)
            .unwrap_or(false);
        if let Some(audio) = &mut self.audio {
            audio.stop();
        }

        match library::save_track_metadata(
            &target_path,
            &self.tag_editor.title,
            &self.tag_editor.artist,
            &self.tag_editor.album,
            parsed_year,
            self.tag_editor.pending_cover_path.as_deref(),
        ) {
            Ok(()) => {
                let updated = library::read_track_metadata(&target_path);
                self.tracks[idx] = updated.clone();
                storage::save_library(&self.tracks);
                self.playlist_tex_cache.clear();

                self.tag_editor.load_from_track(&updated);
                self.tag_editor.feedback_msg = "Tags & cover saved to file!".to_string();
                self.reload_cover_texture(ctx, &updated.path);
                storage::trim_working_set();

                if let Some(bridge) = &mut self.media_bridge {
                    bridge.set_metadata(&updated);
                }

                if was_playing {
                    if let Some(audio) = &mut self.audio {
                        let _ = audio.play_file(&updated.path);
                    }
                }
            }
            Err(err) => {
                self.tag_editor.feedback_msg = format!("Error: {err}");
            }
        }
    }

    fn get_or_load_image_file(&mut self, ctx: &egui::Context, path: &Path) -> Option<TextureHandle> {
        let key = format!("file:{}", path.display());
        if let Some(tex) = self.playlist_tex_cache.get(&key) {
            return Some(tex.clone());
        }
        if self.playlist_tex_cache.len() >= 16 {
            self.playlist_tex_cache.clear();
        }
        let bytes = std::fs::read(path).ok()?;
        let tex = load_texture_from_bytes(ctx, &key, &bytes)?;
        self.playlist_tex_cache.insert(key, tex.clone());
        Some(tex)
    }

    fn get_or_load_track_art(&mut self, ctx: &egui::Context, path: &Path) -> Option<TextureHandle> {
        let key = format!("track:{}", path.display());
        if let Some(tex) = self.playlist_tex_cache.get(&key) {
            return Some(tex.clone());
        }
        if self.playlist_tex_cache.len() >= 16 {
            self.playlist_tex_cache.clear();
        }
        let bytes = library::extract_cover_bytes(path)?;
        let tex = load_texture_from_bytes(ctx, &key, &bytes)?;
        self.playlist_tex_cache.insert(key, tex.clone());
        Some(tex)
    }

    fn resolve_playlist_thumb(&mut self, ctx: &egui::Context, pl: &Playlist) -> Option<TextureHandle> {
        if let Some(custom_path) = &pl.cover_path {
            if let Some(tex) = self.get_or_load_image_file(ctx, custom_path) {
                return Some(tex);
            }
        }
        for t_path in &pl.track_paths {
            if let Some(tex) = self.get_or_load_track_art(ctx, t_path) {
                return Some(tex);
            }
        }
        None
    }

    fn render_sidebar_playlists(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        let count = self.playlists.len();
        let row_w = ui.available_width().min(256.0);
        let cover_size = ((row_w - 105.0) * 0.55).clamp(36.0, 56.0);

        for p_idx in 0..count {
            let pl = self.playlists[p_idx].clone();
            let selected = self.active_view == ActiveView::Playlist(p_idx);
            let thumb = self.resolve_playlist_thumb(ctx, &pl);

            let row_height = if thumb.is_some() { cover_size + 10.0 } else { 26.0 };
            let (rect, response) =
                ui.allocate_exact_size(Vec2::new(row_w, row_height), Sense::click());

            let bg = if selected {
                Color32::from_rgb(32, 46, 40)
            } else if response.hovered() {
                Color32::from_rgb(46, 50, 72)
            } else {
                Color32::TRANSPARENT
            };

            if bg != Color32::TRANSPARENT {
                ui.painter().rect_filled(rect, 3.0_f32, bg);
            }
            if selected {
                ui.painter().rect_stroke(
                    rect,
                    3.0_f32,
                    Stroke::new(1.0_f32, SPOTIFY_EMERALD),
                    egui::StrokeKind::Inside,
                );
            }

            let mut text_left = rect.left() + 8.0;

            if let Some(tex) = thumb {
                let img_rect = Rect::from_min_size(
                    egui::pos2(rect.left() + 5.0, rect.center().y - cover_size * 0.5),
                    Vec2::new(cover_size, cover_size),
                );
                ui.painter().image(
                    tex.id(),
                    img_rect,
                    Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
                ui::paint_bevel(ui.painter(), img_rect, false);
                text_left = img_rect.right() + 10.0;
            }

            let title_color = if selected {
                SPOTIFY_EMERALD
            } else if response.hovered() {
                Color32::WHITE
            } else {
                Color32::LIGHT_GRAY
            };

            if cover_size >= 48.0 && rect.height() > 40.0 {
                ui.painter().text(
                    egui::pos2(text_left, rect.center().y - 8.0),
                    egui::Align2::LEFT_CENTER,
                    &pl.name,
                    egui::FontId::proportional(14.0),
                    title_color,
                );
                ui.painter().text(
                    egui::pos2(text_left, rect.center().y + 9.0),
                    egui::Align2::LEFT_CENTER,
                    format!("{} tracks", pl.track_paths.len()),
                    egui::FontId::monospace(11.0),
                    Color32::GRAY,
                );
            } else {
                let label_text = format!("{} ({})", pl.name, pl.track_paths.len());
                ui.painter().text(
                    egui::pos2(text_left, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    label_text,
                    egui::FontId::proportional(13.5),
                    title_color,
                );
            }

            if response.clicked() {
                self.active_view = ActiveView::Playlist(p_idx);
                self.renaming_playlist = false;
            }
            ui.add_space(4.0);
        }
    }

    fn render_playlist_banner(&mut self, ctx: &egui::Context, ui: &mut egui::Ui, p_idx: usize) {
        let Some(playlist) = self.playlists.get(p_idx).cloned() else {
            return;
        };

        let custom_tex = playlist
            .cover_path
            .as_ref()
            .and_then(|p| self.get_or_load_image_file(ctx, p));

        let mut collage_textures: Vec<TextureHandle> = Vec::new();
        if custom_tex.is_none() {
            for t_path in &playlist.track_paths {
                if collage_textures.len() >= 4 {
                    break;
                }
                if let Some(tex) = self.get_or_load_track_art(ctx, t_path) {
                    if !collage_textures.iter().any(|existing| existing.id() == tex.id()) {
                        collage_textures.push(tex);
                    }
                }
            }
        }

        let resp = egui::Frame::new()
            .fill(SLATE_PANEL)
            .inner_margin(14.0)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    if let Some(tex) = custom_tex {
                        ui.image((tex.id(), Vec2::new(104.0, 104.0)));
                    } else if collage_textures.len() >= 4 {
                        egui::Grid::new("playlist_collage_2x2")
                            .spacing([2.0, 2.0])
                            .show(ui, |ui| {
                                ui.image((collage_textures[0].id(), Vec2::new(51.0, 51.0)));
                                ui.image((collage_textures[1].id(), Vec2::new(51.0, 51.0)));
                                ui.end_row();
                                ui.image((collage_textures[2].id(), Vec2::new(51.0, 51.0)));
                                ui.image((collage_textures[3].id(), Vec2::new(51.0, 51.0)));
                                ui.end_row();
                            });
                    } else if let Some(first_tex) = collage_textures.first() {
                        ui.image((first_tex.id(), Vec2::new(104.0, 104.0)));
                    } else {
                        let (rect, _) = ui.allocate_exact_size(Vec2::new(104.0, 104.0), Sense::hover());
                        ui.painter().rect_filled(rect, 1.0_f32, Color32::BLACK);
                        ui.painter().rect_stroke(
                            rect,
                            1.0_f32,
                            Stroke::new(1.0_f32, WINAMP_CYAN),
                            egui::StrokeKind::Inside,
                        );
                        ui.painter().text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "PLAYLIST",
                            egui::FontId::monospace(12.0),
                            PHOSPHOR_GREEN,
                        );
                    }

                    ui.add_space(16.0);

                    ui.vertical(|ui| {
                        ui.label(RichText::new("CUSTOM PLAYLIST").monospace().small().color(WINAMP_CYAN));

                        if self.renaming_playlist {
                            ui.horizontal(|ui| {
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.rename_playlist_buf)
                                        .desired_width(220.0),
                                );
                                if ui::hardware_button(ui, "SAVE", Vec2::new(54.0, 24.0), true, SPOTIFY_EMERALD).clicked() {
                                    let trimmed = self.rename_playlist_buf.trim().to_string();
                                    if !trimmed.is_empty() {
                                        if let Some(pl) = self.playlists.get_mut(p_idx) {
                                            pl.name = trimmed;
                                        }
                                        storage::save_playlists(&self.playlists);
                                    }
                                    self.renaming_playlist = false;
                                }
                                if ui::hardware_button(ui, "CANCEL", Vec2::new(62.0, 24.0), false, Color32::GRAY).clicked() {
                                    self.renaming_playlist = false;
                                }
                            });
                        } else {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(&playlist.name).size(26.0).strong().color(Color32::WHITE));
                                ui.add_space(8.0);
                                if ui::hardware_button(ui, "RENAME", Vec2::new(64.0, 22.0), false, Color32::LIGHT_GRAY).clicked() {
                                    self.rename_playlist_buf = playlist.name.clone();
                                    self.renaming_playlist = true;
                                }
                            });
                        }

                        ui.label(
                            RichText::new(format!("{} tracks", playlist.track_paths.len()))
                                .small()
                                .color(Color32::LIGHT_GRAY),
                        );
                        ui.add_space(8.0);

                        ui.horizontal_wrapped(|ui| {
                            if ui::hardware_button(
                                ui,
                                "+ ADD SONGS",
                                Vec2::new(98.0, 24.0),
                                self.add_songs_modal_open,
                                SPOTIFY_EMERALD,
                            )
                            .clicked()
                            {
                                self.add_songs_modal_open = !self.add_songs_modal_open;
                                self.add_songs_filter.clear();
                            }

                            if ui::hardware_button(
                                ui,
                                "+ FOLDER",
                                Vec2::new(82.0, 24.0),
                                false,
                                WINAMP_CYAN,
                            )
                            .clicked()
                            {
                                if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                                    let scanned = library::scan_folder(&folder);
                                    let paths: Vec<PathBuf> =
                                        scanned.into_iter().map(|t| t.path).collect();
                                    self.import_audio_files(paths, Some(p_idx));
                                }
                            }

                            if ui::hardware_button(
                                ui,
                                "+ FILES",
                                Vec2::new(72.0, 24.0),
                                false,
                                SPOTIFY_EMERALD,
                            )
                            .clicked()
                            {
                                if let Some(files) = rfd::FileDialog::new()
                                    .add_filter(
                                        "Audio Files",
                                        &["mp3", "flac", "wav", "ogg", "m4a", "aac"],
                                    )
                                    .pick_files()
                                {
                                    self.import_audio_files(files, Some(p_idx));
                                }
                            }

                            if ui::hardware_button(ui, "SET COVER", Vec2::new(88.0, 24.0), false, WINAMP_CYAN).clicked() {
                                if let Some(img) = rfd::FileDialog::new()
                                    .add_filter("Cover Image", &["png", "jpg", "jpeg"])
                                    .pick_file()
                                {
                                    if let Ok(saved_path) = storage::import_playlist_cover(&img) {
                                        if let Some(pl) = self.playlists.get_mut(p_idx) {
                                            pl.cover_path = Some(saved_path);
                                        }
                                        storage::save_playlists(&self.playlists);
                                        self.playlist_tex_cache.clear();
                                        storage::trim_working_set();
                                    }
                                }
                            }

                            if ui::hardware_button(
                                ui,
                                "DELETE PLAYLIST",
                                Vec2::new(124.0, 24.0),
                                false,
                                Color32::from_rgb(255, 110, 110),
                            )
                            .clicked()
                            {
                                self.playlists.remove(p_idx);
                                storage::save_playlists(&self.playlists);
                                self.active_view = ActiveView::AllSongs;
                                self.add_songs_modal_open = false;
                                self.renaming_playlist = false;
                            }
                        });
                    });
                });
            });
        ui::paint_bevel(ui.painter(), resp.response.rect, false);
        ui.add_space(10.0);
    }

    fn render_stats_view(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("OFFLINE LISTENING METRICS")
                    .monospace()
                    .strong()
                    .size(20.0)
                    .color(PHOSPHOR_GREEN),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui::hardware_button(
                    ui,
                    "RESET STATS",
                    Vec2::new(96.0, 24.0),
                    false,
                    Color32::from_rgb(255, 110, 110),
                )
                .clicked()
                {
                    self.metrics = MetricsData::default();
                    self.session_seconds_listened = 0;
                    self.last_counted_play_path = None;
                    storage::save_metrics(&self.metrics);
                }
            });
        });
        ui.separator();
        ui.add_space(6.0);

        let total_mins = self.metrics.total_seconds_listened / 60;
        let session_mins = self.session_seconds_listened / 60;
        let session_secs = self.session_seconds_listened % 60;

        ui.columns(4, |cols| {
            let cards = [
                ("TOTAL PLAYS", format!("{}", self.metrics.total_plays), SPOTIFY_EMERALD),
                ("ALL-TIME LISTENED", format!("{} min", total_mins), WINAMP_CYAN),
                (
                    "THIS SESSION",
                    format!("{:02}m {:02}s", session_mins, session_secs),
                    PHOSPHOR_GREEN,
                ),
                (
                    "ENGINE MEMORY",
                    format!("{:.1} MB", self.cached_ram_mb),
                    Color32::WHITE,
                ),
            ];
            for (i, (label, val, accent)) in cards.iter().enumerate() {
                let card = egui::Frame::new()
                    .fill(SLATE_PANEL)
                    .inner_margin(12.0)
                    .show(&mut cols[i], |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(RichText::new(*label).monospace().small().color(Color32::GRAY));
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(val)
                                .monospace()
                                .strong()
                                .size(20.0)
                                .color(*accent),
                        );
                    });
                ui::paint_bevel(cols[i].painter(), card.response.rect, false);
            }
        });

        ui.add_space(16.0);

        let mut ranked_tracks: Vec<(Track, u64, u64)> = self
            .tracks
            .iter()
            .filter_map(|t| {
                let m = self.metrics.per_track.get(&t.path)?;
                if m.play_count > 0 || m.seconds_listened > 0 {
                    Some((t.clone(), m.play_count, m.seconds_listened))
                } else {
                    None
                }
            })
            .collect();
        ranked_tracks.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| b.2.cmp(&a.2)));

        let mut artist_map: HashMap<String, (u64, u64)> = HashMap::new();
        for (track, plays, secs) in &ranked_tracks {
            let entry = artist_map.entry(track.artist.clone()).or_insert((0, 0));
            entry.0 += plays;
            entry.1 += secs;
        }
        let mut ranked_artists: Vec<(String, u64, u64)> = artist_map
            .into_iter()
            .map(|(a, (p, s))| (a, p, s))
            .collect();
        ranked_artists.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| b.2.cmp(&a.2)));

        let mut play_clicked: Option<PathBuf> = None;

        ui.columns(2, |cols| {
            // Left: Top Songs
            let left_frame = egui::Frame::new()
                .fill(SLATE_PANEL)
                .inner_margin(12.0)
                .show(&mut cols[0], |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        RichText::new("TOP TRACKS")
                            .monospace()
                            .strong()
                            .color(SPOTIFY_EMERALD),
                    );
                    ui.separator();
                    if ranked_tracks.is_empty() {
                        ui.label(
                            RichText::new("Play a song for 10+ seconds to record plays.")
                                .color(Color32::GRAY),
                        );
                    } else {
                        for (idx, (track, plays, secs)) in ranked_tracks.iter().take(10).enumerate()
                        {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(format!("#{}", idx + 1))
                                        .monospace()
                                        .strong()
                                        .color(WINAMP_CYAN),
                                );
                                if ui
                                    .add(
                                        egui::Label::new(
                                            RichText::new(&track.title).color(Color32::WHITE),
                                        )
                                        .truncate()
                                        .sense(Sense::click()),
                                    )
                                    .clicked()
                                {
                                    play_clicked = Some(track.path.clone());
                                }
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label(
                                            RichText::new(format!(
                                                "{} plays · {}m",
                                                plays,
                                                secs / 60
                                            ))
                                            .monospace()
                                            .small()
                                            .color(PHOSPHOR_GREEN),
                                        );
                                    },
                                );
                            });
                            ui.add_space(4.0);
                        }
                    }
                });
            ui::paint_bevel(cols[0].painter(), left_frame.response.rect, false);

            // Right: Top Artists
            let right_frame = egui::Frame::new()
                .fill(SLATE_PANEL)
                .inner_margin(12.0)
                .show(&mut cols[1], |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        RichText::new("TOP ARTISTS")
                            .monospace()
                            .strong()
                            .color(WINAMP_CYAN),
                    );
                    ui.separator();
                    if ranked_artists.is_empty() {
                        ui.label(
                            RichText::new("No artist metrics recorded yet.").color(Color32::GRAY),
                        );
                    } else {
                        for (idx, (artist, plays, secs)) in
                            ranked_artists.iter().take(10).enumerate()
                        {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(format!("#{}", idx + 1))
                                        .monospace()
                                        .strong()
                                        .color(SPOTIFY_EMERALD),
                                );
                                ui.add(
                                    egui::Label::new(RichText::new(artist).color(Color32::WHITE))
                                        .truncate(),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label(
                                            RichText::new(format!(
                                                "{} plays · {}m",
                                                plays,
                                                secs / 60
                                            ))
                                            .monospace()
                                            .small()
                                            .color(Color32::LIGHT_GRAY),
                                        );
                                    },
                                );
                            });
                            ui.add_space(4.0);
                        }
                    }
                });
            ui::paint_bevel(cols[1].painter(), right_frame.response.rect, false);
        });

        if let Some(path) = play_clicked {
            self.play_track_by_path(ctx, &path);
        }
    }
}

impl eframe::App for RustifyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.style_mut(|style| {
            style.visuals.selection.bg_fill = Color32::from_rgb(32, 46, 40);
            style.visuals.selection.stroke = Stroke::new(1.0_f32, SPOTIFY_EMERALD);
        });

        // Poll Bluetooth / wired earphone & Windows SMTC media transport commands
        let pending_media_cmds = self
            .media_bridge
            .as_ref()
            .map(|b| b.poll())
            .unwrap_or_default();

        for cmd in pending_media_cmds {
            match cmd {
                HardwareMediaCmd::Play => {
                    if let Some(audio) = &mut self.audio {
                        if audio.state == PlaybackState::Stopped {
                            self.advance_track(ctx, false);
                        } else if audio.state == PlaybackState::Paused {
                            audio.toggle_play_pause();
                        }
                    }
                }
                HardwareMediaCmd::Pause => {
                    if let Some(audio) = &mut self.audio {
                        if audio.state == PlaybackState::Playing {
                            audio.toggle_play_pause();
                        }
                    }
                }
                HardwareMediaCmd::Toggle => {
                    let state = self
                        .audio
                        .as_ref()
                        .map(|a| a.state)
                        .unwrap_or(PlaybackState::Stopped);
                    if state == PlaybackState::Stopped {
                        self.advance_track(ctx, false);
                    } else if let Some(audio) = &mut self.audio {
                        audio.toggle_play_pause();
                    }
                }
                HardwareMediaCmd::Next => self.advance_track(ctx, false),
                HardwareMediaCmd::Previous => self.play_previous(ctx),
                HardwareMediaCmd::Stop => {
                    if let Some(audio) = &mut self.audio {
                        audio.stop();
                    }
                }
            }
        }

        let finished = self.audio.as_ref().map(|a| a.is_finished()).unwrap_or(false);
        if finished {
            self.advance_track(ctx, true);
        }

        let typing_in_text_field = ctx.wants_keyboard_input();
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, Key::F)) {
            self.focus_search_next_frame = true;
        }

        if !typing_in_text_field {
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Space)) {
                let state = self.audio.as_ref().map(|a| a.state).unwrap_or(PlaybackState::Stopped);
                if state == PlaybackState::Stopped {
                    self.advance_track(ctx, false);
                } else if let Some(audio) = &mut self.audio {
                    audio.toggle_play_pause();
                }
            }

            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::ArrowRight)) {
                if let Some(audio) = &mut self.audio {
                    let next_pos = audio.position() + Duration::from_secs(5);
                    audio.seek(next_pos);
                }
            }

            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::ArrowLeft)) {
                if let Some(audio) = &mut self.audio {
                    let prev_pos = audio.position().saturating_sub(Duration::from_secs(5));
                    audio.seek(prev_pos);
                }
            }

            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::ArrowUp)) {
                if let Some(audio) = &mut self.audio {
                    let vol = (audio.volume + 0.05).min(1.0);
                    audio.set_volume(vol);
                    self.muted = false;
                }
            }

            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::ArrowDown)) {
                if let Some(audio) = &mut self.audio {
                    let vol = (audio.volume - 0.05).max(0.0);
                    audio.set_volume(vol);
                    self.muted = vol == 0.0;
                }
            }
        }

        let playback_state = self
            .audio
            .as_ref()
            .map(|a| a.state)
            .unwrap_or(PlaybackState::Stopped);

        if playback_state == PlaybackState::Playing {
            ctx.request_repaint_after(Duration::from_millis(60));
        } else if playback_state == PlaybackState::Paused {
            ctx.request_repaint_after(Duration::from_millis(250));
        }

        let elapsed = self
            .audio
            .as_ref()
            .map(|a| a.position())
            .unwrap_or(Duration::ZERO);
        let elapsed_secs = elapsed.as_secs();

        if let Some(bridge) = &mut self.media_bridge {
            bridge.set_playback(playback_state, elapsed);
        }

        // Accumulate listening seconds and count a play after 10 seconds of active playback
        if playback_state == PlaybackState::Playing && elapsed_secs != self.last_tick_second {
            self.last_tick_second = elapsed_secs;
            self.cached_ram_mb = storage::current_process_ram_mb();

            if let Some(active_path) = &self.selected_track_path {
                self.session_seconds_listened += 1;
                self.metrics.total_seconds_listened += 1;

                let entry = self
                    .metrics
                    .per_track
                    .entry(active_path.clone())
                    .or_default();
                entry.seconds_listened += 1;

                if elapsed_secs >= 10 && self.last_counted_play_path.as_ref() != Some(active_path) {
                    entry.play_count += 1;
                    self.metrics.total_plays += 1;
                    self.last_counted_play_path = Some(active_path.clone());
                    storage::save_metrics(&self.metrics);
                } else if self.metrics.total_seconds_listened % 15 == 0 {
                    storage::save_metrics(&self.metrics);
                }
            }
        }

        let active_track = self
            .selected_track_path
            .as_ref()
            .and_then(|p| self.tracks.iter().find(|t| &t.path == p))
            .cloned();
        let total_secs = active_track.as_ref().map(|t| t.duration_secs).unwrap_or(0);

        // 1. BOTTOM NOW-PLAYING DECK (Left Track Info | Centered Transport Controls | Right Volume & TAG)
        let bottom_resp = egui::TopBottomPanel::bottom("now_playing_bar")
            .exact_height(92.0)
            .frame(egui::Frame::new().fill(SLATE_PANEL).inner_margin(12.0))
            .show(ctx, |ui| {
                ui.columns(3, |cols| {
                    // Column 0 (Left): Album Art + Active Track Metadata
                    cols[0].horizontal(|ui| {
                        if let Some(tex) = &self.cover_texture {
                            let resp = ui.image((tex.id(), Vec2::new(62.0, 62.0)));
                            ui::paint_bevel(ui.painter(), resp.rect, false);
                        } else {
                            let (rect, _) = ui.allocate_exact_size(Vec2::new(62.0, 62.0), Sense::hover());
                            ui.painter().rect_filled(rect, 1.0_f32, Color32::BLACK);
                            ui::paint_bevel(ui.painter(), rect, true);
                            ui.painter().text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                "DISC",
                                egui::FontId::monospace(12.0),
                                PHOSPHOR_GREEN,
                            );
                        }

                        ui.add_space(10.0);

                        ui.vertical(|ui| {
                            if let Some(track) = &active_track {
                                ui.add(egui::Label::new(RichText::new(&track.title).strong().color(Color32::WHITE)).truncate());
                                ui.add(egui::Label::new(RichText::new(&track.artist).small().color(Color32::LIGHT_GRAY)).truncate());
                                let ext = track
                                    .path
                                    .extension()
                                    .and_then(|e| e.to_str())
                                    .unwrap_or("AUDIO")
                                    .to_uppercase();
                                let khz = track.sample_rate_hz.unwrap_or(44100) as f32 / 1000.0;
                                ui.label(
                                    RichText::new(format!("[{} · {:.1} kHz]", ext, khz))
                                        .monospace()
                                        .small()
                                        .color(WINAMP_CYAN),
                                );
                            } else {
                                ui.label(RichText::new("No Track Selected").strong().color(Color32::WHITE));
                                ui.label(RichText::new(&self.status_msg).small().color(Color32::GRAY));
                            }
                        });
                    });

                    // Column 1 (Center): Centered Transport Buttons + Emerald Progress Scrubber
                    cols[1].vertical_centered(|ui| {
                        let buttons_w = 40.0 + 36.0 + 42.0 + 36.0 + 36.0 + 62.0 + (5.0 * 4.0);
                        ui.allocate_ui_with_layout(
                            Vec2::new(buttons_w, 26.0),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                ui.spacing_mut().item_spacing.x = 4.0;

                                if ui::hardware_button(
                                    ui,
                                    "SHF",
                                    Vec2::new(40.0, 26.0),
                                    self.shuffle,
                                    WINAMP_CYAN,
                                )
                                .clicked()
                                {
                                    self.shuffle = !self.shuffle;
                                }

                                if ui::hardware_button(ui, "|<<", Vec2::new(36.0, 26.0), false, Color32::WHITE).clicked() {
                                    self.play_previous(ctx);
                                }

                                let is_playing = playback_state == PlaybackState::Playing;
                                let play_label = if is_playing { "||" } else { ">" };
                                if ui::hardware_button(ui, play_label, Vec2::new(42.0, 26.0), is_playing, SPOTIFY_EMERALD).clicked() {
                                    if playback_state == PlaybackState::Stopped {
                                        self.advance_track(ctx, false);
                                    } else if let Some(audio) = &mut self.audio {
                                        audio.toggle_play_pause();
                                    }
                                }

                                if ui::hardware_button(ui, "[]", Vec2::new(36.0, 26.0), false, Color32::WHITE).clicked() {
                                    if let Some(audio) = &mut self.audio {
                                        audio.stop();
                                    }
                                }

                                if ui::hardware_button(ui, ">>|", Vec2::new(36.0, 26.0), false, Color32::WHITE).clicked() {
                                    self.advance_track(ctx, false);
                                }

                                let rpt_active = self.repeat_mode != RepeatMode::Off;
                                let rpt_accent = if self.repeat_mode == RepeatMode::One {
                                    PHOSPHOR_GREEN
                                } else {
                                    WINAMP_CYAN
                                };
                                if ui::hardware_button(
                                    ui,
                                    self.repeat_mode.label(),
                                    Vec2::new(62.0, 26.0),
                                    rpt_active,
                                    rpt_accent,
                                )
                                .clicked()
                                {
                                    self.repeat_mode = self.repeat_mode.next();
                                }
                            },
                        );

                        ui.add_space(6.0);

                        let slider_w = (ui.available_width() - 92.0).clamp(180.0, 340.0);
                        let scrubber_total_w = slider_w + 84.0;
                        ui.allocate_ui_with_layout(
                            Vec2::new(scrubber_total_w, 18.0),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                ui.label(RichText::new(format_secs(elapsed_secs)).monospace().small().color(Color32::LIGHT_GRAY));
                                let mut pos_f32 = elapsed_secs.min(total_secs) as f32;
                                let max_f32 = total_secs.max(1) as f32;
                                if ui::emerald_slider(ui, &mut pos_f32, max_f32, slider_w, Color32::from_rgb(200, 205, 220)) {
                                    if let Some(audio) = &mut self.audio {
                                        audio.seek(Duration::from_secs(pos_f32 as u64));
                                    }
                                }
                                ui.label(RichText::new(format_secs(total_secs)).monospace().small().color(Color32::LIGHT_GRAY));
                            },
                        );
                    });

                    // Column 2 (Right): Volume Controls + TAG Drawer Toggle
                    cols[2].with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui::hardware_button(ui, "TAG", Vec2::new(52.0, 28.0), self.tag_drawer_open, PHOSPHOR_GREEN).clicked() {
                            self.tag_drawer_open = !self.tag_drawer_open;
                        }

                        ui.add_space(14.0);
                        if let Some(audio) = &mut self.audio {
                            let mut vol = audio.volume;
                            if ui::emerald_slider(ui, &mut vol, 1.0, 96.0, SPOTIFY_EMERALD) {
                                audio.set_volume(vol);
                                self.muted = vol == 0.0;
                            }
                            ui.add_space(6.0);
                            let vol_label = if self.muted || audio.volume == 0.0 { "MUTE" } else { "VOL" };
                            if ui::hardware_button(ui, vol_label, Vec2::new(42.0, 22.0), self.muted, Color32::from_rgb(255, 110, 110)).clicked() {
                                if self.muted {
                                    audio.set_volume(self.pre_mute_volume);
                                    self.muted = false;
                                } else {
                                    self.pre_mute_volume = audio.volume.max(0.2);
                                    audio.set_volume(0.0);
                                    self.muted = true;
                                }
                            }
                        }
                    });
                });
            });
        ui::paint_bevel(
            &ctx.layer_painter(egui::LayerId::background()),
            bottom_resp.response.rect,
            false,
        );

        // 2. LEFT NAVIGATION SIDEBAR
        egui::SidePanel::left("sidebar_v2")
            .resizable(true)
            .default_width(210.0)
            .width_range(175.0..=280.0)
            .frame(egui::Frame::new().fill(SLATE_PANEL).inner_margin(12.0))
            .show(ctx, |ui| {
                let content_w = ui.available_width().min(256.0);
                ui.set_max_width(content_w);

                ui.horizontal(|ui| {
                    ui.label(RichText::new("RUSTIFY").monospace().strong().size(17.0).color(PHOSPHOR_GREEN));
                    ui.label(RichText::new("// 2.9").monospace().size(13.0).color(WINAMP_CYAN));
                });
                ui.separator();

                ui.label(RichText::new("LIBRARY").small().strong().color(Color32::LIGHT_GRAY));
                ui.add_space(4.0);
                let is_all = self.active_view == ActiveView::AllSongs;
                if ui::hardware_button(
                    ui,
                    &format!("ALL SONGS ({})", self.tracks.len()),
                    Vec2::new(content_w, 26.0),
                    is_all,
                    SPOTIFY_EMERALD,
                )
                .clicked()
                {
                    self.active_view = ActiveView::AllSongs;
                    self.search_query.clear();
                    self.renaming_playlist = false;
                }

                ui.add_space(4.0);
                let is_stats = self.active_view == ActiveView::Stats;
                if ui::hardware_button(
                    ui,
                    "LISTENING STATS",
                    Vec2::new(content_w, 26.0),
                    is_stats,
                    PHOSPHOR_GREEN,
                )
                .clicked()
                {
                    self.active_view = ActiveView::Stats;
                    self.renaming_playlist = false;
                }

                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    let half_w = (content_w - 4.0) * 0.5;
                    if ui::hardware_button(
                        ui,
                        "+ FOLDER",
                        Vec2::new(half_w, 26.0),
                        false,
                        WINAMP_CYAN,
                    )
                    .clicked()
                    {
                        if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                            let scanned = library::scan_folder(&folder);
                            let paths: Vec<PathBuf> = scanned.into_iter().map(|t| t.path).collect();
                            self.import_audio_files(paths, None);
                        }
                    }

                    if ui::hardware_button(
                        ui,
                        "+ FILES",
                        Vec2::new(half_w, 26.0),
                        false,
                        SPOTIFY_EMERALD,
                    )
                    .clicked()
                    {
                        if let Some(files) = rfd::FileDialog::new()
                            .add_filter("Audio Files", &["mp3", "flac", "wav", "ogg", "m4a", "aac"])
                            .pick_files()
                        {
                            self.import_audio_files(files, None);
                        }
                    }
                });

                ui.add_space(14.0);
                ui.label(RichText::new("PLAYLISTS").small().strong().color(Color32::LIGHT_GRAY));
                ui.add_space(4.0);

                if self.creating_playlist {
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.new_playlist_name)
                                .hint_text("Playlist name...")
                                .desired_width((content_w - 44.0).max(90.0)),
                        );
                        if ui::hardware_button(ui, "OK", Vec2::new(38.0, 22.0), true, SPOTIFY_EMERALD).clicked() {
                            let trimmed = self.new_playlist_name.trim().to_string();
                            if !trimmed.is_empty() {
                                self.playlists.push(Playlist {
                                    name: trimmed,
                                    track_paths: Vec::new(),
                                    cover_path: None,
                                });
                                storage::save_playlists(&self.playlists);
                                self.active_view = ActiveView::Playlist(self.playlists.len() - 1);
                            }
                            self.new_playlist_name.clear();
                            self.creating_playlist = false;
                        }
                    });
                } else if ui::hardware_button(
                    ui,
                    "+ NEW PLAYLIST",
                    Vec2::new(content_w, 24.0),
                    false,
                    Color32::WHITE,
                )
                .clicked()
                {
                    self.creating_playlist = true;
                }

                ui.add_space(6.0);
                let max_pl_height = (ui.available_height() - 115.0).max(60.0);
                egui::ScrollArea::vertical()
                    .max_height(max_pl_height)
                    .show(ui, |ui| {
                        self.render_sidebar_playlists(ctx, ui);
                    });

                // Bottom-anchored Winamp Pitch-Black LCD Screen + Live Spectrum Analyzer
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    let lcd_resp = egui::Frame::new()
                        .fill(Color32::BLACK)
                        .inner_margin(10.0)
                        .show(ui, |ui| {
                            ui.set_max_width((content_w - 20.0).max(120.0));
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(format_secs(elapsed_secs))
                                        .monospace()
                                        .size(22.0)
                                        .strong()
                                        .color(PHOSPHOR_GREEN),
                                );

                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    let kbps = active_track.as_ref().and_then(|t| t.bitrate_kbps).unwrap_or(0);
                                    let khz = active_track.as_ref().and_then(|t| t.sample_rate_hz).unwrap_or(44100) as f32 / 1000.0;
                                    ui.label(
                                        RichText::new(format!(
                                            "{:>3} kbps · {:.1} kHz\nRAM: {:.1} MB",
                                            kbps, khz, self.cached_ram_mb
                                        ))
                                        .monospace()
                                        .small()
                                        .color(WINAMP_CYAN),
                                    );
                                });
                            });
                            ui.add_space(6.0);
                            ui::draw_winamp_spectrum(ui, playback_state, elapsed.as_secs_f32());
                        });
                    ui::paint_bevel(ui.painter(), lcd_resp.response.rect, true);
                });
            });

        // 3. COLLAPSIBLE & DRAGGABLE WINAMP TAG EDITOR DRAWER
        if self.tag_drawer_open {
            egui::TopBottomPanel::bottom("tag_editor_drawer")
                .resizable(true)
                .default_height(165.0)
                .height_range(145.0..=340.0)
                .frame(egui::Frame::new().fill(SLATE_DARK).inner_margin(12.0))
                .show(ctx, |ui| {
                    let save_clicked = ui::render_tag_editor_drawer(
                        ui,
                        &mut self.tag_editor,
                        active_track.as_ref(),
                        self.cover_texture.as_ref(),
                    );
                    if save_clicked {
                        self.save_edited_tags(ctx);
                    }
                });
        }

        // 4. CENTRAL TRACK LIBRARY / PLAYLIST / STATS VIEW
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(CHARCOAL_BG).inner_margin(16.0))
            .show(ctx, |ui| {
                if self.active_view == ActiveView::Stats {
                    self.render_stats_view(ctx, ui);
                    return;
                }

                if let ActiveView::Playlist(p_idx) = self.active_view {
                    self.render_playlist_banner(ctx, ui, p_idx);
                }

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Search:").color(Color32::LIGHT_GRAY));
                    let search_resp = ui.add(
                        egui::TextEdit::singleline(&mut self.search_query)
                            .hint_text("Filter by title, artist, or album... (Ctrl+F)")
                            .desired_width(340.0),
                    );
                    if self.focus_search_next_frame {
                        search_resp.request_focus();
                        self.focus_search_next_frame = false;
                    }

                    if self.sort_col != SortColumn::DefaultOrder {
                        ui.add_space(12.0);
                        if ui::hardware_button(ui, "RESET SORT", Vec2::new(86.0, 22.0), false, WINAMP_CYAN).clicked() {
                            self.sort_col = SortColumn::DefaultOrder;
                            self.sort_asc = true;
                        }
                    }
                });
                ui.add_space(6.0);
                ui.separator();

                let visible_paths = self.visible_track_paths();

                if visible_paths.is_empty() {
                    ui.centered_and_justified(|ui| {
                        let msg = match self.active_view {
                            ActiveView::AllSongs | ActiveView::Stats => {
                                "Your library is empty.\nClick '+ FOLDER' or '+ FILES' in the sidebar to import local music."
                            }
                            ActiveView::Playlist(_) => {
                                "This playlist has no songs yet.\nClick '+ ADD SONGS', '+ FOLDER', or '+ FILES' above."
                            }
                        };
                        ui.label(RichText::new(msg).color(Color32::GRAY));
                    });
                } else {
                    let query = self.search_query.to_lowercase();
                    let filtered_tracks: Vec<Track> = visible_paths
                        .iter()
                        .filter_map(|p| self.tracks.iter().find(|t| &t.path == p).cloned())
                        .filter(|t| {
                            query.is_empty()
                                || t.title.to_lowercase().contains(&query)
                                || t.artist.to_lowercase().contains(&query)
                                || t.album.to_lowercase().contains(&query)
                        })
                        .collect();

                    let mut play_path: Option<PathBuf> = None;
                    let mut edit_path: Option<PathBuf> = None;
                    let mut remove_from_playlist_path: Option<PathBuf> = None;
                    let mut remove_from_library_path: Option<PathBuf> = None;
                    let mut add_to_playlist: Option<(usize, PathBuf)> = None;
                    let mut sort_clicked: Option<SortColumn> = None;

                    ui.visuals_mut().selection.bg_fill = Color32::from_rgb(32, 46, 40);
                    ui.visuals_mut().selection.stroke = Stroke::new(1.0_f32, SPOTIFY_EMERALD);

                    let title_hdr = self.sort_header_label("TITLE", SortColumn::Title);
                    let artist_hdr = self.sort_header_label("ARTIST", SortColumn::Artist);
                    let album_hdr = self.sort_header_label("ALBUM", SortColumn::Album);
                    let time_hdr = self.sort_header_label("TIME", SortColumn::Duration);

                    TableBuilder::new(ui)
                        .striped(true)
                        .resizable(false)
                        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                        .column(Column::exact(36.0))
                        .column(Column::remainder().at_least(220.0))
                        .column(Column::initial(160.0).at_least(110.0))
                        .column(Column::initial(160.0).at_least(110.0))
                        .column(Column::exact(68.0))
                        .column(Column::exact(104.0))
                        .header(26.0, |mut header| {
                            header.col(|ui| {
                                ui.label(RichText::new("#").monospace().strong().color(Color32::GRAY));
                            });
                            header.col(|ui| {
                                let color = if self.sort_col == SortColumn::Title { SPOTIFY_EMERALD } else { Color32::GRAY };
                                if ui.add(egui::Label::new(RichText::new(title_hdr).strong().color(color)).sense(Sense::click())).clicked() {
                                    sort_clicked = Some(SortColumn::Title);
                                }
                            });
                            header.col(|ui| {
                                let color = if self.sort_col == SortColumn::Artist { SPOTIFY_EMERALD } else { Color32::GRAY };
                                if ui.add(egui::Label::new(RichText::new(artist_hdr).strong().color(color)).sense(Sense::click())).clicked() {
                                    sort_clicked = Some(SortColumn::Artist);
                                }
                            });
                            header.col(|ui| {
                                let color = if self.sort_col == SortColumn::Album { SPOTIFY_EMERALD } else { Color32::GRAY };
                                if ui.add(egui::Label::new(RichText::new(album_hdr).strong().color(color)).sense(Sense::click())).clicked() {
                                    sort_clicked = Some(SortColumn::Album);
                                }
                            });
                            header.col(|ui| {
                                let color = if self.sort_col == SortColumn::Duration { SPOTIFY_EMERALD } else { Color32::GRAY };
                                if ui.add(egui::Label::new(RichText::new(time_hdr).monospace().strong().color(color)).sense(Sense::click())).clicked() {
                                    sort_clicked = Some(SortColumn::Duration);
                                }
                            });
                            header.col(|ui| {
                                ui.label(RichText::new("ACTION").strong().color(Color32::GRAY));
                            });
                        })
                        .body(|mut body| {
                            for (row_idx, track) in filtered_tracks.iter().enumerate() {
                                let is_selected = self.selected_track_path.as_ref() == Some(&track.path);
                                let title_color = if is_selected { SPOTIFY_EMERALD } else { Color32::WHITE };

                                body.row(30.0, |mut row| {
                                    row.set_selected(is_selected);

                                    row.col(|ui| {
                                        let idx_text = if is_selected {
                                            RichText::new(">").monospace().strong().color(SPOTIFY_EMERALD)
                                        } else {
                                            RichText::new(format!("{}", row_idx + 1)).monospace().color(Color32::GRAY)
                                        };
                                        ui.label(idx_text);
                                    });

                                    row.col(|ui| {
                                        let title_resp = ui.add(
                                            egui::Label::new(RichText::new(&track.title).color(title_color))
                                                .truncate()
                                                .sense(Sense::click()),
                                        );
                                        if title_resp.clicked() {
                                            play_path = Some(track.path.clone());
                                        }
                                        title_resp.context_menu(|ui| {
                                            if ui.button("Play Now").clicked() {
                                                play_path = Some(track.path.clone());
                                                ui.close_menu();
                                            }
                                            if ui.button("Edit Tags & Cover").clicked() {
                                                edit_path = Some(track.path.clone());
                                                ui.close_menu();
                                            }
                                            if !self.playlists.is_empty() {
                                                ui.separator();
                                                for (p_i, pl) in self.playlists.iter().enumerate() {
                                                    if ui.button(format!("Add to '{}'", pl.name)).clicked() {
                                                        add_to_playlist = Some((p_i, track.path.clone()));
                                                        ui.close_menu();
                                                    }
                                                }
                                            }
                                            ui.separator();
                                            if ui.button("Remove from Library").clicked() {
                                                remove_from_library_path = Some(track.path.clone());
                                                ui.close_menu();
                                            }
                                        });
                                    });

                                    row.col(|ui| {
                                        ui.add(egui::Label::new(RichText::new(&track.artist).color(Color32::LIGHT_GRAY)).truncate());
                                    });

                                    row.col(|ui| {
                                        ui.add(egui::Label::new(RichText::new(&track.album).color(Color32::GRAY)).truncate());
                                    });

                                    row.col(|ui| {
                                        ui.label(RichText::new(track.formatted_duration()).monospace().color(Color32::LIGHT_GRAY));
                                    });

                                    row.col(|ui| match self.active_view {
                                        ActiveView::AllSongs | ActiveView::Stats => {
                                            if !self.playlists.is_empty() {
                                                ui.menu_button(RichText::new("+ PL").monospace().small(), |ui| {
                                                    for (p_i, pl) in self.playlists.iter().enumerate() {
                                                        if ui.button(&pl.name).clicked() {
                                                            add_to_playlist = Some((p_i, track.path.clone()));
                                                            ui.close_menu();
                                                        }
                                                    }
                                                });
                                            }
                                            if ui::hardware_button(
                                                ui,
                                                "DEL",
                                                Vec2::new(36.0, 22.0),
                                                false,
                                                Color32::from_rgb(255, 110, 110),
                                            )
                                            .on_hover_text("Remove from Rustify library (keeps file on disk)")
                                            .clicked()
                                            {
                                                remove_from_library_path = Some(track.path.clone());
                                            }
                                        }
                                        ActiveView::Playlist(_) => {
                                            if ui::hardware_button(
                                                ui,
                                                "REMOVE",
                                                Vec2::new(62.0, 22.0),
                                                false,
                                                Color32::from_rgb(255, 110, 110),
                                            )
                                            .clicked()
                                            {
                                                remove_from_playlist_path = Some(track.path.clone());
                                            }
                                        }
                                    });
                                });
                            }
                        });

                    if let Some(col) = sort_clicked {
                        self.toggle_sort(col);
                    }
                    if let Some(path) = play_path {
                        self.play_track_by_path(ctx, &path);
                    }
                    if let Some(path) = edit_path {
                        self.select_track_only(ctx, &path);
                        self.tag_drawer_open = true;
                    }
                    if let Some((p_i, path)) = add_to_playlist {
                        if let Some(pl) = self.playlists.get_mut(p_i) {
                            if !pl.track_paths.contains(&path) {
                                pl.track_paths.push(path);
                                storage::save_playlists(&self.playlists);
                                self.playlist_tex_cache.clear();
                            }
                        }
                    }
                    if let Some(path) = remove_from_playlist_path {
                        if let ActiveView::Playlist(p_idx) = self.active_view {
                            if let Some(pl) = self.playlists.get_mut(p_idx) {
                                pl.track_paths.retain(|p| p != &path);
                                storage::save_playlists(&self.playlists);
                                self.playlist_tex_cache.clear();
                            }
                        }
                    }
                    if let Some(path) = remove_from_library_path {
                        self.remove_track_from_library(&path);
                    }
                }
            });

        // 5. ADD SONGS TO PLAYLIST MODAL PICKER
        if self.add_songs_modal_open {
            if let ActiveView::Playlist(p_idx) = self.active_view {
                let mut open = self.add_songs_modal_open;
                let pl_name = self
                    .playlists
                    .get(p_idx)
                    .map(|p| p.name.clone())
                    .unwrap_or_default();

                egui::Window::new(format!("ADD SONGS // {}", pl_name.to_uppercase()))
                    .open(&mut open)
                    .collapsible(false)
                    .resizable(true)
                    .default_size([460.0, 380.0])
                    .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                    .frame(
                        egui::Frame::window(&ctx.style())
                            .fill(SLATE_PANEL)
                            .stroke(Stroke::new(1.5_f32, BEVEL_LIGHT))
                            .inner_margin(12.0),
                    )
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Filter:").color(Color32::LIGHT_GRAY));
                            ui.add(
                                egui::TextEdit::singleline(&mut self.add_songs_filter)
                                    .hint_text("Search library tracks...")
                                    .desired_width(280.0),
                            );
                        });
                        ui.separator();

                        if self.tracks.is_empty() {
                            ui.label(
                                RichText::new("No songs in library. Click '+ FOLDER' or '+ FILES' first.")
                                    .color(Color32::GRAY),
                            );
                        } else {
                            let filter = self.add_songs_filter.to_lowercase();
                            let mut changed = false;

                            egui::ScrollArea::vertical().show(ui, |ui| {
                                for track in &self.tracks {
                                    if !filter.is_empty()
                                        && !track.title.to_lowercase().contains(&filter)
                                        && !track.artist.to_lowercase().contains(&filter)
                                    {
                                        continue;
                                    }

                                    let already_added = self
                                        .playlists
                                        .get(p_idx)
                                        .map(|pl| pl.track_paths.contains(&track.path))
                                        .unwrap_or(false);

                                    ui.horizontal(|ui| {
                                        let btn_label = if already_added { "ADDED" } else { "+ ADD" };
                                        let btn_color = if already_added {
                                            Color32::GRAY
                                        } else {
                                            SPOTIFY_EMERALD
                                        };

                                        if ui::hardware_button(
                                            ui,
                                            btn_label,
                                            Vec2::new(62.0, 22.0),
                                            already_added,
                                            btn_color,
                                        )
                                        .clicked()
                                        {
                                            if let Some(pl) = self.playlists.get_mut(p_idx) {
                                                if already_added {
                                                    pl.track_paths.retain(|p| p != &track.path);
                                                } else {
                                                    pl.track_paths.push(track.path.clone());
                                                }
                                                changed = true;
                                            }
                                        }

                                        ui.add_space(6.0);
                                        ui.vertical(|ui| {
                                            ui.label(
                                                RichText::new(&track.title)
                                                    .color(if already_added {
                                                        SPOTIFY_EMERALD
                                                    } else {
                                                        Color32::WHITE
                                                    }),
                                            );
                                            ui.label(
                                                RichText::new(format!(
                                                    "{} · {}",
                                                    track.artist,
                                                    track.formatted_duration()
                                                ))
                                                .small()
                                                .color(Color32::GRAY),
                                            );
                                        });
                                    });
                                    ui.separator();
                                }
                            });

                            if changed {
                                storage::save_playlists(&self.playlists);
                                self.playlist_tex_cache.clear();
                            }
                        }
                    });

                self.add_songs_modal_open = open;
            } else {
                self.add_songs_modal_open = false;
            }
        }
    }
}