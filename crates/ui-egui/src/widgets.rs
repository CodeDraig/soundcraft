//! Small custom widgets in the studio style.

use crate::icons;
use crate::theme::{Tokens, bold, regular};
use egui::{Align2, Color32, CornerRadius, Rect, Response, Sense, Stroke, StrokeKind, Ui, pos2, vec2};

mod pan;
pub use pan::{pan_knob, pan_text};
mod meter;
pub use meter::{meter, meter_pos, multi_meter};
mod fader;
pub use fader::{db_text, fader};
mod surround;
pub use surround::{PannerEdit, PannerSpeaker, surround_panner};

/// A toolbar button with an icon; `on` draws the blue selected state.
pub fn icon_button(ui: &mut Ui, size: egui::Vec2, icon: &str, on: bool, tip: &str) -> Response {
    let t = Tokens::DARK;
    let (r, resp) = ui.allocate_exact_size(size, Sense::click());
    let fill = if on {
        t.accent
    } else if resp.hovered() {
        t.button_hi
    } else {
        t.button
    };
    ui.painter().rect(r, CornerRadius::same(3), fill, Stroke::new(1.0, t.button_border), StrokeKind::Inside);
    let col = if on { Color32::WHITE } else { t.text };
    let s = r.height().min(r.width()) * 0.78;
    icons::draw(ui.painter(), Rect::from_center_size(r.center(), vec2(s, s)), icon, col);
    resp.on_hover_text(tip)
}

/// Square text toggle used for S / M / I / record etc.
pub fn text_toggle(ui: &mut Ui, size: egui::Vec2, text: &str, on: bool, on_color: Color32, tip: &str) -> Response {
    let t = Tokens::DARK;
    let (r, resp) = ui.allocate_exact_size(size, Sense::click());
    let fill = if on {
        on_color
    } else if resp.hovered() {
        t.button_hi
    } else {
        t.button
    };
    ui.painter().rect(r, CornerRadius::same(2), fill, Stroke::new(1.0, t.button_border), StrokeKind::Inside);
    let col = if on { t.text_dark } else { t.text };
    ui.painter().text(r.center(), Align2::CENTER_CENTER, text, bold((size.y * 0.62).clamp(8.0, 13.0)), col);
    resp.on_hover_text(tip)
}

/// Record-enable button (red dot).
pub fn rec_toggle(ui: &mut Ui, size: egui::Vec2, on: bool, tip: &str) -> Response {
    let t = Tokens::DARK;
    let (r, resp) = ui.allocate_exact_size(size, Sense::click());
    let fill = if on {
        t.rec
    } else if resp.hovered() {
        t.button_hi
    } else {
        t.button
    };
    ui.painter().rect(r, CornerRadius::same(2), fill, Stroke::new(1.0, t.button_border), StrokeKind::Inside);
    ui.painter().circle_filled(r.center(), size.y * 0.24, if on { Color32::WHITE } else { Color32::from_rgb(190, 190, 190) });
    resp.on_hover_text(tip)
}

/// A dropdown-looking selector box. Returns the response (caller opens a menu on click).
pub fn selector_box(ui: &mut Ui, width: f32, height: f32, text: &str, color: Color32) -> Response {
    let t = Tokens::DARK;
    let (r, resp) = ui.allocate_exact_size(vec2(width, height), Sense::click());
    ui.painter().rect(
        r,
        CornerRadius::same(2),
        if resp.hovered() { t.button_hi } else { t.button },
        Stroke::new(1.0, t.button_border),
        StrokeKind::Inside,
    );
    let tr = Rect::from_min_max(pos2(r.max.x - height * 0.8, r.min.y), r.max);
    icons::draw(ui.painter(), tr.shrink(height * 0.15), "triangle_down", t.text_dim);
    let galley = ui.painter().layout(text.to_string(), regular((height * 0.62).clamp(8.0, 12.0)), color, (width - height).max(10.0));
    let pos = pos2(r.min.x + (r.width() - height * 0.6 - galley.size().x) * 0.5, r.center().y - galley.size().y * 0.5);
    ui.painter().with_clip_rect(r.shrink(1.0)).galley(pos, galley, color);
    resp
}
