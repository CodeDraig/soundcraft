//! MIDI note operation and real-time-property forms.

use super::result_line;
use crate::SoundApp;
use egui::vec2;
use serde_json::json;

pub(super) fn midi_ops(app: &mut SoundApp, ctx: &egui::Context) {
    let mut open = app.ui.show_midi_ops;
    if !open {
        return;
    }
    egui::Window::new("MIDI Operations").open(&mut open).default_size(vec2(360.0, 220.0)).show(ctx, |ui| {
        egui::ComboBox::from_label("Operation").selected_text(app.ops.midi_op.clone()).show_ui(ui, |ui| {
            for c in ["quantize", "transpose", "change velocity", "flatten performance", "restore performance"] {
                ui.selectable_value(&mut app.ops.midi_op, c.to_string(), c);
            }
        });
        match app.ops.midi_op.as_str() {
            "quantize" => {
                egui::ComboBox::from_label("Grid").selected_text(app.ops.quant_grid.clone()).show_ui(ui, |ui| {
                    for g in ["1/4", "1/8", "1/8t", "1/16", "1/16t", "1/32"] {
                        ui.selectable_value(&mut app.ops.quant_grid, g.to_string(), g);
                    }
                });
                ui.add(egui::Slider::new(&mut app.ops.quant_strength, 0.0..=100.0).text("Strength %"));
                ui.add(egui::Slider::new(&mut app.ops.quant_swing, 0.0..=100.0).text("Swing %"));
            }
            "transpose" => {
                ui.add(egui::Slider::new(&mut app.ops.transpose, -48..=48).text("Semitones"));
            }
            "change velocity" => {
                ui.add(egui::Slider::new(&mut app.ops.velocity_add, -127..=127).text("Add"));
            }
            _ => {}
        }
        ui.label(egui::RichText::new("Applies to the selected MIDI clips (or those under the selection).").small());
        if ui.button(egui::RichText::new("Apply").strong()).clicked() {
            let r = match app.ops.midi_op.as_str() {
                "quantize" => {
                    app.run("event.quantize", json!({"grid": app.ops.quant_grid, "strength": app.ops.quant_strength, "swing": app.ops.quant_swing}))
                }
                "transpose" => app.run("event.transpose", json!({"semitones": app.ops.transpose})),
                "change velocity" => app.run("event.change_velocity", json!({"add": app.ops.velocity_add})),
                "flatten performance" => app.run("event.flatten_performance", json!({})),
                _ => app.run("event.restore_performance", json!({})),
            };
            result_line(ui, &r);
        }
    });
    app.ui.show_midi_ops = open;
}

pub(super) fn rtp(app: &mut SoundApp, ctx: &egui::Context) {
    let mut open = app.ui.show_rtp;
    if !open {
        return;
    }
    egui::Window::new("MIDI Real-Time Properties").open(&mut open).default_size(vec2(340.0, 200.0)).show(ctx, |ui| {
        ui.label("Non-destructive, applied at playback to the selected MIDI tracks.");
        let mut changed = false;
        changed |= ui.add(egui::Slider::new(&mut app.ops.rtp_velocity, -127..=127).text("Velocity +")).changed();
        changed |= ui.add(egui::Slider::new(&mut app.ops.rtp_transpose, -48..=48).text("Transpose")).changed();
        changed |= ui.add(egui::Slider::new(&mut app.ops.rtp_duration, 1.0..=400.0).text("Duration %")).changed();
        changed |= ui.add(egui::Slider::new(&mut app.ops.rtp_delay, -960..=960).text("Delay (ticks)")).changed();
        if changed {
            let p = json!({"velocity": app.ops.rtp_velocity, "transpose": app.ops.rtp_transpose, "duration": app.ops.rtp_duration, "delay": app.ops.rtp_delay});
            let _ = app.engine.execute_merged("event.midi_rtp", &p, "rtp");
        }
        ui.horizontal(|ui| {
            if ui.button("Write to Notes").clicked() {
                let _ = app.run("track.write_midi_rtp", json!({}));
            }
            if ui.button("Clear").clicked() {
                let _ = app.run("event.midi_rtp", json!({"clear": true}));
                app.ops.rtp_velocity = 0;
                app.ops.rtp_transpose = 0;
                app.ops.rtp_duration = 100.0;
                app.ops.rtp_delay = 0;
            }
        });
    });
    app.ui.show_rtp = open;
}
