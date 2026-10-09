//! Event menu: time/tempo operations, MIDI operations, markers.

use super::*;
use crate::cmd;
use serde_json::json;
use soundcraft_model::ClipContent;

mod time;
use time::{cut_time, insert_time, meter, tempo};
mod midi;
use midi::{add_note, duration, midi_op, quantize, velocity};
mod markers;
use markers::{add_marker, delete_marker, edit_marker, recall_marker, step_marker};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!("event.tempo", "Tempo Change", [], None, "{bpm, at?: position (default session start)}", always, tempo),
        cmd!("event.tempo_remove", "Remove Tempo Change", [], None, "{at}", always, |e, p| {
            let at = position_param(e, "event.tempo_remove", p, "at")?.unwrap_or(0);
            let s = e.session_mut();
            let tick = s.tempo.samples_to_ticks(at, s.sample_rate);
            Ok(json!({"removed": s.tempo.remove_tempo(tick)}))
        }),
        cmd!("event.meter", "Change Meter...", ["Event", "Time Operations"], None, "{numerator, denominator, at?: position}", always, meter),
        cmd!("event.tempo_scale", "Scale...", ["Event", "Tempo Operations"], None, "{factor}", always, |e, p| {
            let f = f64_or(p, "factor", 1.0);
            e.session_mut().tempo.scale_tempos(f).map_err(|err| bad("event.tempo_scale", err.to_string()))?;
            Ok(json!({}))
        }),
        cmd!("event.tempo_constant", "Constant...", ["Event", "Tempo Operations"], None, "{bpm, start?, end?}", always, tempo),
        cmd!("event.insert_time", "Insert Time...", ["Event", "Time Operations"], None, "{start?, length | end}", has_tracks, insert_time),
        cmd!("event.cut_time", "Cut Time...", ["Event", "Time Operations"], None, "{start?, end?}", has_tracks, cut_time),
        cmd!(
            "event.quantize",
            "Quantize...",
            ["Event", "MIDI Operations"],
            Some("Alt+0"),
            "{grid?: 1/16 | ticks, strength?: 100, swing?: 0, clips?}",
            has_selection,
            quantize
        ),
        cmd!("event.transpose", "Transpose...", ["Event", "MIDI Operations"], Some("Alt+T"), "{semitones, clips?}", has_selection, |e, p| {
            let st = i64_or(p, "semitones", 0).clamp(-127, 127) as i32;
            midi_op(e, p, move |n| soundcraft_midi::ops::transpose(n, st))
        }),
        cmd!(
            "event.change_velocity",
            "Change Velocity...",
            ["Event", "MIDI Operations"],
            None,
            "{set?|add?|scale?|min?,max?, clips?}",
            has_selection,
            velocity
        ),
        cmd!(
            "event.change_duration",
            "Change Duration...",
            ["Event", "MIDI Operations"],
            Some("Alt+P"),
            "{set?|add?|scale?|legato?: gap, clips?}",
            has_selection,
            duration
        ),
        cmd!("event.remove_duplicate_notes", "Remove Duplicate Notes", ["Event"], None, "{clips?}", has_selection, |e, p| {
            let ids = clip_ids_param(e, p);
            let s = e.session_mut();
            let mut n = 0;
            for id in ids {
                if let Some(c) = s.find_clip_mut(id)
                    && let ClipContent::Midi { sequence } = &mut c.content
                {
                    n += sequence.remove_duplicates()
                }
            }
            Ok(json!({"removed": n}))
        }),
        cmd!("event.add_key_change", "Add Key Change...", ["Event"], None, "{key: 'C major', at?}", always, |e, p| {
            let key = str_param(p, "key").unwrap_or("C major").to_string();
            let at = position_param(e, "event.add_key_change", p, "at")?.unwrap_or(0);
            let s = e.session_mut();
            s.key_signatures.retain(|(t, _)| *t != at);
            s.key_signatures.push((at, key));
            s.key_signatures.sort_by_key(|k| k.0);
            Ok(json!({}))
        }),
        cmd!("event.renumber_bars", "Renumber Bars...", ["Event"], None, "{}", always, |_, _| Ok(json!({"note": "bars start at 1"}))),
        cmd!("event.all_notes_off", "All MIDI Notes Off", ["Event"], Some("Cmd+Shift+."), "{}", always, |e, _| {
            e.transport_requests.push(crate::TransportRequest::AllNotesOff);
            Ok(json!({}))
        }),
        cmd!("event.midi_note", "Add MIDI Note", [], None, "{clip, pitch, start_ticks, length_ticks, velocity?: 100}", has_selection, add_note),
        cmd!(
            "markers.add",
            "New Memory Location",
            [],
            Some("Enter"),
            "{name?, at?, start?, end?, kind?: marker|selection|none, ruler?: 1..5}",
            always,
            add_marker
        ),
        cmd!("markers.delete", "Delete Memory Location", [], None, "{number | id}", always, delete_marker),
        cmd!("markers.edit", "Edit Memory Location", [], None, "{number | id, name?, comments?, at?, color?}", always, edit_marker),
        cmd!(noundo "markers.recall", "Recall Memory Location", [], Some("Period + n + Period"), "{number}", always, recall_marker),
        cmd!(noundo "markers.next", "Go to Next Marker", [], None, "{}", always, |e, _| step_marker(e, 1)),
        cmd!(noundo "markers.previous", "Go to Previous Marker", [], None, "{}", always, |e, _| step_marker(e, -1)),
    ]
}
