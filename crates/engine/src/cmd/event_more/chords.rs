//! Duration-weighted chord detection and chord markers.

use super::super::*;
use super::selection_ticks;
use serde_json::json;
use soundcraft_model::ClipContent;
use soundcraft_model::MarkerKind;
use soundcraft_model::TrackId;
use soundcraft_time::Samples;

const NOTE_NAMES: [&str; 12] = ["C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B"];

const CHORDS: [(&str, &[usize]); 12] = [
    ("", &[0, 4, 7]),
    ("m", &[0, 3, 7]),
    ("dim", &[0, 3, 6]),
    ("aug", &[0, 4, 8]),
    ("sus2", &[0, 2, 7]),
    ("sus4", &[0, 5, 7]),
    ("7", &[0, 4, 7, 10]),
    ("maj7", &[0, 4, 7, 11]),
    ("m7", &[0, 3, 7, 10]),
    ("m7b5", &[0, 3, 6, 10]),
    ("6", &[0, 4, 7, 9]),
    ("m6", &[0, 3, 7, 9]),
];

/// Name the chord formed by duration-weighted pitch classes (`bass` = lowest pitch class).
pub fn chord_name(weights: &[f64; 12], bass: usize) -> Option<String> {
    let present = weights.iter().filter(|w| **w > 0.0).count();
    if present < 3 {
        return None;
    }
    let total: f64 = weights.iter().sum();
    let maxw = weights.iter().copied().fold(0.0, f64::max);
    let mut best: Option<(f64, usize, &str)> = None;
    for root in 0..12 {
        for (suffix, tones) in CHORDS {
            let w = |i: usize| weights.get((root + i) % 12).copied().unwrap_or(0.0);
            let inside: f64 = tones.iter().map(|i| w(*i)).sum();
            let missing = tones.iter().filter(|i| w(**i) <= 0.0).count() as f64;
            let mut score = inside - (total - inside) - missing * 0.6 * maxw - tones.len() as f64 * 0.01 * maxw;
            if root == bass {
                score += 0.1 * maxw;
            }
            if best.is_none_or(|(b, _, _)| score > b) {
                best = Some((score, root, suffix));
            }
        }
    }
    let (_, root, suffix) = best?;
    let mut name = format!("{}{suffix}", NOTE_NAMES.get(root).copied().unwrap_or("C"));
    if bass != root {
        name.push('/');
        name.push_str(NOTE_NAMES.get(bass % 12).copied().unwrap_or("C"));
    }
    Some(name)
}

pub(super) fn extract_chords(e: &mut Engine, p: &Value) -> Result<Value> {
    let id = "event.extract_chords";
    let (_, t0, t1) = selection_ticks(e, p, id)?;
    let s = e.session();
    let picked = tracks_param(e, id, p)?;
    let mut tracks: Vec<TrackId> = picked.into_iter().filter(|t| s.track(*t).is_some_and(|x| x.kind.is_midi())).collect();
    if tracks.is_empty() {
        tracks = s.tracks.iter().filter(|t| t.kind.is_midi()).map(|t| t.id).collect();
    }
    // Absolute (tick) notes from every MIDI clip on those tracks, cut to their clip.
    let mut notes: Vec<(i64, i64, u8)> = Vec::new();
    for t in &tracks {
        for c in s.track(*t).map(|tr| tr.clips()).unwrap_or(&[]) {
            let ClipContent::Midi { sequence } = &c.content else { continue };
            let base = s.tempo.samples_to_ticks(c.start, s.sample_rate);
            let end = s.tempo.samples_to_ticks(c.end(), s.sample_rate);
            for n in &sequence.notes {
                let a = base + n.start;
                let b = (base + n.end()).min(end);
                if b > a {
                    notes.push((a, b, n.pitch));
                }
            }
        }
    }
    let first_bar = s.tempo.bar_beat_at_tick(t0).bar;
    let mut bar = first_bar;
    let mut chords: Vec<Value> = Vec::new();
    let mut marks: Vec<(Samples, String)> = Vec::new();
    let mut last = String::new();
    while chords.len() < 2000 {
        let a = s.tempo.bar_start_tick(bar).max(t0);
        let b = s.tempo.bar_start_tick(bar + 1).min(t1);
        if a >= t1 {
            break;
        }
        let mut w = [0.0f64; 12];
        let mut bass: Option<u8> = None;
        for (na, nb, pitch) in &notes {
            let ov = (*nb).min(b) - (*na).max(a);
            if ov > 0 {
                if let Some(x) = w.get_mut(usize::from(*pitch) % 12) {
                    *x += ov as f64;
                }
                bass = Some(bass.map_or(*pitch, |bp| bp.min(*pitch)));
            }
        }
        if let Some(name) = bass.and_then(|bp| chord_name(&w, usize::from(bp) % 12)) {
            let at = s.tempo.tick_to_samples(a, s.sample_rate);
            chords.push(json!({"bar": bar, "chord": name, "at": at}));
            if name != last {
                marks.push((at, name.clone()));
                last = name;
            }
        }
        bar += 1;
    }
    let s = e.session_mut();
    for (at, name) in &marks {
        let mid = s.add_marker(name, MarkerKind::Marker, *at, *at);
        if let Some(m) = s.markers.iter_mut().find(|m| m.id == mid) {
            m.ruler = 2;
        }
    }
    Ok(json!({"chords": chords, "markers": marks.len()}))
}
