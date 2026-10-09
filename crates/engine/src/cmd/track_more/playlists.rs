//! Target and recent playlist selection.

use super::super::more_util::activate_playlist;
use super::super::more_util::target_playlist;
use super::super::*;
use serde_json::json;

pub(super) fn designate_target(e: &mut Engine, p: &Value) -> Result<Value> {
    let tracks = tracks_required(e, "track.designate_target_playlist", p)?;
    let idx = p.get("index").and_then(Value::as_u64);
    let s = e.session_mut();
    let mut out = Vec::new();
    for t in &tracks {
        let Some(tr) = s.track(*t) else { continue };
        let i = idx.map_or(tr.active_playlist, |x| usize::try_from(x).unwrap_or(usize::MAX));
        if i >= tr.playlists.len() {
            return Err(bad("track.designate_target_playlist", "no such playlist"));
        }
        s.edit.values.insert(format!("playlist.target.{}", t.0), i as f64);
        out.push(json!({"track": t, "target": i}));
    }
    Ok(json!({"tracks": out}))
}

pub(super) fn show_target(e: &mut Engine, p: &Value) -> Result<Value> {
    let tracks = tracks_required(e, "track.show_target_playlist", p)?;
    let s = e.session_mut();
    let mut n = 0;
    for t in &tracks {
        if let Some(i) = target_playlist(s, *t)
            && activate_playlist(s, *t, i)
        {
            n += 1;
        }
    }
    Ok(json!({"shown": n}))
}

pub(super) fn toggle_recent(e: &mut Engine, p: &Value) -> Result<Value> {
    let tracks = tracks_required(e, "track.toggle_recent_playlist", p)?;
    let s = e.session_mut();
    let mut n = 0;
    for t in &tracks {
        let recent = s.edit.values.get(&format!("playlist.recent.{}", t.0)).copied().filter(|v| v.is_finite() && *v >= 0.0);
        if let Some(i) = recent
            && activate_playlist(s, *t, i as usize)
        {
            n += 1;
        }
    }
    Ok(json!({"toggled": n}))
}
