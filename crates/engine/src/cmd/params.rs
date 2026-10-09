//! Validated command parameter decoding and target resolution.

use crate::Engine;
use crate::EngineError;
use crate::Result;
use serde_json::Value;
use soundcraft_model::ClipId;
use soundcraft_model::TrackId;
use soundcraft_time::Range;
use soundcraft_time::Samples;

// ---- params ------------------------------------------------------------------------------

pub fn bad(cmd: &str, msg: impl Into<String>) -> EngineError {
    EngineError::BadParams(cmd.to_string(), msg.into())
}

pub fn f64_or(p: &Value, key: &str, d: f64) -> f64 {
    p.get(key).and_then(Value::as_f64).filter(|v| v.is_finite()).unwrap_or(d)
}

pub fn f32_or(p: &Value, key: &str, d: f32) -> f32 {
    f64_or(p, key, f64::from(d)) as f32
}

pub fn i64_or(p: &Value, key: &str, d: i64) -> i64 {
    p.get(key).and_then(|v| v.as_i64().or_else(|| v.as_f64().filter(|f| f.is_finite()).map(|f| f.round() as i64))).unwrap_or(d)
}

pub fn bool_or(p: &Value, key: &str, d: bool) -> bool {
    p.get(key).and_then(Value::as_bool).unwrap_or(d)
}

pub fn str_param<'a>(p: &'a Value, key: &str) -> Option<&'a str> {
    p.get(key).and_then(Value::as_str)
}

/// Resolve a track from `track` (id number or name) params.
pub fn track_param(e: &Engine, cmd: &str, p: &Value, key: &str) -> Result<Option<TrackId>> {
    let Some(v) = p.get(key) else { return Ok(None) };
    let s = e.session();
    if let Some(n) = v.as_u64() {
        return s.track(TrackId(n)).map(|t| Some(t.id)).ok_or_else(|| bad(cmd, format!("no track with id {n}")));
    }
    if let Some(name) = v.as_str() {
        return s.track_by_name(name).map(|t| Some(t.id)).ok_or_else(|| bad(cmd, format!("no track named `{name}`")));
    }
    Err(bad(cmd, format!("`{key}` must be a track id or name")))
}

/// Tracks to act on: `tracks` (array of ids/names) or `track`, else the selected tracks.
pub fn tracks_param(e: &Engine, cmd: &str, p: &Value) -> Result<Vec<TrackId>> {
    if let Some(arr) = p.get("tracks").and_then(Value::as_array) {
        let s = e.session();
        let mut out = Vec::new();
        for v in arr {
            let id = match (v.as_u64(), v.as_str()) {
                (Some(n), _) => s.track(TrackId(n)).map(|t| t.id),
                (_, Some(name)) => s.track_by_name(name).map(|t| t.id),
                _ => None,
            };
            out.push(id.ok_or_else(|| bad(cmd, format!("unknown track {v}")))?);
        }
        return Ok(out);
    }
    if let Some(t) = track_param(e, cmd, p, "track")? {
        return Ok(vec![t]);
    }
    Ok(e.target_tracks())
}

/// Like `tracks_param` but errors when empty.
pub fn tracks_required(e: &Engine, cmd: &str, p: &Value) -> Result<Vec<TrackId>> {
    let t = tracks_param(e, cmd, p)?;
    if t.is_empty() { Err(bad(cmd, "no tracks: pass `track`/`tracks` or select tracks")) } else { Ok(t) }
}

/// Largest position accepted from parameters: 2^40 samples (about 265 days at 48 kHz). Clamping
/// here keeps every later addition and subtraction on positions far away from overflow.
pub const MAX_POSITION: Samples = 1 << 40;

/// Position from `key`: a number of samples, or `{"seconds": x}`, or a string in the main counter format.
/// Values are clamped to ±[`MAX_POSITION`].
pub fn position_param(e: &Engine, cmd: &str, p: &Value, key: &str) -> Result<Option<Samples>> {
    Ok(position_param_raw(e, cmd, p, key)?.map(|v| v.clamp(-MAX_POSITION, MAX_POSITION)))
}

fn position_param_raw(e: &Engine, cmd: &str, p: &Value, key: &str) -> Result<Option<Samples>> {
    let Some(v) = p.get(key) else { return Ok(None) };
    let s = e.session();
    if let Some(n) = v.as_i64() {
        return Ok(Some(n));
    }
    if let Some(f) = v.as_f64() {
        return Ok(Some(soundcraft_time::to_samples(f)));
    }
    if let Some(secs) = v.get("seconds").and_then(Value::as_f64) {
        return Ok(Some(s.sample_rate.samples(secs)));
    }
    if let Some(text) = v.as_str() {
        // Try each format, starting with an explicit prefix like "bars:5|1|000".
        let (fmt, body) = match text.split_once(':') {
            Some((f, b)) if soundcraft_time::TimeFormat::from_id(f).is_some() => (soundcraft_time::TimeFormat::from_id(f), b),
            _ => (None, text),
        };
        let fmts: Vec<soundcraft_time::TimeFormat> = match fmt {
            Some(f) => vec![f],
            None => {
                let mut v = vec![s.edit.main_counter];
                v.extend(soundcraft_time::TimeFormat::ALL);
                v
            }
        };
        for f in fmts {
            if let Ok(x) = soundcraft_time::parse_position(body, f, s.sample_rate, &s.tempo, s.frame_rate, s.timecode_start) {
                return Ok(Some(x));
            }
        }
        return Err(bad(cmd, format!("cannot parse position `{text}`")));
    }
    Err(bad(cmd, format!("`{key}` must be samples, {{seconds}}, or a time string")))
}

/// Range from `start`/`end` params, else the edit selection.
pub fn range_param(e: &Engine, cmd: &str, p: &Value) -> Result<Range> {
    let st = position_param(e, cmd, p, "start")?;
    let en = position_param(e, cmd, p, "end")?;
    let len = position_param(e, cmd, p, "length")?;
    match (st, en, len) {
        (Some(a), Some(b), _) => Ok(Range::new(a, b)),
        (Some(a), None, Some(l)) => Ok(Range::new(a, a.saturating_add(l))),
        (Some(a), None, None) => Ok(Range::point(a)),
        _ => Ok(e.session().edit.selection),
    }
}

pub fn clip_ids_param(e: &Engine, p: &Value) -> Vec<ClipId> {
    if let Some(arr) = p.get("clips").and_then(Value::as_array) {
        return arr.iter().filter_map(Value::as_u64).map(ClipId).collect();
    }
    if let Some(n) = p.get("clip").and_then(Value::as_u64) {
        return vec![ClipId(n)];
    }
    let s = e.session();
    if !s.edit.selected_clips.is_empty() {
        return s.edit.selected_clips.clone();
    }
    // Clips touched by the edit selection on selected tracks.
    let r = s.edit.selection;
    s.tracks
        .iter()
        .filter(|t| s.edit.selected_tracks.contains(&t.id))
        .flat_map(|t| t.clips().iter().filter(move |c| if r.is_empty() { c.range().contains(r.start) } else { c.range().overlaps(&r) }).map(|c| c.id))
        .collect()
}
