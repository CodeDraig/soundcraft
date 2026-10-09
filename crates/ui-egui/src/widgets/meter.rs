//! Single- and multichannel level-meter drawing.

use crate::theme::Tokens;
use crate::theme::regular;
use egui::Color32;
use egui::Rect;
use egui::Stroke;
use egui::Ui;
use egui::pos2;
use egui::vec2;

/// Vertical peak meter with green/yellow/red zones; values are linear peaks.
pub fn meter(ui: &Ui, r: Rect, level: f32, hold: f32, clip: bool) {
    let t = Tokens::DARK;
    ui.painter().rect_filled(r, 0.0, Color32::from_rgb(8, 8, 8));
    let pos = |v: f32| -> f32 {
        let db = if v <= 1e-6 { -80.0 } else { 20.0 * v.log10() };
        meter_pos(db)
    };
    let h = r.height() - 4.0;
    let inner = Rect::from_min_max(pos2(r.min.x + 1.0, r.min.y + 3.0), pos2(r.max.x - 1.0, r.max.y - 1.0));
    let lv = pos(level);
    if lv > 0.0 {
        let top = inner.max.y - h * lv;
        // Draw in zones.
        let zones =
            [(0.0, meter_pos(-12.0), t.meter_green), (meter_pos(-12.0), meter_pos(-3.0), t.meter_yellow), (meter_pos(-3.0), 1.0, t.meter_red)];
        for (a, b, col) in zones {
            let y0 = inner.max.y - h * a;
            let y1 = (inner.max.y - h * b).max(top);
            if y1 < y0 {
                ui.painter().rect_filled(Rect::from_min_max(pos2(inner.min.x, y1), pos2(inner.max.x, y0)), 0.0, col);
            }
        }
    }
    let hv = pos(hold);
    if hv > 0.01 {
        let y = inner.max.y - h * hv;
        ui.painter().line_segment(
            [pos2(inner.min.x, y), pos2(inner.max.x, y)],
            Stroke::new(1.0, if hv > meter_pos(-3.0) { t.meter_red } else { t.meter_yellow }),
        );
    }
    let clip_r = Rect::from_min_max(r.min, pos2(r.max.x, r.min.y + 2.5));
    ui.painter().rect_filled(clip_r, 0.0, if clip { t.meter_red } else { Color32::from_rgb(60, 20, 20) });
}

/// dB → 0..1 meter position (logarithmic-ish scale like hardware meters).
pub fn meter_pos(db: f32) -> f32 {
    if !db.is_finite() || db <= -80.0 {
        return 0.0;
    }
    if db >= 0.0 {
        return 1.0;
    }
    // -80..-40: bottom 10 %, -40..-20: 20 %, -20..0: 70 %.
    if db < -40.0 {
        (db + 80.0) / 40.0 * 0.1
    } else if db < -20.0 {
        0.1 + (db + 40.0) / 20.0 * 0.2
    } else {
        0.3 + (db + 20.0) / 20.0 * 0.7
    }
}

/// A bank of thin vertical peak meters, one per channel, labelled underneath (rotated text).
pub fn multi_meter(ui: &Ui, r: Rect, levels: &[f32], labels: &[String], clip: bool) {
    let n = levels.len().max(1);
    let label_h = 16.0;
    let mr = Rect::from_min_max(r.min, pos2(r.max.x, r.max.y - label_h));
    let gap = 1.0;
    let w = ((mr.width() - gap * (n as f32 - 1.0)) / n as f32).max(1.0);
    for (i, lv) in levels.iter().enumerate() {
        let x = mr.min.x + i as f32 * (w + gap);
        let br = Rect::from_min_size(pos2(x, mr.min.y), vec2(w, mr.height()));
        meter(ui, br, *lv, *lv, clip);
        if let Some(l) = labels.get(i) {
            let galley = ui.painter().layout_no_wrap(l.clone(), regular(7.0), Tokens::DARK.text_dim);
            let gw = galley.size().x;
            let gh = galley.size().y;
            // Rotated 90° anticlockwise, reading bottom-to-top, centred under the bar.
            let pos = pos2(br.center().x - gh * 0.5, mr.max.y + 2.0 + gw.min(label_h - 2.0));
            ui.painter().add(egui::epaint::TextShape::new(pos, galley, Tokens::DARK.text_dim).with_angle(-std::f32::consts::FRAC_PI_2));
        }
    }
}
