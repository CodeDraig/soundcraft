//! Undo-history display.

use crate::SoundApp;
use crate::theme::Tokens;
use egui::vec2;
use serde_json::json;

pub(super) fn undo_history(app: &mut SoundApp, ctx: &egui::Context) {
    let mut open = app.ui.show_undo_history;
    if !open {
        return;
    }
    egui::Window::new("Undo History").open(&mut open).default_size(vec2(260.0, 300.0)).show(ctx, |ui| {
        let hist = app.engine.undo_history();
        let n = hist.len();
        egui::ScrollArea::vertical().show(ui, |ui| {
            for (i, h) in hist.iter().enumerate() {
                if ui.selectable_label(i + 1 == n, format!("{}  {h}", i + 1)).clicked() {
                    for _ in 0..(n - i - 1) {
                        let _ = app.run("edit.undo", json!({}));
                    }
                }
            }
            if let Some(r) = app.engine.redo_label() {
                ui.label(egui::RichText::new(format!("(redo) {r}")).italics().color(Tokens::DARK.text_dim));
            }
        });
    });
    app.ui.show_undo_history = open;
}
