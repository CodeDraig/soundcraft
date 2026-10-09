//! Merging marker clipboard entries without duplicates.

use super::super::*;
use serde_json::json;

pub(super) fn has_marker_clipboard(e: &Engine) -> std::result::Result<(), String> {
    if e.clipboard.markers.is_empty() { Err("no markers on the clipboard".into()) } else { Ok(()) }
}

pub(super) fn paste_merge_markers(e: &mut Engine, p: &Value) -> Result<Value> {
    let id = "edit.paste_merge_markers";
    if e.clipboard.markers.is_empty() {
        return Err(bad(id, "no markers on the clipboard (copy a range that contains markers)"));
    }
    let at = position_param(e, id, p, "at")?.unwrap_or(e.session().edit.selection.start).max(0);
    let copied = e.clipboard.markers.clone();
    let s = e.session_mut();
    let mut added = 0;
    for m in copied {
        let start = at.saturating_add(m.start);
        if s.markers.iter().any(|x| x.start == start && x.name == m.name && x.ruler == m.ruler) {
            continue;
        }
        let new = s.add_marker(&m.name, m.kind, start, start.saturating_add(m.end - m.start));
        if let Some(x) = s.markers.iter_mut().find(|x| x.id == new) {
            x.comments = m.comments;
            x.color = m.color;
            x.ruler = m.ruler;
        }
        added += 1;
    }
    Ok(json!({"added": added}))
}
