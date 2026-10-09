//! Tempo, meter and timeline insertion or removal.

use super::super::*;
use serde_json::json;

pub(super) fn tempo(e: &mut Engine, p: &Value) -> Result<Value> {
    let bpm = f64_or(p, "bpm", f64::NAN);
    let at = position_param(e, "event.tempo", p, "at")?.or(position_param(e, "event.tempo", p, "start")?).unwrap_or(0);
    let s = e.session_mut();
    let tick = s.tempo.samples_to_ticks(at, s.sample_rate);
    s.tempo.set_tempo(tick, bpm).map_err(|err| bad("event.tempo", err.to_string()))?;
    Ok(json!({"tick": tick, "bpm": bpm}))
}

pub(super) fn meter(e: &mut Engine, p: &Value) -> Result<Value> {
    let num = i64_or(p, "numerator", 4).clamp(1, 64) as u32;
    let den = i64_or(p, "denominator", 4).clamp(1, 64) as u32;
    let at = position_param(e, "event.meter", p, "at")?.unwrap_or(0);
    let s = e.session_mut();
    let tick = s.tempo.samples_to_ticks(at, s.sample_rate);
    s.tempo.set_meter(tick, num, den).map_err(|err| bad("event.meter", err.to_string()))?;
    Ok(json!({"meter": format!("{num}/{den}")}))
}

pub(super) fn insert_time(e: &mut Engine, p: &Value) -> Result<Value> {
    let r = range_param(e, "event.insert_time", p)?;
    if r.is_empty() {
        return Err(bad("event.insert_time", "give `start` and `length` or `end`"));
    }
    let s = e.session_mut();
    let ids: Vec<_> = s.tracks.iter().map(|t| t.id).collect();
    for t in ids {
        crate::edit::insert_silence(s, t, r);
        if let Some(tr) = s.track_mut(t) {
            for l in &mut tr.automation {
                l.shift_from(r.start, r.len());
            }
        }
    }
    for m in &mut s.markers {
        if m.start >= r.start {
            m.start += r.len();
            m.end += r.len();
        }
    }
    Ok(json!({"inserted": r.len()}))
}

pub(super) fn cut_time(e: &mut Engine, p: &Value) -> Result<Value> {
    let r = range_param(e, "event.cut_time", p)?;
    if r.is_empty() {
        return Err(bad("event.cut_time", "selection is empty"));
    }
    let s = e.session_mut();
    let ids: Vec<_> = s.tracks.iter().map(|t| t.id).collect();
    for t in ids {
        crate::edit::clear_range(s, t, r, true);
        if let Some(tr) = s.track_mut(t) {
            for l in &mut tr.automation {
                l.clear_range(r.start, r.end);
                l.shift_from(r.end, -r.len());
            }
        }
    }
    s.markers.retain(|m| !r.contains(m.start));
    for m in &mut s.markers {
        if m.start >= r.end {
            m.start -= r.len();
            m.end -= r.len();
        }
    }
    Ok(json!({"removed": r.len()}))
}
