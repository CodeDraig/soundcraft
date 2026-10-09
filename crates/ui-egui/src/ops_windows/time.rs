//! Timeline insertion and meter operation form.

use super::result_line;
use crate::SoundApp;
use egui::vec2;
use serde_json::json;

pub(super) fn time_ops(app: &mut SoundApp, ctx: &egui::Context) {
    let mut open = app.ui.show_time_ops;
    if !open {
        return;
    }
    egui::Window::new("Time Operations").open(&mut open).default_size(vec2(360.0, 200.0)).show(ctx, |ui| {
        egui::ComboBox::from_label("Operation").selected_text(app.ops.time_op.clone()).show_ui(ui, |ui| {
            for c in ["insert", "cut", "change meter", "move song start"] {
                ui.selectable_value(&mut app.ops.time_op, c.to_string(), c);
            }
        });
        match app.ops.time_op.as_str() {
            "change meter" => {
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut app.ops.meter_num).range(1..=64));
                    ui.label("/");
                    egui::ComboBox::from_id_salt("den").selected_text(app.ops.meter_den.to_string()).show_ui(ui, |ui| {
                        for d in [2u32, 4, 8, 16] {
                            ui.selectable_value(&mut app.ops.meter_den, d, d.to_string());
                        }
                    });
                });
            }
            "insert" => {
                ui.add(egui::DragValue::new(&mut app.ops.time_seconds).range(0.0..=3600.0).suffix(" s"));
            }
            _ => {
                ui.label(egui::RichText::new("Uses the edit selection.").small());
            }
        }
        if ui.button(egui::RichText::new("Apply").strong()).clicked() {
            let at = app.engine.session().edit.selection.start;
            let r = match app.ops.time_op.as_str() {
                "insert" => app.run("event.insert_time", json!({"start": at, "length": {"seconds": app.ops.time_seconds}})),
                "cut" => app.run("event.cut_time", json!({})),
                "change meter" => app.run("event.meter", json!({"numerator": app.ops.meter_num, "denominator": app.ops.meter_den, "at": at})),
                _ => app.run("event.move_song_start", json!({"to": at})),
            };
            result_line(ui, &r);
        }
    });
    app.ui.show_time_ops = open;
}
