//! Bulk insert bypass and send muting.

use super::super::*;
use serde_json::json;
use soundcraft_dsp::Category;
use soundcraft_model::Track;

pub(super) fn bypass_where(e: &mut Engine, p: &Value, pick: fn(usize, Option<Category>) -> bool) -> Result<Value> {
    let tracks = tracks_required(e, "track.bypass_inserts", p)?;
    let s = e.session_mut();
    let hits = |tr: &Track| -> Vec<usize> {
        tr.mixer
            .inserts
            .iter()
            .enumerate()
            .filter_map(|(i, x)| x.as_ref().map(|ins| (i, ins)))
            .filter(|(i, ins)| pick(*i, soundcraft_dsp::plugin_info(&ins.plugin).map(|info| info.category)))
            .map(|(i, _)| i)
            .collect()
    };
    let all_bypassed = tracks
        .iter()
        .filter_map(|t| s.track(*t))
        .all(|tr| hits(tr).iter().all(|i| tr.mixer.inserts.get(*i).and_then(Option::as_ref).is_some_and(|x| x.bypass)));
    let target = p.get("bypass").and_then(Value::as_bool).unwrap_or(!all_bypassed);
    let mut n = 0;
    for t in &tracks {
        let Some(tr) = s.track_mut(*t) else { continue };
        for i in hits(tr) {
            if let Some(Some(ins)) = tr.mixer.inserts.get_mut(i) {
                ins.bypass = target;
                n += 1;
            }
        }
    }
    Ok(json!({"bypass": target, "inserts": n}))
}

pub(super) fn mute_sends_where(e: &mut Engine, p: &Value, from: usize, to: usize) -> Result<Value> {
    let tracks = tracks_required(e, "track.mute_sends", p)?;
    let s = e.session_mut();
    let all_muted = tracks.iter().filter_map(|t| s.track(*t)).all(|tr| tr.mixer.sends.iter().skip(from).take(to - from).flatten().all(|x| x.mute));
    let target = p.get("mute").and_then(Value::as_bool).unwrap_or(!all_muted);
    let mut n = 0;
    for t in &tracks {
        if let Some(tr) = s.track_mut(*t) {
            for snd in tr.mixer.sends.iter_mut().skip(from).take(to - from).flatten() {
                snd.mute = target;
                n += 1;
            }
        }
    }
    Ok(json!({"mute": target, "sends": n}))
}
