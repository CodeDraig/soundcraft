//! MCP tool dispatch and bounded command batches.

use super::{Server, error_result, text_result};
use crate::Backend;
use serde_json::Value;
use serde_json::json;

impl<B: Backend> Server<B> {
    pub(super) fn exec(&mut self, cmd: &str, params: Value) -> Result<Value, String> {
        self.backend.call("engine.execute", json!({"command": cmd, "params": params}))
    }

    /// Dispatch a tool call.
    pub fn call_tool(&mut self, name: &str, args: &Value) -> Value {
        let a = |k: &str| args.get(k).cloned();
        let r: Result<Value, String> = match name {
            "list_commands" => self.backend.call("engine.commands", json!({"filter": a("filter")})),
            "execute" => {
                let Some(cmd) = args.get("command").and_then(Value::as_str) else { return error_result("`command` required") };
                self.exec(cmd, a("params").unwrap_or(json!({})))
            }
            "batch" => {
                let keep = args.get("keep_going").and_then(Value::as_bool).unwrap_or(false);
                let calls = args.get("calls").and_then(Value::as_array).cloned().unwrap_or_default();
                let mut out = Vec::new();
                let mut failed = false;
                for c in calls.iter().take(1000) {
                    let cmd = c.get("command").and_then(Value::as_str).unwrap_or("");
                    let r = self.exec(cmd, c.get("params").cloned().unwrap_or(json!({})));
                    match r {
                        Ok(v) => out.push(json!({"command": cmd, "ok": true, "result": v})),
                        Err(e) => {
                            out.push(json!({"command": cmd, "ok": false, "error": e}));
                            failed = true;
                            if !keep {
                                break;
                            }
                        }
                    }
                }
                if failed && !keep { Err(serde_json::to_string(&out).unwrap_or_default()) } else { Ok(json!(out)) }
            }
            "inspect_session" => self.backend.call("session.inspect", json!({"detail": a("detail")})),
            "new_session" => self.exec("session.new", args.clone()),
            "open_session" => self.exec("session.open", args.clone()),
            "save_session" => self.exec("session.save", args.clone()),
            "import_audio" => self.exec("file.import_audio", args.clone()),
            "bounce_mix" => self.exec("file.bounce_mix", args.clone()),
            "parity" => self.backend.call("engine.parity", json!({})),
            "screenshot" => {
                let r = self.backend.call("ui.screenshot", args.clone());
                if let Ok(v) = &r
                    && let Some(b64) = v.get("png_base64").and_then(Value::as_str)
                {
                    return json!({"content": [{"type": "image", "data": b64, "mimeType": "image/png"}], "isError": false});
                }
                r
            }
            "ui_inspect" => self.backend.call("ui.inspect", json!({})),
            "ui_set" => self.backend.call("ui.set", a("state").unwrap_or(json!({}))),
            "menu_invoke" => self.backend.call("ui.menu.invoke", args.clone()),
            "click" => self.backend.call("ui.click", args.clone()),
            "drag" => self.backend.call("ui.drag", args.clone()),
            "key" => self.backend.call("ui.key", args.clone()),
            other => Err(format!("unknown tool `{other}`")),
        };
        match r {
            Ok(v) => text_result(&v),
            Err(e) => error_result(&e),
        }
    }
}
