//! Command enablement predicates.

use crate::Engine;

// ---- enablement --------------------------------------------------------------------------

pub fn always(_: &Engine) -> std::result::Result<(), String> {
    Ok(())
}

pub fn has_tracks(e: &Engine) -> std::result::Result<(), String> {
    if e.session().tracks.is_empty() { Err("the session has no tracks".into()) } else { Ok(()) }
}

pub fn has_selection(e: &Engine) -> std::result::Result<(), String> {
    let s = e.session();
    if s.edit.selected_tracks.is_empty() && s.edit.selected_clips.is_empty() { Err("nothing is selected".into()) } else { Ok(()) }
}

pub fn has_range(e: &Engine) -> std::result::Result<(), String> {
    let s = e.session();
    if (s.edit.selected_tracks.is_empty() || s.edit.selection.is_empty()) && s.edit.selected_clips.is_empty() {
        Err("make an edit selection first".into())
    } else {
        Ok(())
    }
}

pub fn can_undo(e: &Engine) -> std::result::Result<(), String> {
    if e.can_undo() { Ok(()) } else { Err("nothing to undo".into()) }
}

pub fn can_redo(e: &Engine) -> std::result::Result<(), String> {
    if e.can_redo() { Ok(()) } else { Err("nothing to redo".into()) }
}

pub fn has_clipboard(e: &Engine) -> std::result::Result<(), String> {
    if e.clipboard.tracks.is_empty() { Err("the clipboard is empty".into()) } else { Ok(()) }
}
