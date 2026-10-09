//! Memory-location navigation and editing.

use crate::SoundApp;
use crate::theme::mono;
use egui::vec2;
use serde_json::json;
use soundcraft_time::format_position;

pub(super) fn memory_locations(app: &mut SoundApp, ctx: &egui::Context) {
    let mut open = app.ui.show_memory_locations;
    if !open {
        return;
    }
    egui::Window::new("Memory Locations").open(&mut open).default_size(vec2(480.0, 280.0)).show(ctx, |ui| {
        ui.horizontal(|ui| {
            if ui.button("+ Marker").clicked() {
                let _ = app.run("markers.add", json!({}));
            }
            if ui.button("+ Selection").clicked() {
                let _ = app.run("markers.add", json!({"kind": "selection"}));
            }
        });
        let rows: Vec<(u32, String, String, String, String)> = {
            let s = app.engine.session();
            let f = |x: i64| format_position(x, s.edit.main_counter, s.sample_rate, &s.tempo, s.frame_rate, s.timecode_start);
            s.markers
                .iter()
                .map(|m| {
                    let kind = match m.kind {
                        soundcraft_model::MarkerKind::Marker => "marker",
                        soundcraft_model::MarkerKind::Selection => "selection",
                        soundcraft_model::MarkerKind::None => "none",
                    };
                    let len = if m.kind == soundcraft_model::MarkerKind::Selection { f(m.end - m.start) } else { String::new() };
                    (m.number, m.name.clone(), f(m.start), len, kind.to_string())
                })
                .collect()
        };
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("mem_grid").striped(true).num_columns(6).show(ui, |ui| {
                for h in ["#", "Name", "Location", "Length", "Type", ""] {
                    ui.label(egui::RichText::new(h).strong());
                }
                ui.end_row();
                for (n, name, loc, len, kind) in rows {
                    if ui.button(n.to_string()).on_hover_text("Recall").clicked() {
                        let _ = app.run("markers.recall", json!({"number": n}));
                    }
                    let key = egui::Id::new(("mem_name", n));
                    let mut edit: String = ui.ctx().memory(|m| m.data.get_temp(key)).unwrap_or_else(|| name.clone());
                    let r = ui.add(egui::TextEdit::singleline(&mut edit).desired_width(140.0));
                    if r.changed() {
                        ui.ctx().memory_mut(|m| m.data.insert_temp(key, edit.clone()));
                    }
                    if r.lost_focus() && edit != name {
                        let _ = app.run("markers.edit", json!({"number": n, "rename": edit}));
                        ui.ctx().memory_mut(|m| m.data.remove::<String>(key));
                    }
                    ui.label(egui::RichText::new(loc).font(mono(11.0)));
                    ui.label(egui::RichText::new(len).font(mono(11.0)));
                    ui.label(kind);
                    if ui.small_button("✕").on_hover_text("Delete").clicked() {
                        let _ = app.run("markers.delete", json!({"number": n}));
                    }
                    ui.end_row();
                }
            });
        });
    });
    app.ui.show_memory_locations = open;
}
