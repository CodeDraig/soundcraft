//! Automation lane rendering and parameter ranges.

use super::{sample_at, x_of};
use crate::theme::Tokens;
use crate::theme::regular;
use egui::Align2;
use egui::Color32;
use egui::Pos2;
use egui::Rect;
use egui::Shape;
use egui::Stroke;
use egui::pos2;
use soundcraft_model::AutoParam;
use soundcraft_model::Session;
use soundcraft_model::Track;

pub(super) fn draw_automation(painter: &egui::Painter, s: &Session, track: &Track, p: &AutoParam, tl: Rect, lane: Rect) {
    let t = Tokens::DARK;
    let (lo, hi, _) = p.range();
    let (lo, hi) = if matches!(p, AutoParam::Volume | AutoParam::SendLevel(_)) {
        (-60.0f32, 12.0f32)
    } else if matches!(p, AutoParam::Plugin { .. }) {
        plugin_range(track, p)
    } else {
        (lo, hi)
    };
    let def = match p {
        AutoParam::Volume => track.mixer.volume_db,
        AutoParam::Pan(i) => track.mixer.pan.get(usize::from(*i)).copied().unwrap_or(0.0),
        _ => 0.0,
    };
    let yv = |v: f32| -> f32 {
        let k = ((v.clamp(lo, hi) - lo) / (hi - lo).max(1e-6)).clamp(0.0, 1.0);
        lane.max.y - 4.0 - k * (lane.height() - 8.0)
    };
    let lanes = track.lane(p);
    let pts: Vec<(f32, f32)> = match lanes {
        Some(l) if !l.points.is_empty() => {
            let mut v: Vec<(f32, f32)> = Vec::new();
            v.push((lane.min.x, yv(l.value_at(sample_at(s, tl, lane.min.x), def))));
            for pt in &l.points {
                let x = x_of(s, tl, pt.at);
                if x > lane.min.x && x < lane.max.x {
                    v.push((x, yv(pt.value)));
                }
            }
            v.push((lane.max.x, yv(l.value_at(sample_at(s, tl, lane.max.x), def))));
            v
        }
        _ => vec![(lane.min.x, yv(def)), (lane.max.x, yv(def))],
    };
    let line: Vec<Pos2> = pts.iter().map(|(x, y)| pos2(*x, *y)).collect();
    painter.add(Shape::line(line, Stroke::new(1.5, t.automation_line)));
    if let Some(l) = lanes {
        for pt in &l.points {
            let x = x_of(s, tl, pt.at);
            if x > lane.min.x - 4.0 && x < lane.max.x + 4.0 {
                painter.circle(pos2(x, yv(pt.value)), 3.0, Color32::from_rgb(40, 40, 40), Stroke::new(1.5, t.automation_line));
            }
        }
    }
    painter.text(pos2(lane.min.x + 6.0, lane.min.y + 9.0), Align2::LEFT_CENTER, p.label(), regular(10.0), t.text_dim);
}

pub(super) fn plugin_range(track: &Track, p: &AutoParam) -> (f32, f32) {
    if let AutoParam::Plugin { slot, param } = p
        && let Some(Some(ins)) = track.mixer.inserts.get(usize::from(*slot))
        && let Some(info) = soundcraft_dsp::plugin_info(&ins.plugin)
        && let Some(pi) = info.param(param)
    {
        return (pi.min, pi.max);
    }
    (0.0, 1.0)
}
