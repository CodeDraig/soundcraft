//! Target and recent playlist bookkeeping.

use soundcraft_model::Session;
use soundcraft_model::TrackId;

/// The track's designated target playlist index, if valid.
pub fn target_playlist(s: &Session, t: TrackId) -> Option<usize> {
    let v = s.edit.values.get(&format!("playlist.target.{}", t.0)).copied()?;
    if !v.is_finite() || v < 0.0 {
        return None;
    }
    let i = v as usize;
    s.track(t).filter(|tr| i < tr.playlists.len()).map(|_| i)
}

/// Make `idx` the active playlist, remembering the previous one for Toggle Recent Playlist.
pub fn activate_playlist(s: &mut Session, t: TrackId, idx: usize) -> bool {
    let Some(tr) = s.track_mut(t) else { return false };
    if idx >= tr.playlists.len() || idx == tr.active_playlist {
        return false;
    }
    let prev = tr.active_playlist;
    tr.active_playlist = idx;
    s.edit.values.insert(format!("playlist.recent.{}", t.0), prev as f64);
    true
}
