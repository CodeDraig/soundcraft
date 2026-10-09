//! Playlist creation and selection.

use super::super::*;
use serde_json::json;
use soundcraft_model::Playlist;

pub(super) fn new_playlist(e: &mut Engine, p: &Value, dup: bool) -> Result<Value> {
    let tracks = tracks_required(e, "track.playlist_new", p)?;
    let name = str_param(p, "name").map(str::to_string);
    let s = e.session_mut();
    for t in &tracks {
        let mut clips = Vec::new();
        if dup && let Some(tr) = s.track(*t) {
            clips = tr.clips().to_vec();
        }
        for c in &mut clips {
            c.id = s.new_clip_id();
        }
        if let Some(tr) = s.track_mut(*t) {
            let n = name.clone().unwrap_or_else(|| format!("{}.{:02}", tr.name, tr.playlists.len() + 1));
            tr.playlists.push(Playlist { name: n, clips });
            tr.active_playlist = tr.playlists.len() - 1;
        }
    }
    Ok(json!({"tracks": tracks}))
}

pub(super) fn select_playlist(e: &mut Engine, p: &Value) -> Result<Value> {
    let tracks = tracks_required(e, "track.playlist_select", p)?;
    let idx = p.get("index").and_then(Value::as_u64).map(|i| i as usize);
    let name = str_param(p, "name").map(str::to_string);
    let s = e.session_mut();
    for t in &tracks {
        if let Some(tr) = s.track_mut(*t) {
            let i = idx.or_else(|| name.as_ref().and_then(|n| tr.playlists.iter().position(|p| &p.name == n)));
            match i {
                Some(i) if i < tr.playlists.len() => tr.active_playlist = i,
                _ => return Err(bad("track.playlist_select", "no such playlist")),
            }
        }
    }
    Ok(json!({}))
}
