//! Timeline pointer gestures and clip editing.

use super::{plugin_range, sample_at, x_of};
use crate::Gesture;
use crate::SoundApp;
use crate::theme::Tokens;
use crate::theme::rgb;
use egui::Color32;
use egui::Rect;
use egui::Sense;
use egui::Shape;
use egui::Stroke;
use egui::StrokeKind;
use egui::Ui;
use egui::pos2;
use serde_json::json;
use soundcraft_model::AutoParam;
use soundcraft_model::Clip;
use soundcraft_model::ClipId;
use soundcraft_model::Session;
use soundcraft_model::Tool;
use soundcraft_model::Track;
use soundcraft_model::TrackId;
use soundcraft_time::Range;
use soundcraft_time::Samples;

// ---- interaction -----------------------------------------------------------------------------

pub(super) fn snap(s: &Session, at: Samples) -> Samples {
    if s.edit.edit_mode.is_grid() { s.edit.grid.snap(at, s.sample_rate, &s.tempo, s.frame_rate) } else { at }
}

fn clip_at(track: &Track, at: Samples) -> Option<&Clip> {
    track.clips().iter().rev().find(|c| c.range().contains(at))
}

pub(super) fn lane_interaction(app: &mut SoundApp, ui: &mut Ui, track: &Track, lane: Rect, tl: Rect) {
    let id = ui.id().with(("lane", track.id.0));
    let resp = ui.interact(lane, id, Sense::click_and_drag());
    // Audio files dropped from the Clip List.
    if let Some(src) = resp.dnd_release_payload::<crate::DragSource>()
        && let Some(p) = ui.ctx().pointer_latest_pos()
    {
        let s = app.engine.session();
        let at = snap(s, sample_at(s, tl, p.x).max(0));
        let _ = app.run("clip.place_source", json!({"source": src.0, "track": track.id.0, "at": at}));
    }
    if resp.dnd_hover_payload::<crate::DragSource>().is_some() {
        ui.painter().rect_stroke(lane, 0.0, Stroke::new(2.0, Tokens::DARK.accent), StrokeKind::Inside);
    }
    let s = app.engine.session().clone();
    let tool = s.edit.tool;
    let mods = ui.input(|i| i.modifiers);
    let Some(p) = resp.interact_pointer_pos().or_else(|| resp.hover_pos()) else { return };
    let at = sample_at(&s, tl, p.x).max(0);
    let hit = clip_at(track, at).cloned();
    let upper = p.y < lane.center().y;
    let near_edge = |c: &Clip| -> Option<bool> {
        let xs = x_of(&s, tl, c.start);
        let xe = x_of(&s, tl, c.end());
        if (p.x - xs).abs() < 6.0 {
            Some(true)
        } else if (p.x - xe).abs() < 6.0 {
            Some(false)
        } else {
            None
        }
    };
    // Effective tool under the Smart tool.
    let eff = match tool {
        Tool::Smart => {
            if let Some(c) = &hit {
                if near_edge(c).is_some() {
                    Tool::Trim
                } else if upper {
                    Tool::Selector
                } else {
                    Tool::Grabber
                }
            } else {
                Tool::Selector
            }
        }
        other => other,
    };
    if resp.hovered() {
        let icon = match eff {
            Tool::Trim => egui::CursorIcon::ResizeHorizontal,
            Tool::Grabber => egui::CursorIcon::Grab,
            Tool::Selector => egui::CursorIcon::Text,
            Tool::Zoom => egui::CursorIcon::ZoomIn,
            Tool::Pencil => egui::CursorIcon::Crosshair,
            _ => egui::CursorIcon::Default,
        };
        ui.ctx().set_cursor_icon(icon);
    }
    let auto_view = AutoParam::parse(&track.view);
    let fade_corner = if tool == Tool::Smart {
        hit.as_ref().and_then(|c| {
            let xs = x_of(&s, tl, c.start);
            let xe = x_of(&s, tl, c.end());
            let top = p.y < lane.min.y + lane.height() * 0.28;
            if !top {
                None
            } else if (p.x - xs) > 0.0 && (p.x - xs) < 14.0 {
                Some(true)
            } else if (xe - p.x) > 0.0 && (xe - p.x) < 14.0 {
                Some(false)
            } else {
                None
            }
        })
    } else {
        None
    };
    if fade_corner.is_some() && resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeNwSe);
    }
    if resp.drag_started()
        && let (Some(fade_in), Some(c)) = (fade_corner, hit.as_ref())
    {
        app.gesture = Some(Gesture::Fade { clip: c.id, track: track.id, fade_in, to: at });
    } else if resp.drag_started() {
        app.gesture = match eff {
            Tool::Selector | Tool::Smart => Some(Gesture::Select { track: track.id, anchor: snap(&s, at), tracks: vec![track.id] }),
            Tool::Grabber => hit.as_ref().map(|c| {
                let clips = if s.edit.selected_clips.contains(&c.id) { s.edit.selected_clips.clone() } else { vec![c.id] };
                Gesture::MoveClips { clips, grab_at: at, delta: 0, from_track: track.id, to_track: track.id }
            }),
            Tool::Trim => hit.as_ref().map(|c| match near_edge(c) {
                Some(false) => Gesture::TrimEnd { clip: c.id, track: track.id, to: c.end() },
                _ if at - c.start < c.end() - at => Gesture::TrimStart { clip: c.id, track: track.id, to: c.start },
                _ => Gesture::TrimEnd { clip: c.id, track: track.id, to: c.end() },
            }),
            Tool::Scrubber => Some(Gesture::Scrub { last: at }),
            Tool::Pencil => Some(Gesture::Pencil { track: track.id, points: Vec::new() }),
            Tool::Zoom => Some(Gesture::ZoomBox { start: p.x }),
        };
    }
    if resp.dragged() {
        let to_track = app.edit_layout.rows.iter().find(|(_, r)| p.y >= r[1] && p.y <= r[3]).map(|(i, _)| TrackId(*i));
        match &mut app.gesture {
            Some(Gesture::Select { tracks, .. }) => {
                if let Some(t2) = to_track {
                    // Extend across the tracks between the anchor row and the pointer row.
                    let order: Vec<TrackId> = app.edit_layout.rows.iter().map(|(i, _)| TrackId(*i)).collect();
                    let a = order.iter().position(|x| *x == track.id).unwrap_or(0);
                    let b = order.iter().position(|x| *x == t2).unwrap_or(a);
                    let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
                    *tracks = order.get(lo..=hi).map(<[TrackId]>::to_vec).unwrap_or_default();
                }
            }
            Some(Gesture::MoveClips { grab_at, delta, to_track: tt, .. }) => {
                let raw = at - *grab_at;
                *delta = if s.edit.edit_mode.is_grid() {
                    snap(&s, hit.as_ref().map_or(*grab_at, |c| c.start) + raw) - hit.as_ref().map_or(*grab_at, |c| c.start)
                } else {
                    raw
                };
                if s.edit.edit_mode == soundcraft_model::EditMode::Spot {
                    *delta = 0;
                }
                if let Some(t2) = to_track {
                    *tt = t2;
                }
            }
            Some(Gesture::TrimStart { to, .. } | Gesture::TrimEnd { to, .. }) => *to = snap(&s, at),
            Some(Gesture::Fade { to, .. }) => *to = at,
            Some(Gesture::Scrub { last }) => {
                *last = at;
                let _ = app.engine.execute("transport.locate", &json!({"at": at}));
            }
            Some(Gesture::Pencil { points, .. }) => {
                if let Some(param) = &auto_view {
                    let (lo, hi, _) = param.range();
                    let (lo, hi) = if matches!(param, AutoParam::Volume | AutoParam::SendLevel(_)) {
                        (-60.0, 12.0)
                    } else if matches!(param, AutoParam::Plugin { .. }) {
                        plugin_range(track, param)
                    } else {
                        (lo, hi)
                    };
                    let k = ((lane.max.y - 4.0 - p.y) / (lane.height() - 8.0)).clamp(0.0, 1.0);
                    let v = lo + (hi - lo) * k;
                    if points.last().is_none_or(|(a, _)| (at - a).abs() as f64 > s.edit.zoom.samples_per_px * 3.0) {
                        points.push((at, v));
                    }
                }
            }
            _ => {}
        }
        // Live selection feedback.
        if let Some(Gesture::Select { anchor, tracks, .. }) = &app.gesture {
            let r = Range::new(*anchor, snap(&s, at));
            let ids: Vec<u64> = tracks.iter().map(|x| x.0).collect();
            let _ = app.engine.execute("edit.select", &json!({"tracks": ids, "start": r.start, "end": r.end}));
        }
    }
    if resp.drag_stopped() {
        let g = app.gesture.take();
        match g {
            Some(Gesture::MoveClips { clips, delta, from_track, to_track, .. }) => {
                let ids: Vec<u64> = clips.iter().map(|c| c.0).collect();
                let mut params = json!({"clips": ids, "by": delta});
                if to_track != from_track {
                    params["track"] = json!(to_track.0);
                }
                if delta != 0 || to_track != from_track {
                    let _ = app.run("edit.move_clips", params);
                }
            }
            Some(Gesture::Fade { clip, track, fade_in, to }) => {
                if let Some((_, c)) = app.engine.session().find_clip(clip).map(|(t, c)| (t, c.clone())) {
                    let (a, b) = if fade_in { (c.start, to.clamp(c.start + 1, c.end())) } else { (to.clamp(c.start, c.end() - 1), c.end()) };
                    let _ = app.run("edit.fades_create", json!({"tracks": [track.0], "start": a, "end": b, "shape": "s-curve"}));
                }
            }
            Some(Gesture::TrimStart { clip, track, to }) => trim_clip(app, clip, track, to, true),
            Some(Gesture::TrimEnd { clip, track, to }) => trim_clip(app, clip, track, to, false),
            Some(Gesture::Pencil { track, points }) => {
                if let Some(param) = &auto_view {
                    let pid = automation_id(param);
                    for (a, v) in points {
                        let _ = app.run("automation.set_point", json!({"track": track.0, "param": pid, "at": a, "value": v}));
                    }
                }
            }
            Some(Gesture::ZoomBox { start }) => {
                let a = sample_at(&s, tl, start.min(p.x));
                let b = sample_at(&s, tl, start.max(p.x));
                if b > a {
                    let spp = ((b - a) as f64 / f64::from(tl.width())).max(0.25);
                    let _ = app.run("view.zoom_set", json!({"samples_per_px": spp}));
                    let _ = app.run("view.scroll", json!({"to": a}));
                }
            }
            _ => {}
        }
    }
    if resp.clicked() {
        match eff {
            Tool::Grabber => {
                if let Some(c) = &hit {
                    let mut ids = if mods.shift { s.edit.selected_clips.iter().map(|x| x.0).collect::<Vec<_>>() } else { Vec::new() };
                    ids.push(c.id.0);
                    let _ = app.run("edit.select", json!({"clips": ids}));
                }
            }
            Tool::Zoom => {
                let _ = app.run(if mods.alt { "view.zoom_out" } else { "view.zoom_in" }, json!({}));
            }
            Tool::Pencil => {
                if let Some(param) = &auto_view {
                    let (lo, hi, _) = param.range();
                    let (lo, hi) = if matches!(param, AutoParam::Volume | AutoParam::SendLevel(_)) { (-60.0, 12.0) } else { (lo, hi) };
                    let k = ((lane.max.y - 4.0 - p.y) / (lane.height() - 8.0)).clamp(0.0, 1.0);
                    let _ = app.run(
                        "automation.set_point",
                        json!({"track": track.id.0, "param": automation_id(param), "at": at, "value": lo + (hi - lo) * k}),
                    );
                }
            }
            _ => {
                let at = snap(&s, at);
                if mods.shift && s.edit.selected_tracks.contains(&track.id) {
                    let sel = s.edit.selection;
                    let r = if at < sel.start { Range::new(at, sel.end) } else { Range::new(sel.start, at) };
                    let _ = app.run("edit.select", json!({"start": r.start, "end": r.end}));
                } else {
                    let _ = app.run("edit.select", json!({"tracks": [track.id.0], "start": at}));
                }
            }
        }
    }
    if resp.double_clicked()
        && eff != Tool::Pencil
        && let Some(c) = &hit
    {
        let _ = app.run("edit.select", json!({"clips": [c.id.0]}));
    }
    resp.context_menu(|ui| clip_context_menu(app, ui, hit.as_ref().map(|c| c.id)));
}

fn automation_id(p: &AutoParam) -> String {
    match p {
        AutoParam::Volume => "volume".into(),
        AutoParam::Pan(0) => "pan".into(),
        AutoParam::Pan(_) => "pan2".into(),
        AutoParam::Mute => "mute".into(),
        AutoParam::SendLevel(i) => format!("send_{}_level", (b'a' + i) as char),
        AutoParam::SendPan(i) => format!("send_{}_pan", (b'a' + i) as char),
        AutoParam::SendMute(i) => format!("send_{}_mute", (b'a' + i) as char),
        AutoParam::Plugin { slot, param } => format!("plugin:{}:{param}", (b'a' + slot) as char),
    }
}

fn trim_clip(app: &mut SoundApp, clip: ClipId, track: TrackId, to: Samples, start: bool) {
    let cmd = if start { "edit.trim_start_to_insertion" } else { "edit.trim_end_to_insertion" };
    // Trims through the command layer: select the clip's track and position the insertion.
    let sel_before = app.engine.session().edit.selection;
    let tracks_before: Vec<u64> = app.engine.session().edit.selected_tracks.iter().map(|x| x.0).collect();
    let c = app.engine.session().find_clip(clip).map(|(_, c)| c.clone());
    if let Some(c) = c {
        if start && to < c.start || !start && to > c.end() {
            // Extending: use trim to fill selection with handles.
            let r = if start { Range::new(to, c.end()) } else { Range::new(c.start, to) };
            let _ = app.run("edit.trim_to_fill_selection", json!({"clips": [clip.0], "start": r.start, "end": r.end}));
        } else {
            let _ = app.run(cmd, json!({"tracks": [track.0], "at": to}));
        }
    }
    let _ = app.engine.execute("edit.select", &json!({"tracks": tracks_before, "start": sel_before.start, "end": sel_before.end}));
}

fn clip_context_menu(app: &mut SoundApp, ui: &mut Ui, clip: Option<ClipId>) {
    let Some(c) = clip else {
        if ui.button("Paste").clicked() {
            let _ = app.run("edit.paste", json!({}));
        }
        return;
    };
    let ids = json!([c.0]);
    for (label, cmd) in [
        ("Mute/Unmute", "edit.mute_clips"),
        ("Edit Lock/Unlock", "clip.edit_lock"),
        ("Time Lock/Unlock", "clip.time_lock"),
        ("Quantize to Grid", "clip.quantize_to_grid"),
        ("Render Clip Gain", "clip.gain_render"),
        ("Bring to Front", "clip.bring_to_front"),
        ("Send to Back", "clip.send_to_back"),
    ] {
        if ui.button(label).clicked() {
            let _ = app.run(cmd, json!({"clips": ids}));
        }
    }
    ui.separator();
    if ui.button("Rename...").clicked() {
        let name = app.engine.session().find_clip(c).map(|(_, x)| x.name.clone()).unwrap_or_default();
        app.dialogs.open_rename_clip(c, &name);
    }
    if ui.button("Delete").clicked() {
        let _ = app.run("edit.select", json!({"clips": ids}));
        let _ = app.run("edit.clear", json!({}));
    }
}

/// Selection overlay, insertion point and playhead across rulers and tracks.
pub(super) fn overlay(app: &mut SoundApp, ui: &mut Ui, tl: Rect, area: Rect) {
    let t = Tokens::DARK;
    let mut scroll_to: Option<Samples> = None;
    let s = app.engine.session();
    let painter = ui.painter().with_clip_rect(area);
    if s.edit.flag("view.marker.ruler_lines") {
        for m in &s.markers {
            let x = x_of(s, tl, m.start);
            if x > tl.min.x && x < tl.max.x {
                painter.line_segment(
                    [pos2(x, tl.min.y), pos2(x, tl.max.y)],
                    Stroke::new(1.0, m.color.map_or(Color32::from_rgba_unmultiplied(232, 200, 64, 70), |c| rgb(c).gamma_multiply(0.4))),
                );
            }
        }
    }
    let sel = s.edit.selection;
    // Timeline selection markers on the main ruler.
    if !sel.is_empty() {
        for x in [x_of(s, tl, sel.start), x_of(s, tl, sel.end)] {
            painter.add(Shape::convex_polygon(
                vec![pos2(x - 5.0, area.min.y + 2.0), pos2(x + 5.0, area.min.y + 2.0), pos2(x, area.min.y + 9.0)],
                t.accent,
                Stroke::NONE,
            ));
        }
    } else if !app.is_playing() {
        let x = x_of(s, tl, sel.start);
        let blink = (ui.input(|i| i.time) * 2.0) as i64 % 2 == 0;
        if blink {
            for (_, r) in &app.edit_layout.rows {
                if s.edit.selected_tracks.iter().any(|_| true) {
                    painter.line_segment([pos2(x, r[1].max(tl.min.y)), pos2(x, r[3])], Stroke::new(1.0, t.insertion));
                }
            }
        }
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
    }
    if app.is_playing() {
        let x = x_of(s, tl, app.position());
        painter.line_segment([pos2(x, area.min.y), pos2(x, area.max.y)], Stroke::new(1.5, t.playhead));
        // Page scrolling.
        if s.edit.scrolling != "none" && (x > tl.max.x || x < tl.min.x) {
            scroll_to = Some(app.position());
        }
    }
    // Ghost of a clip move.
    if let Some(Gesture::MoveClips { clips, delta, to_track, .. }) = &app.gesture {
        let row = app.edit_layout.rows.iter().find(|(i, _)| *i == to_track.0).map(|(_, r)| *r);
        for c in clips {
            if let (Some((_, clip)), Some(r)) = (s.find_clip(*c), row) {
                let x0 = x_of(s, tl, clip.start + delta);
                let x1 = x_of(s, tl, clip.end() + delta);
                painter.rect_stroke(
                    Rect::from_min_max(pos2(x0, r[1] + 1.0), pos2(x1, r[3] - 1.0)),
                    2.0,
                    Stroke::new(2.0, Color32::WHITE),
                    StrokeKind::Inside,
                );
            }
        }
    }
    if let Some(Gesture::TrimStart { to, clip, .. } | Gesture::TrimEnd { to, clip, .. }) = &app.gesture
        && let Some((tid, _)) = s.find_clip(*clip)
        && let Some((_, r)) = app.edit_layout.rows.iter().find(|(i, _)| *i == tid.0)
    {
        let x = x_of(s, tl, *to);
        painter.line_segment([pos2(x, r[1]), pos2(x, r[3])], Stroke::new(2.0, Color32::WHITE));
    }
    if let Some(Gesture::Fade { clip, fade_in, to, .. }) = &app.gesture
        && let Some((tid, c)) = s.find_clip(*clip)
        && let Some((_, r)) = app.edit_layout.rows.iter().find(|(i, _)| *i == tid.0)
    {
        let (a, b) = if *fade_in { (c.start, *to) } else { (*to, c.end()) };
        let (x0, x1) = (x_of(s, tl, a), x_of(s, tl, b));
        let (y0, y1) = (r[1] + 2.0, r[3] - 2.0);
        let line = if *fade_in { [pos2(x0, y1), pos2(x1, y0)] } else { [pos2(x0, y0), pos2(x1, y1)] };
        painter.line_segment(line, Stroke::new(2.0, Color32::WHITE));
    }
    if let Some(at) = scroll_to {
        let _ = app.engine.execute("view.scroll", &json!({"to": at}));
    }
}
