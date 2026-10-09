//! Copying or moving selected material to playlists.

use super::super::more_util::target_playlist;
use super::super::*;
use super::{nonempty_range, shuffle};
use crate::edit as ops;
use serde_json::json;
use soundcraft_model::Clip;
use soundcraft_model::Playlist;

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Dest {
    Target,
    New,
    Duplicate,
}

pub(super) fn to_playlist(e: &mut Engine, p: &Value, dest: Dest, mv: bool, id: &str) -> Result<Value> {
    let tracks = tracks_required(e, id, p)?;
    let r = nonempty_range(e, p, id)?;
    let sh = shuffle(e.session());
    let s = e.session_mut();
    let mut out = Vec::new();
    for t in &tracks {
        if s.track(*t).is_none_or(|tr| !tr.kind.has_playlist()) {
            continue;
        }
        let idx = match dest {
            Dest::Target => {
                let i = target_playlist(s, *t).ok_or_else(|| bad(id, "designate a target playlist first (Track › Designate as Target Playlist)"))?;
                if s.track(*t).is_some_and(|tr| tr.active_playlist == i) {
                    return Err(bad(id, "the target playlist is the main playlist"));
                }
                i
            }
            Dest::New | Dest::Duplicate => {
                let mut clips: Vec<Clip> =
                    if dest == Dest::Duplicate { s.track(*t).map(|tr| tr.clips().to_vec()).unwrap_or_default() } else { Vec::new() };
                for c in &mut clips {
                    c.id = s.new_clip_id();
                }
                let Some(tr) = s.track_mut(*t) else { continue };
                let name = format!("{}.{:02}", tr.name, tr.playlists.len() + 1);
                tr.playlists.push(Playlist { name, clips });
                tr.playlists.len() - 1
            }
        };
        let clips = ops::copy_range(s, *t, r);
        let main = s.track(*t).map_or(0, |tr| tr.active_playlist);
        if let Some(tr) = s.track_mut(*t) {
            tr.active_playlist = idx;
        }
        for mut c in clips {
            c.id = s.new_clip_id();
            c.start = c.start.saturating_add(r.start);
            ops::place_clip(s, *t, c);
        }
        if let Some(tr) = s.track_mut(*t) {
            tr.active_playlist = main;
        }
        if mv {
            ops::clear_range(s, *t, r, sh);
        }
        out.push(json!({"track": t, "playlist": idx}));
    }
    Ok(json!({"tracks": out}))
}
