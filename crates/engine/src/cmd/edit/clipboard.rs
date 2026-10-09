//! Clip and automation clipboard operations.

use super::super::*;
use super::shuffle;
use crate::Clipboard;
use crate::edit as ops;
use serde_json::json;

pub(super) fn cut_copy(e: &mut Engine, p: &Value, cut: bool, id: &str) -> Result<Value> {
    let tracks = tracks_required(e, id, p)?;
    let range = range_param(e, id, p)?;
    if range.is_empty() {
        return Err(bad(id, "selection is empty"));
    }
    let mut cb = Clipboard { length: range.len(), ..Clipboard::default() };
    let follow = e.session().edit.automation_follows_edit;
    for t in &tracks {
        cb.tracks.push(ops::copy_range(e.session(), *t, range));
        let lanes = e
            .session()
            .track(*t)
            .map(|tr| {
                tr.automation
                    .iter()
                    .map(|l| {
                        let mut l2 = l.clone();
                        l2.points = l
                            .points
                            .iter()
                            .filter(|p| range.contains(p.at))
                            .map(|p| soundcraft_model::AutomationPoint { at: p.at - range.start, value: p.value })
                            .collect();
                        l2
                    })
                    .collect()
            })
            .unwrap_or_default();
        cb.automation.push(lanes);
    }
    cb.markers = e
        .session()
        .markers
        .iter()
        .filter(|m| m.kind == soundcraft_model::MarkerKind::Marker && range.contains(m.start))
        .map(|m| {
            let mut m = m.clone();
            m.end -= m.start;
            m.start -= range.start;
            m.end += m.start;
            m
        })
        .collect();
    if cut {
        let sh = shuffle(e.session());
        let s = e.session_mut();
        for t in &tracks {
            ops::clear_range(s, *t, range, sh);
            if follow && let Some(tr) = s.track_mut(*t) {
                for l in &mut tr.automation {
                    l.clear_range(range.start, range.end);
                    if sh {
                        l.shift_from(range.end, -range.len());
                    }
                }
            }
        }
        s.edit.selection = soundcraft_time::Range::point(range.start);
    }
    let n: usize = cb.tracks.iter().map(Vec::len).sum();
    e.clipboard = cb;
    Ok(json!({"clips": n, "length": range.len()}))
}

pub(super) fn paste(e: &mut Engine, p: &Value) -> Result<Value> {
    let tracks = tracks_required(e, "edit.paste", p)?;
    let at = position_param(e, "edit.paste", p, "at")?.unwrap_or(e.session().edit.selection.start);
    let cb = e.clipboard.clone();
    let sh = shuffle(e.session());
    let follow = e.session().edit.automation_follows_edit;
    let s = e.session_mut();
    let mut pasted = 0;
    for (i, t) in tracks.iter().enumerate() {
        let Some(clips) = cb.tracks.get(i % cb.tracks.len().max(1)) else { continue };
        pasted += ops::paste(s, *t, at, clips, cb.length, sh).len();
        if follow && let (Some(lanes), Some(tr)) = (cb.automation.get(i % cb.automation.len().max(1)), s.track_mut(*t)) {
            for l in lanes {
                let dst = tr.lane_mut(&l.param);
                if !l.points.is_empty() {
                    dst.clear_range(at, at + cb.length);
                }
                for pt in &l.points {
                    dst.set_point(pt.at + at, pt.value);
                }
            }
        }
    }
    s.edit.selection = soundcraft_time::Range::new(at, at + cb.length);
    Ok(json!({"pasted": pasted}))
}

pub(super) fn clear(e: &mut Engine, p: &Value) -> Result<Value> {
    let tracks = tracks_required(e, "edit.clear", p)?;
    let range = range_param(e, "edit.clear", p)?;
    let ids = clip_ids_param(e, p);
    let sh = shuffle(e.session());
    let s = e.session_mut();
    let mut n = 0;
    if range.is_empty() && !ids.is_empty() {
        for t in &mut s.tracks {
            if let Some(pl) = t.playlist_mut() {
                let before = pl.clips.len();
                pl.clips.retain(|c| !ids.contains(&c.id));
                n += before - pl.clips.len();
            }
        }
        s.edit.selected_clips.clear();
    } else {
        for t in &tracks {
            n += ops::clear_range(s, *t, range, sh).len();
        }
    }
    Ok(json!({"removed": n}))
}

pub(super) fn reset_clip_gain(e: &mut Engine, p: &Value, _id: &str) -> Result<Value> {
    let ids = clip_ids_param(e, p);
    let s = e.session_mut();
    for id in &ids {
        if let Some(c) = s.find_clip_mut(*id) {
            c.gain_db = 0.0;
            c.gain_env.clear();
        }
    }
    Ok(json!({"clips": ids.len()}))
}

pub(super) fn clear_automation(e: &mut Engine, p: &Value, which: Option<&str>) -> Result<Value> {
    let tracks = tracks_required(e, "edit.clear_automation", p)?;
    let r = range_param(e, "edit.clear_automation", p)?;
    let s = e.session_mut();
    let mut n = 0;
    for t in &tracks {
        if let Some(tr) = s.track_mut(*t) {
            for l in &mut tr.automation {
                let hit = match which {
                    None => true,
                    Some("pan") => matches!(l.param, soundcraft_model::AutoParam::Pan(_)),
                    Some(_) => matches!(l.param, soundcraft_model::AutoParam::Plugin { .. }),
                };
                if hit {
                    n += l.clear_range(r.start, r.end);
                }
            }
        }
    }
    Ok(json!({"points_removed": n}))
}
