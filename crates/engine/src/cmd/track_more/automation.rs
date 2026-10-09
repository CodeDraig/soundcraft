//! Trim and VCA automation coalescing.

use super::super::*;
use serde_json::json;
use soundcraft_model::AutoParam;
use soundcraft_model::AutomationLane;
use soundcraft_model::Track;
use soundcraft_model::TrackId;
use soundcraft_model::TrackKind;
use soundcraft_time::Samples;

/// Trim automation lives in a lane addressed past the last insert slot, so the mixer ignores it
/// until it is coalesced into the volume lane.
pub fn trim_param() -> AutoParam {
    AutoParam::Plugin { slot: u8::MAX, param: "trim".into() }
}

pub(super) fn trim_write(e: &mut Engine, p: &Value) -> Result<Value> {
    let id = "track.trim_automation_write";
    let tracks = tracks_required(e, id, p)?;
    let r = range_param(e, id, p)?;
    if r.is_empty() {
        return Err(bad(id, "the selection is empty"));
    }
    let db = p.get("db").and_then(Value::as_f64).filter(|v| v.is_finite()).ok_or_else(|| bad(id, "`db` required"))?.clamp(-144.0, 24.0) as f32;
    let s = e.session_mut();
    for t in &tracks {
        if let Some(tr) = s.track_mut(*t) {
            tr.lane_mut(&trim_param()).write_range(r.start, r.end, db, 0.0);
        }
    }
    Ok(json!({"db": db}))
}

/// Add an offset curve (dB) to a track's volume automation (creating it from the fader level).
fn add_to_volume(tr: &mut Track, offset: &AutomationLane, offset_default: f32) {
    let fader = tr.mixer.volume_db;
    let vol = tr.lane(&AutoParam::Volume).filter(|l| !l.points.is_empty()).cloned();
    let mut times: Vec<Samples> = offset.points.iter().map(|p| p.at).collect();
    if let Some(v) = &vol {
        times.extend(v.points.iter().map(|p| p.at));
    }
    if times.is_empty() {
        tr.mixer.volume_db = (fader + offset_default).clamp(-144.0, 12.0);
        return;
    }
    times.sort_unstable();
    times.dedup();
    let base = |at: Samples| vol.as_ref().map_or(fader, |l| l.value_at(at, fader));
    let pts: Vec<(Samples, f32)> = times.iter().map(|&at| (at, base(at) + offset.value_at(at, offset_default))).collect();
    let lane = tr.lane_mut(&AutoParam::Volume);
    lane.points.clear();
    for (at, v) in pts {
        lane.set_point(at, v);
    }
}

pub(super) fn coalesce_trim(e: &mut Engine, p: &Value) -> Result<Value> {
    let tracks = tracks_required(e, "track.coalesce_trim", p)?;
    let s = e.session_mut();
    let mut n = 0;
    for t in &tracks {
        let Some(tr) = s.track_mut(*t) else { continue };
        let Some(trim) = tr.lane(&trim_param()).cloned() else { continue };
        if !trim.points.is_empty() {
            add_to_volume(tr, &trim, 0.0);
            n += 1;
        }
        tr.automation.retain(|l| l.param != trim_param());
    }
    Ok(json!({"tracks": n}))
}

pub(super) fn coalesce_vca(e: &mut Engine, p: &Value) -> Result<Value> {
    let id = "track.coalesce_vca";
    let picked = tracks_param(e, id, p)?;
    let s = e.session_mut();
    let vcas: Vec<TrackId> = if picked.is_empty() {
        s.tracks.iter().filter(|t| t.kind == TrackKind::Vca).map(|t| t.id).collect()
    } else {
        picked.into_iter().filter(|t| s.track(*t).is_some_and(|x| x.kind == TrackKind::Vca)).collect()
    };
    if vcas.is_empty() {
        return Err(bad(id, "select a VCA master"));
    }
    let mut out = Vec::new();
    for v in vcas {
        let Some(vca) = s.track(v).cloned() else { continue };
        let lane = vca.lane(&AutoParam::Volume).cloned().unwrap_or_else(|| AutomationLane::new(AutoParam::Volume));
        let level = vca.mixer.volume_db;
        let members: Vec<TrackId> = s.tracks.iter().filter(|t| t.mixer.vca == Some(v.0)).map(|t| t.id).collect();
        for m in &members {
            if let Some(tr) = s.track_mut(*m) {
                add_to_volume(tr, &lane, level);
            }
        }
        if let Some(tr) = s.track_mut(v) {
            tr.automation.retain(|l| l.param != AutoParam::Volume);
            tr.mixer.volume_db = 0.0;
        }
        out.push(json!({"vca": v, "members": members}));
    }
    Ok(json!({"coalesced": out}))
}
