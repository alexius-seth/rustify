mod library;
mod models;

use eframe::egui::{self, Color32, ColorImage, RichText, Stroke, TextureHandle, TextureOptions};
use models::{PlaybackState, Track};

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("RUSTIFY // Lightweight Audio Engine")
            .with_inner_size([1040.0, 680.0])
            .with_min_inner_size([820.0, 540.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Rustify",
        options,
        Box::new(|cc| {
            apply_hybrid_theme(&cc.egui_ctx);
            Ok(Box::new(RustifyApp::default()))
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

struct RustifyApp {
    tracks: Vec<Track>,
    selected_index: Option<usize>,
    cover_texture: Option<TextureHandle>,
    search_query: String,
    playback_state: PlaybackState,
    volume: f32,
    tag_drawer_open: bool,
}

impl Default for RustifyApp {
    fn default() -> Self {
        Self {
            tracks: Vec::new(),
            selected_index: None,
            cover_texture: None,
            search_query: String::new(),
            playback_state: PlaybackState::Stopped,
            volume: 0.8,
            tag_drawer_open: false,
        }
    }
}

impl RustifyApp {
    fn select_track(&mut self, ctx: &egui::Context, index: usize) {
        self.selected_index = Some(index);
        self.playback_state = PlaybackState::Playing;
        self.cover_texture = None;

        if let Some(track) = self.tracks.get(index) {
            if let Some(bytes) = library::extract_cover_bytes(&track.path) {
                if let Ok(dyn_img) = image::load_from_memory(&bytes) {
                    let rgba = dyn_img.to_rgba8();
                    let size = [rgba.width() as usize, rgba.height() as usize];
                    let color_image = ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
                    self.cover_texture =
                        Some(ctx.load_texture("active_cover", color_image, TextureOptions::LINEAR));
                }
            }
        }
    }
}

impl eframe::App for RustifyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let slate_panel = Color32::from_rgb(35, 37, 54);
        let phosphor_green = Color32::from_rgb(0, 255, 65);
        let emerald = Color32::from_rgb(29, 185, 84);
        let cyan_badge = Color32::from_rgb(122, 162, 247);

        let active_track = self.selected_index.and_then(|idx| self.tracks.get(idx)).cloned();

        // 1. BOTTOM NOW-PLAYING DECK
        egui::TopBottomPanel::bottom("now_playing_bar")
            .exact_height(88.0)
            .frame(egui::Frame::new().fill(slate_panel).inner_margin(12.0))
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    // Album Art Thumbnail (or Retro Fallback Box)
                    if let Some(tex) = &self.cover_texture {
                        ui.image((tex.id(), egui::vec2(58.0, 58.0)));
                    } else {
                        egui::Frame::new()
                            .fill(Color32::BLACK)
                            .stroke(Stroke::new(1.0_f32, cyan_badge))
                            .show(ui, |ui| {
                                ui.set_min_size(egui::vec2(58.0, 58.0));
                                ui.centered_and_justified(|ui| {
                                    ui.label(RichText::new("DISC").monospace().small().color(phosphor_green));
                                });
                            });
                    }

                    ui.add_space(10.0);

                    // Left: Track Info & Format Badge
                    ui.vertical(|ui| {
                        if let Some(track) = &active_track {
                            ui.label(RichText::new(&track.title).strong().color(Color32::WHITE));
                            ui.label(RichText::new(&track.artist).small().color(Color32::LIGHT_GRAY));
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
                            ui.label(RichText::new("Import a folder and click a song").small().color(Color32::GRAY));
                        }
                    });

                    ui.add_space(40.0);

                    // Center: Winamp-style Hardware Transport Buttons
                    ui.horizontal(|ui| {
                        if ui.button("|<<").clicked() {
                            if let Some(idx) = self.selected_index {
                                if idx > 0 {
                                    self.select_track(ctx, idx - 1);
                                }
                            }
                        }
                        let play_label = match self.playback_state {
                            PlaybackState::Playing => " || ",
                            _ => "  >  ",
                        };
                        if ui.button(RichText::new(play_label).strong().color(emerald)).clicked() {
                            self.playback_state = match self.playback_state {
                                PlaybackState::Playing => PlaybackState::Paused,
                                _ => PlaybackState::Playing,
                            };
                        }
                        if ui.button(" [] ").clicked() {
                            self.playback_state = PlaybackState::Stopped;
                        }
                        if ui.button(">>|").clicked() {
                            if let Some(idx) = self.selected_index {
                                if idx + 1 < self.tracks.len() {
                                    self.select_track(ctx, idx + 1);
                                }
                            }
                        }
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let tag_btn = ui.selectable_label(
                            self.tag_drawer_open,
                            RichText::new(" [TAG] ").monospace().strong(),
                        );
                        if tag_btn.clicked() {
                            self.tag_drawer_open = !self.tag_drawer_open;
                        }

                        ui.add_space(16.0);
                        ui.add(egui::Slider::new(&mut self.volume, 0.0..=1.0).show_value(false).text("Vol"));
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
                if ui.button(format!("All Songs ({})", self.tracks.len())).clicked() {
                    self.search_query.clear();
                }
                if ui.button("+ Import Folder").clicked() {
                    if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                        self.tracks = library::scan_folder(&folder);
                        self.selected_index = None;
                        self.cover_texture = None;
                    }
                }

                ui.add_space(12.0);
                ui.label(RichText::new("PLAYLISTS").small().strong().color(Color32::LIGHT_GRAY));
                if ui.button("+ New Playlist").clicked() {}

                // Bottom-anchored Winamp Pitch-Black LCD Screen
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    egui::Frame::new()
                        .fill(Color32::BLACK)
                        .stroke(Stroke::new(1.5_f32, Color32::from_rgb(69, 73, 106)))
                        .inner_margin(10.0)
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                let time_str = active_track
                                    .as_ref()
                                    .map(|t| t.formatted_duration())
                                    .unwrap_or_else(|| "00:00".to_string());
                                ui.label(RichText::new(time_str).monospace().size(20.0).strong().color(phosphor_green));

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
                            ui.add_space(4.0);
                            ui.label(RichText::new("[|||....||||....|||]").monospace().color(emerald));
                        });
                });
            });

        // 3. COLLAPSIBLE WINAMP TAG EDITOR DRAWER
        if self.tag_drawer_open {
            egui::TopBottomPanel::bottom("tag_editor_drawer")
                .exact_height(130.0)
                .frame(egui::Frame::new().fill(Color32::from_rgb(28, 30, 44)).inner_margin(12.0))
                .show(ctx, |ui| {
                    ui.label(RichText::new("TAG EDITOR // ID3v2 & VORBIS").monospace().strong().color(cyan_badge));
                    ui.separator();
                    ui.label("Phase 4 will add live tag editing and cover art embedding here.");
                });
        }

        // 4. CENTRAL TRACK LIBRARY VIEW
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Search:");
                ui.add(
                    egui::TextEdit::singleline(&mut self.search_query)
                        .hint_text("Filter by title, artist, or album...")
                        .desired_width(320.0),
                );
            });
            ui.separator();

            if self.tracks.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.label(
                        RichText::new("Your library is empty.\nClick '+ Import Folder' in the sidebar to scan local music.")
                            .color(Color32::GRAY),
                    );
                });
            } else {
                let query = self.search_query.to_lowercase();
                let mut clicked_index: Option<usize> = None;

                egui::ScrollArea::vertical().show(ui, |ui| {
                    egui::Grid::new("track_table")
                        .num_columns(5)
                        .spacing([16.0, 8.0])
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label(RichText::new("#").strong().color(Color32::GRAY));
                            ui.label(RichText::new("TITLE").strong().color(Color32::GRAY));
                            ui.label(RichText::new("ARTIST").strong().color(Color32::GRAY));
                            ui.label(RichText::new("ALBUM").strong().color(Color32::GRAY));
                            ui.label(RichText::new("TIME").strong().color(Color32::GRAY));
                            ui.end_row();

                            for (idx, track) in self.tracks.iter().enumerate() {
                                if !query.is_empty()
                                    && !track.title.to_lowercase().contains(&query)
                                    && !track.artist.to_lowercase().contains(&query)
                                    && !track.album.to_lowercase().contains(&query)
                                {
                                    continue;
                                }

                                let is_selected = self.selected_index == Some(idx);
                                let title_color = if is_selected { emerald } else { Color32::WHITE };

                                ui.label(RichText::new(format!("{}", idx + 1)).monospace().color(Color32::GRAY));
                                if ui
                                    .selectable_label(is_selected, RichText::new(&track.title).color(title_color))
                                    .clicked()
                                {
                                    clicked_index = Some(idx);
                                }
                                ui.label(RichText::new(&track.artist).color(Color32::LIGHT_GRAY));
                                ui.label(RichText::new(&track.album).color(Color32::GRAY));
                                ui.label(RichText::new(track.formatted_duration()).monospace().color(Color32::LIGHT_GRAY));
                                ui.end_row();
                            }
                        });
                });

                if let Some(idx) = clicked_index {
                    self.select_track(ctx, idx);
                }
            }
        });
    }
}