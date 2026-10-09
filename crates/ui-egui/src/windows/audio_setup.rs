//! Playback engine and I/O setup windows.

use super::win;
use crate::SoundApp;
use egui::vec2;
use serde_json::json;

/// Setup › Playback Engine / Hardware.
pub fn playback_engine(app: &mut SoundApp, ctx: &egui::Context) {
    let mut open = app.ui.show_playback_engine;
    win(ctx, &mut open, "Playback Engine", vec2(380.0, 220.0), |ui| {
        egui::Grid::new("pe").num_columns(2).show(ui, |ui| {
            ui.label("Output device");
            ui.label(app.player.as_ref().map_or("none".to_string(), |p| p.device_name.clone()));
            ui.end_row();
            ui.label("Device rate");
            ui.label(app.player.as_ref().map_or("-".to_string(), |p| format!("{} Hz", p.device_rate)));
            ui.end_row();
            ui.label("Session rate");
            ui.label(format!("{} Hz", app.engine.session().sample_rate.hz()));
            ui.end_row();
            ui.label("Input device");
            ui.label(
                app.recorder
                    .as_ref()
                    .map_or("opened on first record".to_string(), |r| format!("{} ({} ch @ {} Hz)", r.device_name, r.channels, r.sample_rate)),
            );
            ui.end_row();
            ui.label("Mix block size");
            ui.label("512 samples (resampled to the device rate when they differ)");
            ui.end_row();
        });
        let mut dc = app.engine.session().edit.delay_compensation;
        if ui.checkbox(&mut dc, "Delay compensation").changed() {
            let _ = app.run("options.delay_compensation", json!({"value": dc}));
        }
        if ui.button("Reconnect audio device").clicked() {
            app.player = Some(soundcraft_playback::Player::new(app.engine.session_arc()));
        }
    });
    app.ui.show_playback_engine = open;
}

/// Setup › I/O: busses and output paths.
pub fn io_setup(app: &mut SoundApp, ctx: &egui::Context) {
    let mut open = app.ui.show_io_setup;
    win(ctx, &mut open, "I/O Setup", vec2(420.0, 300.0), |ui| {
        ui.label(egui::RichText::new("Busses").strong());
        let busses: Vec<(String, String)> = app.engine.session().busses.iter().map(|b| (b.name.clone(), b.format.label().to_string())).collect();
        egui::Grid::new("busses").num_columns(3).striped(true).show(ui, |ui| {
            for (name, fmt) in busses {
                let key = egui::Id::new(("bus_name", name.clone()));
                let mut edit: String = ui.ctx().memory(|m| m.data.get_temp(key)).unwrap_or_else(|| name.clone());
                let r = ui.text_edit_singleline(&mut edit);
                ui.ctx().memory_mut(|m| m.data.insert_temp(key, edit.clone()));
                if r.lost_focus() && edit != name {
                    let _ = app.run("mix.bus_rename", json!({"bus": name, "name": edit}));
                }
                ui.label(fmt);
                if ui.small_button("Delete").clicked() {
                    let _ = app.run("mix.bus_delete", json!({"bus": name}));
                }
                ui.end_row();
            }
        });
        if ui.button("New Bus").clicked() {
            let n = app.engine.session().busses.len() + 1;
            let _ = app.run("mix.new_bus", json!({"name": format!("Bus {n}")}));
        }
        ui.separator();
        ui.label(egui::RichText::new("Outputs").strong());
        // Main output format: stereo, or a surround format (the mix and master faders follow).
        let main = app.engine.session().main_format();
        ui.horizontal(|ui| {
            ui.label("Main output");
            egui::ComboBox::from_id_salt("main_output_format").selected_text(main.label()).show_ui(ui, |ui| {
                for f in soundcraft_model::ChannelFormat::ALL.into_iter().filter(|f| (2..=16).contains(&f.channels())) {
                    if ui.selectable_label(f == main, f.label()).clicked() && f != main {
                        let _ = app.run("setup.main_format", json!({"format": f.label()}));
                    }
                }
            });
            ui.label(egui::RichText::new(format!("{} ch", main.channels())).small());
        });
        for o in app.engine.session().outputs.clone() {
            ui.label(format!("{} — {} from channel {}", o.name, o.format.label(), o.first_channel + 1));
        }
    });
    app.ui.show_io_setup = open;
}
