//! Session clip-list sidebar.

use super::panel_header;
use crate::SoundApp;
use crate::theme::Tokens;
use crate::theme::bold;
use crate::theme::regular;
use crate::theme::rgb;
use egui::Align2;
use egui::Color32;
use egui::Rect;
use egui::Sense;
use egui::Ui;
use egui::pos2;
use egui::vec2;
use serde_json::json;

pub fn clip_list(app: &mut SoundApp, ui: &mut Ui) {
    let t = Tokens::DARK;
    panel_header(ui, "CLIPS");
    let items: Vec<(Option<u64>, String, bool, [u8; 3])> = {
        let s = app.engine.session();
        let mut v: Vec<(Option<u64>, String, bool, [u8; 3])> = Vec::new();
        // Whole files first (bold), then clips on tracks.
        for src in &s.sources {
            v.push((Some(src.id.0), format!("{} ({}ch)", src.name, src.channels), true, [150, 150, 150]));
        }
        for tr in &s.tracks {
            for c in tr.clips() {
                v.push((Some(c.id.0), c.name.clone(), false, c.color.unwrap_or(tr.color)));
            }
        }
        v
    };
    let selected: Vec<u64> = app.engine.session().edit.selected_clips.iter().map(|c| c.0).collect();
    let fkey = egui::Id::new("clip_filter");
    let mut filter: String = ui.ctx().memory(|m| m.data.get_temp(fkey)).unwrap_or_default();
    ui.add(egui::TextEdit::singleline(&mut filter).hint_text("🔍 Find clips").desired_width(f32::INFINITY));
    ui.ctx().memory_mut(|m| m.data.insert_temp(fkey, filter.clone()));
    let needle = filter.to_lowercase();
    let items: Vec<_> = items.into_iter().filter(|(_, n, _, _)| needle.is_empty() || n.to_lowercase().contains(&needle)).collect();
    egui::ScrollArea::vertical().id_salt("clips_scroll").auto_shrink([false, false]).show(ui, |ui| {
        for (id, name, whole, color) in items {
            let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 17.0), if whole { Sense::click_and_drag() } else { Sense::click() });
            if whole && let Some(src) = id {
                resp.context_menu(|ui| {
                    if ui.button("Place on New Track").clicked() {
                        let _ = app.run("clip.place_source", json!({"source": src}));
                        ui.close();
                    }
                    let sel_track = app.engine.session().edit.selected_tracks.first().map(|t| t.0);
                    if let Some(t) = sel_track
                        && ui.button("Place on Selected Track").clicked()
                    {
                        let _ = app.run("clip.place_source", json!({"source": src, "track": t}));
                        ui.close();
                    }
                });
                // Whole files drag onto tracks.
                resp.dnd_set_drag_payload(crate::DragSource(src));
                if resp.dragged() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                }
            }
            if !whole && id.is_some_and(|i| selected.contains(&i)) {
                ui.painter().rect_filled(r, 0.0, Color32::from_rgb(52, 70, 96));
            }
            ui.painter().rect_filled(Rect::from_min_size(pos2(r.min.x + 6.0, r.min.y + 4.0), vec2(9.0, 9.0)), 1.0, rgb(color));
            ui.painter().with_clip_rect(r).text(
                pos2(r.min.x + 22.0, r.center().y),
                Align2::LEFT_CENTER,
                &name,
                if whole { bold(11.0) } else { regular(11.0) },
                t.text,
            );
            if resp.clicked()
                && !whole
                && let Some(i) = id
            {
                let _ = app.run("edit.select", json!({"clips": [i]}));
            }
        }
    });
}
