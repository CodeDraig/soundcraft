//! Command shortcut reference window.

use super::win;
use crate::SoundApp;
use crate::theme::mono;
use egui::vec2;

/// Setup › Keyboard Shortcuts.
pub fn shortcuts_window(app: &mut SoundApp, ctx: &egui::Context) {
    let mut open = app.ui.show_shortcuts;
    win(ctx, &mut open, "Keyboard Shortcuts", vec2(520.0, 480.0), |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("sc").num_columns(3).striped(true).show(ui, |ui| {
                for c in soundcraft_engine::command_specs().iter().filter(|c| c.shortcut.is_some()) {
                    ui.label(egui::RichText::new(c.shortcut.unwrap_or("")).font(mono(11.0)));
                    ui.label(c.label);
                    ui.label(egui::RichText::new(c.menu.join(" › ")).small());
                    ui.end_row();
                }
            });
        });
    });
    app.ui.show_shortcuts = open;
}
