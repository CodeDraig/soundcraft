//! Helpers shared by the newer command modules: settings kept in `EditState.flags`/`values`,
//! per-track settings, target playlists, clip-gain envelopes and stretch rendering.

use super::*;
use serde_json::json;
use soundcraft_model::{EditState, TrackId};

mod playlists;
pub use playlists::{activate_playlist, target_playlist};
mod clip_gain;
pub use clip_gain::{clip_db, set_clip_env};
mod stretch;
pub use stretch::{render_stretched, replace_clip};

/// Toggle (or set from `value`) a flag in `edit.flags`; returns the new state.
pub fn toggle_flag(e: &mut Engine, p: &Value, id: &str) -> Result<Value> {
    let s = e.session_mut();
    let on = p.get("value").and_then(Value::as_bool).unwrap_or(!s.edit.flag(id));
    s.edit.set_flag(id, on);
    Ok(json!({"flag": id, "value": on}))
}

/// Turn `id` on and every other flag of `group` off (radio-style menu items).
pub fn exclusive_flag(e: &mut Engine, id: &str, group: &[&str]) -> Result<Value> {
    let s = e.session_mut();
    for g in group {
        s.edit.set_flag(g, false);
    }
    s.edit.set_flag(id, true);
    Ok(json!({"flag": id, "value": true}))
}

/// Set several flags at once.
pub fn set_flags(e: &mut Engine, ids: &[&str], on: bool) -> Value {
    let s = e.session_mut();
    for id in ids {
        s.edit.set_flag(id, on);
    }
    json!({"flags": ids, "value": on})
}

/// String settings live in `edit.flags` as `key=value` (one per key).
pub fn set_text(edit: &mut EditState, key: &str, value: Option<&str>) {
    let prefix = format!("{key}=");
    edit.flags.retain(|f| !f.starts_with(&prefix));
    if let Some(v) = value.map(str::trim).filter(|v| !v.is_empty()) {
        let v: String = v.chars().take(200).collect();
        edit.flags.insert(format!("{prefix}{v}"));
    }
}

pub fn text(edit: &EditState, key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    edit.flags.iter().find_map(|f| f.strip_prefix(&prefix).map(str::to_string))
}

/// All `values` whose key starts with `prefix`, as a JSON object (prefix stripped).
pub fn values_with_prefix(edit: &EditState, prefix: &str) -> Value {
    let m: serde_json::Map<String, Value> =
        edit.values.iter().filter_map(|(k, v)| k.strip_prefix(prefix).map(|rest| (rest.to_string(), json!(v)))).collect();
    Value::Object(m)
}

/// Store a finite number from `p[key]` into `values[target]`, clamped. Returns whether it was given.
pub fn store_value(edit: &mut EditState, p: &Value, key: &str, target: &str, lo: f64, hi: f64) -> bool {
    match p.get(key).and_then(Value::as_f64).filter(|v| v.is_finite()) {
        Some(v) => {
            edit.values.insert(target.to_string(), v.clamp(lo, hi));
            true
        }
        None => false,
    }
}

/// Key for a per-track value, e.g. `rtp.12.velocity`.
pub fn track_key(prefix: &str, t: TrackId, name: &str) -> String {
    format!("{prefix}.{}.{name}", t.0)
}

#[cfg(test)]
mod tests {
    use super::clip_gain::env_db;

    use super::*;

    #[test]
    fn text_settings_round_trip() {
        let mut ed = EditState::default();
        set_text(&mut ed, "engine.device", Some("Built-in"));
        set_text(&mut ed, "engine.device", Some("Interface"));
        assert_eq!(text(&ed, "engine.device").as_deref(), Some("Interface"));
        set_text(&mut ed, "engine.device", None);
        assert_eq!(text(&ed, "engine.device"), None);
    }

    #[test]
    fn envelope_interpolates() {
        let env = [(0, 0.0f32), (100, -10.0)];
        assert!((env_db(&env, 50) + 5.0).abs() < 1e-6);
        assert_eq!(env_db(&[], 50), 0.0);
        assert_eq!(env_db(&env, 500), -10.0);
    }
}
