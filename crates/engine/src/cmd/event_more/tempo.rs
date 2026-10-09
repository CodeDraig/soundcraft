//! Song start, tempo curves, tempo stretching and beat identification.

use super::super::*;
use super::note_ticks;
use serde_json::json;
use soundcraft_model::Session;
use soundcraft_time::Range;
use soundcraft_time::Samples;
use soundcraft_time::TICKS_PER_QUARTER;
use soundcraft_time::TempoMap;

/// Most tempo events one curve may write.
const MAX_TEMPO_EVENTS: i64 = 10_000;

#[derive(Clone, Copy)]
pub(super) enum Curve {
    Linear,
    Parabolic,
    Sigmoid,
}

pub(super) fn time_state(s: &Session) -> Value {
    json!({
        "tempos": s.tempo.tempos(),
        "meters": s.tempo.meters(),
        "song_start": s.edit.value("song.start", 0.0),
        "selection": s.edit.selection,
        "sample_rate": s.sample_rate.hz(),
    })
}

pub(super) fn move_song_start(e: &mut Engine, p: &Value) -> Result<Value> {
    let id = "event.move_song_start";
    let to = position_param(e, id, p, "to")?.ok_or_else(|| bad(id, "`to` required"))?.max(0);
    let s = e.session_mut();
    let from = s.edit.value("song.start", 0.0).max(0.0) as Samples;
    let delta = to - from;
    if delta == 0 {
        return Ok(json!({"song_start": to, "moved": 0}));
    }
    let bpm0 = s.tempo.tempo_at_tick(0);
    let dticks = (delta as f64 * bpm0 * TICKS_PER_QUARTER as f64 / (60.0 * s.sample_rate.as_f64())).round() as i64;
    // Tempo and meter events after the start move with the song.
    let old = s.tempo.clone();
    let mut map = TempoMap::new(old.tempo_at_tick(0), old.meter_at_tick(0).numerator, old.meter_at_tick(0).denominator)
        .map_err(|err| bad(id, err.to_string()))?;
    for t in old.tempos().iter().filter(|t| t.tick > 0) {
        map.set_tempo((t.tick + dticks).max(1), t.bpm).map_err(|err| bad(id, err.to_string()))?;
    }
    for m in old.meters().iter().filter(|m| m.tick > 0) {
        map.set_meter((m.tick + dticks).max(1), m.numerator, m.denominator).map_err(|err| bad(id, err.to_string()))?;
    }
    s.tempo = map;
    let mut moved = 0;
    for tr in s.tracks.iter_mut().filter(|t| t.ticks_timebase && t.kind.has_playlist()) {
        if let Some(pl) = tr.playlist_mut() {
            for c in &mut pl.clips {
                c.start = c.start.saturating_add(delta).max(0);
                moved += 1;
            }
            pl.sort();
        }
    }
    s.edit.values.insert("song.start".into(), to as f64);
    Ok(json!({"song_start": to, "moved": moved}))
}

pub(super) fn selection_ticks(e: &Engine, p: &Value, id: &str) -> Result<(Range, i64, i64)> {
    let r = range_param(e, id, p)?;
    if r.is_empty() {
        return Err(bad(id, "select a range on the timeline"));
    }
    let s = e.session();
    let t0 = s.tempo.samples_to_ticks(r.start, s.sample_rate);
    let t1 = s.tempo.samples_to_ticks(r.end, s.sample_rate);
    if t1 <= t0 {
        return Err(bad(id, "the selection is too short"));
    }
    Ok((r, t0, t1))
}

pub(super) fn tempo_curve(e: &mut Engine, p: &Value, curve: Curve, id: &str) -> Result<Value> {
    let (_, t0, t1) = selection_ticks(e, p, id)?;
    let res = note_ticks(p.get("resolution"), TICKS_PER_QUARTER / 2);
    if (t1 - t0) / res > MAX_TEMPO_EVENTS {
        return Err(bad(id, "too many tempo events: use a coarser resolution"));
    }
    let s = e.session_mut();
    let a = f64_or(p, "start_bpm", s.tempo.tempo_at_tick(t0));
    let b = p.get("end_bpm").and_then(Value::as_f64).filter(|v| v.is_finite()).ok_or_else(|| bad(id, "`end_bpm` required"))?;
    let curvature = f64_or(p, "curvature", 0.5).clamp(-1.0, 1.0);
    let after = s.tempo.tempo_at_tick(t1);
    let had_end = s.tempo.tempos().iter().any(|x| x.tick == t1);
    let inside: Vec<i64> = s.tempo.tempos().iter().map(|x| x.tick).filter(|t| *t > t0 && *t < t1).collect();
    for t in inside {
        s.tempo.remove_tempo(t);
    }
    let mut n = 0;
    let mut tick = t0;
    while tick < t1 {
        let f = (tick - t0) as f64 / (t1 - t0) as f64;
        let g = match curve {
            Curve::Linear => f,
            Curve::Parabolic => f.powf(2f64.powf(curvature * 2.0)),
            Curve::Sigmoid => 0.5 - 0.5 * (f * std::f64::consts::PI).cos(),
        };
        s.tempo.set_tempo(tick, a + (b - a) * g).map_err(|err| bad(id, err.to_string()))?;
        n += 1;
        tick += res;
    }
    if !had_end {
        s.tempo.set_tempo(t1, after).map_err(|err| bad(id, err.to_string()))?;
    }
    Ok(json!({"events": n, "start_tick": t0, "end_tick": t1}))
}

pub(super) fn tempo_stretch(e: &mut Engine, p: &Value) -> Result<Value> {
    let id = "event.tempo_stretch";
    let (r, t0, t1) = selection_ticks(e, p, id)?;
    let factor = match position_param(e, id, p, "length")? {
        Some(l) if l > 0 => l as f64 / r.len() as f64,
        _ => f64_or(p, "factor", f64::NAN),
    };
    if !factor.is_finite() || !(0.05..=20.0).contains(&factor) {
        return Err(bad(id, "give `factor` (0.05..20) or a new `length`"));
    }
    let s = e.session_mut();
    let after = s.tempo.tempo_at_tick(t1);
    let had_end = s.tempo.tempos().iter().any(|x| x.tick == t1);
    let mut events: Vec<(i64, f64)> = s.tempo.tempos().iter().filter(|x| x.tick > t0 && x.tick < t1).map(|x| (x.tick, x.bpm)).collect();
    events.push((t0, s.tempo.tempo_at_tick(t0)));
    for (tick, bpm) in &events {
        s.tempo.set_tempo(*tick, bpm / factor).map_err(|err| bad(id, err.to_string()))?;
    }
    if !had_end {
        s.tempo.set_tempo(t1, after).map_err(|err| bad(id, err.to_string()))?;
    }
    Ok(json!({"factor": factor, "events": events.len()}))
}

pub(super) fn identify_beat(e: &mut Engine, p: &Value) -> Result<Value> {
    let id = "event.identify_beat";
    let (r, t0, _) = selection_ticks(e, p, id)?;
    let s = e.session();
    let m = s.tempo.meter_at_tick(t0);
    let bars = f64_or(p, "bars", 1.0).max(0.0);
    let beats = f64_or(p, "beats", 0.0).max(0.0);
    let total_beats = bars * f64::from(m.numerator.max(1)) + beats;
    if total_beats <= 0.0 {
        return Err(bad(id, "`bars`/`beats` must describe a positive length"));
    }
    let quarters = total_beats * 4.0 / f64::from(m.denominator.max(1));
    let secs = s.sample_rate.seconds(r.len());
    let bpm = quarters * 60.0 / secs;
    let s = e.session_mut();
    s.tempo.set_tempo(t0, bpm).map_err(|err| bad(id, err.to_string()))?;
    Ok(json!({"bpm": bpm, "tick": t0}))
}
