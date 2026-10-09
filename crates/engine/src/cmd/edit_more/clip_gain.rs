//! Clip gain clipboard and volume coalescing.

use super::super::more_util::clip_db;
use super::super::more_util::set_clip_env;
use super::super::*;
use super::nonempty_range;
use crate::Clipboard;
use serde_json::json;
use soundcraft_model::AutoParam;
use soundcraft_model::AutomationLane;
use soundcraft_model::Clip;
use soundcraft_time::Range;
use soundcraft_time::Samples;

pub(super) fn has_clip_gain_clipboard(e: &Engine) -> std::result::Result<(), String> {
    if e.clipboard.clip_gain.is_empty() { Err("no clip gain has been copied".into()) } else { Ok(()) }
}

pub(super) fn copy_clip_gain(e: &mut Engine, p: &Value) -> Result<Value> {
    let id = "edit.copy_clip_gain";
    let tracks = tracks_required(e, id, p)?;
    let r = nonempty_range(e, p, id)?;
    let s = e.session();
    let mut out = Vec::new();
    for t in &tracks {
        let mut env: Vec<(Samples, f32)> = Vec::new();
        for c in s.track(*t).map(|tr| tr.clips()).unwrap_or(&[]) {
            let Some(i) = c.range().intersect(&r) else { continue };
            env.push((i.start - r.start, clip_db(c, i.start - c.start)));
            for &(o, _) in &c.gain_env {
                let at = c.start + o;
                if at > i.start && at < i.end - 1 {
                    env.push((at - r.start, clip_db(c, o)));
                }
            }
            env.push((i.end - 1 - r.start, clip_db(c, i.end - 1 - c.start)));
        }
        env.sort_by_key(|x| x.0);
        env.dedup_by_key(|x| x.0);
        out.push(env);
    }
    let n = out.iter().map(Vec::len).sum::<usize>();
    e.clipboard = Clipboard { length: r.len(), clip_gain: out, ..Clipboard::default() };
    Ok(json!({"points": n, "length": r.len()}))
}

pub(super) fn paste_clip_gain(e: &mut Engine, p: &Value) -> Result<Value> {
    let id = "edit.paste_clip_gain";
    let tracks = tracks_required(e, id, p)?;
    let at = position_param(e, id, p, "at")?.unwrap_or(e.session().edit.selection.start);
    let cb = e.clipboard.clone();
    let paste = Range::new(at, at.saturating_add(cb.length.max(1)));
    let s = e.session_mut();
    let mut n = 0;
    for (i, t) in tracks.iter().enumerate() {
        let Some(env) = cb.clip_gain.get(i % cb.clip_gain.len().max(1)) else { continue };
        let Some(pl) = s.track_mut(*t).and_then(|tr| tr.playlist_mut()) else { continue };
        for c in pl.clips.iter_mut().filter(|c| c.range().overlaps(&paste)) {
            let Some(ov) = c.range().intersect(&paste) else { continue };
            let (a, b) = (ov.start - c.start, ov.end - c.start);
            let mut pts: Vec<(Samples, f32)> = Vec::new();
            // Keep the existing curve outside the paste range, anchored at its edges.
            let mut old: Vec<(Samples, f32)> = c.gain_env.iter().map(|&(o, _)| (o, clip_db(c, o))).collect();
            old.push((0, clip_db(c, 0)));
            old.push((c.length, clip_db(c, c.length)));
            pts.extend(old.into_iter().filter(|(o, _)| *o < a || *o > b));
            if a > 0 {
                pts.push((a - 1, clip_db(c, a - 1)));
            }
            if b < c.length {
                pts.push((b, clip_db(c, b)));
            }
            for &(o, v) in env {
                let rel = at + o - c.start;
                if rel >= a && rel < b {
                    pts.push((rel, v));
                }
            }
            set_clip_env(c, pts);
            n += 1;
        }
    }
    Ok(json!({"clips": n}))
}

fn volume_at(lane: Option<&AutomationLane>, fader: f32, at: Samples) -> f32 {
    match lane {
        Some(l) if !l.points.is_empty() => l.value_at(at, fader),
        _ => fader,
    }
}

pub(super) fn coalesce_volume_to_clip_gain(e: &mut Engine, p: &Value) -> Result<Value> {
    let tracks = tracks_required(e, "edit.automation_coalesce_volume_to_clip_gain", p)?;
    let s = e.session_mut();
    let mut n = 0;
    for t in &tracks {
        let Some(tr) = s.track_mut(*t) else { continue };
        if !tr.kind.has_playlist() || tr.kind.is_midi() {
            continue;
        }
        let lane = tr.lane(&AutoParam::Volume).cloned();
        let fader = tr.mixer.volume_db;
        let Some(pl) = tr.playlist_mut() else { continue };
        for c in pl.clips.iter_mut().filter(|c| c.is_audio()) {
            let mut offs: Vec<Samples> = vec![0, c.length];
            offs.extend(c.gain_env.iter().map(|x| x.0));
            if let Some(l) = &lane {
                offs.extend(l.points.iter().filter(|pt| c.range().contains(pt.at)).map(|pt| pt.at - c.start));
            }
            offs.sort_unstable();
            offs.dedup();
            let pts: Vec<(Samples, f32)> = offs.iter().map(|&o| (o, clip_db(c, o) + volume_at(lane.as_ref(), fader, c.start + o))).collect();
            set_clip_env(c, pts);
            n += 1;
        }
        tr.automation.retain(|l| l.param != AutoParam::Volume);
        tr.mixer.volume_db = 0.0;
    }
    Ok(json!({"clips": n}))
}

pub(super) fn coalesce_clip_gain_to_volume(e: &mut Engine, p: &Value) -> Result<Value> {
    let tracks = tracks_required(e, "edit.automation_coalesce_clip_gain_to_volume", p)?;
    let s = e.session_mut();
    let mut n = 0;
    for t in &tracks {
        let Some(tr) = s.track_mut(*t) else { continue };
        let clips: Vec<Clip> = tr.clips().iter().filter(|c| c.is_audio() && (c.gain_db != 0.0 || !c.gain_env.is_empty())).cloned().collect();
        if clips.is_empty() {
            continue;
        }
        let lane = tr.lane(&AutoParam::Volume).cloned();
        let fader = tr.mixer.volume_db;
        let mut times: Vec<Samples> = lane.as_ref().map(|l| l.points.iter().map(|pt| pt.at).collect()).unwrap_or_default();
        for c in &clips {
            times.push(c.start);
            times.push(c.end() - 1);
            times.extend(c.gain_env.iter().map(|x| c.start + x.0));
        }
        times.sort_unstable();
        times.dedup();
        let gain_at = |at: Samples| clips.iter().find(|c| c.range().contains(at)).map_or(0.0, |c| clip_db(c, at - c.start));
        let pts: Vec<(Samples, f32)> = times.iter().map(|&at| (at, volume_at(lane.as_ref(), fader, at) + gain_at(at))).collect();
        let l = tr.lane_mut(&AutoParam::Volume);
        l.points.clear();
        for (at, v) in pts {
            l.set_point(at.max(0), v);
        }
        if let Some(pl) = tr.playlist_mut() {
            for c in pl.clips.iter_mut().filter(|c| clips.iter().any(|x| x.id == c.id)) {
                c.gain_db = 0.0;
                c.gain_env.clear();
                n += 1;
            }
        }
    }
    Ok(json!({"clips": n}))
}
