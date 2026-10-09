//! Stereo pan knob drawing and gesture response.

use crate::theme::Tokens;
use egui::Color32;
use egui::Response;
use egui::Sense;
use egui::Stroke;
use egui::Ui;
use egui::vec2;

/// Rotary knob for pan / send pan (-1..1). Drag vertically; double-click resets.
pub fn pan_knob(ui: &mut Ui, size: f32, value: &mut f32, tip: &str) -> Response {
    let t = Tokens::DARK;
    let (r, resp) = ui.allocate_exact_size(vec2(size, size), Sense::click_and_drag());
    if resp.dragged() {
        let d = resp.drag_delta();
        *value = (*value + (d.x - d.y) * 0.01).clamp(-1.0, 1.0);
    }
    if resp.double_clicked() {
        *value = 0.0;
    }
    let c = r.center();
    let rad = size * 0.42;
    ui.painter().circle(c, rad, Color32::from_rgb(26, 26, 28), Stroke::new(1.0, Color32::from_rgb(90, 90, 94)));
    let n = 24;
    let start = std::f32::consts::PI * 0.75;
    let sweep = std::f32::consts::PI * 1.5;
    let arc = |a0: f32, a1: f32| -> Vec<egui::Pos2> {
        (0..=n)
            .map(|i| {
                let a = a0 + (a1 - a0) * i as f32 / n as f32;
                c + vec2(a.cos(), a.sin()) * (rad + 2.5)
            })
            .collect()
    };
    let mid = start + sweep * 0.5;
    let at = start + sweep * (*value * 0.5 + 0.5);
    let (a0, a1) = if at < mid { (at, mid) } else { (mid, at) };
    ui.painter().add(egui::Shape::line(arc(a0, a1), Stroke::new(2.0, t.counter_text)));
    let tip_pos = c + vec2(at.cos(), at.sin()) * rad * 0.85;
    ui.painter().line_segment([c, tip_pos], Stroke::new(2.0, Color32::from_rgb(230, 230, 230)));
    resp.on_hover_text(tip)
}

/// Format a pan value like a console: "<45", "0", "45>".
pub fn pan_text(v: f32) -> String {
    let n = (v * 100.0).round() as i32;
    match n {
        0 => "0".into(),
        n if n < 0 => format!("<{}", -n),
        n => format!("{n}>"),
    }
}
