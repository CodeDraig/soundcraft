//! Merging MIDI clipboard material into destination clips.

use super::super::*;
use crate::edit as ops;
use serde_json::json;
use soundcraft_model::ClipContent;
use soundcraft_time::Samples;

pub(super) fn paste_merge_midi(e: &mut Engine, p: &Value) -> Result<Value> {
    let id = "edit.paste_merge_midi";
    let tracks = tracks_required(e, id, p)?;
    let at = position_param(e, id, p, "at")?.unwrap_or(e.session().edit.selection.start).max(0);
    let cb = e.clipboard.clone();
    let s = e.session_mut();
    let tempo = s.tempo.clone();
    let sr = s.sample_rate;
    let mut merged = 0;
    let mut placed = 0;
    for (i, t) in tracks.iter().enumerate() {
        let Some(clips) = cb.tracks.get(i % cb.tracks.len().max(1)) else { continue };
        for c in clips {
            let pos = at.saturating_add(c.start);
            let ClipContent::Midi { sequence } = &c.content else {
                let mut nc = c.clone();
                nc.id = s.new_clip_id();
                nc.start = pos;
                ops::place_clip(s, *t, nc);
                placed += 1;
                continue;
            };
            let target = s.track(*t).and_then(|tr| {
                let host = tr.clips().iter().find(|x| matches!(x.content, ClipContent::Midi { .. }) && x.range().contains(pos))?;
                let next_start = tr.clips().iter().filter(|x| x.start > host.start && x.id != host.id).map(|x| x.start).min();
                Some((host.id, host.start, next_start))
            });
            match target {
                Some((hid, hstart, next_start)) => {
                    let diff = tempo.samples_to_ticks(pos, sr) - tempo.samples_to_ticks(hstart, sr);
                    let want_end = pos.saturating_add(c.length);
                    let limit = next_start.unwrap_or(Samples::MAX);
                    if let Some(h) = s.find_clip_mut(hid)
                        && let ClipContent::Midi { sequence: hs } = &mut h.content
                    {
                        for n in &sequence.notes {
                            let mut n = *n;
                            n.start = n.start.saturating_add(diff);
                            hs.notes.push(n);
                        }
                        for cc in &sequence.ctrls {
                            let mut cc = *cc;
                            cc.tick = cc.tick.saturating_add(diff);
                            hs.ctrls.push(cc);
                        }
                        hs.sort();
                        if want_end > h.end() {
                            h.trim_end_to(want_end.min(limit));
                        }
                        merged += 1;
                    }
                }
                None => {
                    let mut nc = c.clone();
                    nc.id = s.new_clip_id();
                    nc.start = pos;
                    ops::place_clip(s, *t, nc);
                    placed += 1;
                }
            }
        }
    }
    Ok(json!({"merged": merged, "placed": placed}))
}
