//! Step input, input quantization and retrospective MIDI recording.

use super::super::more_util::store_value;
use super::super::*;
use super::note_ticks;
use serde_json::json;
use soundcraft_midi::Note;
use soundcraft_midi::ops::QuantizeOptions;
use soundcraft_model::Clip;
use soundcraft_model::ClipContent;
use soundcraft_model::Session;
use soundcraft_model::TrackId;
use soundcraft_time::Range;
use soundcraft_time::Samples;
use soundcraft_time::TICKS_PER_QUARTER;

pub(super) fn input_quantize_state(s: &Session) -> Value {
    json!({
        "enabled": s.edit.flag("midi.input_quantize"),
        "grid_ticks": s.edit.value("midi.input_quantize.grid_ticks", (TICKS_PER_QUARTER / 4) as f64),
        "strength": s.edit.value("midi.input_quantize.strength", 100.0),
        "swing": s.edit.value("midi.input_quantize.swing", 0.0),
    })
}

pub(super) fn input_quantize(e: &mut Engine, p: &Value) -> Result<Value> {
    let s = e.session_mut();
    if let Some(on) = p.get("enabled").and_then(Value::as_bool) {
        s.edit.set_flag("midi.input_quantize", on);
    }
    if p.get("grid").is_some() {
        let g = note_ticks(p.get("grid"), TICKS_PER_QUARTER / 4);
        s.edit.values.insert("midi.input_quantize.grid_ticks".into(), g as f64);
    }
    store_value(&mut s.edit, p, "strength", "midi.input_quantize.strength", 0.0, 100.0);
    store_value(&mut s.edit, p, "swing", "midi.input_quantize.swing", 0.0, 100.0);
    Ok(input_quantize_state(s))
}

fn quantize_opts(s: &Session) -> QuantizeOptions {
    QuantizeOptions {
        grid_ticks: (s.edit.value("midi.input_quantize.grid_ticks", (TICKS_PER_QUARTER / 4) as f64) as i64).clamp(1, TICKS_PER_QUARTER * 16),
        strength: (s.edit.value("midi.input_quantize.strength", 100.0) / 100.0).clamp(0.0, 1.0) as f32,
        swing: (s.edit.value("midi.input_quantize.swing", 0.0) / 100.0).clamp(0.0, 1.0) as f32,
        ..QuantizeOptions::default()
    }
}

fn midi_target(e: &Engine, p: &Value, id: &str) -> Result<TrackId> {
    if let Some(t) = track_param(e, id, p, "track")? {
        return if e.session().track(t).is_some_and(|x| x.kind.is_midi()) {
            Ok(t)
        } else {
            Err(bad(id, "`track` is not a MIDI or instrument track"))
        };
    }
    let s = e.session();
    s.edit
        .selected_tracks
        .iter()
        .copied()
        .find(|t| s.track(*t).is_some_and(|x| x.kind.is_midi()))
        .ok_or_else(|| bad(id, "select a MIDI or instrument track"))
}

pub(super) fn step_input(e: &mut Engine, p: &Value) -> Result<Value> {
    let id = "event.step_input";
    let t = midi_target(e, p, id)?;
    let dur = note_ticks(p.get("duration"), TICKS_PER_QUARTER / 2);
    let vel = i64_or(p, "velocity", 100).clamp(1, 127) as u8;
    let rest = bool_or(p, "rest", false);
    let pitches: Vec<u8> = match p.get("pitches").and_then(Value::as_array) {
        Some(a) => a.iter().filter_map(Value::as_i64).map(|x| x.clamp(0, 127) as u8).take(16).collect(),
        None => vec![i64_or(p, "pitch", 60).clamp(0, 127) as u8],
    };
    let s = e.session_mut();
    let sr = s.sample_rate;
    let at = s.edit.selection.start.max(0);
    let tick = s.tempo.samples_to_ticks(at, sr);
    let end_tick = tick.saturating_add(dur);
    let end = s.tempo.tick_to_samples(end_tick, sr);
    // Continue the clip under (or ending at) the insertion point, else start a new one.
    let host = s.track(t).and_then(|tr| {
        tr.clips().iter().filter(|c| matches!(c.content, ClipContent::Midi { .. })).find(|c| c.range().contains(at) || c.end() == at).map(|c| c.id)
    });
    let host = match host {
        Some(h) => h,
        None => {
            let cid = s.new_clip_id();
            let name = s.track(t).map(|x| x.name.clone()).unwrap_or_default();
            crate::edit::place_clip(s, t, Clip::midi(cid, name, at, (end - at).max(1), soundcraft_midi::Sequence::default()));
            cid
        }
    };
    let next_start = s.track(t).and_then(|tr| tr.clips().iter().filter(|c| c.start > at && c.id != host).map(|c| c.start).min());
    let tempo = s.tempo.clone();
    let mut added = 0;
    if let Some(c) = s.find_clip_mut(host) {
        let base = tempo.samples_to_ticks(c.start, sr);
        if let ClipContent::Midi { sequence } = &mut c.content {
            if !rest {
                for pitch in &pitches {
                    sequence.notes.push(Note { pitch: *pitch, velocity: vel, release_velocity: 64, channel: 0, start: tick - base, length: dur });
                    added += 1;
                }
            }
            sequence.sort();
        }
        if end > c.end() {
            c.trim_end_to(end.min(next_start.unwrap_or(Samples::MAX)));
        }
    }
    if bool_or(p, "advance", true) {
        s.edit.selection = Range::point(end);
    }
    Ok(json!({"clip": host, "notes": added, "next": end}))
}

pub(super) fn retro_record(e: &mut Engine, p: &Value) -> Result<Value> {
    let id = "event.retrospective_record";
    let Some(list) = p.get("notes").and_then(Value::as_array).filter(|a| !a.is_empty()) else {
        return Ok(json!({"recorded": 0}));
    };
    let t = midi_target(e, p, id)?;
    let s = e.session();
    let sr = s.sample_rate;
    let mut raw: Vec<(i64, Note)> = Vec::new();
    for v in list.iter().take(100_000) {
        let tick = match (v.get("start_ticks").and_then(Value::as_i64), v.get("start")) {
            (Some(tk), _) => tk.max(0),
            (None, Some(_)) => s.tempo.samples_to_ticks(position_param(e, id, v, "start")?.unwrap_or(0).max(0), sr),
            _ => return Err(bad(id, "each note needs `start` or `start_ticks`")),
        };
        let note = Note {
            pitch: i64_or(v, "pitch", 60).clamp(0, 127) as u8,
            velocity: i64_or(v, "velocity", 100).clamp(1, 127) as u8,
            release_velocity: 64,
            channel: i64_or(v, "channel", 0).clamp(0, 15) as u8,
            start: 0,
            length: i64_or(v, "length_ticks", TICKS_PER_QUARTER / 4).clamp(1, TICKS_PER_QUARTER * 1024),
        };
        raw.push((tick, note));
    }
    let first = raw.iter().map(|x| x.0).min().unwrap_or(0);
    // Start the clip on the bar line so the quantize grid lines up with the music.
    let clip_tick = s.tempo.bar_start_tick(s.tempo.bar_beat_at_tick(first).bar).min(first).max(0);
    let end_tick = raw.iter().map(|(tk, n)| tk.saturating_add(n.length)).max().unwrap_or(first + 1);
    let mut seq = soundcraft_midi::Sequence::default();
    for (tk, mut n) in raw {
        n.start = tk - clip_tick;
        seq.notes.push(n);
    }
    seq.sort();
    let performance = seq.clone();
    let quantized = s.edit.flag("midi.input_quantize");
    if quantized {
        soundcraft_midi::ops::quantize(&mut seq.notes, &quantize_opts(s));
        seq.sort();
    }
    let start = s.tempo.tick_to_samples(clip_tick, sr);
    let end = s.tempo.tick_to_samples(end_tick, sr);
    let n = seq.notes.len();
    let s = e.session_mut();
    let cid = s.new_clip_id();
    let name = s.track(t).map(|x| x.name.clone()).unwrap_or_default();
    crate::edit::place_clip(s, t, Clip::midi(cid, name, start, (end - start).max(1), seq));
    s.midi_originals.insert(cid.0, performance);
    Ok(json!({"recorded": n, "clip": cid, "quantized": quantized}))
}
