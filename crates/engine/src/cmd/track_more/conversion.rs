//! Track kind and MIDI material conversion.

use super::super::*;
use serde_json::json;
use soundcraft_model::ChannelFormat;
use soundcraft_model::Clip;
use soundcraft_model::ClipContent;
use soundcraft_model::Route;
use soundcraft_model::Track;
use soundcraft_model::TrackKind;

pub(super) fn convert_aux(e: &mut Engine, p: &Value) -> Result<Value> {
    let tracks = tracks_required(e, "track.convert_aux_to_folder", p)?;
    let s = e.session_mut();
    let mut out = Vec::new();
    for t in &tracks {
        let Some(tr) = s.track(*t) else { continue };
        if tr.kind != TrackKind::Aux {
            continue;
        }
        let input = tr.mixer.input.clone();
        if let Some(tr) = s.track_mut(*t) {
            tr.kind = TrackKind::Folder;
            tr.folder_open = true;
        }
        let mut members = 0;
        if let Route::Bus(_) = input {
            for m in s.tracks.iter_mut().filter(|m| m.id != *t && m.mixer.output == input) {
                m.folder = Some(*t);
                members += 1;
            }
        }
        out.push(json!({"track": t, "members": members}));
    }
    if out.is_empty() {
        return Err(bad("track.convert_aux_to_folder", "select one or more aux input tracks"));
    }
    Ok(json!({"converted": out}))
}

pub(super) fn extract_midi(e: &mut Engine, p: &Value) -> Result<Value> {
    let tracks = tracks_required(e, "track.extract_midi", p)?;
    let s = e.session_mut();
    let mut out = Vec::new();
    for t in &tracks {
        let Some(src) = s.track(*t).cloned() else { continue };
        if !src.kind.is_midi() {
            continue;
        }
        let midi: Vec<Clip> = src.clips().iter().filter(|c| matches!(c.content, ClipContent::Midi { .. })).cloned().collect();
        let nt = s.add_track(TrackKind::Midi, ChannelFormat::Mono, Some(&format!("{} MIDI", src.name)));
        if let (Some(i), Some(ni)) = (s.track_index(*t), s.track_index(nt)) {
            let tr = s.tracks.remove(ni);
            s.tracks.insert((i + 1).min(s.tracks.len()), tr);
        }
        let mut clips = Vec::new();
        for mut c in midi {
            c.id = s.new_clip_id();
            clips.push(c);
        }
        let n = clips.len();
        if let Some(pl) = s.track_mut(nt).and_then(Track::playlist_mut) {
            pl.clips = clips;
            pl.sort();
        }
        if let Some(tr) = s.track_mut(nt) {
            tr.color = src.color;
            tr.ticks_timebase = src.ticks_timebase;
        }
        out.push(json!({"from": t, "track": nt, "clips": n}));
    }
    if out.is_empty() {
        return Err(bad("track.extract_midi", "select a MIDI or instrument track"));
    }
    Ok(json!({"tracks": out}))
}
