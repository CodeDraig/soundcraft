//! Current plugin-preset persistence and preset menu.

use crate::SoundApp;
use egui::Ui;
use serde_json::json;

/// Folder for a plugin's user presets.
fn preset_dir(app: &SoundApp, plugin: &str) -> Option<std::path::PathBuf> {
    app.preset_dir.as_ref().map(|d| d.join(crate::preset_folder_name(plugin)))
}

pub(super) fn preset_menu(
    app: &mut SoundApp,
    ui: &mut Ui,
    tid: soundcraft_model::TrackId,
    slot: usize,
    info: &soundcraft_dsp::PluginInfo,
    ins: &soundcraft_model::Insert,
) {
    if ui.button("<factory default>").clicked() {
        let defaults: serde_json::Map<String, serde_json::Value> = info.params.iter().map(|p| (p.id.to_string(), json!(p.default))).collect();
        let _ = app.run("mix.insert_params", json!({"track": tid.0, "slot": slot, "params": defaults, "preset": "<factory default>"}));
        ui.close();
    }
    let Some(dir) = preset_dir(app, info.id) else {
        ui.label("User presets need the desktop app.");
        return;
    };
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .map(|rd| rd.flatten().filter_map(|e| e.path().file_stem().and_then(|x| x.to_str()).map(str::to_string)).collect())
        .unwrap_or_default();
    names.sort();
    for n in &names {
        if ui.button(n).clicked() {
            let loaded =
                std::fs::read_to_string(dir.join(format!("{n}.json"))).map_err(|e| e.to_string()).and_then(|text| PluginPreset::read(&text, info.id));
            match loaded {
                Ok(preset) => {
                    let _ = app.run("mix.insert_params", json!({"track": tid.0, "slot": slot, "params": preset.params, "preset": n}));
                }
                Err(e) => app.ui.status = format!("Could not load preset {n}: {e}"),
            }
            ui.close();
        }
    }
    ui.separator();
    let key = egui::Id::new(("preset_name", tid.0, slot));
    let mut name: String = ui.ctx().memory(|m| m.data.get_temp(key)).unwrap_or_else(|| format!("{} preset", info.name));
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut name).desired_width(140.0));
        if ui.button("Save").clicked() {
            let clean = soundcraft_engine::io::sanitize_name(&name);
            let body = serde_json::to_value(PluginPreset { format: PRESET_FORMAT.into(), plugin: info.id.into(), params: ins.params.clone() });
            let ok = body.is_ok_and(|body| {
                std::fs::create_dir_all(&dir).is_ok() && std::fs::write(dir.join(format!("{clean}.json")), body.to_string()).is_ok()
            });
            app.ui.status = if ok { format!("Saved preset {clean}") } else { "Could not save the preset".into() };
            if ok {
                let _ = app.run("mix.insert_params", json!({"track": tid.0, "slot": slot, "params": {}, "preset": clean}));
            }
        }
    });
    ui.ctx().memory_mut(|m| m.data.insert_temp(key, name));
}

const PRESET_FORMAT: &str = "soundcraft-plugin-preset";

#[derive(serde::Serialize, serde::Deserialize)]
struct PluginPreset {
    format: String,
    plugin: String,
    params: std::collections::BTreeMap<String, f32>,
}

impl PluginPreset {
    fn read(text: &str, plugin: &str) -> Result<Self, String> {
        let preset: Self = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if preset.format != PRESET_FORMAT || preset.plugin != plugin {
            return Err("not a preset for this SoundCraft plugin".into());
        }
        Ok(preset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_presets_require_current_identity_and_parameters() {
        let current = json!({"format": PRESET_FORMAT, "plugin": "gain", "params": {"gain_db": -6.0}});
        let preset = PluginPreset::read(&current.to_string(), "gain").unwrap();
        assert_eq!(serde_json::to_value(preset).unwrap(), current);
        for field in ["format", "plugin", "params"] {
            let mut missing = current.clone();
            missing.as_object_mut().unwrap().remove(field);
            assert!(PluginPreset::read(&missing.to_string(), "gain").is_err());
        }
        assert!(PluginPreset::read(&current.to_string(), "compressor").is_err());
        let mut wrong_format = current;
        wrong_format["format"] = json!("other");
        assert!(PluginPreset::read(&wrong_format.to_string(), "gain").is_err());
    }
}
