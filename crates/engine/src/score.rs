//! Notation: MIDI tracks laid out as measures of notes and rests, written as MusicXML
//! (File › Export › Sibelius, File › Send To › Sibelius) or engraved to SVG (File › Print Score).
//!
//! Notes are quantized to sixteenths. Each part is a single voice: notes that start together form
//! a chord, overlapping notes are cut at the next onset, and anything crossing a bar line is split
//! and tied.

use soundcraft_model::{ClipContent, Session, TrackId, TrackKind};
use soundcraft_time::TICKS_PER_QUARTER;

mod musicxml;
pub use musicxml::musicxml;
use musicxml::note_type;
mod engraving;
pub use engraving::svg;

/// Ticks per score unit (a sixteenth).
const UNIT: i64 = TICKS_PER_QUARTER / 4;
/// Upper bound on engraved measures, so a hostile session cannot make a huge document.
const MAX_MEASURES: usize = 2000;
/// Note values a single notehead can show, in sixteenths (longest first).
const VALUES: [u32; 8] = [16, 12, 8, 6, 4, 3, 2, 1];

/// Score Setup (File › Score Setup…). Stored in the session's edit flags/values.
#[derive(Debug, Clone, PartialEq)]
pub struct ScoreSetup {
    pub title: String,
    pub composer: String,
    pub bars_per_system: u32,
    pub show_track_names: bool,
}

const TITLE_KEY: &str = "score.title=";
const COMPOSER_KEY: &str = "score.composer=";

impl ScoreSetup {
    pub fn from_session(s: &Session) -> Self {
        let text = |key: &str| s.edit.flags.iter().find_map(|f| f.strip_prefix(key)).map(str::to_string);
        let bars = s.edit.value("score.bars_per_system", 4.0);
        Self {
            title: text(TITLE_KEY).unwrap_or_else(|| s.name.clone()),
            composer: text(COMPOSER_KEY).unwrap_or_default(),
            bars_per_system: if bars.is_finite() { bars.clamp(1.0, 16.0) as u32 } else { 4 },
            show_track_names: !s.edit.flag("score.hide_names"),
        }
    }

    pub fn store(&self, s: &mut Session) {
        s.edit.flags.retain(|f| !f.starts_with(TITLE_KEY) && !f.starts_with(COMPOSER_KEY));
        let clean = |t: &str| t.chars().filter(|c| !c.is_control()).take(200).collect::<String>();
        s.edit.flags.insert(format!("{TITLE_KEY}{}", clean(&self.title)));
        s.edit.flags.insert(format!("{COMPOSER_KEY}{}", clean(&self.composer)));
        s.edit.values.insert("score.bars_per_system".into(), f64::from(self.bars_per_system.clamp(1, 16)));
        s.edit.set_flag("score.hide_names", !self.show_track_names);
    }
}

/// One measure: its meter and its items (notes, chords or rests), in sixteenths.
#[derive(Debug, Clone, PartialEq)]
pub struct Measure {
    pub numerator: u32,
    pub denominator: u32,
    pub items: Vec<Item>,
}

/// A notehead group (chord) or a rest (no pitches).
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub pitches: Vec<u8>,
    /// Duration in sixteenths; always one of the supported notation durations.
    pub units: u32,
    pub tie_start: bool,
    pub tie_stop: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Part {
    pub name: String,
    /// True when the part reads best in bass clef.
    pub bass: bool,
    pub measures: Vec<Measure>,
}

/// Lay out the MIDI (and instrument) tracks `tracks` (all of them when empty).
pub fn layout(s: &Session, tracks: &[TrackId]) -> Vec<Part> {
    let sr = s.sample_rate;
    let mut raw: Vec<(String, Vec<(i64, i64, u8)>)> = Vec::new();
    for t in &s.tracks {
        if !matches!(t.kind, TrackKind::Midi | TrackKind::Instrument) || (!tracks.is_empty() && !tracks.contains(&t.id)) {
            continue;
        }
        let mut notes = Vec::new();
        for c in t.clips() {
            let ClipContent::Midi { sequence } = &c.content else { continue };
            if c.muted {
                continue;
            }
            let base = s.tempo.samples_to_ticks(c.start, sr);
            let end = s.tempo.samples_to_ticks(c.start.saturating_add(c.length), sr);
            for n in &sequence.notes {
                let st = base.saturating_add(n.start);
                if st < base || st >= end || n.length <= 0 {
                    continue;
                }
                let a = quantize(st);
                let b = quantize(st.saturating_add(n.length).min(end)).max(a + 1);
                notes.push((a, b, n.pitch));
            }
        }
        raw.push((t.name.clone(), notes));
    }
    let last = raw.iter().flat_map(|(_, n)| n.iter().map(|x| x.1)).max().unwrap_or(0);
    let bars = measure_grid(s, last);
    raw.into_iter()
        .map(|(name, notes)| {
            let avg = if notes.is_empty() { 60.0 } else { notes.iter().map(|n| f64::from(n.2)).sum::<f64>() / notes.len() as f64 };
            Part { name, bass: avg < 57.0, measures: fill(&bars, &notes) }
        })
        .collect()
}

fn quantize(tick: i64) -> i64 {
    (tick.max(0) + UNIT / 2) / UNIT
}

/// (start unit, length in units, numerator, denominator) per measure, covering `end_unit`.
fn measure_grid(s: &Session, end_unit: i64) -> Vec<(i64, i64, u32, u32)> {
    let mut v = Vec::new();
    let mut bar = 1;
    loop {
        let a = s.tempo.bar_start_tick(bar);
        let b = s.tempo.bar_start_tick(bar + 1);
        let m = s.tempo.meter_at_tick(a);
        let (ua, ub) = (a / UNIT, b / UNIT);
        if ub <= ua {
            break;
        }
        v.push((ua, ub - ua, m.numerator, m.denominator));
        bar += 1;
        if ub >= end_unit.max(1) || v.len() >= MAX_MEASURES {
            break;
        }
    }
    v
}

/// Split `units` into note values that a single notehead can show.
fn split_units(mut units: i64) -> Vec<u32> {
    let mut out = Vec::new();
    while units > 0 {
        let v = VALUES.iter().copied().find(|v| i64::from(*v) <= units).unwrap_or(1);
        out.push(v);
        units -= i64::from(v);
    }
    out
}

fn fill(bars: &[(i64, i64, u32, u32)], notes: &[(i64, i64, u8)]) -> Vec<Measure> {
    // Onsets → chords, each held until its longest note ends or the next onset.
    let mut onsets: std::collections::BTreeMap<i64, (i64, Vec<u8>)> = std::collections::BTreeMap::new();
    for (a, b, p) in notes {
        let e = onsets.entry(*a).or_insert((*b, Vec::new()));
        e.0 = e.0.max(*b);
        if !e.1.contains(p) {
            e.1.push(*p);
        }
    }
    let starts: Vec<i64> = onsets.keys().copied().collect();
    let mut spans: Vec<(i64, i64, Vec<u8>)> = Vec::new();
    for (i, (a, (b, mut ps))) in onsets.into_iter().enumerate() {
        let end = starts.get(i + 1).map_or(b, |n| b.min(*n));
        ps.sort_unstable();
        spans.push((a, end, ps));
    }
    let mut measures = Vec::new();
    for (ms, ml, num, den) in bars {
        let me = ms + ml;
        let mut items = Vec::new();
        let mut cursor = *ms;
        let emit = |from: i64, to: i64, pitches: &[u8], tied_in: bool, tied_out: bool, items: &mut Vec<Item>| {
            let parts = split_units(to - from);
            let n = parts.len();
            for (k, u) in parts.into_iter().enumerate() {
                let rest = pitches.is_empty();
                items.push(Item {
                    pitches: pitches.to_vec(),
                    units: u,
                    tie_stop: !rest && (tied_in || k > 0),
                    tie_start: !rest && (tied_out || k + 1 < n),
                });
            }
        };
        for (a, b, ps) in spans.iter().filter(|(a, b, _)| *b > *ms && *a < me) {
            let from = (*a).max(*ms).max(cursor);
            let to = (*b).min(me);
            if to <= from {
                continue;
            }
            if from > cursor {
                emit(cursor, from, &[], false, false, &mut items);
            }
            emit(from, to, ps, *a < *ms, *b > me, &mut items);
            cursor = to;
        }
        if cursor < me {
            emit(cursor, me, &[], false, false, &mut items);
        }
        measures.push(Measure { numerator: *num, denominator: *den, items });
    }
    measures
}

/// (step, alter, octave) spelled with sharps.
fn spell(pitch: u8) -> (char, i32, i32) {
    const STEPS: [(char, i32); 12] =
        [('C', 0), ('C', 1), ('D', 0), ('D', 1), ('E', 0), ('F', 0), ('F', 1), ('G', 0), ('G', 1), ('A', 0), ('A', 1), ('B', 0)];
    let (step, alter) = STEPS[usize::from(pitch % 12)];
    (step, alter, i32::from(pitch / 12) - 1)
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::engraving::staff_step;

    use super::*;

    #[test]
    fn split_units_uses_representable_values() {
        assert_eq!(split_units(16), vec![16]);
        assert_eq!(split_units(5), vec![4, 1]);
        assert_eq!(split_units(7), vec![6, 1]);
        assert!(split_units(0).is_empty());
    }

    #[test]
    fn spelling_and_staff_steps() {
        assert_eq!(spell(60), ('C', 0, 4));
        assert_eq!(spell(61), ('C', 1, 4));
        assert_eq!(staff_step(64, false), 0); // E4: bottom treble line
        assert_eq!(staff_step(77, false), 8); // F5: top treble line
        assert_eq!(staff_step(43, true), 0); // G2: bottom bass line
    }

    #[test]
    fn measures_fill_exactly_and_tie_across_bars() {
        // 4/4: one bar = 16 units. A note from unit 12 to 20 crosses the bar line.
        let bars = vec![(0, 16, 4, 4), (16, 16, 4, 4)];
        let m = fill(&bars, &[(0, 4, 60), (0, 4, 64), (12, 20, 67)]);
        assert_eq!(m.len(), 2);
        for meas in &m {
            assert_eq!(meas.items.iter().map(|i| i.units).sum::<u32>(), 16);
        }
        assert_eq!(m[0].items[0].pitches, vec![60, 64]);
        assert!(m[0].items.last().is_some_and(|i| i.tie_start));
        assert!(m[1].items[0].tie_stop);
    }

    #[test]
    fn demo_exports_well_formed_documents() {
        let e = crate::demo::demo_engine();
        let parts = layout(e.session(), &[]);
        assert!(!parts.is_empty(), "demo has MIDI tracks");
        let setup = ScoreSetup::from_session(e.session());
        let x = musicxml(&parts, &setup);
        assert!(x.contains("<score-partwise") && x.contains("<pitch>") && x.trim_end().ends_with("</score-partwise>"));
        assert_eq!(x.matches("<measure ").count(), x.matches("</measure>").count());
        let s = svg(&parts, &setup);
        assert!(s.starts_with("<svg") && s.contains("<ellipse"));
    }
}
