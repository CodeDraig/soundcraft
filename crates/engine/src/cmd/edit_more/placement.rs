//! Spacing and snapping selected clips.

use super::super::*;
use crate::edit as ops;
use serde_json::json;
use soundcraft_model::Clip;
use soundcraft_model::ClipId;
use soundcraft_time::Samples;

pub(super) fn space_clips(e: &mut Engine, p: &Value) -> Result<Value> {
    let id = "edit.space_clips";
    let ids = clip_ids_param(e, p);
    let gap = position_param(e, id, p, "gap")?.unwrap_or(0);
    let s2s = match str_param(p, "mode") {
        None | Some("end_to_start") => false,
        Some("start_to_start") => true,
        Some(m) => return Err(bad(id, format!("unknown mode `{m}`"))),
    };
    if ids.len() < 2 {
        return Err(bad(id, "select two or more clips"));
    }
    let s = e.session_mut();
    let track_ids: Vec<_> = s.tracks.iter().map(|t| t.id).collect();
    let mut moved = 0;
    for t in track_ids {
        let mut sel: Vec<Clip> = s
            .track(t)
            .map_or_else(Vec::new, |tr| tr.clips().iter().filter(|c| ids.contains(&c.id) && !c.time_locked && !c.edit_locked).cloned().collect());
        if sel.len() < 2 {
            continue;
        }
        sel.sort_by_key(|c| (c.start, c.id.0));
        if let Some(pl) = s.track_mut(t).and_then(|tr| tr.playlist_mut()) {
            pl.clips.retain(|c| !sel.iter().any(|x| x.id == c.id));
        }
        let mut cur = sel.first().map_or(0, |c| c.start);
        for mut c in sel {
            c.start = cur.max(0);
            cur = if s2s { c.start.saturating_add(gap) } else { c.end().saturating_add(gap) };
            ops::place_clip(s, t, c);
            moved += 1;
        }
    }
    Ok(json!({"clips": moved, "gap": gap}))
}

pub(super) fn snap(e: &mut Engine, p: &Value, next: bool) -> Result<Value> {
    let ids = clip_ids_param(e, p);
    let s = e.session_mut();
    let mut order: Vec<(ClipId, Samples)> = ids.iter().filter_map(|id| s.find_clip(*id).map(|(_, c)| (*id, c.start))).collect();
    // Snap the clip nearest the destination first so neighbours stay put.
    order.sort_by_key(|x| if next { -x.1 } else { x.1 });
    let mut moved = 0;
    for (id, _) in order {
        let Some((t, c)) = s.find_clip(id).map(|(t, c)| (t, c.clone())) else { continue };
        let others =
            s.track(t).map(|tr| tr.clips().iter().filter(|x| x.id != id && !ids.contains(&x.id)).cloned().collect::<Vec<_>>()).unwrap_or_default();
        let delta = if next {
            others.iter().filter(|x| x.start >= c.end()).map(|x| x.start).min().map(|st| st - c.end())
        } else {
            others.iter().filter(|x| x.end() <= c.start).map(Clip::end).max().map(|en| en - c.start)
        };
        if let Some(d) = delta.filter(|d| *d != 0) {
            moved += ops::move_clips(s, &[id], d, None);
        }
    }
    Ok(json!({"moved": moved}))
}
