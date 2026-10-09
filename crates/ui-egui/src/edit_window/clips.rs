//! Clip drawing, waveforms, fades and edit grid.

use super::{sample_at, x_of};
use crate::SoundApp;
use crate::theme::Tokens;
use crate::theme::bold;
use crate::theme::clip_colors;
use crate::theme::regular;
use egui::Align2;
use egui::Color32;
use egui::CornerRadius;
use egui::Pos2;
use egui::Rect;
use egui::Shape;
use egui::Stroke;
use egui::StrokeKind;
use egui::pos2;
use soundcraft_model::Clip;
use soundcraft_model::ClipContent;
use soundcraft_model::Session;
use soundcraft_model::Track;
use soundcraft_time::GridValue;
use soundcraft_time::NoteValue;
use soundcraft_time::Range;
use soundcraft_time::TimeFormat;
use soundcraft_time::format_position;

pub(super) fn grid_lines(painter: &egui::Painter, s: &Session, tl: Rect, lane: Rect) {
    let t = Tokens::DARK;
    let view = Range::new(sample_at(s, tl, tl.min.x).max(0), sample_at(s, tl, tl.max.x));
    let sr = s.sample_rate;
    // Bar lines always; grid lines when enabled and dense enough.
    let bar = GridValue::Note { value: NoteValue::Bar, dotted: false, triplet: false };
    let bar_px = bar.step_samples(view.start, sr, &s.tempo, s.frame_rate) as f64 / s.edit.zoom.samples_per_px;
    if bar_px > 6.0 {
        for at in bar.lines(view.start, view.end, sr, &s.tempo, s.frame_rate, 600) {
            let x = x_of(s, tl, at);
            painter.line_segment([pos2(x, lane.min.y), pos2(x, lane.max.y)], Stroke::new(1.0, t.bar_line));
        }
    }
    if s.edit.grid_lines {
        let step_px = s.edit.grid.step_samples(view.start, sr, &s.tempo, s.frame_rate) as f64 / s.edit.zoom.samples_per_px;
        if step_px > 7.0 {
            for at in s.edit.grid.lines(view.start, view.end, sr, &s.tempo, s.frame_rate, 2000) {
                let x = x_of(s, tl, at);
                painter.line_segment([pos2(x, lane.min.y), pos2(x, lane.max.y)], Stroke::new(1.0, t.grid_line));
            }
        }
    }
}

pub(super) fn draw_clip(app: &SoundApp, painter: &egui::Painter, s: &Session, track: &Track, clip: &Clip, tl: Rect, lane: Rect, dim: bool) {
    let x0 = x_of(s, tl, clip.start);
    let x1 = x_of(s, tl, clip.end()).max(x0 + 1.0);
    let r = Rect::from_min_max(pos2(x0, lane.min.y + 1.0), pos2(x1, lane.max.y - 1.0));
    let selected = s.edit.selected_clips.contains(&clip.id)
        || (s.edit.selected_tracks.contains(&track.id)
            && !s.edit.selection.is_empty()
            && s.edit.selection.start <= clip.start
            && s.edit.selection.end >= clip.end());
    let color = clip.color.unwrap_or(track.color);
    let (mut body, bar, mut wave) = clip_colors(color, selected);
    if clip.muted || dim {
        body = body.gamma_multiply(0.45);
        wave = Color32::from_rgb(70, 70, 70);
    }
    let body = if s.edit.flag("view.clip.transparency") { body.gamma_multiply(0.55) } else { body };
    painter.rect(r, CornerRadius::same(2), body, Stroke::new(1.0, Color32::from_rgb(10, 10, 10)), StrokeKind::Inside);
    let name_h = if lane.height() >= 40.0 && s.edit.flag("view.clip.name") { 13.0 } else { 0.0 };
    if name_h > 0.0 {
        let nb = Rect::from_min_max(r.min, pos2(r.max.x, r.min.y + name_h));
        painter.rect_filled(nb, CornerRadius { nw: 2, ne: 2, sw: 0, se: 0 }, if clip.muted { Color32::from_rgb(70, 70, 70) } else { bar });
        let name_clip = nb.intersect(lane);
        painter.with_clip_rect(name_clip).text(
            pos2(nb.min.x.max(lane.min.x) + 4.0, nb.center().y),
            Align2::LEFT_CENTER,
            &clip.name,
            bold(10.5),
            Color32::from_rgb(12, 12, 12),
        );
    }
    let body_r = Rect::from_min_max(pos2(r.min.x, r.min.y + name_h), r.max);
    let vis = body_r.intersect(lane);
    match &clip.content {
        ClipContent::Audio { source, offset } => {
            if let Some(audio) = s.pool.get(*source) {
                let nch = track.channels().min(audio.peaks.len().max(1)).max(1);
                let ch_h = body_r.height() / nch as f32;
                let zoom = s.edit.zoom.waveform_zoom;
                let rectified = s.edit.flag("waveform.rectified");
                let power = s.edit.flag("waveform.power");
                let outlines = s.edit.flag("waveform.outlines");
                let cols = vis.width().max(0.0) as usize;
                if cols > 0 && vis.height() > 2.0 {
                    let a = (offset + sample_at(s, tl, vis.min.x) - clip.start) as f64;
                    let b = (offset + sample_at(s, tl, vis.max.x) - clip.start) as f64;
                    for ch in 0..nch {
                        let Some(pk) = audio.peaks.get(ch % audio.peaks.len().max(1)) else { continue };
                        let cy = body_r.min.y + ch_h * (ch as f32 + 0.5);
                        let half = ch_h * 0.46;
                        let colsv = pk.columns(a, b, cols);
                        let mut mesh = egui::Mesh::default();
                        for (i, (mn, mx)) in colsv.iter().enumerate() {
                            let x = vis.min.x + i as f32;
                            let rel = sample_at(s, tl, x) - clip.start;
                            let g = clip.gain_at(rel.clamp(0, clip.length));
                            let (mut top, mut bot) = if rectified {
                                let a = (mx.abs().max(mn.abs()) * g * zoom).clamp(0.0, 1.0);
                                let b = cy + half;
                                (b - a * half * 2.0, b)
                            } else {
                                (cy - (mx * g * zoom).clamp(-1.0, 1.0) * half, cy - (mn * g * zoom).clamp(-1.0, 1.0) * half)
                            };
                            if power {
                                // Power view: compress peaks logarithmically around the centre.
                                let k = |y: f32| cy + (y - cy).signum() * ((y - cy).abs() / half).sqrt() * half;
                                top = k(top);
                                bot = k(bot);
                            }
                            if outlines {
                                bot = top + 1.0;
                            }
                            let bot = if (bot - top) < 1.0 { top + 1.0 } else { bot };
                            let idx = mesh.vertices.len() as u32;
                            mesh.colored_vertex(pos2(x, top), wave);
                            mesh.colored_vertex(pos2(x + 1.0, top), wave);
                            mesh.colored_vertex(pos2(x + 1.0, bot), wave);
                            mesh.colored_vertex(pos2(x, bot), wave);
                            mesh.add_triangle(idx, idx + 1, idx + 2);
                            mesh.add_triangle(idx, idx + 2, idx + 3);
                        }
                        painter.with_clip_rect(vis).add(Shape::mesh(mesh));
                        if nch > 1 && ch + 1 < nch {
                            let y = body_r.min.y + ch_h * (ch as f32 + 1.0);
                            painter.line_segment([pos2(vis.min.x, y), pos2(vis.max.x, y)], Stroke::new(1.0, body.gamma_multiply(0.7)));
                        }
                    }
                }
            } else {
                painter.text(body_r.center(), Align2::CENTER_CENTER, "media offline", regular(10.0), Color32::from_rgb(220, 120, 120));
            }
        }
        ClipContent::Video { .. } => {} // thumbnails: video_track::draw_lane
        ClipContent::Midi { sequence } => {
            let (lo, hi) = sequence.notes.iter().fold((127u8, 0u8), |(lo, hi), n| (lo.min(n.pitch), hi.max(n.pitch)));
            let (lo, hi) = if lo > hi { (48, 72) } else { (lo.saturating_sub(2), hi.saturating_add(2)) };
            let span = f32::from(hi.saturating_sub(lo).max(1));
            let base = s.tempo.samples_to_ticks(clip.start, s.sample_rate);
            let p = painter.with_clip_rect(vis);
            for n in &sequence.notes {
                let a = s.tempo.tick_to_samples(base + n.start, s.sample_rate);
                let b = s.tempo.tick_to_samples(base + n.start + n.length, s.sample_rate).min(clip.end());
                let nx0 = x_of(s, tl, a);
                let nx1 = x_of(s, tl, b).max(nx0 + 2.0);
                if nx1 < vis.min.x || nx0 > vis.max.x {
                    continue;
                }
                let y = body_r.max.y - 3.0 - (f32::from(n.pitch.saturating_sub(lo)) / span) * (body_r.height() - 6.0);
                let nh = ((body_r.height() - 6.0) / span).clamp(1.5, 6.0);
                let vcol = Color32::from_rgb(20, 20 + n.velocity, 40);
                p.rect_filled(
                    Rect::from_min_max(pos2(nx0, y - nh * 0.5), pos2(nx1, y + nh * 0.5)),
                    1.0,
                    if selected { Color32::from_rgb(16, 16, 16) } else { vcol },
                );
            }
        }
    }
    // Clip gain line.
    let show_gain = s.edit.flag("view.clip.gain_line") && (!clip.gain_env.is_empty() || clip.gain_db.abs() > 0.01 || track.view == "clip_gain");
    if show_gain && body_r.height() > 12.0 {
        let gy = |db: f32| body_r.max.y - 3.0 - ((db.clamp(-36.0, 36.0) + 36.0) / 72.0) * (body_r.height() - 6.0);
        let col = Color32::from_rgba_unmultiplied(255, 255, 255, 170);
        if clip.gain_env.is_empty() {
            painter.line_segment([pos2(x0, gy(clip.gain_db)), pos2(x1, gy(clip.gain_db))], Stroke::new(1.0, col));
        } else {
            let mut pts = vec![pos2(x0, gy(clip.gain_db + clip.gain_env.first().map_or(0.0, |p| p.1)))];
            for (o, db) in &clip.gain_env {
                let p = pos2(x_of(s, tl, clip.start + o), gy(clip.gain_db + db));
                pts.push(p);
                painter.circle_filled(p, 2.5, col);
            }
            pts.push(pos2(x1, gy(clip.gain_db + clip.gain_env.last().map_or(0.0, |p| p.1))));
            painter.add(Shape::line(pts, Stroke::new(1.0, col)));
        }
        if s.edit.flag("view.clip.gain_info") || clip.gain_db.abs() > 0.01 {
            painter.text(pos2(r.max.x - 4.0, body_r.max.y - 8.0), Align2::RIGHT_CENTER, format!("{:+.1} dB", clip.gain_db), regular(9.5), col);
        }
    }
    // Fades.
    let fade_col = Color32::from_rgba_unmultiplied(255, 255, 255, 90);
    if clip.fade_in.len > 0 {
        let fx = x_of(s, tl, clip.start + clip.fade_in.len);
        let pts: Vec<Pos2> = (0..=16)
            .map(|i| {
                let k = i as f32 / 16.0;
                pos2(x0 + (fx - x0) * k, body_r.max.y - body_r.height() * clip.fade_in.shape.gain(k))
            })
            .collect();
        painter.add(Shape::line(pts, Stroke::new(1.2, fade_col)));
        painter.line_segment([pos2(x0, body_r.min.y), pos2(fx, body_r.min.y)], Stroke::new(1.0, fade_col));
    }
    if clip.fade_out.len > 0 {
        let fx = x_of(s, tl, clip.end() - clip.fade_out.len);
        let pts: Vec<Pos2> = (0..=16)
            .map(|i| {
                let k = i as f32 / 16.0;
                pos2(fx + (x1 - fx) * k, body_r.max.y - body_r.height() * clip.fade_out.shape.gain(1.0 - k))
            })
            .collect();
        painter.add(Shape::line(pts, Stroke::new(1.2, fade_col)));
    }
    // Optional clip overlays (View › Clip).
    let ov = Color32::from_rgb(16, 16, 16);
    if s.edit.flag("view.clip.rating") && clip.rating > 0 && name_h > 0.0 {
        painter.text(pos2(r.max.x - 18.0, r.min.y + 7.0), Align2::RIGHT_CENTER, "★".repeat(usize::from(clip.rating.min(5))), regular(9.0), ov);
    }
    if s.edit.flag("view.clip.sync_point") && clip.sync_point > 0 {
        let x = x_of(s, tl, clip.start + clip.sync_point);
        painter.add(Shape::convex_polygon(
            vec![pos2(x - 4.0, r.max.y - 1.0), pos2(x + 4.0, r.max.y - 1.0), pos2(x, r.max.y - 7.0)],
            Color32::WHITE,
            Stroke::NONE,
        ));
        painter.line_segment([pos2(x, r.min.y + name_h), pos2(x, r.max.y)], Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 90)));
    }
    let time_text = if s.edit.flag("view.clip.time.current") {
        Some(format_position(clip.start, s.edit.main_counter, s.sample_rate, &s.tempo, s.frame_rate, s.timecode_start))
    } else if s.edit.flag("view.clip.time.original") || s.edit.flag("view.clip.time.user") {
        clip.source().and_then(|src| s.source(src)).map(|src| {
            format_position(
                i64::try_from(src.time_reference).unwrap_or(0) + clip.source_offset(),
                TimeFormat::Timecode,
                s.sample_rate,
                &s.tempo,
                s.frame_rate,
                0,
            )
        })
    } else {
        None
    };
    if let Some(tt) = time_text
        && r.height() > 30.0
    {
        painter.with_clip_rect(r.intersect(lane)).text(pos2(r.min.x.max(lane.min.x) + 4.0, r.max.y - 7.0), Align2::LEFT_CENTER, tt, regular(9.0), ov);
    }
    if s.edit.flag("view.clip.effects_status") && s.edit.values.keys().any(|k| k.starts_with(&format!("clip_fx.{}.", clip.id.0))) {
        painter.text(pos2(r.max.x - 6.0, r.max.y - 7.0), Align2::RIGHT_CENTER, "FX", bold(9.0), Color32::from_rgb(250, 230, 120));
    }
    if s.edit.flag("view.clip.overlap_shadows") {
        // Shade where a later clip covers this one (crossfades and layered edits).
        for o in track.clips() {
            if o.id != clip.id && o.start > clip.start && o.start < clip.end() {
                let x0 = x_of(s, tl, o.start);
                let x1 = x_of(s, tl, clip.end().min(o.end()));
                painter.rect_filled(Rect::from_min_max(pos2(x0, r.min.y), pos2(x1, r.max.y)), 0.0, Color32::from_black_alpha(70));
            }
        }
    }
    if clip.edit_locked || clip.time_locked {
        painter.text(pos2(r.max.x - 10.0, r.min.y + 7.0), Align2::CENTER_CENTER, "🔒", regular(9.0), Color32::WHITE);
    }
    let _ = app;
}
