//! Clip effect clipboard operations.

use super::super::*;
use crate::Clipboard;
use serde_json::json;
use soundcraft_model::ClipId;

pub(super) fn has_clip_effects_clipboard(e: &Engine) -> std::result::Result<(), String> {
    if e.clipboard.clip_effects.is_empty() { Err("no clip effects have been copied".into()) } else { Ok(()) }
}

fn fx_prefix(c: ClipId) -> String {
    format!("clip_fx.{}.", c.0)
}

pub(super) fn clip_effects(e: &mut Engine, p: &Value, copy: bool, remove: bool, id: &str) -> Result<Value> {
    let ids = clip_ids_param(e, p);
    let first = *ids.first().ok_or_else(|| bad(id, "select a clip"))?;
    if copy {
        let pre = fx_prefix(first);
        let fx: Vec<(String, f64)> =
            e.session().edit.values.iter().filter_map(|(k, v)| k.strip_prefix(&pre).map(|name| (name.to_string(), *v))).collect();
        if fx.is_empty() {
            return Err(bad(id, "the clip has no clip effects"));
        }
        e.clipboard = Clipboard { clip_effects: fx, ..Clipboard::default() };
    }
    let mut removed = 0;
    if remove {
        let s = e.session_mut();
        for c in &ids {
            let pre = fx_prefix(*c);
            let before = s.edit.values.len();
            s.edit.values.retain(|k, _| !k.starts_with(&pre));
            removed += before - s.edit.values.len();
            s.edit.set_flag(&format!("clip_fx.bypass.{}", c.0), false);
        }
    }
    Ok(json!({"clips": ids.len(), "copied": e.clipboard.clip_effects.len(), "removed": removed}))
}

pub(super) fn paste_clip_effects(e: &mut Engine, p: &Value) -> Result<Value> {
    let ids = clip_ids_param(e, p);
    if ids.is_empty() {
        return Err(bad("edit.paste_clip_effects", "select a clip"));
    }
    let fx = e.clipboard.clip_effects.clone();
    let s = e.session_mut();
    for c in &ids {
        let pre = fx_prefix(*c);
        s.edit.values.retain(|k, _| !k.starts_with(&pre));
        for (k, v) in &fx {
            s.edit.values.insert(format!("{pre}{k}"), *v);
        }
    }
    Ok(json!({"clips": ids.len()}))
}
