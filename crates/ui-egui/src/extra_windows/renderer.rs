//! Surround speaker layout and object-position display.

use crate::SoundApp;
use crate::theme::Tokens;
use crate::theme::bold;
use crate::theme::regular;
use egui::Align2;
use egui::Color32;
use egui::Sense;
use egui::Stroke;
use egui::pos2;
use egui::vec2;
use soundcraft_model::Speaker;
use soundcraft_model::TrackKind;

pub(super) fn renderer(app: &mut SoundApp, ctx: &egui::Context) {
    let mut open = app.extra.show_renderer;
    if !open {
        return;
    }
    let t = Tokens::DARK;
    let fmt = app.engine.session().main_format();
    let peaks: Vec<f32> = app.player.as_ref().map(|p| p.meters().main.peaks).unwrap_or_default();
    let objects: Vec<(String, f32, f32, f32)> = app
        .engine
        .session()
        .tracks
        .iter()
        .filter(|tr| tr.kind != TrackKind::Master && app.engine.session().edit.flag(&format!("object.{}", tr.id.0)))
        .map(|tr| {
            let sp = tr.mixer.surround.unwrap_or_default();
            (tr.name.clone(), sp.x, sp.y, sp.z)
        })
        .collect();
    egui::Window::new(egui::RichText::new("Renderer").font(bold(13.0))).open(&mut open).default_size(vec2(380.0, 440.0)).resizable(true).show(
        ctx,
        |ui| {
            ui.label(egui::RichText::new(format!("Main output: {}", fmt.label())).font(regular(11.0)).color(t.text));
            let side = ui.available_width().min(360.0);
            let (r, _) = ui.allocate_exact_size(vec2(side, side), Sense::hover());
            let p = ui.painter_at(r);
            p.rect_filled(r, 6.0, Color32::from_rgb(16, 19, 23));
            let c = r.center();
            let rad = side * 0.40;
            p.circle_stroke(c, rad, Stroke::new(1.0, Color32::from_rgb(60, 68, 78)));
            p.circle_stroke(c, rad * 0.55, Stroke::new(1.0, Color32::from_rgb(44, 50, 58)));
            p.text(pos2(c.x, r.min.y + 8.0), Align2::CENTER_TOP, "front", regular(9.0), t.text_dim);
            for (i, sp) in fmt.speakers().iter().enumerate() {
                let level = peaks.get(i).copied().unwrap_or(0.0).clamp(0.0, 1.0);
                let ring = if sp.is_height() { rad * 0.55 } else { rad };
                let a = sp.azimuth().to_radians();
                let pos = if *sp == Speaker::Lfe { pos2(c.x, c.y + rad * 0.2) } else { pos2(c.x + ring * a.sin(), c.y - ring * a.cos()) };
                let glow = Color32::from_rgb(40 + (level * 60.0) as u8, 90 + (level * 140.0) as u8, 70 + (level * 40.0) as u8);
                p.circle_filled(pos, 9.0 + level * 6.0, glow);
                p.circle_stroke(pos, 9.0, Stroke::new(1.0, Color32::from_rgb(150, 160, 170)));
                p.text(pos, Align2::CENTER_CENTER, format!("{sp:?}"), bold(8.0), Color32::WHITE);
            }
            for (name, x, y, z) in &objects {
                let pos = pos2(c.x + x.clamp(-1.0, 1.0) * rad, c.y - y.clamp(-1.0, 1.0) * rad);
                p.circle_filled(pos, 5.0 + z.clamp(0.0, 1.0) * 3.0, Color32::from_rgb(240, 180, 60));
                p.text(pos2(pos.x + 8.0, pos.y), Align2::LEFT_CENTER, name, regular(9.0), Color32::from_rgb(240, 200, 120));
            }
            ui.add_space(4.0);
            if objects.is_empty() {
                ui.label(
                    egui::RichText::new("No object tracks. Use the Object view in the Mix window to route tracks as objects.")
                        .small()
                        .color(t.text_dim),
                );
            } else {
                ui.label(egui::RichText::new(format!("{} object track(s)", objects.len())).small().color(t.text_dim));
            }
        },
    );
    app.extra.show_renderer = open;
}
