//! Send slot controls and routing choices.

use super::route_name;
use crate::SoundApp;
use crate::theme::Tokens;
use crate::theme::regular;
use crate::widgets::db_text;
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
use soundcraft_model::Track;
use soundcraft_model::TrackId;

pub(super) fn send_slot(app: &mut SoundApp, ui: &mut Ui, track: &Track, slot: usize, r: Rect) {
    let t = Tokens::DARK;
    let id = track.id;
    let snd = track.mixer.sends.get(slot).cloned().flatten();
    let resp = ui.interact(r, ui.id().with(("snd", id.0, slot)), Sense::click_and_drag());
    let fill = if snd.is_some() { Color32::from_rgb(46, 66, 56) } else { t.slot_bg };
    ui.painter().rect(r, CornerRadius::same(2), fill, Stroke::new(1.0, Color32::from_rgb(16, 16, 16)), StrokeKind::Inside);
    match &snd {
        Some(s) => {
            let name = route_name(app, &s.target);
            // Level bar.
            let k = soundcraft_model::fader_db_to_pos(s.level_db);
            ui.painter().rect_filled(
                Rect::from_min_max(pos2(r.min.x + 1.0, r.max.y - 3.0), pos2(r.min.x + 1.0 + (r.width() - 2.0) * k, r.max.y - 1.0)),
                0.0,
                t.counter_text,
            );
            ui.painter().with_clip_rect(r).text(
                r.center() - vec2(0.0, 1.0),
                Align2::CENTER_CENTER,
                name,
                regular(10.0),
                if s.mute { t.mute } else { t.text },
            );
            if resp.dragged() {
                let db = (s.level_db.max(-60.0) - resp.drag_delta().y * 0.3).clamp(-144.0, 12.0);
                let _ =
                    app.engine.execute_merged("mix.send_level", &json!({"track": id.0, "slot": slot, "db": db}), &format!("send:{}:{slot}", id.0));
            }
            if resp.double_clicked() {
                let _ = app.run("mix.send_level", json!({"track": id.0, "slot": slot, "db": 0.0}));
            }
            let _ = resp.clone().on_hover_text(format!("Send {}: {} dB (drag to change)", (b'A' + slot as u8) as char, db_text(s.level_db)));
        }
        None => {
            ui.painter().circle_filled(pos2(r.min.x + 6.0, r.center().y), 1.5, t.text_dim);
        }
    }
    let popup = if snd.is_none() { egui::Popup::menu(&resp) } else { egui::Popup::context_menu(&resp) };
    popup.show(|ui| {
        if snd.is_some() && ui.button("no send").clicked() {
            let _ = app.run("mix.send_remove", json!({"track": id.0, "slot": slot}));
        }
        let busses: Vec<String> = app.engine.session().busses.iter().map(|b| b.name.clone()).collect();
        for b in busses {
            if ui.button(&b).clicked() {
                let _ = app.run("mix.send", json!({"track": id.0, "slot": slot, "bus": b}));
            }
        }
        ui.separator();
        if ui.button("new bus...").clicked() {
            let n = app.engine.session().busses.len() + 1;
            let _ = app.run("mix.send", json!({"track": id.0, "slot": slot, "bus": format!("Bus {n}")}));
        }
    });
}

pub(super) fn route_menu(app: &mut SoundApp, ui: &mut Ui, id: TrackId, input: bool) {
    let key = if input { "input" } else { "output" };
    let cmd = if input { "track.input" } else { "track.output" };
    if ui.button(if input { "no input" } else { "no output" }).clicked() {
        let _ = app.run(cmd, json!({"track": id.0, key: "none"}));
    }
    let main_name = app.engine.session().outputs.first().map_or_else(|| "Out 1-2".to_string(), |o| o.name.clone());
    if !input && ui.button(main_name).clicked() {
        let _ = app.run(cmd, json!({"track": id.0, key: "main"}));
    }
    if input {
        for i in 1..=8 {
            if ui.button(format!("In {i}")).clicked() {
                let _ = app.run(cmd, json!({"track": id.0, key: format!("In {i}")}));
            }
        }
    }
    ui.separator();
    let busses: Vec<String> = app.engine.session().busses.iter().map(|b| b.name.clone()).collect();
    ui.menu_button("bus", |ui| {
        for b in &busses {
            if ui.button(b).clicked() {
                let _ = app.run(cmd, json!({"track": id.0, key: b}));
            }
        }
        if ui.button("new bus...").clicked() {
            let n = busses.len() + 1;
            let _ = app.run(cmd, json!({"track": id.0, key: format!("Bus {n}")}));
        }
    });
}
