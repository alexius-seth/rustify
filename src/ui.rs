use crate::models::{PlaybackState, Track};
use eframe::egui::{self, Color32, Pos2, Rect, Response, RichText, Sense, Stroke, TextureHandle, Vec2};
use std::path::PathBuf;

// Hybrid Color Tokens
pub const CHARCOAL_BG: Color32 = Color32::from_rgb(18, 18, 18); // #121212
pub const SLATE_PANEL: Color32 = Color32::from_rgb(35, 37, 54); // #232536
pub const SLATE_DARK: Color32 = Color32::from_rgb(26, 28, 42);
pub const BEVEL_LIGHT: Color32 = Color32::from_rgb(69, 73, 106); // #45496A
pub const BEVEL_DARK: Color32 = Color32::from_rgb(17, 18, 27); // #11121B
pub const PHOSPHOR_GREEN: Color32 = Color32::from_rgb(0, 255, 65); // #00FF41
pub const SPOTIFY_EMERALD: Color32 = Color32::from_rgb(29, 185, 84); // #1DB954
pub const WINAMP_CYAN: Color32 = Color32::from_rgb(122, 162, 247); // #7AA2F7

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

/// Paints a classic Winamp 3D beveled border (light top/left, dark bottom/right).
pub fn paint_bevel(painter: &egui::Painter, rect: Rect, pressed: bool) {
    let (top_left, bottom_right) = if pressed {
        (BEVEL_DARK, BEVEL_LIGHT)
    } else {
        (BEVEL_LIGHT, BEVEL_DARK)
    };

    let tl = rect.left_top();
    let tr = rect.right_top();
    let bl = rect.left_bottom();
    let br = rect.right_bottom();

    painter.line_segment([tl, tr], Stroke::new(1.2_f32, top_left));
    painter.line_segment([tl, bl], Stroke::new(1.2_f32, top_left));
    painter.line_segment([bl, br], Stroke::new(1.2_f32, bottom_right));
    painter.line_segment([tr, br], Stroke::new(1.2_f32, bottom_right));
}

/// Tactile rectangular Winamp hardware button with 3D bevel edges.
pub fn hardware_button(
    ui: &mut egui::Ui,
    label: &str,
    size: Vec2,
    active: bool,
    accent: Color32,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let painter = ui.painter();

    let is_pressed = response.is_pointer_button_down_on() || active;
    let bg = if active {
        Color32::from_rgb(28, 34, 48)
    } else if response.hovered() {
        Color32::from_rgb(56, 60, 86)
    } else {
        Color32::from_rgb(44, 47, 68)
    };

    painter.rect_filled(rect, 1.0_f32, bg);
    paint_bevel(painter, rect, is_pressed);

    let text_color = if active {
        accent
    } else if response.hovered() {
        Color32::WHITE
    } else {
        Color32::LIGHT_GRAY
    };

    let offset = if is_pressed { Vec2::new(1.0, 1.0) } else { Vec2::ZERO };
    painter.text(
        rect.center() + offset,
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::monospace(12.0),
        text_color,
    );

    response
}

/// Sleek Spotify-style progress/volume bar with emerald fill and hover thumb.
pub fn emerald_slider(
    ui: &mut egui::Ui,
    value: &mut f32,
    max_val: f32,
    width: f32,
    fill_color: Color32,
) -> bool {
    let height = 16.0_f32;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click_and_drag());
    let painter = ui.painter();

    let mut changed = false;
    if response.dragged() || response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            let ratio = ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
            *value = ratio * max_val;
            changed = true;
        }
    }

    let ratio = if max_val > 0.0 {
        (*value / max_val).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let track_h = 4.0_f32;
    let track_rect = Rect::from_min_max(
        Pos2::new(rect.left(), rect.center().y - track_h * 0.5),
        Pos2::new(rect.right(), rect.center().y + track_h * 0.5),
    );
    painter.rect_filled(track_rect, 2.0_f32, Color32::from_rgb(60, 63, 82));

    let fill_x = rect.left() + rect.width() * ratio;
    let fill_rect = Rect::from_min_max(track_rect.min, Pos2::new(fill_x, track_rect.max.y));
    let active_fill = if response.hovered() || response.dragged() {
        SPOTIFY_EMERALD
    } else {
        fill_color
    };
    painter.rect_filled(fill_rect, 2.0_f32, active_fill);

    if response.hovered() || response.dragged() {
        painter.circle_filled(Pos2::new(fill_x, rect.center().y), 5.5_f32, Color32::WHITE);
    }

    changed
}

/// Draws a classic 12-band Winamp Green -> Yellow -> Red LED spectrum analyzer.
pub fn draw_winamp_spectrum(ui: &mut egui::Ui, state: PlaybackState, elapsed_secs: f32) {
    let desired_size = Vec2::new(ui.available_width(), 30.0);
    let (rect, _) = ui.allocate_exact_size(desired_size, Sense::hover());
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
            let seg_rect = Rect::from_min_max(
                Pos2::new(x, y_bottom - seg_height),
                Pos2::new(x + bar_width, y_bottom),
            );

            let color = if s < active_segments {
                if s >= 6 {
                    Color32::from_rgb(255, 42, 0) // #FF2A00
                } else if s >= 4 {
                    Color32::from_rgb(255, 255, 0) // #FFFF00
                } else {
                    Color32::from_rgb(0, 230, 0) // #00E600
                }
            } else {
                Color32::from_rgb(18, 28, 18)
            };

            painter.rect_filled(seg_rect, 1.0_f32, color);
        }
    }
}

/// Renders the collapsible Winamp-style Tag Editor drawer and returns true when [SAVE TAGS] is clicked.
/// Renders the collapsible, vertically resizable Winamp-style Tag Editor drawer.
pub fn render_tag_editor_drawer(
    ui: &mut egui::Ui,
    editor: &mut TagEditorState,
    active_track: Option<&Track>,
    cover_texture: Option<&TextureHandle>,
) -> bool {
    let mut save_requested = false;

    ui.horizontal(|ui| {
        ui.label(
            RichText::new("TAG EDITOR // ID3v2 & VORBIS METADATA")
                .monospace()
                .strong()
                .color(WINAMP_CYAN),
        );
        if !editor.feedback_msg.is_empty() {
            ui.add_space(16.0);
            ui.label(RichText::new(&editor.feedback_msg).small().color(PHOSPHOR_GREEN));
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

    // Scale cover art preview dynamically as the user drags the tag drawer taller
    let art_size = (ui.available_height() - 34.0).clamp(74.0, 190.0);

    ui.horizontal(|ui| {
        // Left: Cover Art Preview + Beveled [CHANGE ART] Picker
        ui.vertical(|ui| {
            if let Some(tex) = cover_texture {
                let resp = ui.image((tex.id(), Vec2::new(art_size, art_size)));
                paint_bevel(ui.painter(), resp.rect, false);
            } else {
                let (rect, _) = ui.allocate_exact_size(Vec2::new(art_size, art_size), Sense::hover());
                ui.painter().rect_filled(rect, 1.0_f32, Color32::BLACK);
                paint_bevel(ui.painter(), rect, true);
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "NO ART",
                    egui::FontId::monospace(11.0),
                    Color32::GRAY,
                );
            }

            ui.add_space(4.0);
            if hardware_button(ui, "CHANGE ART", Vec2::new(art_size, 22.0), false, WINAMP_CYAN).clicked() {
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
                ui.add(egui::TextEdit::singleline(&mut editor.title).desired_width(250.0));

                ui.label(RichText::new("Artist:").color(Color32::LIGHT_GRAY));
                ui.add(egui::TextEdit::singleline(&mut editor.artist).desired_width(230.0));
                ui.end_row();

                ui.label(RichText::new("Album:").color(Color32::LIGHT_GRAY));
                ui.add(egui::TextEdit::singleline(&mut editor.album).desired_width(250.0));

                ui.label(RichText::new("Year:").color(Color32::LIGHT_GRAY));
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut editor.year).desired_width(70.0));
                    ui.add_space(12.0);
                    if hardware_button(ui, "SAVE TAGS", Vec2::new(96.0, 24.0), true, SPOTIFY_EMERALD).clicked() {
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