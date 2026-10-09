//! MIDI note transformations.

use super::super::*;
use serde_json::json;
use soundcraft_midi::ops::DurationOp;
use soundcraft_midi::ops::QuantizeOptions;
use soundcraft_midi::ops::VelocityOp;
use soundcraft_model::ClipContent;
use soundcraft_time::TICKS_PER_QUARTER;

pub(super) fn midi_op(e: &mut Engine, p: &Value, f: impl Fn(&mut [soundcraft_midi::Note])) -> Result<Value> {
    let ids = clip_ids_param(e, p);
    let s = e.session_mut();
    let mut n = 0;
    for id in ids {
        if let Some(c) = s.find_clip_mut(id)
            && let ClipContent::Midi { sequence } = &mut c.content
        {
            f(&mut sequence.notes);
            sequence.sort();
            n += sequence.notes.len();
        }
    }
    Ok(json!({"notes": n}))
}

fn grid_ticks(p: &Value) -> i64 {
    match p.get("grid") {
        Some(Value::Number(n)) => n.as_i64().unwrap_or(240).clamp(1, TICKS_PER_QUARTER * 16),
        Some(Value::String(s)) => match s.trim() {
            "1/1" | "1" | "bar" => TICKS_PER_QUARTER * 4,
            "1/2" => TICKS_PER_QUARTER * 2,
            "1/4" => TICKS_PER_QUARTER,
            "1/8" => TICKS_PER_QUARTER / 2,
            "1/8t" => TICKS_PER_QUARTER / 3,
            "1/16t" => TICKS_PER_QUARTER / 6,
            "1/32" => TICKS_PER_QUARTER / 8,
            "1/64" => TICKS_PER_QUARTER / 16,
            _ => TICKS_PER_QUARTER / 4,
        },
        _ => TICKS_PER_QUARTER / 4,
    }
}

pub(super) fn quantize(e: &mut Engine, p: &Value) -> Result<Value> {
    let opts = QuantizeOptions {
        grid_ticks: grid_ticks(p),
        strength: (f32_or(p, "strength", 100.0) / 100.0).clamp(0.0, 1.0),
        swing: (f32_or(p, "swing", 0.0) / 100.0).clamp(0.0, 1.0),
        ..QuantizeOptions::default()
    };
    midi_op(e, p, move |n| soundcraft_midi::ops::quantize(n, &opts))
}

pub(super) fn velocity(e: &mut Engine, p: &Value) -> Result<Value> {
    let op = if let Some(v) = p.get("set").and_then(Value::as_u64) {
        VelocityOp::Set(v.min(127) as u8)
    } else if let Some(v) = p.get("add").and_then(Value::as_i64) {
        VelocityOp::Add(v.clamp(-127, 127) as i32)
    } else if let Some(v) = p.get("scale").and_then(Value::as_f64) {
        VelocityOp::Scale((v as f32).clamp(0.0, 10.0))
    } else {
        VelocityOp::Limit { min: i64_or(p, "min", 1).clamp(1, 127) as u8, max: i64_or(p, "max", 127).clamp(1, 127) as u8 }
    };
    midi_op(e, p, move |n| soundcraft_midi::ops::change_velocity(n, op))
}

pub(super) fn duration(e: &mut Engine, p: &Value) -> Result<Value> {
    let op = if let Some(v) = p.get("set").and_then(Value::as_i64) {
        DurationOp::Set(v.max(1))
    } else if let Some(v) = p.get("add").and_then(Value::as_i64) {
        DurationOp::Add(v)
    } else if let Some(v) = p.get("scale").and_then(Value::as_f64) {
        DurationOp::Scale((v as f32).clamp(0.01, 100.0))
    } else {
        DurationOp::Legato { gap: i64_or(p, "legato", 0).max(0) }
    };
    midi_op(e, p, move |n| soundcraft_midi::ops::change_duration(n, op))
}

pub(super) fn add_note(e: &mut Engine, p: &Value) -> Result<Value> {
    let ids = clip_ids_param(e, p);
    let id = *ids.first().ok_or_else(|| bad("event.midi_note", "`clip` required"))?;
    let note = soundcraft_midi::Note {
        pitch: i64_or(p, "pitch", 60).clamp(0, 127) as u8,
        velocity: i64_or(p, "velocity", 100).clamp(1, 127) as u8,
        release_velocity: 64,
        channel: i64_or(p, "channel", 0).clamp(0, 15) as u8,
        start: i64_or(p, "start_ticks", 0).max(0),
        length: i64_or(p, "length_ticks", TICKS_PER_QUARTER).max(1),
    };
    let s = e.session_mut();
    match s.find_clip_mut(id).map(|c| &mut c.content) {
        Some(ClipContent::Midi { sequence }) => {
            sequence.notes.push(note);
            sequence.sort();
            Ok(json!({"notes": sequence.notes.len()}))
        }
        _ => Err(bad("event.midi_note", "not a MIDI clip")),
    }
}
