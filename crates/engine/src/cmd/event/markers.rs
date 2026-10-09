//! Memory-location creation, editing and navigation.

use super::super::*;
use serde_json::json;
use soundcraft_model::MarkerKind;

pub(super) fn add_marker(e: &mut Engine, p: &Value) -> Result<Value> {
    let sel = e.session().edit.selection;
    let at = position_param(e, "markers.add", p, "at")?;
    let st = position_param(e, "markers.add", p, "start")?;
    let en = position_param(e, "markers.add", p, "end")?;
    let kind = match str_param(p, "kind") {
        Some("selection") => MarkerKind::Selection,
        Some("none") => MarkerKind::None,
        _ => MarkerKind::Marker,
    };
    let (a, b) = match (at, st, en) {
        (Some(a), _, _) => (a, a),
        (None, Some(a), Some(b)) => (a, b),
        _ if kind == MarkerKind::Selection => (sel.start, sel.end),
        _ => (if e.transport.playing { e.transport.position } else { sel.start }, sel.start),
    };
    let s = e.session_mut();
    let n = s.markers.len() + 1;
    let name = str_param(p, "name").map_or_else(|| format!("Marker {n}"), str::to_string);
    let id = s.add_marker(&name, kind, a.max(0), b.max(a).max(0));
    if let Some(m) = s.markers.iter_mut().find(|m| m.id == id) {
        m.ruler = i64_or(p, "ruler", 1).clamp(1, 5) as u8;
    }
    let number = s.marker(id).map_or(0, |m| m.number);
    Ok(json!({"id": id, "number": number}))
}

fn marker_index(e: &Engine, p: &Value, cmd: &str) -> Result<usize> {
    let s = e.session();
    let idx = if let Some(n) = p.get("number").and_then(Value::as_u64) {
        s.markers.iter().position(|m| u64::from(m.number) == n)
    } else if let Some(n) = p.get("id").and_then(Value::as_u64) {
        s.markers.iter().position(|m| m.id.0 == n)
    } else if let Some(name) = str_param(p, "name") {
        s.markers.iter().position(|m| m.name == name)
    } else {
        None
    };
    idx.ok_or_else(|| bad(cmd, "unknown memory location (give `number`, `id` or `name`)"))
}

pub(super) fn delete_marker(e: &mut Engine, p: &Value) -> Result<Value> {
    let i = marker_index(e, p, "markers.delete")?;
    e.session_mut().markers.remove(i);
    Ok(json!({}))
}

pub(super) fn edit_marker(e: &mut Engine, p: &Value) -> Result<Value> {
    let i = marker_index(e, p, "markers.edit")?;
    let at = position_param(e, "markers.edit", p, "at")?;
    let s = e.session_mut();
    let Some(m) = s.markers.get_mut(i) else { return Err(bad("markers.edit", "gone")) };
    if let Some(n) = p.get("rename").and_then(Value::as_str) {
        m.name = n.to_string();
    }
    if let Some(c) = str_param(p, "comments") {
        m.comments = c.to_string();
    }
    if let Some(a) = at {
        let len = m.end - m.start;
        m.start = a.max(0);
        m.end = m.start + len;
    }
    s.markers.sort_by_key(|m| m.start);
    Ok(json!({}))
}

pub(super) fn recall_marker(e: &mut Engine, p: &Value) -> Result<Value> {
    let i = marker_index(e, p, "markers.recall")?;
    let s = e.session();
    let Some(m) = s.markers.get(i).cloned() else { return Err(bad("markers.recall", "gone")) };
    let s = e.session_mut();
    match m.kind {
        MarkerKind::Marker => s.edit.selection = soundcraft_time::Range::point(m.start),
        MarkerKind::Selection => s.edit.selection = soundcraft_time::Range::new(m.start, m.end),
        MarkerKind::None => {}
    }
    s.edit.playhead = m.start;
    e.transport_requests.push(crate::TransportRequest::Locate(m.start));
    Ok(json!({"at": m.start}))
}

pub(super) fn step_marker(e: &mut Engine, dir: i64) -> Result<Value> {
    let cur = e.session().edit.selection.start;
    let s = e.session();
    let target = if dir > 0 {
        s.markers.iter().map(|m| m.start).filter(|t| *t > cur).min()
    } else {
        s.markers.iter().map(|m| m.start).filter(|t| *t < cur).max()
    };
    let Some(t) = target else { return Ok(json!({"moved": false})) };
    let s = e.session_mut();
    s.edit.selection = soundcraft_time::Range::point(t);
    s.edit.playhead = t;
    e.transport_requests.push(crate::TransportRequest::Locate(t));
    Ok(json!({"at": t}))
}
