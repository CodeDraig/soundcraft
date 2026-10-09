//! Track creation, conversion, duplication and removal.

use super::super::*;
use serde_json::json;
use soundcraft_model::ChannelFormat;
use soundcraft_model::Route;
use soundcraft_model::TrackKind;

pub(super) fn new_track(e: &mut Engine, p: &Value) -> Result<Value> {
    let count = i64_or(p, "count", 1).clamp(1, 128);
    let kind = match str_param(p, "kind") {
        Some(k) => TrackKind::from_id(k).ok_or_else(|| bad("track.new", format!("unknown kind `{k}`")))?,
        None => TrackKind::Audio,
    };
    let default_fmt = if matches!(kind, TrackKind::Master | TrackKind::Aux | TrackKind::Instrument | TrackKind::Folder) {
        ChannelFormat::Stereo
    } else {
        ChannelFormat::Mono
    };
    let format = match str_param(p, "format") {
        Some(f) => ChannelFormat::from_id(f).ok_or_else(|| bad("track.new", format!("unknown format `{f}`")))?,
        None => default_fmt,
    };
    let name = str_param(p, "name").map(str::to_string);
    let ticks = str_param(p, "timebase").map(|t| t == "ticks");
    let instrument = str_param(p, "instrument").map(str::to_string);
    let s = e.session_mut();
    let mut ids = Vec::new();
    for _ in 0..count {
        let id = s.add_track(kind, format, name.as_deref());
        if let Some(t) = s.track_mut(id) {
            if let Some(tb) = ticks {
                t.ticks_timebase = tb;
            }
            if kind == TrackKind::Instrument {
                t.instrument = Some(soundcraft_model::Insert::new(instrument.clone().unwrap_or_else(|| "subtractive_synth".into())));
            }
        }
        ids.push(id);
    }
    s.edit.selected_tracks = ids.clone();
    Ok(json!({"tracks": ids}))
}

pub(super) fn duplicate(e: &mut Engine, p: &Value) -> Result<Value> {
    let tracks = tracks_required(e, "track.duplicate", p)?;
    let (pl, auto, ins, snd) =
        (bool_or(p, "playlists", true), bool_or(p, "automation", true), bool_or(p, "inserts", true), bool_or(p, "sends", true));
    let s = e.session_mut();
    let mut out = Vec::new();
    for t in tracks {
        let Some(idx) = s.track_index(t) else { continue };
        let Some(mut nt) = s.tracks.get(idx).cloned() else { continue };
        nt.id = soundcraft_model::TrackId(s.alloc());
        nt.name = s.unique_track_name(&format!("{}.dup", nt.name));
        if !pl {
            for p in &mut nt.playlists {
                p.clips.clear();
            }
        } else {
            for p in &mut nt.playlists {
                for c in &mut p.clips {
                    c.id = s.new_clip_id();
                }
            }
        }
        if !auto {
            nt.automation.clear();
        }
        if !ins {
            nt.mixer.inserts.iter_mut().for_each(|i| *i = None);
        }
        if !snd {
            nt.mixer.sends.iter_mut().for_each(|i| *i = None);
        }
        out.push(nt.id);
        s.tracks.insert((idx + 1).min(s.tracks.len()), nt);
    }
    s.edit.selected_tracks = out.clone();
    Ok(json!({"tracks": out}))
}

pub(super) fn split_mono(e: &mut Engine, p: &Value) -> Result<Value> {
    let tracks = tracks_required(e, "track.split_into_mono", p)?;
    let s = e.session_mut();
    let mut out = Vec::new();
    for t in tracks {
        let Some(src) = s.track(t).cloned() else { continue };
        if src.channels() < 2 || src.kind != TrackKind::Audio {
            continue;
        }
        for ch in 0..src.channels() {
            let suffix = match (src.channels(), ch) {
                (2, 0) => "L".to_string(),
                (2, 1) => "R".to_string(),
                _ => format!("{}", ch + 1),
            };
            let id = s.add_track(TrackKind::Audio, ChannelFormat::Mono, Some(&format!("{}.{suffix}", src.name)));
            let mut clips = src.clips().to_vec();
            for c in &mut clips {
                c.id = s.new_clip_id();
                // Mono tracks read channel 0 of the source; record the channel in the name for now.
                c.name = format!("{}.{suffix}", c.name);
            }
            if let Some(tr) = s.track_mut(id) {
                tr.mixer.pan = vec![if ch == 0 { -1.0 } else { 1.0 }];
                tr.comments = format!("source_channel={ch}");
                if let Some(pl) = tr.playlist_mut() {
                    pl.clips = clips;
                }
            }
            out.push(id);
        }
    }
    Ok(json!({"tracks": out}))
}

pub(super) fn move_to_folder(e: &mut Engine, p: &Value) -> Result<Value> {
    let tracks = tracks_required(e, "track.move_to_new_folder", p)?;
    let name = str_param(p, "name").map(str::to_string);
    let s = e.session_mut();
    let first_idx = tracks.iter().filter_map(|t| s.track_index(*t)).min().unwrap_or(0);
    let fid = s.add_track(TrackKind::Folder, ChannelFormat::Stereo, Some(name.as_deref().unwrap_or("Folder")));
    // Place the folder above its first member.
    if let Some(i) = s.track_index(fid) {
        let f = s.tracks.remove(i);
        s.tracks.insert(first_idx.min(s.tracks.len()), f);
    }
    // A routing folder sums its members through its own bus.
    let fname = s.track(fid).map(|x| x.name.clone()).unwrap_or_else(|| "Folder".into());
    let bus = s.add_bus(&format!("{fname} Bus"), ChannelFormat::Stereo);
    if let Some(f) = s.track_mut(fid) {
        f.mixer.input = Route::Bus(bus);
    }
    for t in &tracks {
        if let Some(tr) = s.track_mut(*t) {
            tr.folder = Some(fid);
            if tr.mixer.output == Route::Main {
                tr.mixer.output = Route::Bus(bus);
            }
        }
    }
    Ok(json!({"folder": fid, "bus": bus}))
}

pub(super) fn change_width(e: &mut Engine, p: &Value) -> Result<Value> {
    let tracks = tracks_required(e, "track.change_width", p)?;
    let f = str_param(p, "format").and_then(ChannelFormat::from_id).ok_or_else(|| bad("track.change_width", "`format` required"))?;
    let s = e.session_mut();
    for t in &tracks {
        if let Some(tr) = s.track_mut(*t) {
            tr.format = f;
            tr.mixer.pan = soundcraft_model::Mixer::new(f).pan;
        }
    }
    Ok(json!({"format": f}))
}

pub(super) fn delete(e: &mut Engine, p: &Value) -> Result<Value> {
    let tracks = tracks_required(e, "track.delete", p)?;
    let s = e.session_mut();
    s.tracks.retain(|t| !tracks.contains(&t.id));
    s.edit.selected_tracks.retain(|t| !tracks.contains(t));
    for g in &mut s.groups {
        g.members.retain(|t| !tracks.contains(t));
    }
    for t in &mut s.tracks {
        if t.folder.is_some_and(|f| tracks.contains(&f)) {
            t.folder = None;
        }
    }
    Ok(json!({"deleted": tracks.len()}))
}

pub(super) fn move_track(e: &mut Engine, p: &Value) -> Result<Value> {
    let t = track_param(e, "track.move", p, "track")?.ok_or_else(|| bad("track.move", "`track` required"))?;
    let to = i64_or(p, "to", 0).max(0) as usize;
    let s = e.session_mut();
    if let Some(i) = s.track_index(t) {
        let tr = s.tracks.remove(i);
        s.tracks.insert(to.min(s.tracks.len()), tr);
    }
    Ok(json!({}))
}
