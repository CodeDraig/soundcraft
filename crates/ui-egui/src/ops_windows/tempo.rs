//! Tempo curve operation form.

use super::result_line;
use crate::SoundApp;
use egui::vec2;
use serde_json::json;

pub(super) fn tempo_ops(app: &mut SoundApp, ctx: &egui::Context) {
    let mut open = app.ui.show_tempo_ops;
    if !open {
        return;
    }
    egui::Window::new("Tempo Operations").open(&mut open).default_size(vec2(360.0, 200.0)).show(ctx, |ui| {
        egui::ComboBox::from_label("Curve").selected_text(app.ops.tempo_curve.clone()).show_ui(ui, |ui| {
            for c in ["constant", "linear", "parabolic", "s-curve", "scale", "stretch"] {
                ui.selectable_value(&mut app.ops.tempo_curve, c.to_string(), c);
            }
        });
        ui.add(egui::DragValue::new(&mut app.ops.tempo_start).range(5.0..=999.0).prefix("start bpm "));
        ui.add(egui::DragValue::new(&mut app.ops.tempo_end).range(5.0..=999.0).prefix("end bpm "));
        ui.label(egui::RichText::new("Applies across the edit selection (constant: at the insertion point; scale: factor = end / start).").small());
        if ui.button(egui::RichText::new("Apply").strong()).clicked() {
            let (id, p) = match app.ops.tempo_curve.as_str() {
                "constant" => ("event.tempo_constant", json!({"bpm": app.ops.tempo_start})),
                "parabolic" => ("event.tempo_parabolic", json!({"start_bpm": app.ops.tempo_start, "end_bpm": app.ops.tempo_end})),
                "s-curve" => ("event.tempo_s_curve", json!({"start_bpm": app.ops.tempo_start, "end_bpm": app.ops.tempo_end})),
                "scale" => ("event.tempo_scale", json!({"factor": app.ops.tempo_end / app.ops.tempo_start.max(1.0)})),
                "stretch" => ("event.tempo_stretch", json!({"end_bpm": app.ops.tempo_end})),
                _ => ("event.tempo_linear", json!({"start_bpm": app.ops.tempo_start, "end_bpm": app.ops.tempo_end})),
            };
            let r = app.run(id, p);
            result_line(ui, &r);
        }
    });
    app.ui.show_tempo_ops = open;
}
