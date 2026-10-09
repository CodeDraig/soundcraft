//! Automation-only clipboard operations.

use super::super::*;
use super::nonempty_range;
use crate::Clipboard;
use serde_json::json;
use soundcraft_model::AutoParam;
use soundcraft_model::AutomationLane;

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Lanes {
    All,
    Pan,
    Plugin,
}

impl Lanes {
    fn hit(self, p: &AutoParam) -> bool {
        match self {
            Lanes::All => true,
            Lanes::Pan => matches!(p, AutoParam::Pan(_)),
            Lanes::Plugin => matches!(p, AutoParam::Plugin { .. }),
        }
    }
}

pub(super) fn has_automation_clipboard(e: &Engine) -> std::result::Result<(), String> {
    if e.clipboard.automation.iter().flatten().any(|l| !l.points.is_empty()) { Ok(()) } else { Err("no automation has been copied".into()) }
}

pub(super) fn auto_copy(e: &mut Engine, p: &Value, which: Lanes, cut: bool, id: &str) -> Result<Value> {
    let tracks = tracks_required(e, id, p)?;
    let r = nonempty_range(e, p, id)?;
    let mut cb = Clipboard { length: r.len(), ..Clipboard::default() };
    let mut lanes_n = 0;
    for t in &tracks {
        cb.tracks.push(Vec::new());
        let lanes: Vec<AutomationLane> = e
            .session()
            .track(*t)
            .map(|tr| {
                tr.automation
                    .iter()
                    .filter(|l| which.hit(&l.param) && !l.points.is_empty())
                    .map(|l| {
                        let mut out = AutomationLane::new(l.param.clone());
                        out.enabled = l.enabled;
                        let (_, _, def) = l.param.range();
                        out.set_point(0, l.value_at(r.start, def));
                        for pt in l.points.iter().filter(|pt| pt.at > r.start && pt.at < r.end) {
                            out.set_point(pt.at - r.start, pt.value);
                        }
                        out.set_point(r.len() - 1, l.value_at(r.end - 1, def));
                        out
                    })
                    .collect()
            })
            .unwrap_or_default();
        lanes_n += lanes.len();
        cb.automation.push(lanes);
    }
    if cut {
        let s = e.session_mut();
        for t in &tracks {
            if let Some(tr) = s.track_mut(*t) {
                for l in tr.automation.iter_mut().filter(|l| which.hit(&l.param)) {
                    l.clear_range(r.start, r.end);
                }
            }
        }
    }
    e.clipboard = cb;
    Ok(json!({"lanes": lanes_n, "length": r.len()}))
}

fn map_value(v: f32, from: &AutoParam, to: &AutoParam) -> f32 {
    let (a0, a1, _) = from.range();
    let (b0, b1, _) = to.range();
    let finite = |x: f32, y: f32| x > f32::MIN && y < f32::MAX && y > x;
    if from == to || !finite(a0, a1) || !finite(b0, b1) {
        return v;
    }
    b0 + (v - a0) / (a1 - a0) * (b1 - b0)
}

pub(super) fn paste_to_current_automation(e: &mut Engine, p: &Value) -> Result<Value> {
    let id = "edit.paste_to_current_automation";
    let tracks = tracks_required(e, id, p)?;
    let at = position_param(e, id, p, "at")?.unwrap_or(e.session().edit.selection.start).max(0);
    let forced = match str_param(p, "param") {
        Some(x) => Some(AutoParam::parse(x).ok_or_else(|| bad(id, format!("unknown automation param `{x}`")))?),
        None => None,
    };
    let cb = e.clipboard.clone();
    let s = e.session_mut();
    let mut written = 0;
    for (i, t) in tracks.iter().enumerate() {
        let Some(lanes) = cb.automation.get(i % cb.automation.len().max(1)) else { continue };
        let Some(src) = lanes.iter().max_by_key(|l| l.points.len()).filter(|l| !l.points.is_empty()) else { continue };
        let Some(tr) = s.track_mut(*t) else { continue };
        let dst = forced.clone().unwrap_or_else(|| AutoParam::parse(&tr.view).unwrap_or(AutoParam::Volume));
        let lane = tr.lane_mut(&dst);
        lane.clear_range(at, at.saturating_add(cb.length));
        for pt in &src.points {
            lane.set_point(at.saturating_add(pt.at), map_value(pt.value, &src.param, &dst));
            written += 1;
        }
    }
    Ok(json!({"points": written}))
}
