//! Edit selection extension and marker-lane selection.

use super::super::*;
use super::nonempty_range;
use crate::edit as ops;
use serde_json::json;
use soundcraft_time::Range;

pub(super) fn duplicate_extend(e: &mut Engine, p: &Value) -> Result<Value> {
    let id = "edit.duplicate_extend";
    let tracks = tracks_required(e, id, p)?;
    let r = nonempty_range(e, p, id)?;
    let s = e.session_mut();
    let mut n = 0;
    for t in &tracks {
        let clips = ops::copy_range(s, *t, r);
        n += ops::paste(s, *t, r.end, &clips, r.len(), false).len();
        if let Some(tr) = s.track_mut(*t) {
            for l in &mut tr.automation {
                let pts: Vec<_> = l.points.iter().filter(|pt| r.contains(pt.at)).copied().collect();
                for pt in pts {
                    l.set_point(pt.at.saturating_add(r.len()), pt.value);
                }
            }
        }
    }
    s.edit.selection = Range::new(r.start, r.end.saturating_add(r.len()));
    if s.edit.link_timeline_edit {
        s.edit.timeline_selection = s.edit.selection;
    }
    Ok(json!({"clips": n, "selection": s.edit.selection}))
}

pub(super) fn extend_members(e: &mut Engine, up: bool) -> Result<Value> {
    let s = e.session_mut();
    let mut set: std::collections::BTreeSet<u64> = s.edit.selected_tracks.iter().map(|t| t.0).collect();
    let mut roots: Vec<u64> = Vec::new();
    for t in &s.edit.selected_tracks {
        let Some(tr) = s.track(*t) else { continue };
        if up {
            if let Some(f) = tr.folder {
                set.insert(f.0);
                roots.push(f.0);
            }
        } else if tr.is_folder() {
            roots.push(tr.id.0);
        }
    }
    // Add members (recursively, bounded by the track count).
    let mut guard = 0;
    while let Some(f) = roots.pop() {
        guard += 1;
        if guard > s.tracks.len().saturating_mul(2) + 8 {
            break;
        }
        for tr in s.tracks.iter().filter(|x| x.folder.is_some_and(|p| p.0 == f)) {
            if set.insert(tr.id.0) && tr.is_folder() {
                roots.push(tr.id.0);
            }
        }
    }
    s.edit.selected_tracks = s.tracks.iter().filter(|t| set.contains(&t.id.0)).map(|t| t.id).collect();
    Ok(json!({"tracks": s.edit.selected_tracks}))
}

/// Marker ruler lanes (1..5) in the edit selection, kept in `selection.marker_lane.top/bottom`.
pub(super) fn marker_lane(e: &mut Engine, dir: i64, extend: bool) -> Result<Value> {
    let s = e.session_mut();
    let top = s.edit.value("selection.marker_lane.top", 1.0).clamp(1.0, 5.0) as i64;
    let bottom = (s.edit.value("selection.marker_lane.bottom", top as f64).clamp(1.0, 5.0) as i64).max(top);
    let (top, bottom) = match (extend, dir < 0) {
        (false, true) => ((top - 1).max(1), (top - 1).max(1)),
        (false, false) => ((bottom + 1).min(5), (bottom + 1).min(5)),
        (true, true) => ((top - 1).max(1), bottom),
        (true, false) => (top, (bottom + 1).min(5)),
    };
    s.edit.values.insert("selection.marker_lane.top".into(), top as f64);
    s.edit.values.insert("selection.marker_lane.bottom".into(), bottom as f64);
    let r = s.edit.selection;
    let markers: Vec<u32> = s
        .markers
        .iter()
        .filter(|m| (top..=bottom).contains(&i64::from(m.ruler)) && (r.contains(m.start) || m.start == r.start))
        .map(|m| m.number)
        .collect();
    Ok(json!({"lanes": [top, bottom], "markers": markers}))
}
