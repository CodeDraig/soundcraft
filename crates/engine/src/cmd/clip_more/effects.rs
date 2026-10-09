//! Clip effects configuration and rendering.

use super::super::*;
use serde_json::json;
use soundcraft_model::ClipId;

pub(super) fn effects_set(e: &mut Engine, p: &Value) -> Result<Value> {
    let ids = clip_ids_param(e, p);
    let params = p.get("params").and_then(Value::as_object).ok_or_else(|| bad("clip.effects_set", "`params` must be an object of numbers"))?;
    let vals: Vec<(String, f64)> =
        params.iter().filter_map(|(k, v)| v.as_f64().filter(|x| x.is_finite()).map(|x| (k.chars().take(64).collect(), x))).collect();
    let s = e.session_mut();
    for c in &ids {
        for (k, v) in &vals {
            s.edit.values.insert(format!("clip_fx.{}.{k}", c.0), *v);
        }
    }
    Ok(json!({"clips": ids.len(), "params": vals.len()}))
}

pub(super) fn effects_bypass(e: &mut Engine, p: &Value) -> Result<Value> {
    let ids = clip_ids_param(e, p);
    if ids.is_empty() {
        return Err(bad("clip.effects_bypass", "select a clip"));
    }
    let s = e.session_mut();
    let key = |c: &ClipId| format!("clip_fx.bypass.{}", c.0);
    let on = p.get("value").and_then(Value::as_bool).unwrap_or_else(|| !ids.iter().all(|c| s.edit.flag(&key(c))));
    for c in &ids {
        s.edit.set_flag(&key(c), on);
    }
    Ok(json!({"bypass": on, "clips": ids.len()}))
}

pub(super) fn effects_render(e: &mut Engine, p: &Value) -> Result<Value> {
    let ids = clip_ids_param(e, p);
    let mut n = 0;
    for id in &ids {
        if crate::io::render_clip_gain(e, *id)? {
            n += 1;
        }
        let s = e.session_mut();
        let pre = format!("clip_fx.{}.", id.0);
        s.edit.values.retain(|k, _| !k.starts_with(&pre));
        s.edit.set_flag(&format!("clip_fx.bypass.{}", id.0), false);
        if let Some(c) = s.find_clip_mut(*id) {
            c.fade_in.len = c.fade_in.len.min(c.length);
            c.fade_out.len = c.fade_out.len.min(c.length);
        }
    }
    Ok(json!({"rendered": n}))
}
