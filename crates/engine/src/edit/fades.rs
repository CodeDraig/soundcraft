//! Fade creation and removal at clip boundaries.

use soundcraft_model::Clip;
use soundcraft_model::ClipContent;
use soundcraft_model::ClipId;
use soundcraft_model::Fade;
use soundcraft_model::FadeShape;
use soundcraft_model::Session;
use soundcraft_model::TrackId;
use soundcraft_time::Range;
use soundcraft_time::Samples;

/// Create fades from a selection (the incumbent's Edit › Fades › Create semantics):
/// a range across a clip boundary makes a crossfade; at a clip start a fade-in; at its end a fade-out.
pub fn create_fades(s: &mut Session, track: TrackId, range: Range, shape: FadeShape) -> usize {
    if range.is_empty() {
        return 0;
    }
    let handles: Vec<(ClipId, Samples)> = {
        let Some(tr) = s.track(track) else { return 0 };
        tr.clips()
            .iter()
            .map(|c| {
                let avail = match c.content {
                    ClipContent::Audio { source, offset } => {
                        let total = s.source(source).map_or(0, |src| i64::try_from(src.frames).unwrap_or(i64::MAX));
                        total.saturating_sub(offset).saturating_sub(c.length)
                    }
                    ClipContent::Video { source, offset } => {
                        let total = s.video(source).map_or(0, |v| s.sample_rate.samples(v.duration.clamp(0.0, 1e7)));
                        total.saturating_sub(offset).saturating_sub(c.length)
                    }
                    ClipContent::Midi { .. } => 0,
                };
                (c.id, avail)
            })
            .collect()
    };
    let Some(pl) = s.track_mut(track).and_then(|t| t.playlist_mut()) else { return 0 };
    pl.sort();
    let mut made = 0;
    let mut xfaded: Vec<ClipId> = Vec::new();
    let n = pl.clips.len();
    // Crossfades between adjacent clips whose boundary is inside the range.
    for i in 0..n.saturating_sub(1) {
        let (Some(a), Some(b)) = (pl.clips.get(i).cloned(), pl.clips.get(i + 1).cloned()) else { continue };
        let boundary = a.end();
        if b.start == boundary && range.start < boundary && range.end > boundary {
            let before = boundary - range.start.max(a.start);
            let after = range.end.min(b.end()) - boundary;
            let a_handle = handles.iter().find(|h| h.0 == a.id).map_or(0, |h| h.1);
            let b_handle = b.source_offset();
            // Extend each clip into the other's territory using its handles.
            let ext_a = after.min(a_handle).max(0);
            let ext_b = before.min(b_handle).max(0);
            if let Some(ca) = pl.clips.get_mut(i) {
                ca.length += ext_a;
                ca.fade_out = Fade { len: (ext_a + before).min(ca.length), shape };
            }
            if let Some(cb) = pl.clips.get_mut(i + 1) {
                let old = cb.start;
                cb.trim_start_to(old - ext_b);
                cb.fade_in = Fade { len: (ext_b + after).min(cb.length), shape };
            }
            made += 1;
            xfaded.push(a.id);
            xfaded.push(b.id);
        }
    }
    for c in pl.clips.iter_mut().filter(|c| !xfaded.contains(&c.id)) {
        if range.start <= c.start && range.end > c.start && range.end < c.end() {
            c.fade_in = Fade { len: range.end - c.start, shape };
            made += 1;
        } else if range.end >= c.end() && range.start > c.start && range.start < c.end() {
            c.fade_out = Fade { len: c.end() - range.start, shape };
            made += 1;
        }
    }
    made
}

/// Remove fades on clips touching `range`.
pub fn delete_fades(s: &mut Session, track: TrackId, range: Range) -> usize {
    let Some(pl) = s.track_mut(track).and_then(|t| t.playlist_mut()) else { return 0 };
    let mut n = 0;
    for c in &mut pl.clips {
        if c.range().overlaps(&range) || (range.is_empty() && c.range().contains(range.start)) {
            if c.fade_in.len > 0 || c.fade_out.len > 0 {
                n += 1;
            }
            c.fade_in = Fade::default();
            c.fade_out = Fade::default();
        }
    }
    // Overlapping (crossfaded) neighbours are trimmed back to the midpoint.
    pl.sort();
    for i in 0..pl.clips.len().saturating_sub(1) {
        let (Some(a_end), Some(b_start)) = (pl.clips.get(i).map(Clip::end), pl.clips.get(i + 1).map(|c| c.start)) else { continue };
        if a_end > b_start {
            let mid = b_start + (a_end - b_start) / 2;
            if let Some(a) = pl.clips.get_mut(i) {
                a.trim_end_to(mid);
            }
            if let Some(b) = pl.clips.get_mut(i + 1) {
                b.trim_start_to(mid);
            }
        }
    }
    n
}
