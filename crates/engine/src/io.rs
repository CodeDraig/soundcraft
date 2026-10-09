//! Import, export, save/load, rendering helpers (bounce, consolidate, commit) and analysis.

mod audio;
pub use audio::{add_source, import_audio_bytes};
mod video;
pub use video::{import_video_bytes, relink_videos};
mod midi;
pub use midi::{export_midi, import_midi_bytes};
mod bounce;
pub use bounce::{bounce_bytes_with, bounce_stems_with};
mod render_edits;
pub use render_edits::{bounded_to_track, commit_track, consolidate, export_clips, render_clip_gain, strip_silence, transients_in};
mod session;
pub use session::{open_session, save_copy, save_session};
mod recording;
pub use recording::add_recording;

pub fn sanitize_name(n: &str) -> String {
    let s: String = n.chars().map(|c| if c.is_alphanumeric() || matches!(c, '-' | '_' | ' ' | '.') { c } else { '_' }).collect();
    let s = s.trim().trim_start_matches('.').to_string();
    if s.is_empty() { "untitled".into() } else { s.chars().take(100).collect() }
}
