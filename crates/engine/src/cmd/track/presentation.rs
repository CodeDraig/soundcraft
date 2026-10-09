//! Track labels, colors, height and view.

use super::super::*;
use serde_json::json;
use soundcraft_model::TrackHeight;

pub(super) fn rename(e: &mut Engine, p: &Value) -> Result<Value> {
    let t = track_param(e, "track.rename", p, "track")?.ok_or_else(|| bad("track.rename", "`track` required"))?;
    let name = str_param(p, "name").map(str::trim).filter(|n| !n.is_empty()).ok_or_else(|| bad("track.rename", "`name` required"))?;
    let s = e.session_mut();
    if s.tracks.iter().any(|x| x.name == name && x.id != t) {
        return Err(bad("track.rename", format!("a track named `{name}` already exists")));
    }
    if let Some(tr) = s.track_mut(t) {
        tr.name = name.chars().take(64).collect();
    }
    Ok(json!({"name": name}))
}

pub(super) fn comments(e: &mut Engine, p: &Value) -> Result<Value> {
    let t = track_param(e, "track.comments", p, "track")?.ok_or_else(|| bad("track.comments", "`track` required"))?;
    let c = str_param(p, "comments").unwrap_or_default().to_string();
    if let Some(tr) = e.session_mut().track_mut(t) {
        tr.comments = c;
    }
    Ok(json!({}))
}

pub(super) fn color(e: &mut Engine, p: &Value) -> Result<Value> {
    let tracks = tracks_required(e, "track.color", p)?;
    let c = match p.get("color") {
        Some(Value::Array(a)) if a.len() == 3 => {
            let v: Vec<u8> = a.iter().map(|x| x.as_u64().unwrap_or(0).min(255) as u8).collect();
            [v.first().copied().unwrap_or(0), v.get(1).copied().unwrap_or(0), v.get(2).copied().unwrap_or(0)]
        }
        Some(v) if v.is_u64() => soundcraft_model::TRACK_COLORS.get(v.as_u64().unwrap_or(0) as usize % 16).copied().unwrap_or([128, 128, 128]),
        _ => return Err(bad("track.color", "`color` must be [r,g,b] or a palette index")),
    };
    let s = e.session_mut();
    for t in &tracks {
        if let Some(tr) = s.track_mut(*t) {
            tr.color = c;
        }
    }
    Ok(json!({"color": c}))
}

pub(super) fn height(e: &mut Engine, p: &Value) -> Result<Value> {
    let tracks = tracks_required(e, "track.height", p)?;
    let h = str_param(p, "height").and_then(TrackHeight::from_id).ok_or_else(|| bad("track.height", "unknown height"))?;
    let s = e.session_mut();
    for t in &tracks {
        if let Some(tr) = s.track_mut(*t) {
            tr.height = h;
        }
    }
    Ok(json!({"height": h}))
}

pub(super) fn view(e: &mut Engine, p: &Value) -> Result<Value> {
    let tracks = tracks_required(e, "track.view", p)?;
    let v = str_param(p, "view").ok_or_else(|| bad("track.view", "`view` required"))?.to_string();
    let ok = matches!(v.as_str(), "waveform" | "blocks" | "notes" | "regions" | "clip_gain" | "playlists")
        || soundcraft_model::AutoParam::parse(&v).is_some();
    if !ok {
        return Err(bad("track.view", format!("unknown view `{v}`")));
    }
    let s = e.session_mut();
    for t in &tracks {
        if let Some(tr) = s.track_mut(*t) {
            tr.view = v.clone();
        }
    }
    Ok(json!({"view": v}))
}
