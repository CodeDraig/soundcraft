//! UI scale, motion and tooltip customization.

use crate::SoundApp;
use crate::theme::bold;
use egui::vec2;

pub(super) fn apply_customization(app: &mut SoundApp, ctx: &egui::Context) {
    let want = (app.extra.ui_scale.clamp(0.75, 1.5), app.extra.reduce_motion);
    if app.extra.applied == Some(want) {
        return;
    }
    // Only touch the zoom once the user has changed it, so platform scaling stays untouched.
    if app.extra.applied.is_some() || (want.0 - 1.0).abs() > f32::EPSILON {
        ctx.set_zoom_factor(want.0);
    }
    ctx.global_style_mut(|s| s.animation_time = if want.1 { 0.0 } else { 1.0 / 12.0 });
    app.extra.applied = Some(want);
}

pub(super) fn ui_customization(app: &mut SoundApp, ctx: &egui::Context) {
    let mut open = app.extra.show_ui_customization;
    if !open {
        return;
    }
    egui::Window::new(egui::RichText::new("UI Customization").font(bold(13.0)))
        .open(&mut open)
        .default_size(vec2(320.0, 180.0))
        .resizable(false)
        .show(ctx, |ui| {
            egui::Grid::new("ui_custom").num_columns(2).spacing(vec2(12.0, 8.0)).show(ui, |ui| {
                ui.label("Interface scale");
                ui.add(egui::Slider::new(&mut app.extra.ui_scale, 0.75..=1.5).step_by(0.05).custom_formatter(|v, _| format!("{:.0} %", v * 100.0)));
                ui.end_row();
                ui.label("Reduce motion");
                ui.checkbox(&mut app.extra.reduce_motion, "");
                ui.end_row();
                ui.label("Auto-update configuration");
                ui.checkbox(&mut app.extra.auto_update_config, "");
                ui.end_row();
            });
            if ui.button("Reset").clicked() {
                app.extra.ui_scale = 1.0;
                app.extra.reduce_motion = false;
            }
        });
    app.extra.show_ui_customization = open;
}
