//! More Event menu commands: the operations windows, Move Song Start, tempo curves, MIDI
//! operations (split notes, input quantize, step input, restore/flatten performance), MIDI track
//! offsets and real-time properties, chord extraction, Beat Detective, Identify Beat and
//! Retrospective Record.

use super::more_util::values_with_prefix;
use super::*;
use crate::cmd;
use serde_json::json;
use soundcraft_time::TICKS_PER_QUARTER;

mod tempo;
use tempo::{Curve, identify_beat, move_song_start, selection_ticks, tempo_curve, tempo_stretch, time_state};
mod midi_input;
use midi_input::{input_quantize, input_quantize_state, retro_record, step_input};
mod midi_performance;
use midi_performance::{midi_offsets, midi_rtp, performance, select_split};
mod chords;
use chords::extract_chords;
mod beat_detective;
use beat_detective::beat_detective;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(query "event.time_ops_window", "Time Operations Window", ["Event", "Time Operations"], None, "{} — tempo/meter map, song start and selection for the Time Operations window", always, |e, _| Ok(time_state(e.session()))),
        cmd!(query "event.tempo_ops_window", "Tempo Operations Window", ["Event", "Tempo Operations"], None, "{}", always, |e, _| Ok(time_state(e.session()))),
        cmd!(query "event.midi_ops_window", "MIDI Operations Window", ["Event", "MIDI Operations"], None, "{} — input quantize and stored performances", always, |e, _| {
            let s = e.session();
            Ok(json!({"input_quantize": input_quantize_state(s), "performances": s.midi_originals.len(), "rtp": values_with_prefix(&s.edit, "rtp.")}))
        }),
        cmd!(
            "event.move_song_start",
            "Move Song Start...",
            ["Event", "Time Operations"],
            None,
            "{to: position} — moves the song start; tick-based clips and later tempo/meter events move with it",
            always,
            move_song_start
        ),
        cmd!(
            "event.tempo_linear",
            "Linear...",
            ["Event", "Tempo Operations"],
            None,
            "{start?, end?, start_bpm?, end_bpm, resolution?: '1/8'|ticks}",
            always,
            |e, p| tempo_curve(e, p, Curve::Linear, "event.tempo_linear")
        ),
        cmd!(
            "event.tempo_parabolic",
            "Parabolic...",
            ["Event", "Tempo Operations"],
            None,
            "{start?, end?, start_bpm?, end_bpm, curvature?: -1..1 (0.5), resolution?}",
            always,
            |e, p| tempo_curve(e, p, Curve::Parabolic, "event.tempo_parabolic")
        ),
        cmd!(
            "event.tempo_s_curve",
            "S-Curve...",
            ["Event", "Tempo Operations"],
            None,
            "{start?, end?, start_bpm?, end_bpm, resolution?}",
            always,
            |e, p| tempo_curve(e, p, Curve::Sigmoid, "event.tempo_s_curve")
        ),
        cmd!(
            "event.tempo_stretch",
            "Stretch...",
            ["Event", "Tempo Operations"],
            None,
            "{start?, end?, factor? | length?: new length} — scales the tempo inside the selection so it lasts `factor` times as long",
            always,
            tempo_stretch
        ),
        cmd!(
            "event.select_split_notes",
            "Select/Split Notes...",
            ["Event", "MIDI Operations"],
            None,
            "{clips?, pitch_from?: 0, pitch_to?: 127, action?: select|split} — split moves the matching notes to a new MIDI track",
            has_selection,
            select_split
        ),
        cmd!(noundo "event.input_quantize", "Input Quantize...", ["Event", "MIDI Operations"], None, "{enabled?: bool, grid?: '1/16'|ticks, strength?: 0..100, swing?: 0..100}", always, input_quantize),
        cmd!(
            "event.step_input",
            "Step Input...",
            ["Event", "MIDI Operations"],
            None,
            "{track?, pitch?: 60 | pitches?: [..], duration?: '1/8'|ticks, velocity?: 100, rest?: bool, advance?: true}",
            has_tracks,
            step_input
        ),
        cmd!(
            "event.restore_performance",
            "Restore Performance...",
            ["Event", "MIDI Operations"],
            None,
            "{clips?} — returns MIDI clips to their stored original performance",
            has_selection,
            |e, p| performance(e, p, false)
        ),
        cmd!(
            "event.flatten_performance",
            "Flatten Performance...",
            ["Event", "MIDI Operations"],
            None,
            "{clips?} — makes the current notes the performance that Restore returns to",
            has_selection,
            |e, p| performance(e, p, true)
        ),
        cmd!(noundo "event.midi_track_offsets", "MIDI Track Offsets", ["Event"], None, "{track?, offset?: samples|{seconds}} — sets (or lists) per-track MIDI playback offsets", always, midi_offsets),
        cmd!(noundo "event.midi_rtp", "MIDI Real-Time Properties", ["Event"], None, "{tracks?, velocity?: ±127, transpose?: ±127, duration?: % (100), delay?: ticks, clear?: bool}", always, midi_rtp),
        cmd!(
            "event.extract_chords",
            "Extract Chords from Selection",
            ["Event"],
            None,
            "{tracks?, start?, end?} — names the chord in each bar and adds it as a marker on Markers 2",
            has_range,
            extract_chords
        ),
        cmd!(
            "event.beat_detective",
            "Beat Detective",
            ["Event"],
            None,
            "{tracks?, start?, end?, sensitivity?: 0.5, separate?: false, set_tempo?: false}",
            has_range,
            beat_detective
        ),
        cmd!(
            "event.identify_beat",
            "Identify Beat...",
            ["Event"],
            Some("Cmd+I"),
            "{start?, end?, bars?: 1, beats?: 0} — sets the tempo so the selection lasts that many bars",
            has_range,
            identify_beat
        ),
        cmd!(
            "event.retrospective_record",
            "Retrospective Record",
            ["Event"],
            None,
            "{track?, notes?: [{pitch, start?: position | start_ticks?, length_ticks?: 240, velocity?: 100}]} — turns captured MIDI into a clip (input quantize applies)",
            always,
            retro_record
        ),
    ]
}

/// Duration in ticks from a note-value string or a number of ticks.
fn note_ticks(v: Option<&Value>, default: i64) -> i64 {
    match v {
        Some(Value::Number(n)) => n.as_i64().unwrap_or(default).clamp(1, TICKS_PER_QUARTER * 64),
        Some(Value::String(s)) => match s.trim() {
            "1/1" | "1" | "bar" => TICKS_PER_QUARTER * 4,
            "1/2" => TICKS_PER_QUARTER * 2,
            "1/4" => TICKS_PER_QUARTER,
            "1/4t" => TICKS_PER_QUARTER * 2 / 3,
            "1/8" => TICKS_PER_QUARTER / 2,
            "1/8t" => TICKS_PER_QUARTER / 3,
            "1/16" => TICKS_PER_QUARTER / 4,
            "1/16t" => TICKS_PER_QUARTER / 6,
            "1/32" => TICKS_PER_QUARTER / 8,
            "1/64" => TICKS_PER_QUARTER / 16,
            _ => default,
        },
        _ => default,
    }
}

#[cfg(test)]
mod tests {
    use super::super::more_util::track_key;
    use super::chords::chord_name;
    use soundcraft_model::{ClipContent, ClipId, TrackId};

    use super::*;

    fn midi_engine() -> (Engine, TrackId) {
        let mut e = Engine::default();
        e.execute("track.new", &json!({"kind": "midi"})).unwrap();
        let t = e.session().tracks[0].id;
        (e, t)
    }

    #[test]
    fn chord_names() {
        let mut w = [0.0; 12];
        for pc in [0, 4, 7] {
            w[pc] = 1.0;
        }
        assert_eq!(chord_name(&w, 0).as_deref(), Some("C"));
        let mut w = [0.0; 12];
        for pc in [9, 0, 4] {
            w[pc] = 1.0;
        }
        assert_eq!(chord_name(&w, 9).as_deref(), Some("Am"));
        let mut w = [0.0; 12];
        for pc in [7, 11, 2, 5] {
            w[pc] = 1.0;
        }
        assert_eq!(chord_name(&w, 7).as_deref(), Some("G7"));
        assert_eq!(chord_name(&[0.0; 12], 0), None);
    }

    #[test]
    fn retro_record_quantizes_and_restores() {
        let (mut e, t) = midi_engine();
        e.execute("event.input_quantize", &json!({"enabled": true, "grid": "1/4"})).unwrap();
        let r = e
            .execute(
                "event.retrospective_record",
                &json!({"track": t.0, "notes": [{"pitch": 60, "start_ticks": 1010}, {"pitch": 64, "start_ticks": 1930}]}),
            )
            .unwrap();
        assert_eq!(r["recorded"], 2);
        let cid = ClipId(r["clip"].as_u64().unwrap());
        let notes = |e: &Engine| match &e.session().find_clip(cid).unwrap().1.content {
            ClipContent::Midi { sequence } => sequence.notes.iter().map(|n| n.start).collect::<Vec<_>>(),
            _ => vec![],
        };
        assert_eq!(notes(&e), vec![960, 1920]);
        e.execute("edit.select", &json!({"clips": [cid.0]})).unwrap();
        e.execute("event.restore_performance", &json!({})).unwrap();
        assert_eq!(notes(&e), vec![1010, 1930]);
        assert_eq!(e.execute("event.retrospective_record", &json!({})).unwrap()["recorded"], 0);
    }

    #[test]
    fn flatten_then_restore() {
        let (mut e, t) = midi_engine();
        let r = e.execute("event.retrospective_record", &json!({"track": t.0, "notes": [{"pitch": 60, "start_ticks": 100}]})).unwrap();
        let cid = r["clip"].as_u64().unwrap();
        e.execute("edit.select", &json!({"clips": [cid]})).unwrap();
        e.execute("event.transpose", &json!({"semitones": 5})).unwrap();
        e.execute("event.flatten_performance", &json!({})).unwrap();
        e.execute("event.transpose", &json!({"semitones": 5})).unwrap();
        e.execute("event.restore_performance", &json!({})).unwrap();
        match &e.session().find_clip(ClipId(cid)).unwrap().1.content {
            ClipContent::Midi { sequence } => assert_eq!(sequence.notes[0].pitch, 65),
            _ => panic!(),
        }
    }

    #[test]
    fn step_input_appends_and_advances() {
        let (mut e, t) = midi_engine();
        e.execute("edit.select", &json!({"tracks": [t.0], "start": 0})).unwrap();
        e.execute("event.step_input", &json!({"pitch": 60, "duration": "1/4"})).unwrap();
        e.execute("event.step_input", &json!({"pitches": [64, 67], "duration": "1/4"})).unwrap();
        e.execute("event.step_input", &json!({"rest": true, "duration": "1/4"})).unwrap();
        let tr = e.session().track(t).unwrap();
        assert_eq!(tr.clips().len(), 1);
        let c = &tr.clips()[0];
        assert_eq!(c.length, 3 * 24_000);
        match &c.content {
            ClipContent::Midi { sequence } => assert_eq!(sequence.notes.iter().map(|n| n.start).collect::<Vec<_>>(), vec![0, 960, 960]),
            _ => panic!(),
        }
        assert_eq!(e.session().edit.selection.start, 3 * 24_000);
    }

    #[test]
    fn tempo_curves() {
        let mut e = Engine::default();
        e.execute("track.new", &json!({})).unwrap();
        e.execute("event.tempo_linear", &json!({"start": 0, "end": 96_000, "start_bpm": 120.0, "end_bpm": 160.0, "resolution": "1/4"})).unwrap();
        let tempos = e.session().tempo.tempos().to_vec();
        assert_eq!(tempos.first().unwrap().bpm, 120.0);
        assert!(tempos.windows(2).all(|w| w[1].bpm >= w[0].bpm || w[1].tick >= 3840));
        assert_eq!(e.session().tempo.tempo_at_tick(3840), 120.0, "tempo after the selection is kept");
        assert_eq!(tempos.len(), 5);
        let mut e = Engine::default();
        e.execute("event.tempo_stretch", &json!({"start": 0, "end": 96_000, "factor": 2.0})).unwrap();
        assert_eq!(e.session().tempo.tempo_at_tick(0), 60.0);
        assert_eq!(e.session().tempo.tick_to_samples(3840, soundcraft_time::SampleRate::HZ_48000), 192_000);
    }

    #[test]
    fn identify_beat_sets_tempo() {
        let mut e = Engine::default();
        e.execute("event.identify_beat", &json!({"start": 0, "end": 96_000, "bars": 1})).unwrap();
        assert!((e.session().tempo.tempo_at_tick(0) - 120.0).abs() < 1e-9);
        e.execute("event.identify_beat", &json!({"start": 0, "end": 192_000, "bars": 1})).unwrap();
        assert!((e.session().tempo.tempo_at_tick(0) - 60.0).abs() < 1e-9);
    }

    #[test]
    fn split_notes_to_new_track() {
        let (mut e, t) = midi_engine();
        let r = e
            .execute(
                "event.retrospective_record",
                &json!({"track": t.0, "notes": [{"pitch": 36, "start_ticks": 0}, {"pitch": 72, "start_ticks": 0}]}),
            )
            .unwrap();
        let cid = r["clip"].as_u64().unwrap();
        e.execute("edit.select", &json!({"clips": [cid]})).unwrap();
        let r = e.execute("event.select_split_notes", &json!({"pitch_from": 60, "pitch_to": 127, "action": "split"})).unwrap();
        assert_eq!(r["matched"], 1);
        assert_eq!(e.session().tracks.len(), 2);
        let nt = e.session().tracks.iter().find(|x| x.id != t).unwrap();
        match &nt.clips()[0].content {
            ClipContent::Midi { sequence } => assert_eq!(sequence.notes[0].pitch, 72),
            _ => panic!(),
        }
    }

    #[test]
    fn chords_from_demo_become_markers() {
        let mut e = crate::demo::demo_engine();
        let keys = e.session().track_by_name("Keys").unwrap().clone();
        let c = keys.clips()[0].clone();
        let before = e.session().markers.len();
        e.execute("edit.select", &json!({"tracks": [keys.id.0], "start": c.start, "end": c.end()})).unwrap();
        let r = e.execute("event.extract_chords", &json!({})).unwrap();
        assert!(!r["chords"].as_array().unwrap().is_empty(), "{r}");
        assert!(e.session().markers.len() > before);
        assert!(e.session().markers.iter().any(|m| m.ruler == 2));
    }

    #[test]
    fn beat_detective_finds_demo_tempo() {
        let mut e = crate::demo::demo_engine();
        let kick = e.session().track_by_name("Kick").unwrap().clone();
        let c = kick.clips()[0].clone();
        let r = e.execute("event.beat_detective", &json!({"tracks": [kick.id.0], "start": c.start, "end": c.start + 48_000 * 8})).unwrap();
        assert!(r["count"].as_u64().unwrap() > 4, "{r}");
        assert!(r["bpm"].as_f64().is_some());
    }

    #[test]
    fn move_song_start_moves_tick_material() {
        let (mut e, t) = midi_engine();
        e.execute("event.retrospective_record", &json!({"track": t.0, "notes": [{"pitch": 60, "start_ticks": 0}]})).unwrap();
        e.execute("event.move_song_start", &json!({"to": 48_000})).unwrap();
        assert_eq!(e.session().track(t).unwrap().clips()[0].start, 48_000);
        assert_eq!(e.session().edit.value("song.start", 0.0), 48_000.0);
    }

    #[test]
    fn rtp_and_offsets_are_settings() {
        let (mut e, t) = midi_engine();
        let undo = e.undo_history().len();
        e.execute("event.midi_rtp", &json!({"track": t.0, "transpose": 7})).unwrap();
        assert_eq!(e.session().edit.value(&track_key("rtp", t, "transpose"), 0.0), 7.0);
        let r = e.execute("event.midi_track_offsets", &json!({"track": t.0, "offset": 480})).unwrap();
        assert_eq!(r["offsets"][0]["offset"], 480.0);
        assert_eq!(e.undo_history().len(), undo, "settings are not undo steps");
    }
}
