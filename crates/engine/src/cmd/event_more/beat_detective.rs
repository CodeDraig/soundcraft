//! Audio transient-based beat detection.

use super::super::*;
use serde_json::json;
use soundcraft_time::Samples;

pub(super) fn beat_detective(e: &mut Engine, p: &Value) -> Result<Value> {
    let id = "event.beat_detective";
    let tracks = tracks_required(e, id, p)?;
    let r = range_param(e, id, p)?;
    if r.is_empty() {
        return Err(bad(id, "select a range to analyse"));
    }
    if r.len() > e.session().sample_rate.samples(20.0 * 60.0) {
        return Err(bad(id, "analyse at most 20 minutes at a time"));
    }
    let sens = f32_or(p, "sensitivity", 0.5).clamp(0.0, 1.0);
    let s = e.session();
    let sr = s.sample_rate;
    let mut hits: Vec<Samples> = Vec::new();
    for t in &tracks {
        if s.track(*t).is_some_and(|x| x.kind.has_playlist() && !x.kind.is_midi()) {
            hits.extend(crate::io::transients_in(s, *t, r, sens));
        }
    }
    hits.sort_unstable();
    let min_gap = sr.samples(0.03);
    let mut beats: Vec<Samples> = Vec::new();
    for h in hits {
        if beats.last().is_none_or(|l| h - l >= min_gap) {
            beats.push(h);
        }
    }
    let mut iois: Vec<f64> = beats.windows(2).filter_map(|w| Some((w.get(1)? - w.first()?) as f64)).filter(|d| *d > 0.0).collect();
    iois.sort_by(f64::total_cmp);
    let bpm = iois.get(iois.len() / 2).map(|d| {
        let mut b = 60.0 * sr.as_f64() / d;
        let mut guard = 0;
        while b < 70.0 && guard < 16 {
            b *= 2.0;
            guard += 1;
        }
        while b >= 180.0 && guard < 32 {
            b /= 2.0;
            guard += 1;
        }
        (b * 100.0).round() / 100.0
    });
    let separate = bool_or(p, "separate", false);
    let set_tempo = bool_or(p, "set_tempo", false);
    let s = e.session_mut();
    let mut separated = 0;
    if separate {
        for t in &tracks {
            for b in &beats {
                separated += crate::edit::separate_at(s, *t, *b);
            }
        }
    }
    if set_tempo && let Some(b) = bpm {
        let tick = s.tempo.samples_to_ticks(r.start, sr);
        s.tempo.set_tempo(tick, b).map_err(|err| bad(id, err.to_string()))?;
    }
    let shown: Vec<Samples> = beats.iter().take(2000).copied().collect();
    Ok(json!({"beats": shown, "count": beats.len(), "bpm": bpm, "separated": separated}))
}
