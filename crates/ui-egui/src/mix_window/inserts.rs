//! Insert slot controls and plugin selection.

use crate::SoundApp;
use crate::theme::Tokens;
use crate::theme::regular;
use egui::Align2;
use egui::Color32;
use egui::CornerRadius;
use egui::Rect;
use egui::Sense;
use egui::Stroke;
use egui::StrokeKind;
use egui::Ui;
use egui::pos2;
use serde_json::json;
use soundcraft_model::Track;
use soundcraft_model::TrackId;

pub(super) fn insert_slot(app: &mut SoundApp, ui: &mut Ui, track: &Track, slot: usize, r: Rect) {
    let t = Tokens::DARK;
    let id = track.id;
    let ins = track.mixer.inserts.get(slot).cloned().flatten();
    let resp = ui.interact(r, ui.id().with(("ins", id.0, slot)), Sense::click());
    let fill = match &ins {
        Some(i) if i.bypass => Color32::from_rgb(70, 56, 30),
        Some(_) => Color32::from_rgb(52, 62, 80),
        None => t.slot_bg,
    };
    ui.painter().rect(
        r,
        CornerRadius::same(2),
        if resp.hovered() { fill.gamma_multiply(1.3) } else { fill },
        Stroke::new(1.0, Color32::from_rgb(16, 16, 16)),
        StrokeKind::Inside,
    );
    let label = ins.as_ref().and_then(|i| plugin_info(&i.plugin)).map_or("", |p| p.short_name);
    if label.is_empty() {
        ui.painter().circle_filled(pos2(r.min.x + 6.0, r.center().y), 1.5, t.text_dim);
    } else {
        ui.painter().with_clip_rect(r).text(r.center(), Align2::CENTER_CENTER, label, regular(10.5), t.text);
    }
    if resp.clicked() && ins.is_some() && !app.ui.plugin_windows.contains(&(id, slot)) {
        app.ui.plugin_windows.push((id, slot));
    }
    let menu_resp = if ins.is_none() { resp.clone() } else { resp.clone().on_hover_text("Click: open plugin · right-click: change") };
    let popup = if ins.is_none() { egui::Popup::menu(&menu_resp) } else { egui::Popup::context_menu(&menu_resp) };
    popup.show(|ui| plugin_menu(app, ui, id, slot, ins.is_some()));
}

pub fn plugin_menu(app: &mut SoundApp, ui: &mut Ui, id: TrackId, slot: usize, occupied: bool) {
    if occupied {
        if ui.button("Bypass").clicked() {
            let _ = app.run("mix.insert_bypass", json!({"track": id.0, "slot": slot}));
        }
        if ui.button("no insert").clicked() {
            let _ = app.run("mix.insert_remove", json!({"track": id.0, "slot": slot}));
        }
        ui.separator();
    }
    let mut cats: Vec<soundcraft_dsp::Category> = Vec::new();
    for p in soundcraft_dsp::plugins() {
        if !p.is_instrument && !cats.contains(&p.category) {
            cats.push(p.category);
        }
    }
    for c in cats {
        ui.menu_button(format!("{c:?}"), |ui| {
            for p in soundcraft_dsp::plugins().iter().filter(|p| p.category == c && !p.is_instrument) {
                if ui.button(p.name).clicked() {
                    let _ = app.run("mix.insert", json!({"track": id.0, "slot": slot, "plugin": p.id}));
                }
            }
        });
    }
    // Third-party CLAP plugins (scanned once, cached by soundcraft-clap-host).
    ui.menu_button("CLAP", |ui| {
        let found: Vec<_> = soundcraft_clap_host::scan().into_iter().filter(|d| !d.is_instrument).collect();
        if found.is_empty() {
            ui.label("No CLAP plugins found");
        }
        for d in found {
            if ui.button(format!("{} ({})", d.name, d.vendor)).clicked() {
                let _ = app.run("mix.insert", json!({"track": id.0, "slot": slot, "plugin": d.id}));
            }
        }
    });
    // Third-party VST3 plugins (scanned once, cached by soundcraft-vst3-host).
    ui.menu_button("VST3", |ui| {
        let found: Vec<_> = soundcraft_vst3_host::scan().into_iter().filter(|d| !d.is_instrument).collect();
        if found.is_empty() {
            ui.label("No VST3 plugins found");
        }
        for d in found {
            if ui.button(format!("{} ({})", d.name, d.vendor)).clicked() {
                let _ = app.run("mix.insert", json!({"track": id.0, "slot": slot, "plugin": d.id}));
            }
        }
    });
    // Audio Units (macOS; scanned once, cached by soundcraft-au-host).
    if cfg!(target_os = "macos") {
        ui.menu_button("Audio Units", |ui| {
            let found: Vec<_> = soundcraft_au_host::scan().into_iter().filter(|d| !d.is_instrument).collect();
            if found.is_empty() {
                ui.label("No Audio Units found");
            }
            for d in found {
                if ui.button(format!("{} ({})", d.name, d.vendor)).clicked() {
                    let _ = app.run("mix.insert", json!({"track": id.0, "slot": slot, "plugin": d.id}));
                }
            }
        });
    }
}

/// A built-in plugin's description, else a hosted CLAP (`clap:<id>`), VST3 (`vst3:<class id>`) or
/// Audio Unit (`au:<type>:<subtype>:<manufacturer>`) plugin's.
pub fn plugin_info(id: &str) -> Option<&'static soundcraft_dsp::PluginInfo> {
    soundcraft_dsp::plugin_info(id)
        .or_else(|| soundcraft_clap_host::plugin_info(id))
        .or_else(|| soundcraft_vst3_host::plugin_info(id))
        .or_else(|| soundcraft_au_host::plugin_info(id))
}
