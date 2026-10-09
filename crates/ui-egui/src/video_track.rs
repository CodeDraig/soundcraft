//! Video track: the movie pool (opened from `Session::videos` paths, decoded off the UI thread)
//! and thumbnails drawn along video clips in the Edit window.
//!
//! Decoding runs on one background thread on native targets: the playhead frame request always
//! replaces the previous one (latest wins), thumbnails queue behind it. On the web there are no
//! threads, so the same worker runs inline on the UI thread with a per-frame budget.

use crate::SoundApp;
use crate::edit_window::{sample_at, x_of};
use crate::theme::regular;
use egui::{Color32, Painter, Rect, pos2};
use soundcraft_model::{ClipContent, Session, Track};
use soundcraft_time::Samples;
use soundcraft_video::MovieInfo;

mod pool;
pub use pool::{MovieKey, VideoPool};

/// `edit.flags` key set while the Video track is offline (Options › Video Track Online).
pub const OFFLINE_FLAG: &str = "video.offline";

/// True unless Options › Video Track Online is off.
pub fn online(s: &Session) -> bool {
    !s.edit.flag(OFFLINE_FLAG)
}

/// Movie time (seconds) shown at timeline position `at` by a video clip, including the session's
/// Video Sync Offset (positive delays the picture).
pub fn movie_secs(s: &Session, clip_start: Samples, offset: Samples, at: Samples) -> f64 {
    let sync = s.edit.value("video.sync_offset", 0.0);
    let sync = if sync.is_finite() { sync } else { 0.0 };
    (at.saturating_sub(clip_start).saturating_add(offset) as f64 - sync) / s.sample_rate.as_f64().max(1.0)
}

/// The picture at `at`: the topmost active Video track with an unmuted clip there.
pub fn picture_at(s: &Session, at: Samples) -> Option<(MovieKey, String, f64, f64)> {
    for t in s.tracks.iter().filter(|t| t.kind == soundcraft_model::TrackKind::Video && !t.inactive) {
        for c in t.clips() {
            if c.muted || !c.range().contains(at) {
                continue;
            }
            if let ClipContent::Video { source, offset } = c.content
                && let Some(v) = s.video(source)
            {
                return Some((MovieKey::new(source, &v.path), v.path.clone(), movie_secs(s, c.start, offset, at), v.frame_rate));
            }
        }
    }
    None
}

/// Draw thumbnails inside the video clips of `track` (called from the Edit window's track row,
/// after the clip blocks are painted).
pub fn draw_lane(app: &mut SoundApp, painter: &Painter, s: &Session, track: &Track, tl: Rect, lane: Rect) {
    if !track.clips().iter().any(|c| c.is_video()) {
        return;
    }
    let ctx = painter.ctx().clone();
    app.video.poll(&ctx);
    let on = online(s);
    let name_h = if lane.height() >= 40.0 && s.edit.flag("view.clip.name") { 13.0 } else { 0.0 };
    let ppp = ctx.pixels_per_point();
    for clip in track.clips() {
        let ClipContent::Video { source, offset } = clip.content else { continue };
        let x0 = x_of(s, tl, clip.start);
        let x1 = x_of(s, tl, clip.end()).max(x0 + 1.0);
        let body = Rect::from_min_max(pos2(x0 + 1.0, lane.min.y + 2.0 + name_h), pos2(x1 - 1.0, lane.max.y - 2.0));
        let vis = body.intersect(lane);
        if vis.width() < 2.0 || vis.height() < 6.0 {
            continue;
        }
        let p = painter.with_clip_rect(vis);
        let Some(v) = s.video(source) else {
            p.text(vis.center(), egui::Align2::CENTER_CENTER, "movie missing from session", regular(10.0), Color32::from_rgb(220, 120, 120));
            continue;
        };
        if !on {
            p.text(vis.center(), egui::Align2::CENTER_CENTER, "Video Track Offline", regular(10.0), Color32::from_gray(170));
            continue;
        }
        let key = MovieKey::new(source, &v.path);
        if let Some(Err(e)) = app.video.info(key) {
            p.text(vis.center(), egui::Align2::CENTER_CENTER, format!("media offline: {e}"), regular(10.0), Color32::from_rgb(220, 120, 120));
            continue;
        }
        let aspect = app.video.info(key).and_then(|i| i.as_ref().ok()).map_or(v.aspect(), MovieInfo::display_aspect) as f32;
        let th = body.height();
        let tw = (th * aspect).clamp(8.0, 600.0);
        // Pixel height bucket (multiples of 16) so zooming the track does not re-decode constantly.
        let hpx = (((th * ppp) / 16.0).ceil() * 16.0).clamp(16.0, 256.0) as u32;
        let first = ((vis.min.x - body.min.x) / tw).floor().max(0.0) as i64;
        let mut k = first;
        loop {
            let tx = body.min.x + k as f32 * tw;
            if tx > vis.max.x || k > first + 400 {
                break;
            }
            let at = sample_at(s, tl, tx);
            let secs = movie_secs(s, clip.start, offset, at.max(clip.start));
            let r = Rect::from_min_max(pos2(tx, body.min.y), pos2((tx + tw).min(body.max.x), body.max.y));
            match app.video.thumb(&ctx, key, &v.path, secs.max(0.0), hpx) {
                Some(tex) => {
                    let full = Rect::from_min_size(r.min, egui::vec2(tw, th));
                    let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2((r.width() / tw).clamp(0.0, 1.0), 1.0));
                    p.image(tex.id(), Rect::from_min_max(full.min, pos2(r.max.x, full.max.y)), uv, Color32::WHITE);
                }
                None => {
                    p.rect_filled(r.shrink(1.0), 0.0, Color32::from_rgb(24, 24, 26));
                }
            }
            p.line_segment([pos2(tx, body.min.y), pos2(tx, body.max.y)], egui::Stroke::new(1.0, Color32::from_black_alpha(160)));
            k += 1;
        }
    }
}
