//! MIDI performance selection and real-time properties.

use super::super::more_util::store_value;
use super::super::more_util::track_key;
use super::super::more_util::values_with_prefix;
use super::super::*;
use serde_json::json;
use soundcraft_model::ChannelFormat;
use soundcraft_model::ClipContent;
use soundcraft_model::ClipId;
use soundcraft_model::TrackId;
use soundcraft_model::TrackKind;

fn midi_clip_ids(e: &Engine, p: &Value) -> Vec<(TrackId, ClipId)> {
    let s = e.session();
    clip_ids_param(e, p)
        .into_iter()
        .filter_map(|id| s.find_clip(id).filter(|(_, c)| matches!(c.content, ClipContent::Midi { .. })).map(|(t, _)| (t, id)))
        .collect()
}

pub(super) fn select_split(e: &mut Engine, p: &Value) -> Result<Value> {
    let id = "event.select_split_notes";
    let from = i64_or(p, "pitch_from", 0).clamp(0, 127) as u8;
    let to = i64_or(p, "pitch_to", 127).clamp(0, 127) as u8;
    let split = match str_param(p, "action") {
        None | Some("select") => false,
        Some("split") => true,
        Some(a) => return Err(bad(id, format!("unknown action `{a}`"))),
    };
    let clips = midi_clip_ids(e, p);
    if clips.is_empty() {
        return Err(bad(id, "select MIDI clips"));
    }
    let s = e.session_mut();
    let mut matched = 0;
    let mut new_tracks: std::collections::BTreeMap<u64, TrackId> = Default::default();
    for (t, cid) in clips {
        let Some(c) = s.find_clip(cid).map(|(_, c)| c.clone()) else { continue };
        let ClipContent::Midi { sequence } = &c.content else { continue };
        let (hit, rest) = soundcraft_midi::ops::split_notes(&sequence.notes, from, to);
        matched += hit.len();
        if !split || hit.is_empty() {
            continue;
        }
        if let Some(cl) = s.find_clip_mut(cid)
            && let ClipContent::Midi { sequence } = &mut cl.content
        {
            sequence.notes = rest;
        }
        let nt = match new_tracks.get(&t.0) {
            Some(x) => *x,
            None => {
                let name = s.track(t).map_or_else(|| "MIDI".to_string(), |x| format!("{} Split", x.name));
                let nt = s.add_track(TrackKind::Midi, ChannelFormat::Mono, Some(&name));
                new_tracks.insert(t.0, nt);
                nt
            }
        };
        let mut nc = c.clone();
        nc.id = s.new_clip_id();
        nc.content = ClipContent::Midi { sequence: soundcraft_midi::Sequence { notes: hit, ctrls: Vec::new() } };
        crate::edit::place_clip(s, nt, nc);
    }
    Ok(json!({"matched": matched, "split": split, "tracks": new_tracks.values().collect::<Vec<_>>()}))
}

pub(super) fn performance(e: &mut Engine, p: &Value, flatten: bool) -> Result<Value> {
    let clips = midi_clip_ids(e, p);
    let s = e.session_mut();
    let mut n = 0;
    for (_, cid) in clips {
        if flatten {
            if let Some(ClipContent::Midi { sequence }) = s.find_clip(cid).map(|(_, c)| c.content.clone()) {
                s.midi_originals.insert(cid.0, sequence);
                n += 1;
            }
        } else if let Some(orig) = s.midi_originals.get(&cid.0).cloned()
            && let Some(c) = s.find_clip_mut(cid)
            && let ClipContent::Midi { sequence } = &mut c.content
            && *sequence != orig
        {
            *sequence = orig;
            n += 1;
        }
    }
    Ok(json!({"clips": n}))
}

pub(super) fn midi_offsets(e: &mut Engine, p: &Value) -> Result<Value> {
    let id = "event.midi_track_offsets";
    let t = track_param(e, id, p, "track")?;
    let off = position_param(e, id, p, "offset")?;
    let s = e.session_mut();
    if let (Some(t), Some(o)) = (t, off) {
        let lim = s.sample_rate.samples(10.0);
        s.edit.values.insert(format!("midi.offset.{}", t.0), o.clamp(-lim, lim) as f64);
    }
    let all: Vec<Value> = s
        .tracks
        .iter()
        .filter(|tr| tr.kind.is_midi())
        .map(|tr| json!({"track": tr.id, "name": tr.name, "offset": s.edit.value(&format!("midi.offset.{}", tr.id.0), 0.0)}))
        .collect();
    Ok(json!({"offsets": all}))
}

pub(super) fn midi_rtp(e: &mut Engine, p: &Value) -> Result<Value> {
    let tracks = tracks_param(e, "event.midi_rtp", p)?;
    let s = e.session_mut();
    let clear = bool_or(p, "clear", false);
    for t in &tracks {
        if clear {
            let pre = format!("rtp.{}.", t.0);
            s.edit.values.retain(|k, _| !k.starts_with(&pre));
            continue;
        }
        store_value(&mut s.edit, p, "velocity", &track_key("rtp", *t, "velocity"), -127.0, 127.0);
        store_value(&mut s.edit, p, "transpose", &track_key("rtp", *t, "transpose"), -127.0, 127.0);
        store_value(&mut s.edit, p, "duration", &track_key("rtp", *t, "duration"), 1.0, 1000.0);
        store_value(&mut s.edit, p, "delay", &track_key("rtp", *t, "delay"), -1e7, 1e7);
    }
    Ok(json!({"rtp": values_with_prefix(&s.edit, "rtp.")}))
}
