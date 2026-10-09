//! Clip splitting, healing and edge trimming.

use super::super::*;
use crate::edit as ops;
use serde_json::json;

pub(super) fn separate(e: &mut Engine, p: &Value) -> Result<Value> {
    let tracks = tracks_required(e, "edit.separate", p)?;
    let points: Vec<i64> = if let Some(at) = position_param(e, "edit.separate", p, "at")? {
        vec![at]
    } else {
        let r = range_param(e, "edit.separate", p)?;
        if r.is_empty() { vec![r.start] } else { vec![r.start, r.end] }
    };
    let s = e.session_mut();
    let mut n = 0;
    for t in &tracks {
        for at in &points {
            n += ops::separate_at(s, *t, *at);
        }
    }
    Ok(json!({"separated": n}))
}

pub(super) fn separate_on_grid(e: &mut Engine, p: &Value) -> Result<Value> {
    let tracks = tracks_required(e, "edit.separate_on_grid", p)?;
    let r = range_param(e, "edit.separate_on_grid", p)?;
    let s = e.session_mut();
    let lines = s.edit.grid.lines(r.start, r.end, s.sample_rate, &s.tempo, s.frame_rate, 10_000);
    let mut n = 0;
    for t in &tracks {
        for at in &lines {
            n += ops::separate_at(s, *t, *at);
        }
    }
    Ok(json!({"separated": n}))
}

pub(super) fn separate_at_transients(e: &mut Engine, p: &Value) -> Result<Value> {
    let tracks = tracks_required(e, "edit.separate_at_transients", p)?;
    let r = range_param(e, "edit.separate_at_transients", p)?;
    let sens = f32_or(p, "sensitivity", 0.5).clamp(0.0, 1.0);
    let s = e.session_mut();
    let mut n = 0;
    for t in &tracks {
        let hits = crate::io::transients_in(s, *t, r, sens);
        for at in hits {
            n += ops::separate_at(s, *t, at);
        }
    }
    Ok(json!({"separated": n}))
}

pub(super) fn heal(e: &mut Engine, p: &Value) -> Result<Value> {
    let tracks = tracks_required(e, "edit.heal", p)?;
    let mut r = range_param(e, "edit.heal", p)?;
    if r.is_empty() {
        r = soundcraft_time::Range::new(r.start - 1, r.start + 1);
    }
    let s = e.session_mut();
    let n: usize = tracks.iter().map(|t| ops::heal(s, *t, r)).sum();
    Ok(json!({"healed": n}))
}

pub(super) fn trim_to_selection(e: &mut Engine, p: &Value) -> Result<Value> {
    let tracks = tracks_required(e, "edit.trim_to_selection", p)?;
    let r = range_param(e, "edit.trim_to_selection", p)?;
    let s = e.session_mut();
    let n: usize = tracks.iter().map(|t| ops::trim_to(s, *t, r)).sum();
    Ok(json!({"trimmed": n}))
}

pub(super) fn trim_edge(e: &mut Engine, p: &Value, start: bool) -> Result<Value> {
    let tracks = tracks_required(e, "edit.trim", p)?;
    let at = position_param(e, "edit.trim", p, "at")?.unwrap_or(e.session().edit.selection.start);
    let s = e.session_mut();
    let n: usize = tracks.iter().map(|t| if start { ops::trim_start_to(s, *t, at) } else { ops::trim_end_to(s, *t, at) }).sum();
    Ok(json!({"trimmed": n}))
}

pub(super) fn trim_to_file(e: &mut Engine, p: &Value) -> Result<Value> {
    let ids = clip_ids_param(e, p);
    let s = e.session_mut();
    let mut n = 0;
    for id in ids {
        let frames =
            s.find_clip(id).and_then(|(_, c)| c.source()).and_then(|src| s.source(src)).map(|src| i64::try_from(src.frames).unwrap_or(i64::MAX));
        if let (Some(frames), Some(c)) = (frames, s.find_clip_mut(id)) {
            let off = c.source_offset();
            let new_start = c.start - off;
            c.trim_start_to(new_start);
            c.trim_end_to(new_start + frames);
            n += 1;
        }
    }
    Ok(json!({"trimmed": n}))
}

pub(super) fn trim_to_fill(e: &mut Engine, p: &Value) -> Result<Value> {
    let r = range_param(e, "edit.trim_to_fill_selection", p)?;
    let ids = clip_ids_param(e, p);
    let s = e.session_mut();
    let mut n = 0;
    for id in ids {
        let frames =
            s.find_clip(id).and_then(|(_, c)| c.source()).and_then(|src| s.source(src)).map(|src| i64::try_from(src.frames).unwrap_or(i64::MAX));
        if let (Some(frames), Some(c)) = (frames, s.find_clip_mut(id)) {
            let file_start = c.start - c.source_offset();
            c.trim_start_to(r.start.max(file_start));
            c.trim_end_to(r.end.min(file_start + frames));
            n += 1;
        }
    }
    Ok(json!({"trimmed": n}))
}
