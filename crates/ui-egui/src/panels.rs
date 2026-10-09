//! Side panels (Tracks, Groups, Clips) and floating windows.

use crate::SoundApp;
use crate::theme::{Tokens, bold};
use egui::{Align2, Sense, Stroke, Ui, pos2, vec2};

mod tracks;
pub use tracks::tracks_and_groups;
mod clips;
pub use clips::clip_list;
mod transport;
use transport::{big_counter, transport_window};
mod locations;
use locations::memory_locations;
mod history;
use history::undo_history;
mod plugins;
use plugins::plugin_windows;
mod about;
use about::{about, session_info};

fn panel_header(ui: &mut Ui, title: &str) {
    let t = Tokens::DARK;
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 22.0), Sense::hover());
    ui.painter().rect_filled(r, 0.0, t.panel_bg2);
    ui.painter().text(pos2(r.min.x + 8.0, r.center().y), Align2::LEFT_CENTER, title, bold(11.5), t.header_text);
    ui.painter().line_segment([pos2(r.min.x, r.max.y), pos2(r.max.x, r.max.y)], Stroke::new(1.0, t.border));
}

/// Floating windows.
pub fn floating(app: &mut SoundApp, ctx: &egui::Context) {
    transport_window(app, ctx);
    memory_locations(app, ctx);
    big_counter(app, ctx);
    undo_history(app, ctx);
    plugin_windows(app, ctx);
    about(app, ctx);
    session_info(app, ctx);
}
