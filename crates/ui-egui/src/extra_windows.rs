//! Window menu extras: the Renderer (speaker layout of the main output with live levels and the
//! object tracks' positions), UI Customization, and auto-updating the active window configuration.

use crate::SoundApp;
use serde_json::{Value, json};

mod renderer;
use renderer::renderer;
mod customization;
use customization::{apply_customization, ui_customization};

/// How often an auto-updating configuration is refreshed (seconds).
const AUTO_UPDATE_SECS: f64 = 2.0;

#[derive(Debug, Clone)]
pub struct ExtraState {
    pub show_renderer: bool,
    pub show_ui_customization: bool,
    pub auto_update_config: bool,
    pub ui_scale: f32,
    pub reduce_motion: bool,
    pub tooltips: bool,
    last_auto_update: f64,
    applied: Option<(f32, bool)>,
}

impl Default for ExtraState {
    fn default() -> Self {
        Self {
            show_renderer: false,
            show_ui_customization: false,
            auto_update_config: false,
            ui_scale: 1.0,
            reduce_motion: false,
            tooltips: true,
            last_auto_update: 0.0,
            applied: None,
        }
    }
}

/// UI commands handled here (`window.renderer`, `window.ui_customization`, `window.config_auto_update`).
pub fn run(app: &mut SoundApp, id: &str, p: &Value) -> Option<Value> {
    let flag = |v: &mut bool| {
        *v = p.get("value").and_then(Value::as_bool).unwrap_or(!*v);
        json!({"value": *v})
    };
    Some(match id {
        "window.renderer" => flag(&mut app.extra.show_renderer),
        "window.ui_customization" => flag(&mut app.extra.show_ui_customization),
        "window.config_auto_update" => flag(&mut app.extra.auto_update_config),
        _ => return None,
    })
}

pub fn show(app: &mut SoundApp, ctx: &egui::Context) {
    apply_customization(app, ctx);
    auto_update(app, ctx);
    renderer(app, ctx);
    ui_customization(app, ctx);
}

fn auto_update(app: &mut SoundApp, ctx: &egui::Context) {
    if !app.extra.auto_update_config || app.ui.configurations.is_empty() {
        return;
    }
    let now = ctx.input(|i| i.time);
    if now - app.extra.last_auto_update >= AUTO_UPDATE_SECS {
        app.extra.last_auto_update = now;
        let _ = crate::menus::run_ui_command(app, "window.config_update", &json!({}));
    }
}
