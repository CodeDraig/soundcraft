//! Input/output routing and mixer bypass.

use super::super::*;
use serde_json::json;
use soundcraft_model::ChannelFormat;
use soundcraft_model::Route;
use soundcraft_model::TrackKind;

pub(super) fn bypass_inserts(e: &mut Engine, p: &Value) -> Result<Value> {
    let tracks = tracks_required(e, "track.bypass_inserts", p)?;
    let s = e.session_mut();
    let target = p
        .get("bypass")
        .and_then(Value::as_bool)
        .unwrap_or_else(|| !tracks.iter().all(|t| s.track(*t).is_some_and(|tr| tr.mixer.inserts.iter().flatten().all(|i| i.bypass))));
    for t in &tracks {
        if let Some(tr) = s.track_mut(*t) {
            for i in tr.mixer.inserts.iter_mut().flatten() {
                i.bypass = target;
            }
        }
    }
    Ok(json!({"bypass": target}))
}

pub(super) fn mute_sends(e: &mut Engine, p: &Value) -> Result<Value> {
    let tracks = tracks_required(e, "track.mute_sends", p)?;
    let s = e.session_mut();
    let target = p.get("mute").and_then(Value::as_bool).unwrap_or(true);
    for t in &tracks {
        if let Some(tr) = s.track_mut(*t) {
            for snd in tr.mixer.sends.iter_mut().flatten() {
                snd.mute = target;
            }
        }
    }
    Ok(json!({"mute": target}))
}

pub(crate) fn parse_route(s: &soundcraft_model::Session, v: &str) -> Route {
    match v.to_ascii_lowercase().as_str() {
        "" | "none" | "no input" | "no output" => Route::None,
        "main" | "out 1-2" | "master" => Route::Main,
        _ => s.bus_by_name(v).map_or_else(|| Route::Hardware(v.to_string()), |b| Route::Bus(b.id)),
    }
}

pub(super) fn route(e: &mut Engine, p: &Value, input: bool) -> Result<Value> {
    let key = if input { "input" } else { "output" };
    let tracks = tracks_required(e, "track.route", p)?;
    let v = str_param(p, key).ok_or_else(|| bad("track.route", format!("`{key}` required")))?.to_string();
    let create_bus = bool_or(p, "create_bus", true);
    let s = e.session_mut();
    let mut r = parse_route(s, &v);
    if let Route::Hardware(name) = &r
        && create_bus
        && !name.to_ascii_lowercase().starts_with("in ")
    {
        let fmt = tracks
            .first()
            .and_then(|t| s.track(*t))
            .map_or(ChannelFormat::Stereo, |t| if t.kind == TrackKind::Aux { t.format } else { ChannelFormat::Stereo });
        r = Route::Bus(s.add_bus(&name.clone(), fmt));
    }
    for t in &tracks {
        if let Some(tr) = s.track_mut(*t) {
            if input {
                tr.mixer.input = r.clone();
            } else {
                tr.mixer.output = r.clone();
            }
        }
    }
    Ok(json!({key: r}))
}
