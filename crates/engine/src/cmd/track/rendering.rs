//! Committing tracks and creating click material.

use super::super::*;
use serde_json::json;
use soundcraft_model::ChannelFormat;
use soundcraft_model::TrackKind;

pub(super) fn commit(e: &mut Engine, p: &Value) -> Result<Value> {
    let tracks = tracks_required(e, "track.commit", p)?;
    let mut out = Vec::new();
    for t in tracks {
        if let Some(id) = crate::io::commit_track(e, t)? {
            out.push(id);
        }
    }
    Ok(json!({"tracks": out}))
}

pub(super) fn create_click(e: &mut Engine, _: &Value) -> Result<Value> {
    let s = e.session_mut();
    let id = s.add_track(TrackKind::Aux, ChannelFormat::Mono, Some("Click"));
    if let Some(t) = s.track_mut(id) {
        t.comments = "click".into();
    }
    s.edit.click = true;
    Ok(json!({"track": id}))
}
