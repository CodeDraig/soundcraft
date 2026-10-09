//! Selection geometry and track extension.

use super::super::*;
use serde_json::json;

pub(super) fn restore_selection(e: &mut Engine, _: &Value) -> Result<Value> {
    Ok(json!({"selection": e.session().edit.selection}))
}

pub(super) fn select_all(e: &mut Engine, _: &Value) -> Result<Value> {
    let s = e.session_mut();
    s.edit.selected_tracks = s.tracks.iter().filter(|t| !t.hidden).map(|t| t.id).collect();
    let end = s.content_end();
    s.edit.selection = soundcraft_time::Range::new(0, end);
    Ok(json!({"tracks": s.edit.selected_tracks.len(), "end": end}))
}

pub(super) fn select(e: &mut Engine, p: &Value) -> Result<Value> {
    let tracks = if p.get("tracks").is_some() || p.get("track").is_some() { Some(tracks_param(e, "edit.select", p)?) } else { None };
    let st = position_param(e, "edit.select", p, "start")?;
    let en = position_param(e, "edit.select", p, "end")?;
    let clips =
        p.get("clips").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_u64).map(soundcraft_model::ClipId).collect::<Vec<_>>());
    let exact = bool_or(p, "exact", false);
    let s = e.session_mut();
    if let Some(mut t) = tracks {
        // Active edit groups select all their members together.
        if !exact {
            let extra: Vec<soundcraft_model::TrackId> =
                s.groups.iter().filter(|g| g.active && g.edit && g.members.iter().any(|m| t.contains(m))).flat_map(|g| g.members.clone()).collect();
            for m in extra {
                if !t.contains(&m) {
                    t.push(m);
                }
            }
            let order: Vec<soundcraft_model::TrackId> = s.tracks.iter().map(|x| x.id).collect();
            t.sort_by_key(|x| order.iter().position(|o| o == x).unwrap_or(usize::MAX));
        }
        s.edit.selected_tracks = t;
    }
    match (st, en) {
        (Some(a), Some(b)) => s.edit.selection = soundcraft_time::Range::new(a.max(0), b.max(0)),
        (Some(a), None) => s.edit.selection = soundcraft_time::Range::point(a.max(0)),
        _ => {}
    }
    if let Some(c) = clips {
        // Selecting clips also selects their range and tracks (Grabber click).
        let mut range: Option<soundcraft_time::Range> = None;
        let mut trs = Vec::new();
        for id in &c {
            if let Some((tid, clip)) = s.find_clip(*id) {
                range = Some(range.map_or(clip.range(), |r| soundcraft_time::Range::new(r.start.min(clip.start), r.end.max(clip.end()))));
                if !trs.contains(&tid) {
                    trs.push(tid);
                }
            }
        }
        if let Some(r) = range {
            s.edit.selection = r;
            s.edit.selected_tracks = trs;
        }
        s.edit.selected_clips = c;
    } else if st.is_some() {
        s.edit.selected_clips.clear();
    }
    if s.edit.link_timeline_edit {
        s.edit.timeline_selection = s.edit.selection;
    }
    Ok(json!({"selection": s.edit.selection, "tracks": s.edit.selected_tracks, "clips": s.edit.selected_clips}))
}

pub(super) fn shift_selection(e: &mut Engine, dir: i64) -> Result<Value> {
    let s = e.session_mut();
    let len = s.edit.selection.len().max(1);
    s.edit.selection = s.edit.selection.shifted(dir * len);
    if s.edit.selection.start < 0 {
        s.edit.selection = s.edit.selection.shifted(-s.edit.selection.start);
    }
    Ok(json!({"selection": s.edit.selection}))
}

pub(super) fn scale_selection(e: &mut Engine, f: f64) -> Result<Value> {
    let s = e.session_mut();
    let len = soundcraft_time::to_samples(s.edit.selection.len() as f64 * f);
    s.edit.selection.end = s.edit.selection.start.saturating_add(len);
    Ok(json!({"selection": s.edit.selection}))
}

pub(super) fn extend_tracks(e: &mut Engine, dir: i64) -> Result<Value> {
    let s = e.session_mut();
    let idxs: Vec<usize> = s.edit.selected_tracks.iter().filter_map(|t| s.track_index(*t)).collect();
    let next = if dir < 0 { idxs.iter().min().and_then(|i| i.checked_sub(1)) } else { idxs.iter().max().map(|i| i + 1) };
    if let Some(t) = next.and_then(|i| s.tracks.get(i)).map(|t| t.id) {
        if dir < 0 {
            s.edit.selected_tracks.insert(0, t);
        } else {
            s.edit.selected_tracks.push(t);
        }
    } else if idxs.is_empty()
        && let Some(t) = s.tracks.first()
    {
        s.edit.selected_tracks.push(t.id);
    }
    Ok(json!({"tracks": s.edit.selected_tracks}))
}
