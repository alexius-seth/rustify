mod models;

use eframe::egui::{self, Color32, RichText, Stroke, Vec2};
use models::{PlaybackState, Playlist, Track};

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
    // Spotify deep charcoal for main canvas + Winamp metallic slate for panels
    visuals.panel_fill = Color32::from_rgb(18, 18, 18); // #121212
    visuals.window_fill = Color32::from_rgb(35, 37, 54); // #232536
    visuals.extreme_bg_color = Color32::from_rgb(0, 0, 0); // #000000 LCD black

    // Beveled Winamp-style button borders
    visuals.widgets.inactive.bg_fill = Color32::from_rgb(48, 51, 74);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(69, 73, 106)); // #45496A
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(64, 68, 98);
    visuals.widgets.active.bg_fill = Color32::from_rgb(29, 185, 84); // #1DB954 Spotify Emerald
    ctx.set_visuals(visuals);
}

struct RustifyApp {
    tracks: Vec<Track>,
    playlists: Vec<Playlist>,
    search_query: String,
    playback_state: PlaybackState,
    volume: f32,
    tag_drawer_open: bool,
}

impl Default for RustifyApp {
    fn default() -> Self {
        Self {
            tracks: Vec::new(),
            playlists: Vec::new(),
            search_query: String::new(),
            playback_state: PlaybackState::Stopped,
            volume: 0.8,
            tag_drawer_open: false,
        }
    }
}

impl eframe::App for RustifyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let slate_panel = Color32::from_rgb(35, 37, 54);
        let phosphor_green = Color32::from_rgb(0, 255, 65);
        let emerald = Color32::from_rgb(29, 185, 84);
        let cyan_badge = Color32::from_rgb(122, 162, 247);

        // 1. BOTTOM NOW-PLAYING DECK (Spotify + Winamp Beveled Controls)
        egui::TopBottomPanel::bottom("now_playing_bar")
            .exact_height(84.0)
            .frame(egui::Frame::new().fill(slate_panel).inner_margin(12.0))
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    // Left: Track Info & Format Badge
                    ui.vertical(|ui| {
                        ui.label(RichText::new("No Track Loaded").strong().color(Color32::WHITE));
                        ui.label(RichText::new("Select a music folder to begin").small().color(Color32::GRAY));
                        ui.label(RichText::new("[READY · 44.1 kHz]").monospace().small().color(cyan_badge));
                    });

                    ui.add_space(40.0);

                    // Center: Winamp-style Hardware Transport Buttons
                    ui.horizontal(|ui| {
                        let _ = ui.button("|<<");
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
                        let _ = ui.button(">>|");
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // [TAG] Drawer Toggle Button
                        let tag_btn = ui.selectable_label(self.tag_drawer_open, RichText::new(" [TAG] ").monospace().strong());
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
                if ui.button("All Songs").clicked() {}
                if ui.button("+ Import Folder").clicked() {}

                ui.add_space(12.0);
                ui.label(RichText::new("PLAYLISTS").small().strong().color(Color32::LIGHT_GRAY));
                if ui.button("+ New Playlist").clicked() {}

                // Bottom-anchored Winamp Pitch-Black LCD Screen
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    egui::Frame::new()
                        .fill(Color32::BLACK)
                        .stroke(Stroke::new(1.5, Color32::from_rgb(69, 73, 106)))
                        .inner_margin(10.0)
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("00:00").monospace().size(22.0).strong().color(phosphor_green));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.label(RichText::new("000 kbps\n44.1 kHz").monospace().small().color(cyan_badge));
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
                    ui.label("Select a track from the library to edit its Title, Artist, Album, and Cover Art.");
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
                        RichText::new("Your library is empty.\nClick '+ Import Folder' in Phase 2 to load local audio files.")
                            .color(Color32::GRAY),
                    );
                });
            }
        });
    }
}