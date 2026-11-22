mod audio;
mod library;
mod models;
mod storage;
mod ui;

use audio::AudioEngine;
use eframe::egui::{self, Color32, ColorImage, RichText, Stroke, TextureHandle, TextureOptions};
use models::{PlaybackState, Playlist, Track};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;
use ui::TagEditorState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActiveView {
    AllSongs,
    Playlist(usize),
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("RUSTIFY // Lightweight Audio Engine")
            .with_inner_size([1100.0, 730.0])
            .with_min_inner_size([880.0, 580.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Rustify",
        options,
        Box::new(|cc| {
            apply_hybrid_theme(&cc.egui_ctx);
            Ok(Box::new(RustifyApp::new()))
        }),
    )
}

fn apply_hybrid_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = Color32::from_rgb(18, 18, 18); // #121212
    visuals.window_fill = Color32::from_rgb(35, 37, 54); // #232536
    visuals.extreme_bg_color = Color32::from_rgb(0, 0, 0); // #000000

    visuals.widgets.inactive.bg_fill = Color32::from_rgb(48, 51, 74);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(69, 73, 106));
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(64, 68, 98);
    visuals.widgets.active.bg_fill = Color32::from_rgb(29, 185, 84); // #1DB954
    ctx.set_visuals(visuals);
}

fn format_secs(total_secs: u64) -> String {
    format!("{:02}:{:02}", total_secs / 60, total_secs % 60)
}

fn load_texture_from_bytes(ctx: &egui::Context, name: &str, bytes: &[u8]) -> Option<TextureHandle> {
    let dyn_img = image::load_from_memory(bytes).ok()?;
    let rgba = dyn_img.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let color_image = ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
    Some(ctx.load_texture(name, color_image, TextureOptions::LINEAR))
}

struct RustifyApp {
    audio: Option<AudioEngine>,
    status_msg: String,
    tracks: Vec<Track>,
    playlists: Vec<Playlist>,
    active_view: ActiveView,
    creating_playlist: bool,
    new_playlist_name: String,
    selected_track_path: Option<PathBuf>,
    cover_texture: Option<TextureHandle>,
    playlist_tex_cache: HashMap<String, TextureHandle>,
    search_query: String,
    tag_drawer_open: bool,
    tag_editor: TagEditorState,
}

impl RustifyApp {
    fn new() -> Self {
        let tracks = storage::load_library();
        let playlists = storage::load_playlists();
        let (audio, status_msg) = match AudioEngine::new() {
            Ok(engine) => (Some(engine), "Audio engine ready".to_string()),
            Err(err) => (None, err),
        };

        Self {
            audio,
            status_msg,
            tracks,
            playlists,
            active_view: ActiveView::AllSongs,
            creating_playlist: false,
            new_playlist_name: String::new(),
            selected_track_path: None,
            cover_texture: None,
            playlist_tex_cache: HashMap::new(),
            search_query: String::new(),
            tag_drawer_open: false,
            tag_editor: TagEditorState::default(),
        }
    }

    fn reload_cover_texture(&mut self, ctx: &egui::Context, path: &Path) {
        self.cover_texture = None;
        if let Some(bytes) = library::extract_cover_bytes(path) {
            self.cover_texture = load_texture_from_bytes(ctx, "active_cover", &bytes);
        }
    }

    fn play_track_by_path(&mut self, ctx: &egui::Context, path: &Path) {
        let Some(track) = self.tracks.iter().find(|t| t.path == path).cloned() else {
            return;
        };

        self.selected_track_path = Some(track.path.clone());
        self.tag_editor.load_from_track(&track);
        self.reload_cover_texture(ctx, &track.path);

        if let Some(audio) = &mut self.audio {
            match audio.play_file(&track.path) {
                Ok(()) => self.status_msg = format!("Playing: {}", track.title),
                Err(e) => self.status_msg = e,
            }
        }
    }

    fn visible_track_paths(&self) -> Vec<PathBuf> {
        match self.active_view {
            ActiveView::AllSongs => self.tracks.iter().map(|t| t.path.clone()).collect(),
            ActiveView::Playlist(p_idx) => self
                .playlists
                .get(p_idx)
                .map(|p| p.track_paths.clone())
                .unwrap_or_default(),
        }
    }

    fn play_relative(&mut self, ctx: &egui::Context, offset: isize) {
        let queue = self.visible_track_paths();
        if queue.is_empty() {
            return;
        }
        let current_pos = self
            .selected_track_path
            .as_ref()
            .and_then(|p| queue.iter().position(|q| q == p));

        let next_idx = match current_pos {
            Some(pos) => (pos as isize + offset).clamp(0, queue.len() as isize - 1) as usize,
            None => 0,
        };
        self.play_track_by_path(ctx, &queue[next_idx]);
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
        let bytes = library::extract_cover_bytes(path)?;
        let tex = load_texture_from_bytes(ctx, &key, &bytes)?;
        self.playlist_tex_cache.insert(key, tex.clone());
        Some(tex)
    }

    fn render_playlist_banner(&mut self, ctx: &egui::Context, ui: &mut egui::Ui, p_idx: usize) {
        let slate_panel = Color32::from_rgb(35, 37, 54);
        let cyan_badge = Color32::from_rgb(122, 162, 247);
        let phosphor_green = Color32::from_rgb(0, 255, 65);

        let Some(playlist) = self.playlists.get(p_idx).cloned() else {
            return;
        };

        let custom_tex = playlist
            .cover_path
            .as_ref()
            .and_then(|p| self.get_or_load_image_file(ctx, p));

        // Gather up to 4 unique track cover textures for the 2x2 Spotify collage fallback
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

        egui::Frame::new()
            .fill(slate_panel)
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(69, 73, 106)))
            .inner_margin(12.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // 104x104 Cover Art (Custom Image, 2x2 Collage, or Retro Fallback)
                    if let Some(tex) = custom_tex {
                        ui.image((tex.id(), egui::vec2(104.0, 104.0)));
                    } else if collage_textures.len() >= 4 {
                        egui::Grid::new("playlist_collage_2x2")
                            .spacing([2.0, 2.0])
                            .show(ui, |ui| {
                                ui.image((collage_textures[0].id(), egui::vec2(51.0, 51.0)));
                                ui.image((collage_textures[1].id(), egui::vec2(51.0, 51.0)));
                                ui.end_row();
                                ui.image((collage_textures[2].id(), egui::vec2(51.0, 51.0)));
                                ui.image((collage_textures[3].id(), egui::vec2(51.0, 51.0)));
                                ui.end_row();
                            });
                    } else if let Some(first_tex) = collage_textures.first() {
                        ui.image((first_tex.id(), egui::vec2(104.0, 104.0)));
                    } else {
                        let (rect, _) = ui.allocate_exact_size(egui::vec2(104.0, 104.0), egui::Sense::hover());
                        ui.painter().rect_filled(rect, 2.0_f32, Color32::BLACK);
                        ui.painter().rect_stroke(
                            rect,
                            2.0_f32,
                            Stroke::new(1.0_f32, cyan_badge),
                            egui::StrokeKind::Inside,
                        );
                        ui.painter().text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "PLAYLIST",
                            egui::FontId::monospace(12.0),
                            phosphor_green,
                        );
                    }

                    ui.add_space(14.0);

                    ui.vertical(|ui| {
                        ui.label(RichText::new("CUSTOM PLAYLIST").monospace().small().color(cyan_badge));
                        ui.label(RichText::new(&playlist.name).size(24.0).strong().color(Color32::WHITE));
                        ui.label(
                            RichText::new(format!("{} tracks", playlist.track_paths.len()))
                                .small()
                                .color(Color32::LIGHT_GRAY),
                        );
                        ui.add_space(6.0);

                        ui.horizontal(|ui| {
                            if ui.button(RichText::new("[SET COVER]").monospace().small()).clicked() {
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
                                    }
                                }
                            }

                            if ui
                                .button(
                                    RichText::new("[DELETE PLAYLIST]")
                                        .monospace()
                                        .small()
                                        .color(Color32::from_rgb(255, 110, 110)),
                                )
                                .clicked()
                            {
                                self.playlists.remove(p_idx);
                                storage::save_playlists(&self.playlists);
                                self.active_view = ActiveView::AllSongs;
                            }
                        });
                    });
                });
            });
        ui.add_space(8.0);
    }
}

impl eframe::App for RustifyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let slate_panel = Color32::from_rgb(35, 37, 54);
        let charcoal_bg = Color32::from_rgb(18, 18, 18);
        let phosphor_green = Color32::from_rgb(0, 255, 65);
        let emerald = Color32::from_rgb(29, 185, 84);
        let cyan_badge = Color32::from_rgb(122, 162, 247);

        let finished = self.audio.as_ref().map(|a| a.is_finished()).unwrap_or(false);
        if finished {
            let queue = self.visible_track_paths();
            let current_pos = self
                .selected_track_path
                .as_ref()
                .and_then(|p| queue.iter().position(|q| q == p));
            if let Some(pos) = current_pos {
                if pos + 1 < queue.len() {
                    self.play_track_by_path(ctx, &queue[pos + 1]);
                } else if let Some(audio) = &mut self.audio {
                    audio.stop();
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
        }

        let elapsed = self
            .audio
            .as_ref()
            .map(|a| a.position())
            .unwrap_or(Duration::ZERO);
        let elapsed_secs = elapsed.as_secs();

        let active_track = self
            .selected_track_path
            .as_ref()
            .and_then(|p| self.tracks.iter().find(|t| &t.path == p))
            .cloned();
        let total_secs = active_track.as_ref().map(|t| t.duration_secs).unwrap_or(0);

        // 1. BOTTOM NOW-PLAYING DECK
        egui::TopBottomPanel::bottom("now_playing_bar")
            .exact_height(92.0)
            .frame(egui::Frame::new().fill(slate_panel).inner_margin(12.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if let Some(tex) = &self.cover_texture {
                        ui.image((tex.id(), egui::vec2(60.0, 60.0)));
                    } else {
                        let (rect, _) = ui.allocate_exact_size(egui::vec2(60.0, 60.0), egui::Sense::hover());
                        ui.painter().rect_filled(rect, 2.0_f32, Color32::BLACK);
                        ui.painter().rect_stroke(
                            rect,
                            2.0_f32,
                            Stroke::new(1.0_f32, cyan_badge),
                            egui::StrokeKind::Inside,
                        );
                        ui.painter().text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "DISC",
                            egui::FontId::monospace(12.0),
                            phosphor_green,
                        );
                    }

                    ui.add_space(10.0);

                    ui.allocate_ui_with_layout(
                        egui::vec2(230.0, 64.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
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
                                        .color(cyan_badge),
                                );
                            } else {
                                ui.label(RichText::new("No Track Selected").strong().color(Color32::WHITE));
                                ui.label(RichText::new(&self.status_msg).small().color(Color32::GRAY));
                            }
                        },
                    );

                    ui.add_space(16.0);

                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            if ui.button("|<<").clicked() {
                                self.play_relative(ctx, -1);
                            }
                            let play_label = match playback_state {
                                PlaybackState::Playing => " || ",
                                _ => "  >  ",
                            };
                            if ui.button(RichText::new(play_label).strong().color(emerald)).clicked() {
                                if playback_state == PlaybackState::Stopped {
                                    self.play_relative(ctx, 0);
                                } else if let Some(audio) = &mut self.audio {
                                    audio.toggle_play_pause();
                                }
                            }
                            if ui.button(" [] ").clicked() {
                                if let Some(audio) = &mut self.audio {
                                    audio.stop();
                                }
                            }
                            if ui.button(">>|").clicked() {
                                self.play_relative(ctx, 1);
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format_secs(elapsed_secs)).monospace().small().color(Color32::LIGHT_GRAY));
                            let mut pos_f32 = elapsed_secs.min(total_secs) as f32;
                            let max_f32 = (total_secs.max(1)) as f32;
                            let slider = egui::Slider::new(&mut pos_f32, 0.0..=max_f32)
                                .show_value(false)
                                .trailing_fill(true);
                            if ui.add_sized([240.0, 16.0], slider).changed() {
                                if let Some(audio) = &mut self.audio {
                                    audio.seek(Duration::from_secs(pos_f32 as u64));
                                }
                            }
                            ui.label(RichText::new(format_secs(total_secs)).monospace().small().color(Color32::LIGHT_GRAY));
                        });
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let tag_btn = ui.selectable_label(
                            self.tag_drawer_open,
                            RichText::new(" [TAG] ").monospace().strong(),
                        );
                        if tag_btn.clicked() {
                            self.tag_drawer_open = !self.tag_drawer_open;
                        }

                        ui.add_space(12.0);
                        if let Some(audio) = &mut self.audio {
                            let mut vol = audio.volume;
                            if ui
                                .add_sized([90.0, 18.0], egui::Slider::new(&mut vol, 0.0..=1.0).show_value(false))
                                .changed()
                            {
                                audio.set_volume(vol);
                            }
                            ui.label(RichText::new("Vol").small().color(Color32::LIGHT_GRAY));
                        }
                    });
                });
            });

        // 2. LEFT NAVIGATION SIDEBAR + WINAMP LCD TELEMETRY BOX
        egui::SidePanel::left("sidebar")
            .exact_width(230.0)
            .frame(egui::Frame::new().fill(slate_panel).inner_margin(12.0))
            .show(ctx, |ui| {
                ui.label(RichText::new("RUSTIFY // 2.9").monospace().strong().size(16.0).color(phosphor_green));
                ui.separator();

                ui.label(RichText::new("LIBRARY").small().strong().color(Color32::LIGHT_GRAY));
                let is_all = self.active_view == ActiveView::AllSongs;
                if ui
                    .selectable_label(
                        is_all,
                        RichText::new(format!("All Songs ({})", self.tracks.len()))
                            .color(if is_all { emerald } else { Color32::WHITE }),
                    )
                    .clicked()
                {
                    self.active_view = ActiveView::AllSongs;
                    self.search_query.clear();
                }

                if ui.button("+ Import Folder").clicked() {
                    if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                        self.tracks = library::scan_folder(&folder);
                        storage::save_library(&self.tracks);
                        self.selected_track_path = None;
                        self.cover_texture = None;
                        self.playlist_tex_cache.clear();
                        if let Some(audio) = &mut self.audio {
                            audio.stop();
                        }
                    }
                }

                ui.add_space(12.0);
                ui.label(RichText::new("PLAYLISTS").small().strong().color(Color32::LIGHT_GRAY));

                if self.creating_playlist {
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.new_playlist_name)
                                .hint_text("Playlist name...")
                                .desired_width(130.0),
                        );
                        if ui.button("Add").clicked() {
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
                } else if ui.button("+ New Playlist").clicked() {
                    self.creating_playlist = true;
                }

                ui.add_space(4.0);
                for (p_idx, pl) in self.playlists.iter().enumerate() {
                    let selected = self.active_view == ActiveView::Playlist(p_idx);
                    let label = RichText::new(format!("• {} ({})", pl.name, pl.track_paths.len()))
                        .color(if selected { emerald } else { Color32::LIGHT_GRAY });
                    if ui.selectable_label(selected, label).clicked() {
                        self.active_view = ActiveView::Playlist(p_idx);
                    }
                }

                // Bottom-anchored Winamp Pitch-Black LCD Screen + Live Spectrum Analyzer
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    egui::Frame::new()
                        .fill(Color32::BLACK)
                        .stroke(Stroke::new(1.5_f32, Color32::from_rgb(69, 73, 106)))
                        .inner_margin(10.0)
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(format_secs(elapsed_secs))
                                        .monospace()
                                        .size(20.0)
                                        .strong()
                                        .color(phosphor_green),
                                );

                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    let kbps = active_track.as_ref().and_then(|t| t.bitrate_kbps).unwrap_or(0);
                                    let khz = active_track.as_ref().and_then(|t| t.sample_rate_hz).unwrap_or(44100) as f32 / 1000.0;
                                    ui.label(
                                        RichText::new(format!("{:>3} kbps\n{:.1} kHz", kbps, khz))
                                            .monospace()
                                            .small()
                                            .color(cyan_badge),
                                    );
                                });
                            });
                            ui.add_space(6.0);
                            ui::draw_winamp_spectrum(ui, playback_state, elapsed.as_secs_f32());
                        });
                });
            });

        // 3. COLLAPSIBLE WINAMP TAG EDITOR DRAWER
        if self.tag_drawer_open {
            egui::TopBottomPanel::bottom("tag_editor_drawer")
                .exact_height(155.0)
                .frame(egui::Frame::new().fill(Color32::from_rgb(28, 30, 44)).inner_margin(12.0))
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

        // 4. CENTRAL TRACK LIBRARY / PLAYLIST VIEW
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(charcoal_bg).inner_margin(16.0))
            .show(ctx, |ui| {
                if let ActiveView::Playlist(p_idx) = self.active_view {
                    self.render_playlist_banner(ctx, ui, p_idx);
                }

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Search:").color(Color32::LIGHT_GRAY));
                    ui.add(
                        egui::TextEdit::singleline(&mut self.search_query)
                            .hint_text("Filter by title, artist, or album...")
                            .desired_width(320.0),
                    );
                });
                ui.separator();

                let visible_paths = self.visible_track_paths();

                if visible_paths.is_empty() {
                    ui.centered_and_justified(|ui| {
                        let msg = match self.active_view {
                            ActiveView::AllSongs => {
                                "Your library is empty.\nClick '+ Import Folder' in the sidebar to scan local music."
                            }
                            ActiveView::Playlist(_) => {
                                "This playlist has no songs yet.\nGo to 'All Songs' and click '+ PL' on any track to add it here."
                            }
                        };
                        ui.label(RichText::new(msg).color(Color32::GRAY));
                    });
                } else {
                    let query = self.search_query.to_lowercase();
                    let mut clicked_path: Option<PathBuf> = None;
                    let mut remove_from_playlist_idx: Option<usize> = None;
                    let mut add_to_playlist: Option<(usize, PathBuf)> = None;

                    egui::ScrollArea::vertical().show(ui, |ui| {
                        egui::Grid::new("track_table")
                            .num_columns(6)
                            .spacing([16.0, 8.0])
                            .striped(true)
                            .show(ui, |ui| {
                                ui.label(RichText::new("#").strong().color(Color32::GRAY));
                                ui.label(RichText::new("TITLE").strong().color(Color32::GRAY));
                                ui.label(RichText::new("ARTIST").strong().color(Color32::GRAY));
                                ui.label(RichText::new("ALBUM").strong().color(Color32::GRAY));
                                ui.label(RichText::new("TIME").strong().color(Color32::GRAY));
                                ui.label(RichText::new("PLAYLIST").strong().color(Color32::GRAY));
                                ui.end_row();

                                for (row_idx, path) in visible_paths.iter().enumerate() {
                                    let Some(track) = self.tracks.iter().find(|t| &t.path == path) else {
                                        continue;
                                    };

                                    if !query.is_empty()
                                        && !track.title.to_lowercase().contains(&query)
                                        && !track.artist.to_lowercase().contains(&query)
                                        && !track.album.to_lowercase().contains(&query)
                                    {
                                        continue;
                                    }

                                    let is_selected = self.selected_track_path.as_ref() == Some(&track.path);
                                    let title_color = if is_selected { emerald } else { Color32::WHITE };

                                    ui.label(RichText::new(format!("{}", row_idx + 1)).monospace().color(Color32::GRAY));
                                    if ui
                                        .selectable_label(is_selected, RichText::new(&track.title).color(title_color))
                                        .clicked()
                                    {
                                        clicked_path = Some(track.path.clone());
                                    }
                                    ui.label(RichText::new(&track.artist).color(Color32::LIGHT_GRAY));
                                    ui.label(RichText::new(&track.album).color(Color32::GRAY));
                                    ui.label(RichText::new(track.formatted_duration()).monospace().color(Color32::LIGHT_GRAY));

                                    match self.active_view {
                                        ActiveView::AllSongs => {
                                            if self.playlists.is_empty() {
                                                ui.label(RichText::new("-").small().color(Color32::DARK_GRAY));
                                            } else {
                                                ui.menu_button(RichText::new("+ PL").monospace().small(), |ui| {
                                                    for (p_i, pl) in self.playlists.iter().enumerate() {
                                                        if ui.button(&pl.name).clicked() {
                                                            add_to_playlist = Some((p_i, track.path.clone()));
                                                            ui.close_menu();
                                                        }
                                                    }
                                                });
                                            }
                                        }
                                        ActiveView::Playlist(_) => {
                                            if ui.button(RichText::new("Remove").small()).clicked() {
                                                remove_from_playlist_idx = Some(row_idx);
                                            }
                                        }
                                    }

                                    ui.end_row();
                                }
                            });
                    });

                    if let Some(path) = clicked_path {
                        self.play_track_by_path(ctx, &path);
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
                    if let Some(row_idx) = remove_from_playlist_idx {
                        if let ActiveView::Playlist(p_idx) = self.active_view {
                            if let Some(pl) = self.playlists.get_mut(p_idx) {
                                if row_idx < pl.track_paths.len() {
                                    pl.track_paths.remove(row_idx);
                                    storage::save_playlists(&self.playlists);
                                    self.playlist_tex_cache.clear();
                                }
                            }
                        }
                    }
                }
            });
    }
}