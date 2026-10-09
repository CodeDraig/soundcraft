//! Plugin editor windows, response curves and presets.

use crate::SoundApp;
use crate::theme::Tokens;
use crate::theme::mono;
use egui::Color32;
use egui::Sense;
use egui::Stroke;
use egui::Ui;
use egui::pos2;
use egui::vec2;
use serde_json::json;

mod presets;
use presets::preset_menu;

pub(super) fn plugin_windows(app: &mut SoundApp, ctx: &egui::Context) {
    let wins = app.ui.plugin_windows.clone();
    let mut keep = Vec::new();
    for (tid, slot) in wins {
        let Some((tname, ins)) =
            app.engine.session().track(tid).and_then(|t| t.mixer.inserts.get(slot).cloned().flatten().map(|i| (t.name.clone(), i)))
        else {
            continue;
        };
        let Some(info) = crate::mix_window::plugin_info(&ins.plugin) else { continue };
        let mut open = true;
        egui::Window::new(format!("{tname} · {} · {}", (b'a' + slot as u8) as char, info.name))
            .id(egui::Id::new(("plugin", tid.0, slot)))
            .open(&mut open)
            .default_width(340.0)
            .default_pos(egui::pos2(ctx.content_rect().width() - 380.0, 120.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(&ins.preset).color(Tokens::DARK.text_dim));
                    let label = if ins.bypass { "BYPASSED" } else { "Bypass" };
                    if ui.button(label).clicked() {
                        let _ = app.run("mix.insert_bypass", json!({"track": tid.0, "slot": slot}));
                    }
                    ui.menu_button("Presets ▾", |ui| preset_menu(app, ui, tid, slot, info, &ins));
                    if soundcraft_mix::is_third_party(&ins.plugin) {
                        let open = app.player.as_ref().is_some_and(|p| p.editor_open(tid, slot));
                        let available = app.player.as_ref().is_some_and(|p| p.has_editor(tid, slot));
                        let label = if open { "Close Plugin Editor" } else { "Open Plugin Editor" };
                        let resp = ui.add_enabled(available, egui::Button::new(label));
                        let resp =
                            if available { resp } else { resp.on_disabled_hover_text("This plugin has no editor of its own, or it is not loaded") };
                        if resp.clicked()
                            && let Some(p) = &app.player
                        {
                            if open {
                                p.close_editor(tid, slot);
                            } else if let Err(e) = p.open_editor(tid, slot) {
                                app.ui.status = format!("{}: {e}", info.name);
                            }
                        }
                    }
                });
                if info.id == "eq_7band" || info.id == "eq_1band" {
                    eq_curve(ui, info.id, &ins);
                }
                egui::Grid::new(("pg", tid.0, slot)).num_columns(3).show(ui, |ui| {
                    for p in info.params {
                        let v = ins.params.get(p.id).copied().unwrap_or(p.default);
                        ui.label(p.name);
                        let mut x = v;
                        let resp = if !p.choices.is_empty() {
                            let mut idx = x.round().max(0.0) as usize;
                            let r = egui::ComboBox::from_id_salt(("pc", tid.0, slot, p.id))
                                .selected_text(p.choices.get(idx).copied().unwrap_or("?"))
                                .show_ui(ui, |ui| {
                                    for (i, c) in p.choices.iter().enumerate() {
                                        ui.selectable_value(&mut idx, i, *c);
                                    }
                                });
                            x = idx as f32;
                            r.response
                        } else {
                            let mut sl = egui::Slider::new(&mut x, p.min..=p.max).show_value(false);
                            if p.taper == soundcraft_dsp::Taper::Log && p.min > 0.0 {
                                sl = sl.logarithmic(true);
                            }
                            ui.add(sl)
                        };
                        if (x - v).abs() > f32::EPSILON {
                            let _ = app.engine.execute_merged(
                                "mix.insert_param",
                                &json!({"track": tid.0, "slot": slot, "param": p.id, "value": x}),
                                &format!("param:{}:{slot}:{}", tid.0, p.id),
                            );
                            if let Some(pl) = &app.player {
                                // Keep the plugin's own editor in step.
                                pl.editor_set_param(tid, slot, p.id, x);
                            }
                        }
                        let _ = resp;
                        ui.label(egui::RichText::new(p.format(x)).font(mono(11.0)));
                        ui.end_row();
                    }
                });
            });
        if open {
            keep.push((tid, slot));
        }
    }
    app.ui.plugin_windows = keep;
}

fn eq_curve(ui: &mut Ui, id: &str, ins: &soundcraft_model::Insert) {
    let t = Tokens::DARK;
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width().max(300.0), 110.0), Sense::hover());
    ui.painter().rect_filled(r, 4.0, Color32::from_rgb(14, 18, 22));
    let n = 160;
    let freqs: Vec<f32> = (0..n).map(|i| 20.0 * 1000f32.powf(i as f32 / (n - 1) as f32)).collect();
    let params: Vec<(&str, f32)> = ins.params.iter().map(|(k, v)| (k.as_str(), *v)).collect();
    let resp = if id == "eq_7band" {
        soundcraft_dsp::eq7_response(&params, &freqs, 48_000.0)
    } else {
        soundcraft_dsp::eq1_response(&params, &freqs, 48_000.0)
    };
    for db in [-12.0f32, 0.0, 12.0] {
        let y = r.center().y - db / 18.0 * r.height() * 0.5;
        ui.painter().line_segment([pos2(r.min.x, y), pos2(r.max.x, y)], Stroke::new(1.0, Color32::from_rgb(40, 50, 60)));
    }
    let pts: Vec<egui::Pos2> = resp
        .iter()
        .enumerate()
        .map(|(i, db)| pos2(r.min.x + r.width() * i as f32 / (n - 1) as f32, r.center().y - db.clamp(-18.0, 18.0) / 18.0 * r.height() * 0.5))
        .collect();
    ui.painter().add(egui::Shape::line(pts, Stroke::new(2.0, t.counter_text)));
}
