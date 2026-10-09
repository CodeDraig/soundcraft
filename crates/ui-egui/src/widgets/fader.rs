//! Gain fader drawing and gesture response.

use crate::theme::Tokens;
use crate::theme::regular;
use egui::Align2;
use egui::Color32;
use egui::Rect;
use egui::Sense;
use egui::Stroke;
use egui::StrokeKind;
use egui::Ui;
use egui::pos2;
use egui::vec2;

/// A fader: returns the new dB value when dragged. `db` in -144..12.
pub fn fader(ui: &mut Ui, r: Rect, db: f32, id: egui::Id) -> Option<f32> {
    let t = Tokens::DARK;
    let resp = ui.interact(r, id, Sense::click_and_drag());
    let slot = Rect::from_center_size(r.center(), vec2(5.0, r.height() - 14.0));
    ui.painter().rect_filled(slot, 2.0, t.fader_track);
    // Scale marks.
    for mark in [12.0f32, 6.0, 0.0, -5.0, -10.0, -15.0, -20.0, -30.0, -40.0, -60.0] {
        let y = slot.max.y - slot.height() * soundcraft_model::fader_db_to_pos(mark);
        ui.painter().line_segment([pos2(r.min.x + 2.0, y), pos2(slot.min.x - 3.0, y)], Stroke::new(1.0, Color32::from_rgb(110, 110, 110)));
        ui.painter().text(pos2(r.min.x, y), Align2::LEFT_CENTER, format!("{}", mark.abs()), regular(8.0), Color32::from_rgb(150, 150, 150));
    }
    let pos = soundcraft_model::fader_db_to_pos(db);
    let y = slot.max.y - slot.height() * pos;
    let cap = Rect::from_center_size(pos2(slot.center().x, y), vec2(r.width() * 0.62, 22.0));
    ui.painter().rect(cap, 2.0, Color32::from_rgb(70, 70, 72), Stroke::new(1.0, Color32::from_rgb(20, 20, 20)), StrokeKind::Inside);
    ui.painter().line_segment(
        [pos2(cap.min.x + 2.0, cap.center().y), pos2(cap.max.x - 2.0, cap.center().y)],
        Stroke::new(2.0, Color32::from_rgb(230, 230, 230)),
    );
    for dy in [-6.0, -3.0, 3.0, 6.0] {
        ui.painter().line_segment(
            [pos2(cap.min.x + 3.0, cap.center().y + dy), pos2(cap.max.x - 3.0, cap.center().y + dy)],
            Stroke::new(1.0, Color32::from_rgb(40, 40, 40)),
        );
    }
    if resp.double_clicked() || (resp.clicked() && ui.input(|i| i.modifiers.alt)) {
        return Some(0.0);
    }
    if resp.dragged() {
        let fine = ui.input(|i| i.modifiers.command || i.modifiers.ctrl);
        let dy = resp.drag_delta().y / slot.height() * if fine { 0.1 } else { 1.0 };
        let np = (pos - dy).clamp(0.0, 1.0);
        return Some(soundcraft_model::fader_pos_to_db(np).clamp(-144.0, 12.0));
    }
    None
}

pub fn db_text(db: f32) -> String {
    if db <= -143.9 { "-inf".into() } else { format!("{db:.1}") }
}
