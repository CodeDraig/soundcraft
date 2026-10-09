//! Model Context Protocol server (stdio, newline-delimited JSON-RPC 2.0).

use crate::Backend;
use serde_json::{Value, json};
use std::io::{BufRead, Write};

mod tool_catalog;
pub use tool_catalog::tool_definitions;

mod tool_calls;

pub const PROTOCOL_VERSION: &str = "2025-06-18";

const INSTRUCTIONS: &str = "SoundCraft is a digital audio workstation (Pro Tools-class). Everything a user can do is a \
command: call `list_commands` to discover ids and their params, then `execute` (or `batch`). Positions are samples \
(integers), `{\"seconds\": x}`, or time strings like \"bars_beats:5|1|000\" / \"0:12.500\". Tracks are referenced by id \
or name. Verify your work with `inspect_session` (tracks, clips, mixer, automation, markers, selection). Programmatic \
calls never open dialogs. Tools marked APP_ONLY need the desktop app (`soundcraft --control 0` + `soundcraft-cli mcp --connect PORT`).";

pub struct Server<B: Backend> {
    pub backend: B,
}

fn text_result(v: &Value) -> Value {
    let text = serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string());
    json!({"content": [{"type": "text", "text": text}], "isError": false})
}

fn error_result(e: &str) -> Value {
    json!({"content": [{"type": "text", "text": e}], "isError": true})
}

impl<B: Backend> Server<B> {
    pub fn new(backend: B) -> Self {
        Server { backend }
    }

    /// Handle one JSON-RPC message; returns the reply line (None for notifications).
    pub fn handle(&mut self, msg: &Value) -> Option<Value> {
        let id = msg.get("id").cloned();
        let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
        let params = msg.get("params").cloned().unwrap_or(json!({}));
        let id = id?;
        let result: Result<Value, (i64, String)> = match method {
            "initialize" => Ok(json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {"tools": {}, "resources": {}},
                "serverInfo": {"name": "soundcraft", "version": env!("CARGO_PKG_VERSION")},
                "instructions": format!("{INSTRUCTIONS}\nBackend: {}.", self.backend.describe()),
            })),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({"tools": tool_definitions()})),
            "tools/call" => {
                let name = params.get("name").and_then(Value::as_str).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                Ok(self.call_tool(name, &args))
            }
            "resources/list" => Ok(json!({"resources": [
                {"uri": "soundcraft://session", "name": "Session", "mimeType": "application/json", "description": "The open session as JSON"},
                {"uri": "soundcraft://commands", "name": "Commands", "mimeType": "application/json", "description": "All commands"},
            ]})),
            "resources/templates/list" => Ok(json!({"resourceTemplates": []})),
            "resources/read" => {
                let uri = params.get("uri").and_then(Value::as_str).unwrap_or("");
                let r = match uri {
                    "soundcraft://session" => self.backend.call("session.inspect", json!({"detail": "full"})),
                    "soundcraft://commands" => self.backend.call("engine.commands", json!({})),
                    _ => Err(format!("unknown resource {uri}")),
                };
                match r {
                    Ok(v) => Ok(json!({"contents": [{"uri": uri, "mimeType": "application/json", "text": v.to_string()}]})),
                    Err(e) => Err((-32002, e)),
                }
            }
            "" => Err((-32600, "invalid request".into())),
            other => Err((-32601, format!("method not found: {other}"))),
        };
        Some(match result {
            Ok(r) => json!({"jsonrpc": "2.0", "id": id, "result": r}),
            Err((code, m)) => json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": m}}),
        })
    }

    /// Serve until EOF.
    pub fn serve(&mut self, input: impl BufRead, mut out: impl Write) -> std::io::Result<()> {
        for line in input.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let reply = match serde_json::from_str::<Value>(&line) {
                Ok(Value::Array(batch)) => {
                    let replies: Vec<Value> = batch.iter().filter_map(|m| self.handle(m)).collect();
                    if replies.is_empty() { None } else { Some(Value::Array(replies)) }
                }
                Ok(msg) => self.handle(&msg),
                Err(e) => Some(json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": format!("parse error: {e}")}})),
            };
            if let Some(r) = reply {
                writeln!(out, "{r}")?;
                out.flush()?;
            }
        }
        Ok(())
    }
}
