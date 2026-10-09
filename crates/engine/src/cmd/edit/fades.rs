//! Selection-based fade editing.

use super::super::*;
use crate::edit as ops;
use serde_json::json;
use soundcraft_model::FadeShape;

pub(super) fn fades_create(e: &mut Engine, p: &Value) -> Result<Value> {
    let tracks = tracks_required(e, "edit.fades_create", p)?;
    let r = range_param(e, "edit.fades_create", p)?;
    let shape = str_param(p, "shape")
        .map_or(Some(FadeShape::EqualPower), FadeShape::from_id)
        .ok_or_else(|| bad("edit.fades_create", "unknown fade shape"))?;
    let s = e.session_mut();
    let n: usize = tracks.iter().map(|t| ops::create_fades(s, *t, r, shape)).sum();
    if n == 0 {
        return Err(bad("edit.fades_create", "the selection must cover a clip start, end or the boundary between two clips"));
    }
    Ok(json!({"fades": n}))
}

pub(super) fn fades_delete(e: &mut Engine, p: &Value) -> Result<Value> {
    let tracks = tracks_required(e, "edit.fades_delete", p)?;
    let r = range_param(e, "edit.fades_delete", p)?;
    let s = e.session_mut();
    let n: usize = tracks.iter().map(|t| ops::delete_fades(s, *t, r)).sum();
    Ok(json!({"deleted": n}))
}

pub(super) fn fade_to(e: &mut Engine, p: &Value, to_start: bool) -> Result<Value> {
    let tracks = tracks_required(e, "edit.fade_to", p)?;
    let at = e.session().edit.selection.start;
    let s = e.session_mut();
    let mut n = 0;
    for t in &tracks {
        let range = s.track(*t).and_then(|tr| {
            tr.clips()
                .iter()
                .find(|c| c.range().contains(at))
                .map(|c| if to_start { soundcraft_time::Range::new(c.start, at) } else { soundcraft_time::Range::new(at, c.end()) })
        });
        if let Some(r) = range {
            n += ops::create_fades(s, *t, r, FadeShape::EqualPower);
        }
    }
    Ok(json!({"fades": n}))
}
