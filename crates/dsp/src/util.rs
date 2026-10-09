//! Small realtime building blocks shared by the plugins.

mod smoothing;
pub(crate) use smoothing::Smoother;
mod delay;
pub(crate) use delay::DelayLine;
mod noise;
pub(crate) use noise::{PinkNoise, Rng};
mod lfo;
pub(crate) use lfo::Lfo;
mod dc_blocker;
pub(crate) use dc_blocker::DcBlocker;

/// Default sample rate used when a host passes something unusable.
pub(crate) const FALLBACK_SR: f32 = 48_000.0;

/// Sanitizes a host sample rate into 1 kHz ..= 1.536 MHz.
pub(crate) fn sane_sr(sr: f32) -> f32 {
    if sr.is_finite() && sr >= 1000.0 { sr.min(1_536_000.0) } else { FALLBACK_SR }
}

/// Upper bound on channels any plugin will allocate state for.
pub(crate) const MAX_CHANNELS: usize = 64;

pub(crate) fn sane_channels(ch: usize) -> usize {
    ch.min(MAX_CHANNELS)
}

/// Converts milliseconds to (fractional) samples, never negative or non-finite.
#[inline]
pub(crate) fn ms_to_samples(ms: f32, sr: f32) -> f32 {
    let s = ms * 0.001 * sr;
    if s.is_finite() { s.max(0.0) } else { 0.0 }
}

/// One-pole coefficient for a time constant in milliseconds.
#[inline]
pub(crate) fn time_coef(ms: f32, sr: f32) -> f32 {
    let n = ms_to_samples(ms, sr);
    if n < 1.0e-3 { 0.0 } else { (-1.0 / n).exp() }
}

/// Flushes denormals and non-finite values in feedback paths to zero.
#[inline]
pub(crate) fn flush(x: f32) -> f32 {
    if x.is_finite() && x.abs() > 1.0e-25 { x } else { 0.0 }
}

/// Number of frames a plugin may safely touch.
pub(crate) fn frames_in(io: &[Vec<f32>], frames: usize, channels: usize) -> usize {
    io.iter().take(channels).map(Vec::len).fold(frames, usize::min)
}

/// Sorts pending note events by offset in place (insertion sort; tiny lists, no allocation).
pub(crate) fn sort_events<T: Copy>(ev: &mut [T], key: impl Fn(&T) -> usize) {
    for i in 1..ev.len() {
        let mut j = i;
        while j > 0 {
            let (Some(a), Some(b)) = (ev.get(j - 1), ev.get(j)) else { break };
            if key(a) <= key(b) {
                break;
            }
            ev.swap(j - 1, j);
            j -= 1;
        }
    }
}

/// Converts a MIDI note number to Hz (A4 = 440).
#[inline]
pub(crate) fn note_hz(note: f32) -> f32 {
    440.0 * 2f32.powf((note - 69.0) / 12.0)
}
