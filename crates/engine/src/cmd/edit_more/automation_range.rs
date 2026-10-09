//! Trimming and gliding automation over a selection.

use super::super::*;
use super::nonempty_range;
use serde_json::json;
use soundcraft_model::AutoParam;

pub(super) fn trim_glide(e: &mut Engine, p: &Value, glide: bool, pan_only: bool, id: &str) -> Result<Value> {
    let tracks = tracks_required(e, id, p)?;
    let r = nonempty_range(e, p, id)?;
    let s = e.session_mut();
    let mut lanes = 0;
    for t in &tracks {
        let Some(tr) = s.track_mut(*t) else { continue };
        let mut params: Vec<AutoParam> = tr
            .automation
            .iter()
            .filter(|l| l.enabled && !matches!(l.param, AutoParam::Mute | AutoParam::SendMute(_)))
            .filter(|l| !pan_only || matches!(l.param, AutoParam::Pan(_)))
            .map(|l| l.param.clone())
            .collect();
        if pan_only {
            for i in 0..tr.mixer.pan.len().min(2) {
                let pp = AutoParam::Pan(i as u8);
                if !params.contains(&pp) {
                    params.push(pp);
                }
            }
        }
        for param in params {
            let cur = super::super::mix::current_value(tr, &param);
            let (lo, hi, _) = param.range();
            let l = tr.lane_mut(&param);
            let before = l.value_at(r.start - 1, cur);
            let after = l.value_at(r.end, cur);
            let from = l.value_at(r.start, cur);
            if glide {
                l.clear_range(r.start, r.end.saturating_add(2));
                if r.start > 0 {
                    l.set_point(r.start - 1, before);
                }
                l.set_point(r.start, from);
                l.set_point(r.end, cur);
                l.set_point(r.end.saturating_add(1), after);
            } else {
                let delta = cur - from;
                let last = l.value_at(r.end - 1, cur);
                if r.start > 0 {
                    l.set_point(r.start - 1, before);
                }
                l.set_point(r.start, from);
                l.set_point(r.end - 1, last);
                l.set_point(r.end, after);
                for pt in l.points.iter_mut().filter(|pt| pt.at >= r.start && pt.at < r.end) {
                    pt.value = (pt.value + delta).clamp(lo, hi);
                }
            }
            lanes += 1;
        }
    }
    Ok(json!({"lanes": lanes}))
}
