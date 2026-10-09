//! Rendered edits and playlist placement.

use super::super::*;
use crate::edit as ops;
use serde_json::json;

pub(super) fn mute_clips(e: &mut Engine, p: &Value) -> Result<Value> {
    let ids = clip_ids_param(e, p);
    let s = e.session_mut();
    let any_unmuted = ids.iter().any(|id| s.find_clip(*id).is_some_and(|(_, c)| !c.muted));
    for id in &ids {
        if let Some(c) = s.find_clip_mut(*id) {
            c.muted = any_unmuted;
        }
    }
    Ok(json!({"muted": any_unmuted, "clips": ids.len()}))
}

pub(super) fn copy_to_new_playlist(e: &mut Engine, p: &Value) -> Result<Value> {
    let tracks = tracks_required(e, "edit.copy_to_new_playlist", p)?;
    let r = range_param(e, "edit.copy_to_new_playlist", p)?;
    let s = e.session_mut();
    for t in &tracks {
        let clips = ops::copy_range(s, *t, r);
        let mut fresh = Vec::new();
        for mut c in clips {
            c.id = s.new_clip_id();
            c.start += r.start;
            fresh.push(c);
        }
        if let Some(tr) = s.track_mut(*t) {
            let name = format!("{}.{:02}", tr.name, tr.playlists.len() + 1);
            tr.playlists.push(soundcraft_model::Playlist { name, clips: fresh });
            tr.active_playlist = tr.playlists.len() - 1;
        }
    }
    Ok(json!({"tracks": tracks.len()}))
}

pub(super) fn strip_silence(e: &mut Engine, p: &Value) -> Result<Value> {
    let tracks = tracks_required(e, "edit.strip_silence", p)?;
    let r = range_param(e, "edit.strip_silence", p)?;
    let thr = f32_or(p, "threshold_db", -48.0).clamp(-96.0, 0.0);
    let sr = e.session().sample_rate;
    let ms = |k: &str, d: f64| sr.samples(f64_or(p, k, d).clamp(0.0, 10_000.0) / 1000.0);
    let (min_len, pre, post) = (ms("min_length_ms", 50.0), ms("pad_before_ms", 5.0), ms("pad_after_ms", 20.0));
    let s = e.session_mut();
    let mut kept = 0;
    for t in &tracks {
        kept += crate::io::strip_silence(s, *t, r, thr, min_len, pre, post);
    }
    Ok(json!({"clips": kept}))
}

pub(super) fn consolidate(e: &mut Engine, p: &Value) -> Result<Value> {
    let tracks = tracks_required(e, "edit.consolidate", p)?;
    let r = range_param(e, "edit.consolidate", p)?;
    if r.is_empty() {
        return Err(bad("edit.consolidate", "selection is empty"));
    }
    let mut n = 0;
    for t in tracks {
        if crate::io::consolidate(e, t, r)? {
            n += 1;
        }
    }
    Ok(json!({"consolidated": n}))
}
