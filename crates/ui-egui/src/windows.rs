//! Secondary floating windows (Window menu).

use crate::SoundApp;

mod automation;
use automation::automation;
mod colors;
use colors::color_palette;
mod resources;
use resources::{disk_usage, system_usage, task_manager};
mod metadata;
use metadata::metadata;
mod midi;
use midi::{event_list, midi_keyboard};
mod workspace;
use workspace::workspace;
mod configurations;
use configurations::configurations;
mod audio_setup;
pub use audio_setup::{io_setup, playback_engine};
mod shortcuts;
pub use shortcuts::shortcuts_window;
mod clip_effects;
pub use clip_effects::clip_effects;

pub fn show(app: &mut SoundApp, ctx: &egui::Context) {
    automation(app, ctx);
    color_palette(app, ctx);
    disk_usage(app, ctx);
    system_usage(app, ctx);
    task_manager(app, ctx);
    metadata(app, ctx);
    event_list(app, ctx);
    midi_keyboard(app, ctx);
    workspace(app, ctx);
    configurations(app, ctx);
    playback_engine(app, ctx);
    io_setup(app, ctx);
    shortcuts_window(app, ctx);
    clip_effects(app, ctx);
}

fn win(ctx: &egui::Context, open: &mut bool, title: &str, size: egui::Vec2, body: impl FnOnce(&mut egui::Ui)) {
    if !*open {
        return;
    }
    egui::Window::new(title).open(open).default_size(size).show(ctx, body);
}
