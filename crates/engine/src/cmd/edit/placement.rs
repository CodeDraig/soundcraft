//! Clip repetition, placement and timeline shifts.

use super::super::*;
use crate::edit as ops;
use serde_json::json;

pub(super) fn repeat_to_fill(e: &mut Engine, p: &Value) -> Result<Value> {
    let range = range_param(e, "edit.paste_repeat_to_fill", p)?;
    let len = e.clipboard.length.max(1);
    let tracks = tracks_required(e, "edit.paste_repeat_to_fill", p)?;
    let cb = e.clipboard.clone();
    let s = e.session_mut();
    let mut at = range.start;
    let mut n = 0;
    while at < range.end && n < 10_000 {
        for (i, t) in tracks.iter().enumerate() {
            if let Some(clips) = cb.tracks.get(i % cb.tracks.len().max(1)) {
                let trimmed: Vec<_> = clips
                    .iter()
                    .filter(|c| at + c.start < range.end)
                    .map(|c| {
                        let mut c = c.clone();
                        if at + c.end() > range.end {
                            c.trim_end_to(range.end - at);
                        }
                        c
                    })
                    .collect();
                ops::paste(s, *t, at, &trimmed, (range.end - at).min(len), false);
            }
        }
        at += len;
        n += 1;
    }
    Ok(json!({"repeats": n}))
}

pub(super) fn duplicate(e: &mut Engine, p: &Value) -> Result<Value> {
    let tracks = tracks_required(e, "edit.duplicate", p)?;
    let range = range_param(e, "edit.duplicate", p)?;
    if range.is_empty() {
        return Err(bad("edit.duplicate", "selection is empty"));
    }
    let s = e.session_mut();
    let mut n = 0;
    for t in &tracks {
        let clips = ops::copy_range(s, *t, range);
        n += ops::paste(s, *t, range.end, &clips, range.len(), false).len();
        if let Some(tr) = s.track_mut(*t) {
            for l in &mut tr.automation {
                let pts: Vec<_> = l.points.iter().filter(|p| range.contains(p.at)).copied().collect();
                for pt in pts {
                    l.set_point(pt.at + range.len(), pt.value);
                }
            }
        }
    }
    s.edit.selection = range.shifted(range.len());
    Ok(json!({"clips": n}))
}

pub(super) fn repeat(e: &mut Engine, p: &Value) -> Result<Value> {
    let count = i64_or(p, "count", 1).clamp(1, 999);
    let tracks = tracks_required(e, "edit.repeat", p)?;
    let range = range_param(e, "edit.repeat", p)?;
    if range.is_empty() {
        return Err(bad("edit.repeat", "selection is empty"));
    }
    let s = e.session_mut();
    for t in &tracks {
        let clips = ops::copy_range(s, *t, range);
        for k in 1..=count {
            ops::paste(s, *t, range.start + range.len() * k, &clips, range.len(), false);
        }
    }
    Ok(json!({"repeats": count}))
}

pub(super) fn shift(e: &mut Engine, p: &Value) -> Result<Value> {
    let by = position_param(e, "edit.shift", p, "by")?.ok_or_else(|| bad("edit.shift", "`by` is required"))?;
    let later = bool_or(p, "later", true);
    let ids = clip_ids_param(e, p);
    let s = e.session_mut();
    let n = ops::move_clips(s, &ids, if later { by } else { -by }, None);
    Ok(json!({"moved": n}))
}

pub(super) fn insert_silence(e: &mut Engine, p: &Value) -> Result<Value> {
    let tracks = tracks_required(e, "edit.insert_silence", p)?;
    let range = range_param(e, "edit.insert_silence", p)?;
    let s = e.session_mut();
    for t in &tracks {
        ops::insert_silence(s, *t, range);
        if let Some(tr) = s.track_mut(*t) {
            for l in &mut tr.automation {
                l.shift_from(range.start, range.len());
            }
        }
    }
    Ok(json!({"inserted": range.len()}))
}

pub(super) fn nudge(e: &mut Engine, p: &Value) -> Result<Value> {
    let dir = i64_or(p, "direction", 1).signum();
    let count = i64_or(p, "count", 1).clamp(1, 1000);
    let s = e.session();
    let step = s.edit.nudge.step_samples(s.edit.selection.start, s.sample_rate, &s.tempo, s.frame_rate) * count * if dir == 0 { 1 } else { dir };
    let ids = clip_ids_param(e, p);
    let s = e.session_mut();
    let n = if ids.is_empty() { 0 } else { ops::move_clips(s, &ids, step, None) };
    s.edit.selection = s.edit.selection.shifted(step);
    if s.edit.selection.start < 0 {
        s.edit.selection = s.edit.selection.shifted(-s.edit.selection.start);
    }
    Ok(json!({"moved": n, "by": step}))
}

pub(super) fn move_clips(e: &mut Engine, p: &Value) -> Result<Value> {
    let ids = clip_ids_param(e, p);
    if ids.is_empty() {
        return Err(bad("edit.move_clips", "no clips"));
    }
    let to_track = track_param(e, "edit.move_clips", p, "track")?;
    let first = ids.iter().filter_map(|id| e.session().find_clip(*id).map(|(_, c)| c.start)).min().unwrap_or(0);
    let delta = match (position_param(e, "edit.move_clips", p, "by")?, position_param(e, "edit.move_clips", p, "to")?) {
        (Some(by), _) => by,
        (None, Some(to)) => to - first,
        _ => 0,
    };
    // Grid mode snaps the destination.
    let s = e.session();
    let delta = if s.edit.edit_mode.is_grid() && p.get("to").is_some() {
        s.edit.grid.snap(first + delta, s.sample_rate, &s.tempo, s.frame_rate) - first
    } else {
        delta
    };
    let s = e.session_mut();
    let n = ops::move_clips(s, &ids, delta, to_track);
    s.edit.selected_clips = ids;
    Ok(json!({"moved": n, "by": delta}))
}
