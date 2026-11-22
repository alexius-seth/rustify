use crate::models::{PlaybackState, Track};
use eframe::egui::{self, Color32, RichText, Stroke, TextureHandle};
use std::path::PathBuf;

pub struct TagEditorState {
    pub loaded_track_path: Option<PathBuf>,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub year: String,
    pub pending_cover_path: Option<PathBuf>,
    pub feedback_msg: String,
}

impl Default for TagEditorState {
    fn default() -> Self {
        Self {
            loaded_track_path: None,
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            year: String::new(),
            pending_cover_path: None,
            feedback_msg: String::new(),
        }
    }
}

impl TagEditorState {
    pub fn load_from_track(&mut self, track: &Track) {
        self.loaded_track_path = Some(track.path.clone());
        self.title = track.title.clone();
        self.artist = track.artist.clone();
        self.album = track.album.clone();
        self.year = track.year.map(|y| y.to_string()).unwrap_or_default();
        self.pending_cover_path = None;
        self.feedback_msg.clear();
    }
}

/// Draws a classic 12-band Winamp Green -> Yellow -> Red LED spectrum analyzer.
pub fn draw_winamp_spectrum(ui: &mut egui::Ui, state: PlaybackState, elapsed_secs: f32) {
    let desired_size = egui::vec2(ui.available_width(), 28.0);
    let (rect, _) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
    let painter = ui.painter();

    let bands = 12;
    let segments = 8;
    let gap = 3.0_f32;
    let bar_width = (rect.width() - gap * (bands as f32 - 1.0)) / bands as f32;
    let seg_gap = 1.5_f32;
    let seg_height = (rect.height() - seg_gap * (segments as f32 - 1.0)) / segments as f32;

    for b in 0..bands {
        let active_segments = match state {
            PlaybackState::Playing => {
                // Smooth multi-frequency harmonic wave driven by playback time
                let t = elapsed_secs * 7.5 + (b as f32 * 0.65);
                let wave = (t.sin() * 0.5 + 0.5) * 0.65 + ((t * 1.9).cos() * 0.5 + 0.5) * 0.35;
                ((wave * segments as f32).round() as usize).clamp(1, segments)
            }
            PlaybackState::Paused => 1,
            PlaybackState::Stopped => 0,
        };

        let x = rect.left() + b as f32 * (bar_width + gap);

        for s in 0..segments {
            let y_bottom = rect.bottom() - s as f32 * (seg_height + seg_gap);
            let seg_rect = egui::Rect::from_min_max(
                egui::pos2(x, y_bottom - seg_height),
                egui::pos2(x + bar_width, y_bottom),
            );

            let color = if s < active_segments {
                if s >= 6 {
                    Color32::from_rgb(255, 42, 0) // Red peak (#FF2A00)
                } else if s >= 4 {
                    Color32::from_rgb(255, 255, 0) // Yellow mid (#FFFF00)
                } else {
                    Color32::from_rgb(0, 230, 0) // Green base (#00E600)
                }
            } else {
                Color32::from_rgb(18, 28, 18) // Dim unlit LED segment
            };

            painter.rect_filled(seg_rect, 1.0_f32, color);
        }
    }
}

/// Renders the collapsible Winamp-style Tag Editor drawer and returns true when [SAVE] is clicked.
pub fn render_tag_editor_drawer(
    ui: &mut egui::Ui,
    editor: &mut TagEditorState,
    active_track: Option<&Track>,
    cover_texture: Option<&TextureHandle>,
) -> bool {
    let cyan_badge = Color32::from_rgb(122, 162, 247);
    let emerald = Color32::from_rgb(29, 185, 84);
    let phosphor_green = Color32::from_rgb(0, 255, 65);
    let mut save_requested = false;

    ui.horizontal(|ui| {
        ui.label(
            RichText::new("TAG EDITOR // ID3v2 & VORBIS METADATA")
                .monospace()
                .strong()
                .color(cyan_badge),
        );
        if !editor.feedback_msg.is_empty() {
            ui.add_space(16.0);
            ui.label(RichText::new(&editor.feedback_msg).small().color(phosphor_green));
        }
    });
    ui.separator();

    let Some(track) = active_track else {
        ui.label(
            RichText::new("Select any track in the table above to inspect or edit its embedded metadata and cover art.")
                .color(Color32::LIGHT_GRAY),
        );
        return false;
    };

    ui.horizontal(|ui| {
        // Left: Cover Art Preview + [CHANGE ART] Picker
        ui.vertical(|ui| {
            if let Some(tex) = cover_texture {
                ui.image((tex.id(), egui::vec2(76.0, 76.0)));
            } else {
                let (rect, _) = ui.allocate_exact_size(egui::vec2(76.0, 76.0), egui::Sense::hover());
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
                    "NO ART",
                    egui::FontId::monospace(11.0),
                    Color32::GRAY,
                );
            }

            if ui.button(RichText::new("[CHANGE ART]").monospace().small()).clicked() {
                if let Some(img) = rfd::FileDialog::new()
                    .add_filter("Cover Image", &["png", "jpg", "jpeg"])
                    .pick_file()
                {
                    editor.feedback_msg = format!(
                        "Selected cover: {}",
                        img.file_name().and_then(|n| n.to_str()).unwrap_or("image")
                    );
                    editor.pending_cover_path = Some(img);
                }
            }
        });

        ui.add_space(14.0);

        // Right: Metadata Input Grid
        egui::Grid::new("tag_editor_fields")
            .num_columns(4)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label(RichText::new("Title:").color(Color32::LIGHT_GRAY));
                ui.add(egui::TextEdit::singleline(&mut editor.title).desired_width(240.0));

                ui.label(RichText::new("Artist:").color(Color32::LIGHT_GRAY));
                ui.add(egui::TextEdit::singleline(&mut editor.artist).desired_width(220.0));
                ui.end_row();

                ui.label(RichText::new("Album:").color(Color32::LIGHT_GRAY));
                ui.add(egui::TextEdit::singleline(&mut editor.album).desired_width(240.0));

                ui.label(RichText::new("Year:").color(Color32::LIGHT_GRAY));
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut editor.year).desired_width(70.0));
                    ui.add_space(12.0);
                    if ui
                        .button(RichText::new(" [SAVE TAGS] ").monospace().strong().color(emerald))
                        .clicked()
                    {
                        save_requested = true;
                    }
                });
                ui.end_row();

                ui.label(RichText::new("File:").small().color(Color32::GRAY));
                ui.label(
                    RichText::new(
                        track
                            .path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or(""),
                    )
                    .monospace()
                    .small()
                    .color(Color32::GRAY),
                );
            });
    });

    save_requested
}