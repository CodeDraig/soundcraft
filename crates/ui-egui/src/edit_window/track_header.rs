//! Track header controls and optional columns.

use crate::SoundApp;
use crate::icons;
use crate::theme::Tokens;
use crate::theme::bold;
use crate::theme::regular;
use crate::theme::rgb;
use crate::widgets::rec_toggle;
use crate::widgets::selector_box;
use crate::widgets::text_toggle;
use egui::Align2;
use egui::Color32;
use egui::CornerRadius;
use egui::Rect;
use egui::Sense;
use egui::Stroke;
use egui::StrokeKind;
use egui::Ui;
use egui::pos2;
use egui::vec2;
use serde_json::json;
use soundcraft_model::AutoParam;
use soundcraft_model::Track;
use soundcraft_model::TrackHeight;
use soundcraft_model::TrackId;
use soundcraft_model::TrackKind;

pub(super) fn track_header(app: &mut SoundApp, ui: &mut Ui, track: &Track, head: Rect, selected: bool) {
    let t = Tokens::DARK;
    let id = track.id;
    // Drag the bottom edge to resize (snaps to the height presets).
    let edge = Rect::from_min_max(pos2(head.min.x, head.max.y - 4.0), pos2(head.max.x, head.max.y + 2.0));
    let eresp = ui.interact(edge, ui.id().with(("hedge", id.0)), Sense::drag());
    if eresp.hovered() || eresp.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }
    if eresp.drag_stopped()
        && let Some(p) = ui.ctx().pointer_latest_pos()
    {
        let want = (p.y - head.min.y).max(10.0);
        let best = TrackHeight::ALL.into_iter().min_by_key(|h| ((h.points() - want).abs() * 10.0) as i64).unwrap_or(TrackHeight::Medium);
        if best != track.height {
            let _ = app.run("track.height", json!({"tracks": [id.0], "height": best.label()}));
        }
    }
    let x0 = head.min.x + 12.0;
    let h = head.height();
    // Click on header background selects the track.
    let bg = ui.interact(head, ui.id().with(("hdr", id.0)), Sense::click());
    if bg.clicked() {
        let add = ui.input(|i| i.modifiers.command || i.modifiers.shift);
        let mut tracks = if add { app.engine.session().edit.selected_tracks.clone() } else { Vec::new() };
        if add && tracks.contains(&id) {
            tracks.retain(|x| *x != id);
        } else {
            tracks.push(id);
        }
        let ids: Vec<u64> = tracks.iter().map(|x| x.0).collect();
        let _ = app.run("edit.select", json!({"tracks": ids}));
    }
    bg.context_menu(|ui| track_context_menu(app, ui, id));
    // Name field.
    let name_r = Rect::from_min_size(pos2(x0, head.min.y + 4.0), vec2(150.0, 17.0));
    ui.painter().rect(
        name_r,
        CornerRadius::same(2),
        if selected { t.name_field_sel } else { t.name_field },
        Stroke::new(1.0, Color32::from_rgb(20, 20, 20)),
        StrokeKind::Inside,
    );
    let shown_name = if app.engine.session().edit.flag("edit_view.track_number") {
        let n = app.engine.session().track_index(id).map_or(0, |i| i + 1);
        format!("{n}  {}", track.name)
    } else {
        track.name.clone()
    };
    ui.painter().with_clip_rect(name_r).text(name_r.center(), Align2::CENTER_CENTER, &shown_name, bold(12.0), t.text_dark);
    let nresp = ui.interact(name_r, ui.id().with(("name", id.0)), Sense::click_and_drag());
    if nresp.double_clicked() {
        app.dialogs.open_rename_track(id, &track.name);
    } else if nresp.clicked() {
        let _ = app.run("edit.select", json!({"tracks": [id.0]}));
    }
    // Drag the name plate to reorder tracks.
    if nresp.dragged()
        && let Some(p) = ui.ctx().pointer_latest_pos()
    {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        let line_y = app
            .edit_layout
            .rows
            .iter()
            .map(|(_, r)| r[1])
            .chain(app.edit_layout.rows.last().map(|(_, r)| r[3]))
            .min_by_key(|y| ((y - p.y).abs() * 10.0) as i64)
            .unwrap_or(p.y);
        ui.ctx()
            .layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("reorder")))
            .line_segment([pos2(head.min.x, line_y), pos2(head.max.x + 600.0, line_y)], Stroke::new(3.0, Tokens::DARK.accent));
    }
    if nresp.drag_stopped()
        && let Some(p) = ui.ctx().pointer_latest_pos()
    {
        let rows = app.edit_layout.rows.clone();
        let target_row = rows.iter().position(|(_, r)| p.y < (r[1] + r[3]) * 0.5).unwrap_or(rows.len());
        // Convert the visible-row index to a session index.
        let s = app.engine.session();
        let to = rows.get(target_row).and_then(|(tid, _)| s.track_index(TrackId(*tid))).unwrap_or(s.tracks.len());
        let from = s.track_index(id).unwrap_or(0);
        let to = if to > from { to - 1 } else { to };
        if to != from {
            let _ = app.run("track.move", json!({"track": id.0, "to": to}));
        }
    }
    // Playlist selector arrow.
    let pl_r = Rect::from_min_size(pos2(name_r.max.x + 3.0, name_r.min.y), vec2(16.0, 17.0));
    icons::draw(ui.painter(), pl_r.shrink(3.0), "triangle_down", t.text);
    let presp = ui.interact(pl_r, ui.id().with(("pl", id.0)), Sense::click());
    egui::Popup::menu(&presp).show(|ui| {
        if ui.button("New...").clicked() {
            let _ = app.run("track.playlist_new", json!({"track": id.0}));
        }
        if ui.button("Duplicate...").clicked() {
            let _ = app.run("track.playlist_duplicate", json!({"track": id.0}));
        }
        if ui.button("Delete Unused...").clicked() {
            let _ = app.run("track.playlist_delete_unused", json!({"track": id.0}));
        }
        ui.separator();
        for (i, p) in track.playlists.iter().enumerate() {
            if ui.selectable_label(i == track.active_playlist, &p.name).clicked() {
                let _ = app.run("track.playlist_select", json!({"track": id.0, "index": i}));
            }
        }
    });
    if track.is_folder() {
        let fr = Rect::from_min_size(pos2(head.min.x + 6.0, name_r.min.y), vec2(10.0, 17.0));
        icons::draw(ui.painter(), fr, if track.folder_open { "triangle_down" } else { "triangle_right" }, t.text);
        if ui.interact(fr, ui.id().with(("fold", id.0)), Sense::click()).clicked() {
            let _ = app.run("track.folder_toggle", json!({"track": id.0}));
        }
    }
    let kind_label = match track.kind {
        TrackKind::Audio => "",
        TrackKind::Aux => "AUX",
        TrackKind::Master => "MASTER",
        TrackKind::Midi => "MIDI",
        TrackKind::Instrument => "INST",
        TrackKind::Vca => "VCA",
        TrackKind::Folder => "FOLDER",
        TrackKind::Video => "VIDEO",
    };
    if !kind_label.is_empty() {
        ui.painter().text(pos2(pl_r.max.x + 4.0, name_r.center().y), Align2::LEFT_CENTER, kind_label, bold(9.0), t.text_dim);
    }
    if h < 30.0 {
        return;
    }
    // Buttons row.
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(Rect::from_min_size(pos2(x0, name_r.max.y + 5.0), vec2(170.0, 20.0)))
            .layout(egui::Layout::left_to_right(egui::Align::Min)),
    );
    child.spacing_mut().item_spacing.x = 4.0;
    let m = &track.mixer;
    let bs = vec2(26.0, 18.0);
    if track.kind.has_playlist() {
        if rec_toggle(&mut child, bs, m.record_arm, "Record enable").clicked() {
            let _ = app.run("mix.record_arm", json!({"track": id.0}));
        }
        if text_toggle(&mut child, bs, "I", m.input_monitor, t.input, "Input monitoring").clicked() {
            let _ = app.run("mix.input_monitor", json!({"track": id.0}));
        }
    } else {
        child.add_space(bs.x * 2.0 + 4.0);
    }
    if track.kind != TrackKind::Master {
        if text_toggle(&mut child, bs, "S", m.solo, t.solo, "Solo").clicked() {
            let _ = app.run("mix.solo", json!({"track": id.0}));
        }
        if text_toggle(&mut child, bs, "M", m.mute, t.mute, "Mute").clicked() {
            let _ = app.run("mix.mute", json!({"track": id.0}));
        }
    }
    if h < 60.0 {
        return;
    }
    // View selector.
    let view_r = Rect::from_min_size(pos2(x0, head.min.y + 49.0), vec2(118.0, 16.0));
    let mut c2 = ui.new_child(egui::UiBuilder::new().max_rect(view_r));
    let vresp = selector_box(&mut c2, view_r.width(), view_r.height(), &view_label(&track.view), t.text);
    egui::Popup::menu(&vresp).show(|ui| view_menu(app, ui, track));
    // Automation mode.
    let am_r = Rect::from_min_size(pos2(x0, head.min.y + 68.0), vec2(118.0, 16.0));
    let mode = m.automation_mode;
    let col = match mode {
        soundcraft_model::AutomationMode::Read => t.auto_read,
        soundcraft_model::AutomationMode::Write => t.auto_write,
        soundcraft_model::AutomationMode::Touch | soundcraft_model::AutomationMode::TouchLatch => t.auto_touch,
        soundcraft_model::AutomationMode::Latch => t.auto_latch,
        _ => t.text_dim,
    };
    let mut c3 = ui.new_child(egui::UiBuilder::new().max_rect(am_r));
    let aresp = selector_box(&mut c3, am_r.width(), am_r.height(), mode.label(), col);
    egui::Popup::menu(&aresp).show(|ui| {
        for m in soundcraft_model::AutomationMode::ALL {
            if ui.selectable_label(m == mode, m.label()).clicked() {
                let _ = app.run("mix.automation_mode", json!({"track": id.0, "mode": m.label()}));
            }
        }
    });
    // Inline header meter.
    let md = app.meters.get(&id).copied().unwrap_or_default();
    let mr = Rect::from_min_max(pos2(head.max.x - 16.0, head.min.y + 4.0), pos2(head.max.x - 6.0, head.max.y - 4.0));
    if track.channels() >= 2 {
        let w = mr.width() / 2.0 - 0.5;
        crate::widgets::meter(ui, Rect::from_min_size(mr.min, vec2(w, mr.height())), md.level[0], md.hold[0], md.clip);
        crate::widgets::meter(ui, Rect::from_min_size(pos2(mr.min.x + w + 1.0, mr.min.y), vec2(w, mr.height())), md.level[1], md.hold[1], md.clip);
    } else {
        crate::widgets::meter(ui, mr, md.level[0], md.hold[0], md.clip);
    }
    // Volume readout and inserts summary.
    let info = format!("vol {}  pan {}", crate::widgets::db_text(m.volume_db), m.pan.first().map_or("-".into(), |p| crate::widgets::pan_text(*p)));
    ui.painter().text(pos2(view_r.max.x + 8.0, view_r.center().y), Align2::LEFT_CENTER, info, regular(10.0), t.text_dim);
    let ins: Vec<&str> = m.inserts.iter().flatten().filter_map(|i| soundcraft_dsp::plugin_info(&i.plugin).map(|p| p.short_name)).collect();
    if !ins.is_empty() {
        ui.painter().with_clip_rect(Rect::from_min_max(pos2(am_r.max.x + 6.0, am_r.min.y), pos2(head.max.x - 20.0, am_r.max.y))).text(
            pos2(am_r.max.x + 8.0, am_r.center().y),
            Align2::LEFT_CENTER,
            ins.join(" · "),
            regular(10.0),
            t.text_dim,
        );
    }
}

/// One optional Edit-window column for a track (I/O, inserts, sends, comments).
pub(super) fn header_column(app: &mut SoundApp, ui: &mut Ui, track: &Track, col: &str, r: Rect) {
    let t = Tokens::DARK;
    ui.painter().rect_filled(r, 0.0, t.panel_bg);
    ui.painter().line_segment([pos2(r.min.x, r.min.y), pos2(r.min.x, r.max.y)], Stroke::new(1.0, t.border));
    let line_h = 15.0;
    let rows = ((r.height() - 4.0) / line_h).floor().max(1.0) as usize;
    let item = |i: usize| Rect::from_min_size(pos2(r.min.x + 4.0, r.min.y + 3.0 + i as f32 * line_h), vec2(r.width() - 8.0, line_h - 2.0));
    let text = |ui: &Ui, rr: Rect, s: &str, c: Color32| {
        ui.painter().rect_filled(rr, 2.0, t.slot_bg);
        ui.painter().with_clip_rect(rr).text(pos2(rr.min.x + 4.0, rr.center().y), Align2::LEFT_CENTER, s, regular(10.0), c);
    };
    let route = |r: &soundcraft_model::Route| match r {
        soundcraft_model::Route::None => "none".to_string(),
        soundcraft_model::Route::Main => "Out 1-2".to_string(),
        soundcraft_model::Route::Bus(b) => app.engine.session().bus(*b).map_or("bus".into(), |b| b.name.clone()),
        soundcraft_model::Route::Hardware(h) => h.clone(),
    };
    match col {
        "io" => {
            if rows >= 1 {
                text(ui, item(0), &format!("in: {}", route(&track.mixer.input)), t.text);
            }
            if rows >= 2 {
                text(ui, item(1), &format!("out: {}", route(&track.mixer.output)), t.text);
            }
        }
        "inserts_ae" | "inserts_fj" => {
            let off = if col == "inserts_ae" { 0 } else { 5 };
            for i in 0..5.min(rows) {
                let name = track
                    .mixer
                    .inserts
                    .get(off + i)
                    .cloned()
                    .flatten()
                    .and_then(|x| soundcraft_dsp::plugin_info(&x.plugin).map(|p| p.name))
                    .unwrap_or("");
                text(ui, item(i), name, t.text);
            }
        }
        "sends_ae" | "sends_fj" => {
            let off = if col == "sends_ae" { 0 } else { 5 };
            for i in 0..5.min(rows) {
                let s = track
                    .mixer
                    .sends
                    .get(off + i)
                    .cloned()
                    .flatten()
                    .map(|x| format!("{} {:.0}", route(&x.target), x.level_db.max(-99.0)))
                    .unwrap_or_default();
                text(ui, item(i), &s, t.text);
            }
        }
        _ => {
            let mut c = track.comments.clone();
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(r.shrink(3.0)));
            if child.add(egui::TextEdit::multiline(&mut c).desired_width(r.width() - 6.0).font(regular(10.0))).changed() {
                let _ =
                    app.engine.execute_merged("track.comments", &json!({"track": track.id.0, "comments": c}), &format!("comments:{}", track.id.0));
            }
        }
    }
}

fn view_label(v: &str) -> String {
    match AutoParam::parse(v) {
        Some(p) => p.label(),
        None => v.to_string(),
    }
}

fn view_menu(app: &mut SoundApp, ui: &mut Ui, track: &Track) {
    let id = track.id.0;
    let mut opts: Vec<&str> = Vec::new();
    if track.kind.is_midi() {
        opts.extend(["notes", "regions", "blocks"]);
    } else if track.kind.has_playlist() {
        opts.extend(["waveform", "blocks", "clip_gain", "playlists"]);
    }
    opts.extend(["volume", "pan", "mute"]);
    for o in opts {
        if ui.selectable_label(track.view == o, view_label(o)).clicked() {
            let _ = app.run("track.view", json!({"track": id, "view": o}));
        }
    }
    for (slot, ins) in track.mixer.inserts.iter().enumerate() {
        let Some(ins) = ins else { continue };
        let Some(info) = soundcraft_dsp::plugin_info(&ins.plugin) else { continue };
        ui.menu_button(info.name, |ui| {
            for p in info.params {
                let v = format!("plugin:{}:{}", (b'a' + slot as u8) as char, p.id);
                if ui.button(p.name).clicked() {
                    let _ = app.run("track.view", json!({"track": id, "view": v}));
                }
            }
        });
    }
}

fn track_context_menu(app: &mut SoundApp, ui: &mut Ui, id: TrackId) {
    let tid = id.0;
    let items: [(&str, &str); 10] = [
        ("Rename...", "ui.rename_track"),
        ("Duplicate...", "track.duplicate"),
        ("Make Inactive", "track.make_inactive"),
        ("Hide", "track.hide"),
        ("Delete", "track.delete"),
        ("Split into Mono", "track.split_into_mono"),
        ("Commit...", "track.commit"),
        ("Freeze", "track.freeze"),
        ("Move to New Folder...", "track.move_to_new_folder"),
        ("Group...", "track.group"),
    ];
    for (label, cmd) in items {
        if ui.button(label).clicked() {
            if cmd == "ui.rename_track" {
                let name = app.engine.session().track(id).map(|t| t.name.clone()).unwrap_or_default();
                app.dialogs.open_rename_track(id, &name);
            } else {
                let _ = app.run(cmd, json!({"tracks": [tid]}));
            }
        }
    }
    ui.separator();
    ui.menu_button("Track Height", |ui| {
        for h in TrackHeight::ALL {
            if ui.button(h.label()).clicked() {
                let _ = app.run("track.height", json!({"tracks": [tid], "height": h.label()}));
            }
        }
    });
    ui.menu_button("Track Color", |ui| {
        for (i, c) in soundcraft_model::TRACK_COLORS.iter().enumerate() {
            let (r, resp) = ui.allocate_exact_size(vec2(120.0, 14.0), Sense::click());
            ui.painter().rect_filled(r, 2.0, rgb(*c));
            if resp.clicked() {
                let _ = app.run("track.color", json!({"tracks": [tid], "color": i}));
            }
        }
    });
}
