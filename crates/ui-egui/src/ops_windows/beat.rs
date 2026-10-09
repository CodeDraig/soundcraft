//! Beat Detective analysis and application form.

use crate::SoundApp;
use egui::vec2;
use serde_json::json;

pub(super) fn beat_detective(app: &mut SoundApp, ctx: &egui::Context) {
    let mut open = app.ui.show_beat_detective;
    if !open {
        return;
    }
    egui::Window::new("Beat Detective").open(&mut open).default_size(vec2(360.0, 220.0)).show(ctx, |ui| {
        ui.label("Analyses the edit selection on the selected tracks.");
        ui.add(egui::Slider::new(&mut app.ops.beat_sensitivity, 0.0..=1.0).text("Sensitivity"));
        ui.checkbox(&mut app.ops.beat_separate, "Separate clips at detected beats");
        ui.checkbox(&mut app.ops.beat_set_tempo, "Set the session tempo from the beats");
        ui.horizontal(|ui| {
            if ui.button("Analyze").clicked() {
                let r = app.run("event.beat_detective", json!({"sensitivity": app.ops.beat_sensitivity, "separate": false, "set_tempo": false}));
                app.ops.beat_result = r.map_or_else(|e| e, |v| v.to_string());
            }
            if ui.button(egui::RichText::new("Apply").strong()).clicked() {
                let r = app.run(
                    "event.beat_detective",
                    json!({"sensitivity": app.ops.beat_sensitivity, "separate": app.ops.beat_separate, "set_tempo": app.ops.beat_set_tempo}),
                );
                app.ops.beat_result = r.map_or_else(|e| e, |v| v.to_string());
            }
        });
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Identify Beat: selection is");
            ui.add(egui::DragValue::new(&mut app.ops.identify_bars).range(1..=64));
            ui.label("bar(s)");
            if ui.button("Set Tempo").clicked() {
                let r = app.run("event.identify_beat", json!({"bars": app.ops.identify_bars}));
                app.ops.beat_result = r.map_or_else(|e| e, |v| v.to_string());
            }
        });
        if !app.ops.beat_result.is_empty() {
            let s = app.ops.beat_result.clone();
            ui.label(egui::RichText::new(if s.len() > 300 { format!("{}…", &s[..300]) } else { s }).small());
        }
    });
    app.ui.show_beat_detective = open;
}
