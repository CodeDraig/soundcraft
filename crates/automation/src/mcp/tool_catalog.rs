//! MCP tool descriptions and argument schemas.

use serde_json::Value;
use serde_json::json;

pub(super) fn tool(name: &str, desc: &str, props: Value, required: &[&str]) -> Value {
    json!({"name": name, "description": desc, "inputSchema": {"type": "object", "properties": props, "required": required}})
}

pub fn tool_definitions() -> Vec<Value> {
    vec![
        tool(
            "list_commands",
            "List SoundCraft commands (id, label, menu path, shortcut, params doc, enabled).",
            json!({"filter": {"type": "string", "description": "substring of id or label"}}),
            &[],
        ),
        tool(
            "execute",
            "Run one command by id with JSON params. Returns the command's result.",
            json!({"command": {"type": "string"}, "params": {"type": "object"}}),
            &["command"],
        ),
        tool(
            "batch",
            "Run several commands in order; stops at the first error unless keep_going.",
            json!({"calls": {"type": "array", "items": {"type": "object", "properties": {"command": {"type": "string"}, "params": {"type": "object"}}}}, "keep_going": {"type": "boolean"}}),
            &["calls"],
        ),
        tool(
            "inspect_session",
            "The whole session as JSON: tracks (clips, inserts, sends, automation), busses, markers, groups, selection, transport.",
            json!({"detail": {"type": "string", "enum": ["summary", "full"]}}),
            &[],
        ),
        tool(
            "new_session",
            "Start a new session (optionally the built-in demo).",
            json!({"name": {"type": "string"}, "sample_rate": {"type": "integer"}, "template": {"type": "string", "enum": ["blank", "demo"]}}),
            &[],
        ),
        tool("open_session", "Open a .scraft session file.", json!({"path": {"type": "string"}}), &["path"]),
        tool("save_session", "Save the session (path required the first time).", json!({"path": {"type": "string"}}), &[]),
        tool(
            "import_audio",
            "Import an audio file (WAV, AIFF, FLAC, MP3, OGG, AAC/M4A, ALAC, CAF) onto a new or existing track.",
            json!({"path": {"type": "string"}, "track": {"type": ["string", "integer"]}, "at": {}}),
            &["path"],
        ),
        tool(
            "bounce_mix",
            "Render the mix to an audio file. Returns peak dBFS and integrated LUFS.",
            json!({"path": {"type": "string"}, "format": {"type": "string", "enum": ["wav", "aiff", "flac"]}, "bit_depth": {"type": "string"}, "start": {}, "end": {}, "normalize": {"type": "boolean"}}),
            &["path"],
        ),
        tool("parity", "Feature-parity report against the incumbent's menu catalog.", json!({}), &[]),
        tool("screenshot", "APP_ONLY. Screenshot the app window (PNG).", json!({"path": {"type": "string"}}), &[]),
        tool("ui_inspect", "APP_ONLY. UI state: front window, panels, layout rectangles of tracks, dialogs.", json!({}), &[]),
        tool("ui_set", "APP_ONLY. Set UI state fields, e.g. {\"window\": \"Mix\"}.", json!({"state": {"type": "object"}}), &["state"]),
        tool(
            "menu_invoke",
            "APP_ONLY. Invoke a menu item like a click (may open its dialog), e.g. \"Track > New...\".",
            json!({"path": {"type": "string"}}),
            &["path"],
        ),
        tool(
            "click",
            "APP_ONLY. Click at window coordinates (points).",
            json!({"x": {"type": "number"}, "y": {"type": "number"}, "button": {"type": "string"}, "count": {"type": "integer"}}),
            &["x", "y"],
        ),
        tool(
            "drag",
            "APP_ONLY. Drag with the primary button.",
            json!({"x": {"type": "number"}, "y": {"type": "number"}, "to_x": {"type": "number"}, "to_y": {"type": "number"}}),
            &["x", "y", "to_x", "to_y"],
        ),
        tool(
            "key",
            "APP_ONLY. Press a key with modifiers.",
            json!({"key": {"type": "string"}, "cmd": {"type": "boolean"}, "shift": {"type": "boolean"}, "alt": {"type": "boolean"}, "ctrl": {"type": "boolean"}}),
            &["key"],
        ),
    ]
}
