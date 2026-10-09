//! Surround panner drawing and gesture response.

use crate::theme::Tokens;
use crate::theme::regular;
use egui::Align2;
use egui::Color32;
use egui::CornerRadius;
use egui::Rect;
use egui::Sense;
use egui::Stroke;
use egui::StrokeKind;
use egui::Ui;
use egui::pos2;
use egui::vec2;

/// A speaker dot on the surround panner: label, square position (-1..1, y up = front), height layer.
pub struct PannerSpeaker {
    pub label: &'static str,
    pub x: f32,
    pub y: f32,
    pub height: bool,
}

/// What the user did on a [`surround_panner`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PannerEdit {
    /// The puck moved (x, y).
    Position(f32, f32),
    /// Divergence changed (Alt-drag on the square, or the slider).
    Divergence(f32),
    /// Elevation changed (height formats).
    Height(f32),
}

/// Pro Tools-style X/Y surround panner: a square room with speaker dots and a draggable puck.
/// Drag moves the puck; double-click returns it to front centre; Alt-drag (or the slider below)
/// sets divergence; height layouts get a second slider for elevation. Returns the edit, if any.
pub fn surround_panner(ui: &mut Ui, size: f32, speakers: &[PannerSpeaker], puck: (f32, f32), divergence: f32, z: Option<f32>) -> Option<PannerEdit> {
    let t = Tokens::DARK;
    let mut edit = None;
    let (r, resp) = ui.allocate_exact_size(vec2(size, size), Sense::click_and_drag());
    let half = size * 0.5 - 6.0;
    let c = r.center();
    let to_screen = |x: f32, y: f32| pos2(c.x + x * half, c.y - y * half);
    let painter = ui.painter_at(r.expand(1.0));
    painter.rect(r, CornerRadius::same(2), Color32::from_rgb(14, 18, 22), Stroke::new(1.0, Color32::from_rgb(70, 74, 80)), StrokeKind::Inside);
    let grid = Stroke::new(1.0, Color32::from_rgb(38, 44, 50));
    painter.line_segment([to_screen(-1.0, 0.0), to_screen(1.0, 0.0)], grid);
    painter.line_segment([to_screen(0.0, -1.0), to_screen(0.0, 1.0)], grid);
    painter.rect_stroke(Rect::from_two_pos(to_screen(-1.0, -1.0), to_screen(1.0, 1.0)), 0.0, grid, StrokeKind::Middle);
    // "Front" marker.
    painter.line_segment([pos2(c.x - 5.0, r.min.y + 2.0), pos2(c.x + 5.0, r.min.y + 2.0)], Stroke::new(2.0, Color32::from_rgb(90, 110, 130)));
    for s in speakers {
        let (x, y) = if s.height { (s.x * 0.55, s.y * 0.55) } else { (s.x, s.y) };
        let p = to_screen(x, y);
        if s.height {
            painter.circle_stroke(p, 2.5, Stroke::new(1.0, Color32::from_rgb(150, 150, 170)));
        } else {
            painter.circle_filled(p, 3.0, Color32::from_rgb(170, 170, 176));
        }
        if size >= 70.0 && !s.height {
            let inward = vec2(-(x.signum() * if x.abs() > 0.5 { 1.0 } else { 0.0 }), if y.abs() > 0.5 { y.signum() } else { 0.0 });
            let lp = p + vec2(inward.x * 9.0, inward.y * 7.0);
            painter.text(lp, Align2::CENTER_CENTER, s.label, regular(7.5), t.text_dim);
        }
    }
    let alt = ui.input(|i| i.modifiers.alt);
    if resp.double_clicked() {
        edit = Some(PannerEdit::Position(0.0, 1.0));
    } else if resp.dragged() && alt {
        let d = (divergence - resp.drag_delta().y * 0.01).clamp(0.0, 1.0);
        edit = Some(PannerEdit::Divergence(d));
    } else if (resp.dragged() || resp.clicked())
        && !alt
        && let Some(p) = resp.interact_pointer_pos()
    {
        let x = ((p.x - c.x) / half).clamp(-1.0, 1.0);
        let y = ((c.y - p.y) / half).clamp(-1.0, 1.0);
        edit = Some(PannerEdit::Position(x, y));
    }
    let (px, py) = match edit {
        Some(PannerEdit::Position(x, y)) => (x, y),
        _ => puck,
    };
    let pp = to_screen(px, py);
    let div = match edit {
        Some(PannerEdit::Divergence(d)) => d,
        _ => divergence,
    };
    if div > 0.01 {
        painter.circle(pp, 5.0 + div * half, Color32::from_rgba_unmultiplied(80, 160, 230, 28), Stroke::new(1.0, Color32::from_rgb(70, 130, 190)));
    }
    painter.circle(pp, 5.0, t.counter_text, Stroke::new(1.0, Color32::BLACK));
    let tip = "Surround pan: drag the puck · double-click: front centre · Alt-drag: divergence";
    let _ = resp.on_hover_text(tip);
    // Sliders below: divergence, then elevation for height formats.
    let mut sliders: Vec<(&str, f32, u8)> = vec![("div", div, 0)];
    if let Some(z) = z {
        sliders.push(("z", z, 1));
    }
    for (label, v, kind) in sliders {
        let (sr, sresp) = ui.allocate_exact_size(vec2(size, 11.0), Sense::click_and_drag());
        let track = Rect::from_min_max(pos2(sr.min.x + 16.0, sr.center().y - 2.0), pos2(sr.max.x - 2.0, sr.center().y + 2.0));
        ui.painter().text(pos2(sr.min.x + 1.0, sr.center().y), Align2::LEFT_CENTER, label, regular(8.0), t.text_dim);
        ui.painter().rect_filled(track, 1.0, Color32::from_rgb(24, 26, 30));
        ui.painter().rect_filled(
            Rect::from_min_max(track.min, pos2(track.min.x + track.width() * v.clamp(0.0, 1.0), track.max.y)),
            1.0,
            t.counter_text,
        );
        if (sresp.dragged() || sresp.clicked())
            && let Some(p) = sresp.interact_pointer_pos()
        {
            let nv = ((p.x - track.min.x) / track.width().max(1.0)).clamp(0.0, 1.0);
            edit = Some(if kind == 0 { PannerEdit::Divergence(nv) } else { PannerEdit::Height(nv) });
        }
        if sresp.double_clicked() {
            edit = Some(if kind == 0 { PannerEdit::Divergence(0.0) } else { PannerEdit::Height(0.0) });
        }
        let _ = sresp.on_hover_text(if kind == 0 { "Divergence (0 = point source)" } else { "Elevation (height speakers)" });
    }
    edit
}
