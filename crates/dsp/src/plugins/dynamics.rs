//! Dynamics: Compressor/Limiter, Expander/Gate, De-Esser and Maximizer.
//!
//! All detectors are stereo-linked (the loudest channel drives a shared gain) so the image does
//! not wander. Gain computers work in the log domain with smoothed gain reduction.

mod compressor;
pub use compressor::{COMP_INFO, Compressor};
mod gate;
pub use gate::{ExpanderGate, GATE_INFO};
mod de_esser;
pub use de_esser::{DEESS_INFO, DeEsser};
mod maximizer;
pub use maximizer::{MAX_INFO, Maximizer};

/// Static soft-knee gain computer: returns the gain reduction amount in dB (≥ 0).
#[inline]
pub(crate) fn compress_gr(level_db: f32, threshold: f32, ratio: f32, knee: f32) -> f32 {
    let slope = 1.0 - 1.0 / ratio.max(1.0);
    let over = level_db - threshold;
    if knee > 1e-3 && 2.0 * over.abs() <= knee {
        let t = over + knee / 2.0;
        slope * t * t / (2.0 * knee)
    } else if over > 0.0 {
        slope * over
    } else {
        0.0
    }
}

#[inline]
fn ballistic(state: f32, target: f32, att: f32, rel: f32) -> f32 {
    let c = if target > state { att } else { rel };
    target + (state - target) * c
}
