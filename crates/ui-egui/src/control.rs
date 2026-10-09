//! The JSON control channel's UI side. The app's TCP server turns each request line into a
//! [`ControlRequest`]; they are handled on the UI thread here, so every command an agent sends
//! goes through exactly the same code as a click.
//!
//! Methods:
//! - `engine.execute {command, params}`: run any command programmatically (no dialogs)
//! - `engine.commands {filter?}`, `engine.parity`, `session.inspect {detail?}`, `ui.inspect`
//! - `ui.menu.list`, `ui.menu.invoke {path: "Track > New..." | id}` (menu-style: may open dialogs)
//! - `ui.click {x,y,button?,count?,mods?}`, `ui.move {x,y}`, `ui.drag {x,y,to_x,to_y,steps?}`
//! - `ui.key {key, cmd?, shift?, alt?, ctrl?}`, `ui.text {text}`
//! - `ui.screenshot {path?}` (PNG; base64 when no path), `ui.set {...UiState fields}`
//! - `app.quit`

use crate::SoundApp;
use serde_json::{Value, json};
use std::sync::mpsc::{Receiver, Sender, channel};

mod screenshots;
use screenshots::request_screenshot;
pub use screenshots::{PendingShot, collect_screenshots, png_bytes};

pub struct ControlRequest {
    pub method: String,
    pub params: Value,
    pub reply: Sender<Value>,
}

impl ControlRequest {
    pub fn new(method: impl Into<String>, params: Value) -> (Self, Receiver<Value>) {
        let (tx, rx) = channel();
        (ControlRequest { method: method.into(), params, reply: tx }, rx)
    }
}

fn ok(v: Value) -> Value {
    json!({"ok": true, "result": v})
}

fn err(e: impl std::fmt::Display) -> Value {
    json!({"ok": false, "error": e.to_string()})
}

pub fn drain(app: &mut SoundApp, ctx: &egui::Context) {
    let Some(rx) = app.control_rx.take() else { return };
    let mut n = 0;
    while let Ok(req) = rx.try_recv() {
        n += 1;
        if let Some(v) = handle(app, ctx, &req) {
            let _ = req.reply.send(v);
        }
        if n > 64 {
            break;
        }
    }
    app.control_rx = Some(rx);
    if n > 0 {
        ctx.request_repaint();
    }
}

fn key_from(name: &str) -> Option<egui::Key> {
    egui::Key::from_name(name).or(match name.to_ascii_lowercase().as_str() {
        "space" => Some(egui::Key::Space),
        "enter" | "return" => Some(egui::Key::Enter),
        "escape" | "esc" => Some(egui::Key::Escape),
        "tab" => Some(egui::Key::Tab),
        "home" => Some(egui::Key::Home),
        "end" => Some(egui::Key::End),
        "=" => Some(egui::Key::Equals),
        _ => None,
    })
}

fn mods(p: &Value) -> egui::Modifiers {
    let b = |k: &str| p.get(k).and_then(Value::as_bool).unwrap_or(false);
    let cmd = b("cmd");
    egui::Modifiers {
        alt: b("alt"),
        ctrl: b("ctrl") || (cmd && !cfg!(target_os = "macos")),
        shift: b("shift"),
        mac_cmd: cmd && cfg!(target_os = "macos"),
        command: cmd,
    }
}

fn handle(app: &mut SoundApp, ctx: &egui::Context, req: &ControlRequest) -> Option<Value> {
    let p = &req.params;
    let f = |k: &str| p.get(k).and_then(Value::as_f64).unwrap_or(0.0) as f32;
    Some(match req.method.as_str() {
        "engine.execute" => {
            let id = p.get("command").and_then(Value::as_str).unwrap_or("");
            let params = p.get("params").cloned().unwrap_or(json!({}));
            match app.run(id, params) {
                Ok(v) => ok(v),
                Err(e) => err(e),
            }
        }
        "engine.commands" => {
            let mut list = match app.engine.execute("engine.commands", p) {
                Ok(v) => v,
                Err(e) => return Some(err(e)),
            };
            if let Some(arr) = list.as_array_mut() {
                for (id, label, path, sc) in crate::menus::UI_COMMANDS {
                    arr.push(json!({"id": id, "label": label, "menu": path, "shortcut": sc, "enabled": true, "params": "{}"}));
                }
            }
            ok(list)
        }
        "engine.parity" => ok(crate::menus::parity()),
        "session.inspect" => ok(soundcraft_engine::inspect::session(&app.engine, p.get("detail").and_then(Value::as_str) == Some("full"))),
        "ui.inspect" => {
            let mut v = app.inspect(ctx);
            v["edit_layout"] = json!(app.edit_layout);
            ok(v)
        }
        "ui.menu.list" => ok(json!(
            soundcraft_engine::catalog::catalog()
                .iter()
                .map(|e| json!({"path": e, "command": soundcraft_engine::catalog::implemented_by(e, &crate::menus::ui_aliases())}))
                .collect::<Vec<_>>()
        )),
        "ui.menu.invoke" => {
            let path = p.get("path").and_then(Value::as_str).unwrap_or("");
            let id = p
                .get("id")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| soundcraft_engine::catalog::implemented_by(path, &crate::menus::ui_aliases()));
            match id {
                Some(id) => {
                    crate::menus::invoke_menu(app, &id, path);
                    ok(json!({"invoked": id, "dialog": app.dialogs.open_name()}))
                }
                None => err(format!("menu item `{path}` is not implemented yet")),
            }
        }
        "ui.set" => match app.run("ui.set", p.clone()) {
            Ok(v) => ok(v),
            Err(e) => err(e),
        },
        "ui.move" => {
            app.synthetic.push(egui::Event::PointerMoved(egui::pos2(f("x"), f("y"))));
            ok(json!({}))
        }
        "ui.click" => {
            let pos = egui::pos2(f("x"), f("y"));
            let button = match p.get("button").and_then(Value::as_str) {
                Some("right") | Some("secondary") => egui::PointerButton::Secondary,
                _ => egui::PointerButton::Primary,
            };
            let m = mods(p);
            let count = p.get("count").and_then(Value::as_u64).unwrap_or(1).clamp(1, 3);
            app.synthetic.push(egui::Event::PointerMoved(pos));
            for _ in 0..count {
                app.synthetic.push(egui::Event::PointerButton { pos, button, pressed: true, modifiers: m });
                app.synthetic.push(egui::Event::PointerButton { pos, button, pressed: false, modifiers: m });
            }
            ok(json!({}))
        }
        "ui.drag" => {
            let a = egui::pos2(f("x"), f("y"));
            let b = egui::pos2(f("to_x"), f("to_y"));
            let steps = p.get("steps").and_then(Value::as_u64).unwrap_or(8).clamp(1, 200);
            let m = mods(p);
            app.synthetic.push(egui::Event::PointerMoved(a));
            app.synthetic.push(egui::Event::PointerButton { pos: a, button: egui::PointerButton::Primary, pressed: true, modifiers: m });
            for i in 1..=steps {
                let k = i as f32 / steps as f32;
                app.synthetic.push(egui::Event::PointerMoved(a + (b - a) * k));
            }
            app.synthetic.push(egui::Event::PointerButton { pos: b, button: egui::PointerButton::Primary, pressed: false, modifiers: m });
            ok(json!({}))
        }
        "ui.key" => {
            let name = p.get("key").and_then(Value::as_str).unwrap_or("");
            let Some(key) = key_from(name) else { return Some(err(format!("unknown key `{name}`"))) };
            let m = mods(p);
            app.synthetic.push(egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: m });
            app.synthetic.push(egui::Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers: m });
            ok(json!({}))
        }
        "ui.text" => {
            let t = p.get("text").and_then(Value::as_str).unwrap_or("").to_string();
            app.synthetic.push(egui::Event::Text(t));
            ok(json!({}))
        }
        "ui.screenshot" => {
            request_screenshot(app, ctx, req);
            return None;
        }
        "app.quit" => {
            app.quit_requested = true;
            ok(json!({}))
        }
        other => err(format!("unknown method `{other}`")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_uses_current_method_and_command_field() {
        let mut app = SoundApp::new(soundcraft_engine::Engine::default(), None, crate::Services::default());
        let ctx = egui::Context::default();
        for (method, params) in
            [("command", json!({"command": "track.new"})), ("document.inspect", json!({})), ("engine.execute", json!({"id": "track.new"}))]
        {
            let (req, _) = ControlRequest::new(method, params);
            assert_eq!(handle(&mut app, &ctx, &req).unwrap()["ok"], false);
        }
        assert!(app.engine.session().tracks.is_empty());
        let (req, _) = ControlRequest::new("engine.execute", json!({"command": "track.new", "params": {}}));
        assert_eq!(handle(&mut app, &ctx, &req).unwrap()["ok"], true);
        assert_eq!(app.engine.session().tracks.len(), 1);
    }

    #[test]
    fn current_preferences_require_fields_but_ui_updates_remain_partial() {
        let mut app = SoundApp::new(soundcraft_engine::Engine::default(), None, crate::Services::default());
        let current = serde_json::to_value(&app.ui).unwrap();
        let restored: crate::UiState = serde_json::from_value(current.clone()).unwrap();
        assert_eq!(serde_json::to_value(restored).unwrap(), current);
        for field in current.as_object().unwrap().keys() {
            let mut incomplete = current.clone();
            incomplete.as_object_mut().unwrap().remove(field);
            assert!(serde_json::from_value::<crate::UiState>(incomplete).is_err(), "accepted missing {field}");
        }
        let ops = serde_json::to_value(&app.ops).unwrap();
        assert!(serde_json::from_value::<crate::ops_windows::OpsState>(ops.clone()).is_ok());
        for field in ops.as_object().unwrap().keys() {
            let mut incomplete = ops.clone();
            incomplete.as_object_mut().unwrap().remove(field);
            assert!(serde_json::from_value::<crate::ops_windows::OpsState>(incomplete).is_err(), "accepted missing {field}");
        }
        let ctx = egui::Context::default();
        let (req, _) = ControlRequest::new("ui.set", json!({"window": "Mix"}));
        assert_eq!(handle(&mut app, &ctx, &req).unwrap()["ok"], true);
        let mut expected = current;
        expected["window"] = json!("Mix");
        assert_eq!(serde_json::to_value(&app.ui).unwrap(), expected);
    }
}
