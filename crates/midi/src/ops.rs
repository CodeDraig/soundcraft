//! MIDI editing operations (Event Operations): quantize, transpose, velocity, duration, split.
//!
//! All operations work in place on a slice of notes and never change the slice length, so they
//! can be applied to a selection. Randomised operations use a deterministic [`XorShift`](crate::XorShift) seeded
//! with [`DEFAULT_SEED`] (or an explicit seed via the `*_with_seed` variants).

use crate::Note;

mod quantize;
pub use quantize::{QuantizeOptions, quantize, quantize_with_seed};
mod velocity;
pub use velocity::{VelocityOp, change_velocity, change_velocity_with_seed};
mod duration;
pub use duration::{DurationOp, change_duration};

/// Seed used by [`quantize`] and [`change_velocity`] for their random components.
pub const DEFAULT_SEED: u64 = 0x5EED_50DC_4AF7_0001;

fn unit(x: f32) -> f64 {
    if x.is_finite() { f64::from(x).clamp(0.0, 1.0) } else { 0.0 }
}

fn round_i64(x: f64) -> i64 {
    if x.is_finite() { x.round().clamp(i64::MIN as f64, i64::MAX as f64) as i64 } else { 0 }
}

/// Transposes notes by `semitones`. Pitches are **clamped** to 0..=127 (notes are never
/// dropped, so the slice length is unchanged); notes pushed past either end pile up on 0 or 127.
pub fn transpose(notes: &mut [Note], semitones: i32) {
    for n in notes.iter_mut() {
        n.pitch = i32::from(n.pitch).saturating_add(semitones).clamp(0, 127) as u8;
    }
}

/// Select/Split Notes by pitch: returns `(notes with pitch in pitch_from..=pitch_to, the rest)`,
/// both in input order. The bounds are swapped if reversed.
pub fn split_notes(notes: &[Note], pitch_from: u8, pitch_to: u8) -> (Vec<Note>, Vec<Note>) {
    let (lo, hi) = if pitch_from <= pitch_to { (pitch_from, pitch_to) } else { (pitch_to, pitch_from) };
    notes.iter().partition(|n| (lo..=hi).contains(&n.pitch))
}
