//! The Edit window: toolbar, rulers, track headers and playlists.

use crate::theme::{Tokens, bold, regular, rgb};
use crate::{SoundApp, panels, toolbar};
use egui::{Align2, Color32, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use serde_json::json;
use soundcraft_model::{AutoParam, Session, Track, TrackId, TrackKind};
use soundcraft_time::{Range, Samples};

mod rulers;
use rulers::draw_rulers;
pub use rulers::ruler_height;
mod track_header;
use track_header::{header_column, track_header};
mod clips;
use clips::{draw_clip, grid_lines};
mod automation;
use automation::{draw_automation, plugin_range};
mod interaction;
use interaction::{lane_interaction, overlay, snap};

pub const HEADER_W: f32 = 236.0;
pub const COLUMN_W: f32 = 112.0;

/// Optional Edit-window columns (View › Edit Window Views) that are switched on.
pub fn edit_columns(s: &Session) -> Vec<&'static str> {
    let all = s.edit.flag("edit_view.all");
    ["io", "inserts_ae", "inserts_fj", "sends_ae", "sends_fj", "comments"]
        .into_iter()
        .filter(|c| all || s.edit.flag(&format!("edit_view.{c}")))
        .collect()
}

pub fn header_width(s: &Session) -> f32 {
    HEADER_W + COLUMN_W * edit_columns(s).len() as f32
}
pub const RULER_H: f32 = 17.0;

/// Geometry of the last frame, for hit testing and agents (`ui.inspect`).
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct EditLayout {
    pub timeline: [f32; 4],
    pub rows: Vec<(u64, [f32; 4])>,
    pub scroll_y: f32,
    pub content_h: f32,
}

pub fn x_of(s: &Session, tl: Rect, at: Samples) -> f32 {
    tl.min.x + ((at - s.edit.zoom.scroll) as f64 / s.edit.zoom.samples_per_px.max(0.01)) as f32
}

pub fn sample_at(s: &Session, tl: Rect, x: f32) -> Samples {
    s.edit.zoom.scroll + soundcraft_time::to_samples(f64::from(x - tl.min.x) * s.edit.zoom.samples_per_px)
}

pub const LANE_H: f32 = 46.0;

pub fn track_height(t: &Track) -> f32 {
    let alt = if t.view == "playlists" { t.playlists.len().saturating_sub(1) as f32 * LANE_H } else { 0.0 };
    t.height.points() + alt
}

pub fn show(app: &mut SoundApp, ui: &mut Ui) {
    let t = Tokens::DARK;
    egui::Panel::top("edit_toolbar").exact_size(toolbar::HEIGHT).frame(egui::Frame::NONE.fill(t.toolbar_bg)).show(ui, |ui| toolbar::show(app, ui));
    egui::Panel::bottom("edit_status").exact_size(22.0).frame(egui::Frame::NONE.fill(t.toolbar_bg)).show(ui, |ui| status_bar(app, ui));
    egui::Panel::bottom("lower_dock_tabs").exact_size(22.0).frame(egui::Frame::NONE.fill(t.panel_bg2)).show(ui, |ui| dock_tabs(app, ui));
    if app.ui.show_midi_editor {
        egui::Panel::bottom("midi_editor")
            .resizable(true)
            .default_size(320.0)
            .min_size(260.0)
            .frame(egui::Frame::NONE.fill(t.panel_bg))
            .show(ui, |ui| crate::midi_editor::show(app, ui));
    }
    if app.ui.show_universe {
        egui::Panel::top("universe").exact_size(64.0).frame(egui::Frame::NONE.fill(Color32::from_rgb(18, 18, 19))).show(ui, |ui| universe(app, ui));
    }
    if app.ui.show_tracks_list {
        egui::Panel::left("tracks_list")
            .exact_size(168.0)
            .frame(egui::Frame::NONE.fill(t.panel_bg))
            .show(ui, |ui| panels::tracks_and_groups(app, ui));
    }
    if app.ui.show_clip_list {
        egui::Panel::right("clip_list").exact_size(230.0).frame(egui::Frame::NONE.fill(t.panel_bg)).show(ui, |ui| panels::clip_list(app, ui));
    }
    egui::CentralPanel::default().frame(egui::Frame::NONE.fill(t.window_bg)).show(ui, |ui| main_area(app, ui));
}

/// Universe: the whole session in miniature; the box is the visible area (drag or click to move).
fn universe(app: &mut SoundApp, ui: &mut Ui) {
    let r = ui.max_rect().shrink2(vec2(8.0, 4.0));
    let s = app.engine.session();
    let total = (s.content_end() + s.sample_rate.samples(10.0)).max(1) as f32;
    let tracks: Vec<&Track> = s.tracks.iter().filter(|t| !t.hidden).collect();
    let n = tracks.len().max(1) as f32;
    let rh = (r.height() / n).max(1.0);
    for (i, t) in tracks.iter().enumerate() {
        let y = r.min.y + i as f32 * rh;
        for c in t.clips() {
            let x0 = r.min.x + c.start as f32 / total * r.width();
            let x1 = r.min.x + c.end() as f32 / total * r.width();
            ui.painter().rect_filled(
                Rect::from_min_max(pos2(x0, y + 0.5), pos2(x1.max(x0 + 1.0), y + rh - 0.5)),
                0.0,
                rgb(c.color.unwrap_or(t.color)),
            );
        }
    }
    let vis0 = s.edit.zoom.scroll as f32 / total;
    let tlw = app.edit_layout.timeline[2] - app.edit_layout.timeline[0];
    let vis1 = vis0 + (f64::from(tlw.max(100.0)) * s.edit.zoom.samples_per_px) as f32 / total;
    let vr = Rect::from_min_max(pos2(r.min.x + vis0 * r.width(), r.min.y - 2.0), pos2(r.min.x + vis1.min(1.0) * r.width(), r.max.y + 2.0));
    ui.painter().rect_stroke(vr, 2.0, Stroke::new(1.5, Color32::WHITE), StrokeKind::Outside);
    if app.is_playing() {
        let x = r.min.x + app.position() as f32 / total * r.width();
        ui.painter().line_segment([pos2(x, r.min.y), pos2(x, r.max.y)], Stroke::new(1.0, Tokens::DARK.playhead));
    }
    let resp = ui.interact(r, ui.id().with("universe"), Sense::click_and_drag());
    if let Some(p) = resp.interact_pointer_pos()
        && (resp.clicked() || resp.dragged())
    {
        let half = (vis1 - vis0) * 0.5;
        let k = ((p.x - r.min.x) / r.width() - half).clamp(0.0, 1.0);
        let _ = app.engine.execute("view.scroll", &json!({"to": (k * total) as i64}));
    }
}

fn dock_tabs(app: &mut SoundApp, ui: &mut Ui) {
    let t = Tokens::DARK;
    let r = ui.max_rect();
    let mut x = r.min.x + 10.0;
    for (label, on, id) in [
        ("MIDI EDITOR", app.ui.show_midi_editor, "window.midi_editor"),
        ("MEMORY LOCATIONS", app.ui.show_memory_locations, "window.memory_locations"),
        ("UNDO HISTORY", app.ui.show_undo_history, "window.undo_history"),
    ] {
        let g = ui.painter().layout_no_wrap(label.to_string(), bold(10.5), if on { Color32::WHITE } else { t.text_dim });
        let tr = Rect::from_min_size(pos2(x, r.min.y + 2.0), vec2(g.size().x + 16.0, r.height() - 4.0));
        if on {
            ui.painter().rect_filled(tr, 3.0, t.accent_dark);
        }
        ui.painter().galley(pos2(tr.min.x + 8.0, tr.center().y - g.size().y * 0.5), g, Color32::WHITE);
        if ui.interact(tr, ui.id().with(("dock", label)), Sense::click()).clicked() {
            let _ = app.run(id, json!({}));
        }
        x = tr.max.x + 6.0;
    }
}

fn status_bar(app: &mut SoundApp, ui: &mut Ui) {
    let t = Tokens::DARK;
    let r = ui.max_rect();
    let s = app.engine.session();
    let left = format!(
        "{} · {} Hz · {} · {} tracks · {}",
        s.name,
        s.sample_rate.hz(),
        match s.bit_depth {
            soundcraft_model::BitDepthSetting::Int16 => "16-bit",
            soundcraft_model::BitDepthSetting::Int24 => "24-bit",
            soundcraft_model::BitDepthSetting::Float32 => "32-bit float",
        },
        s.tracks.len(),
        app.ui.status
    );
    ui.painter().text(pos2(r.min.x + 10.0, r.center().y), Align2::LEFT_CENTER, left, regular(11.0), t.text_dim);
    let undo = app.engine.undo_label().map_or(String::new(), |l| format!("Undo {l}"));
    ui.painter().text(
        pos2(r.max.x - 10.0, r.center().y),
        Align2::RIGHT_CENTER,
        if app.frame_ms < 50.0 { format!("{undo}   {:.1} ms/frame", app.frame_ms) } else { undo },
        regular(11.0),
        t.text_dim,
    );
}

fn main_area(app: &mut SoundApp, ui: &mut Ui) {
    let t = Tokens::DARK;
    let full = ui.max_rect();
    let rulers: Vec<String> = app.engine.session().edit.rulers.clone();
    let mut rulers = rulers;
    if app.engine.session().edit.flag("view.ruler.tempo_editor")
        && let Some(i) = rulers.iter().position(|r| r == "tempo")
    {
        rulers.insert(i + 1, "tempo_editor".into());
    }
    let rulers_h = rulers.iter().map(|r| ruler_height(r)).sum::<f32>() + 6.0;
    let hscroll_h = 14.0;
    let hw = header_width(app.engine.session());
    let header = Rect::from_min_max(full.min, pos2(full.min.x + hw, full.max.y - hscroll_h));
    let tl = Rect::from_min_max(pos2(full.min.x + hw, full.min.y + rulers_h), pos2(full.max.x - 12.0, full.max.y - hscroll_h));
    let rulers_rect = Rect::from_min_max(pos2(full.min.x, full.min.y), pos2(full.max.x, full.min.y + rulers_h));
    app.edit_layout.timeline = [tl.min.x, tl.min.y, tl.max.x, tl.max.y];
    ui.painter().rect_filled(full, 0.0, t.window_bg);
    // Wheel: vertical scroll tracks, shift/horizontal scroll timeline, cmd = zoom.
    let hovered = ui.rect_contains_pointer(Rect::from_min_max(pos2(full.min.x, tl.min.y), full.max));
    if hovered {
        let (delta, mods) = ui.input(|i| (i.smooth_scroll_delta, i.modifiers));
        if mods.command && delta.y.abs() > 0.0 {
            let id = if delta.y > 0.0 { "view.zoom_in" } else { "view.zoom_out" };
            let _ = app.run(id, json!({}));
        } else {
            let dx = if mods.shift { delta.y } else { delta.x };
            if dx.abs() > 0.0 {
                let _ = app.engine.execute("view.scroll", &json!({"by_px": -dx}));
            }
            if !mods.shift && delta.y.abs() > 0.0 {
                app.edit_layout.scroll_y = (app.edit_layout.scroll_y - delta.y).clamp(0.0, (app.edit_layout.content_h - tl.height() + 40.0).max(0.0));
            }
        }
    }
    draw_rulers(app, ui, rulers_rect, tl, &rulers);
    // Tracks.
    let ids: Vec<TrackId> =
        app.engine.session().tracks.iter().filter(|x| !x.hidden && !folder_collapsed(app.engine.session(), x)).map(|x| x.id).collect();
    let mut y = tl.min.y - app.edit_layout.scroll_y;
    app.edit_layout.rows.clear();
    let clip_rect = Rect::from_min_max(pos2(full.min.x, tl.min.y), pos2(full.max.x, tl.max.y));
    let mut content_h = 0.0;
    for id in ids {
        let Some(h) = app.engine.session().track(id).map(track_height) else { continue };
        let row = Rect::from_min_max(pos2(full.min.x, y), pos2(tl.max.x, y + h));
        if row.max.y >= clip_rect.min.y && row.min.y <= clip_rect.max.y {
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(row));
            child.set_clip_rect(clip_rect);
            track_row(app, &mut child, id, row, tl);
        }
        app.edit_layout.rows.push((id.0, [row.min.x, row.min.y, row.max.x, row.max.y]));
        y += h + 1.0;
        content_h += h + 1.0;
    }
    app.edit_layout.content_h = content_h;
    // Empty area click clears track selection.
    let empty = Rect::from_min_max(pos2(full.min.x, y.max(tl.min.y)), tl.max);
    if empty.height() > 4.0 && ui.interact(empty, ui.id().with("empty"), Sense::click()).clicked() {
        let _ = app.run("edit.select_none", json!({}));
    }
    overlay(app, ui, tl, Rect::from_min_max(pos2(tl.min.x, rulers_rect.min.y), tl.max));
    let _ = header;
    // Horizontal scrollbar.
    let sb = Rect::from_min_max(pos2(tl.min.x, full.max.y - hscroll_h), pos2(tl.max.x, full.max.y));
    hscrollbar(app, ui, sb, tl);
    // Vertical scrollbar gutter.
    let vs = Rect::from_min_max(pos2(tl.max.x, tl.min.y), pos2(full.max.x, tl.max.y));
    ui.painter().rect_filled(vs, 0.0, t.panel_bg);
    if content_h > tl.height() {
        let frac = tl.height() / content_h;
        let off = app.edit_layout.scroll_y / content_h;
        let thumb = Rect::from_min_size(pos2(vs.min.x + 2.0, vs.min.y + vs.height() * off), vec2(vs.width() - 4.0, (vs.height() * frac).max(20.0)));
        ui.painter().rect_filled(thumb, 3.0, t.button_hi);
        let resp = ui.interact(vs, ui.id().with("vscroll"), Sense::drag());
        if resp.dragged() {
            app.edit_layout.scroll_y = (app.edit_layout.scroll_y + resp.drag_delta().y / frac).clamp(0.0, (content_h - tl.height() + 40.0).max(0.0));
        }
    }
}

fn folder_collapsed(s: &Session, t: &Track) -> bool {
    t.folder.and_then(|f| s.track(f)).is_some_and(|f| !f.folder_open)
}

fn hscrollbar(app: &mut SoundApp, ui: &mut Ui, sb: Rect, tl: Rect) {
    let t = Tokens::DARK;
    ui.painter().rect_filled(sb, 0.0, t.panel_bg);
    let s = app.engine.session();
    let total = (s.content_end() + s.sample_rate.samples(30.0)).max(1) as f64;
    let vis = f64::from(tl.width()) * s.edit.zoom.samples_per_px;
    let frac = (vis / total).clamp(0.02, 1.0) as f32;
    let off = (s.edit.zoom.scroll as f64 / total).clamp(0.0, 1.0) as f32;
    let thumb = Rect::from_min_size(pos2(sb.min.x + sb.width() * off, sb.min.y + 3.0), vec2((sb.width() * frac).max(24.0), sb.height() - 6.0));
    ui.painter().rect_filled(thumb, 3.0, t.button_hi);
    let resp = ui.interact(sb, ui.id().with("hscroll"), Sense::drag());
    if resp.dragged() {
        let dx = resp.drag_delta().x / sb.width();
        let by_px = f64::from(dx) * total / s.edit.zoom.samples_per_px;
        let _ = app.engine.execute("view.scroll", &json!({"by_px": by_px}));
    }
}

// ---- track rows ------------------------------------------------------------------------------

fn track_row(app: &mut SoundApp, ui: &mut Ui, id: TrackId, row: Rect, tl: Rect) {
    let t = Tokens::DARK;
    let s = app.engine.session();
    let Some(track) = s.track(id).cloned() else { return };
    let selected = s.edit.selected_tracks.contains(&id);
    let head = Rect::from_min_max(row.min, pos2(row.min.x + HEADER_W, row.max.y));
    let cols = edit_columns(app.engine.session());
    for (i, c) in cols.iter().enumerate() {
        let cr = Rect::from_min_size(pos2(head.max.x + i as f32 * COLUMN_W, row.min.y), vec2(COLUMN_W, row.height()));
        header_column(app, ui, &track, c, cr);
    }
    let main_h = track.height.points();
    let lane = Rect::from_min_max(pos2(tl.min.x, row.min.y), pos2(tl.max.x, row.min.y + main_h));
    // Header background.
    ui.painter().rect_filled(head, 0.0, if selected { Color32::from_rgb(52, 58, 66) } else { t.panel_bg2 });
    ui.painter().rect_filled(Rect::from_min_size(head.min, vec2(5.0, head.height())), 0.0, rgb(track.color));
    ui.painter().line_segment([pos2(head.min.x, row.max.y + 0.5), pos2(tl.max.x, row.max.y + 0.5)], Stroke::new(1.0, t.border));
    track_header(app, ui, &track, head, selected);
    // Lane.
    let painter = ui.painter().with_clip_rect(lane.intersect(ui.clip_rect()));
    painter.rect_filled(lane, 0.0, if selected { t.playlist_alt } else { t.playlist_bg });
    let s = app.engine.session().clone();
    grid_lines(&painter, &s, tl, lane);
    let view = Range::new(sample_at(&s, tl, tl.min.x), sample_at(&s, tl, tl.max.x));
    let auto_view = AutoParam::parse(&track.view);
    for clip in track.clips() {
        if !clip.range().overlaps(&view) {
            continue;
        }
        draw_clip(app, &painter, &s, &track, clip, tl, lane, auto_view.is_some());
    }
    crate::video_track::draw_lane(app, &painter, &s, &track, tl, lane);
    if let Some(p) = &auto_view {
        draw_automation(&painter, &s, &track, p, tl, lane);
    }
    if track.kind == TrackKind::Master || track.kind == TrackKind::Aux || track.kind == TrackKind::Vca {
        let p = auto_view.clone().unwrap_or(AutoParam::Volume);
        draw_automation(&painter, &s, &track, &p, tl, lane);
    }
    // Selection highlight on this track.
    let sel = s.edit.selection;
    if selected && !sel.is_empty() {
        let x0 = x_of(&s, tl, sel.start).max(lane.min.x);
        let x1 = x_of(&s, tl, sel.end).min(lane.max.x);
        if x1 > x0 {
            painter.rect_filled(Rect::from_min_max(pos2(x0, lane.min.y), pos2(x1, lane.max.y)), 0.0, t.selection);
        }
    }
    lane_interaction(app, ui, &track, lane, tl);
    if track.view == "playlists" {
        playlist_lanes(app, ui, &track, row, main_h, tl);
    }
}

/// Alternate playlists as lanes under the main one; clicking a clip promotes it (comping).
fn playlist_lanes(app: &mut SoundApp, ui: &mut Ui, track: &Track, row: Rect, main_h: f32, tl: Rect) {
    let t = Tokens::DARK;
    let s = app.engine.session().clone();
    let mut y = row.min.y + main_h;
    for (i, pl) in track.playlists.iter().enumerate() {
        if i == track.active_playlist {
            continue;
        }
        let lane = Rect::from_min_max(pos2(tl.min.x, y), pos2(tl.max.x, y + LANE_H));
        let head = Rect::from_min_max(pos2(row.min.x + 12.0, y), pos2(tl.min.x, y + LANE_H));
        ui.painter().rect_filled(lane, 0.0, Color32::from_rgb(30, 30, 31));
        ui.painter().text(pos2(head.min.x + 4.0, head.center().y), Align2::LEFT_CENTER, &pl.name, regular(10.5), t.text_dim);
        let painter = ui.painter().with_clip_rect(lane.intersect(ui.clip_rect()));
        for c in &pl.clips {
            let r = Rect::from_min_max(pos2(x_of(&s, tl, c.start), lane.min.y + 3.0), pos2(x_of(&s, tl, c.end()), lane.max.y - 3.0));
            painter.rect(r, 2.0, rgb(track.color).gamma_multiply(0.45), Stroke::new(1.0, Color32::BLACK), StrokeKind::Inside);
            painter.text(pos2(r.min.x + 4.0, r.min.y + 7.0), Align2::LEFT_CENTER, &c.name, regular(9.5), t.text);
        }
        let resp = ui.interact(lane, ui.id().with(("pl_lane", track.id.0, i)), Sense::click());
        if resp.clicked()
            && let Some(p) = resp.interact_pointer_pos()
        {
            let at = sample_at(&s, tl, p.x);
            let sel = s.edit.selection;
            let range = if !sel.is_empty() && sel.contains(at) {
                Some((sel.start, sel.end))
            } else {
                pl.clips.iter().find(|c| c.range().contains(at)).map(|c| (c.start, c.end()))
            };
            if let Some((a, b)) = range {
                let _ = app.run("track.playlist_promote", json!({"track": track.id.0, "playlist": i, "start": a, "end": b}));
            }
        }
        resp.on_hover_text("Click a take (or the selection) to promote it to the main playlist");
        y += LANE_H;
    }
}
