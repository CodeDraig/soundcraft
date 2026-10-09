//! Real-time MIDI properties and committed note changes.

use super::super::more_util::track_key;
use super::super::*;
use serde_json::json;
use soundcraft_model::ClipContent;
use soundcraft_model::Session;
use soundcraft_model::Track;
use soundcraft_model::TrackId;

/// Real-time properties for a track (stored by Event › MIDI Real-Time Properties).
pub struct Rtp {
    pub velocity: i64,
    pub transpose: i64,
    pub duration_pct: f64,
    pub delay_ticks: i64,
}

pub fn rtp_of(s: &Session, t: TrackId) -> Rtp {
    let v = |name: &str, d: f64| s.edit.value(&track_key("rtp", t, name), d);
    Rtp {
        velocity: v("velocity", 0.0).clamp(-127.0, 127.0) as i64,
        transpose: v("transpose", 0.0).clamp(-127.0, 127.0) as i64,
        duration_pct: v("duration", 100.0).clamp(1.0, 1000.0),
        delay_ticks: v("delay", 0.0).clamp(-1e7, 1e7) as i64,
    }
}

pub(super) fn write_rtp(e: &mut Engine, p: &Value) -> Result<Value> {
    let tracks = tracks_required(e, "track.write_midi_rtp", p)?;
    let s = e.session_mut();
    let mut notes = 0;
    for t in &tracks {
        let r = rtp_of(s, *t);
        let Some(pl) = s.track_mut(*t).and_then(Track::playlist_mut) else { continue };
        for c in &mut pl.clips {
            let ClipContent::Midi { sequence } = &mut c.content else { continue };
            for n in &mut sequence.notes {
                n.velocity = (i64::from(n.velocity) + r.velocity).clamp(1, 127) as u8;
                n.pitch = (i64::from(n.pitch) + r.transpose).clamp(0, 127) as u8;
                n.length = ((n.length as f64 * r.duration_pct / 100.0).round() as i64).max(1);
                n.start = n.start.saturating_add(r.delay_ticks).max(0);
                notes += 1;
            }
            sequence.sort();
        }
        let pre = format!("rtp.{}.", t.0);
        s.edit.values.retain(|k, _| !k.starts_with(&pre));
    }
    Ok(json!({"notes": notes}))
}
