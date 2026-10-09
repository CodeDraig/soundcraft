//! AudioSuite-style whole-buffer processing on planar `f32` audio.
//!
//! These functions may allocate (they are not for the audio thread) but never panic: lengths,
//! rates and ratios are sanitized and output sizes are capped at [`MAX_OFFLINE_SAMPLES`] per
//! channel.

use crate::osc::Osc;
pub use crate::osc::Waveform;
use crate::util::sane_sr;
use crate::{db_to_gain, gain_to_db};

mod resample;
pub use resample::resample;
use resample::resample_ratio;
mod stretch;
pub use stretch::{pitch_shift, time_stretch};
mod detection;
pub use detection::{detect_transients, non_silent_ranges};
mod plugin_render;
pub use plugin_render::{apply_plugin, apply_plugin_with_tail};

/// Largest number of samples per channel any offline function will produce.
pub const MAX_OFFLINE_SAMPLES: usize = 1 << 28;

/// Fade curves. [`FadeShape::gain`] is the fade-in curve; a fade-out uses `gain(1 - t)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
pub enum FadeShape {
    Linear,
    /// Constant power (sin/cos), for crossfades between uncorrelated material.
    #[default]
    EqualPower,
    /// Raised cosine.
    SCurve,
    /// Slow start, fast finish.
    Exponential,
    /// Fast start, slow finish.
    Logarithmic,
}

impl FadeShape {
    pub const ALL: [FadeShape; 5] = [FadeShape::Linear, FadeShape::EqualPower, FadeShape::SCurve, FadeShape::Exponential, FadeShape::Logarithmic];

    /// Fade-in gain at position `t` in 0..1 (0 → silent, 1 → unity). NaN is treated as 0.
    pub fn gain(self, t: f32) -> f32 {
        let t = if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) };
        const K: f32 = 4.0;
        match self {
            FadeShape::Linear => t,
            FadeShape::EqualPower => (t * std::f32::consts::FRAC_PI_2).sin(),
            FadeShape::SCurve => 0.5 - 0.5 * (t * std::f32::consts::PI).cos(),
            FadeShape::Exponential => ((K * t).exp() - 1.0) / (K.exp() - 1.0),
            FadeShape::Logarithmic => (1.0 + (K.exp() - 1.0) * t).ln() / K,
        }
    }
}

fn max_len(ch: &[Vec<f32>]) -> usize {
    ch.iter().map(Vec::len).max().unwrap_or(0)
}

fn peak(ch: &[Vec<f32>]) -> f32 {
    ch.iter().flatten().filter(|v| v.is_finite()).fold(0.0f32, |m, v| m.max(v.abs()))
}

/// Peak-normalizes to `target_db` dBFS, either all channels together or each on its own.
/// Silent channels are left alone.
pub fn normalize(ch: &mut [Vec<f32>], target_db: f32, per_channel: bool) {
    let target = db_to_gain(if target_db.is_nan() { 0.0 } else { target_db.clamp(-144.0, 24.0) });
    if per_channel {
        for c in ch.iter_mut() {
            let p = peak(std::slice::from_ref(c));
            if p > 0.0 {
                let g = target / p;
                c.iter_mut().for_each(|s| *s *= g);
            }
        }
    } else {
        let p = peak(ch);
        if p > 0.0 {
            let g = target / p;
            ch.iter_mut().flatten().for_each(|s| *s *= g);
        }
    }
}

/// Applies a gain change in dB.
pub fn gain(ch: &mut [Vec<f32>], db: f32) {
    let g = db_to_gain(if db.is_nan() { 0.0 } else { db.min(96.0) });
    ch.iter_mut().flatten().for_each(|s| *s *= g);
}

/// Flips polarity.
pub fn invert(ch: &mut [Vec<f32>]) {
    ch.iter_mut().flatten().for_each(|s| *s = -*s);
}

/// Reverses each channel in time.
pub fn reverse(ch: &mut [Vec<f32>]) {
    ch.iter_mut().for_each(|c| c.reverse());
}

/// Removes each channel's DC offset (subtracts its mean).
pub fn remove_dc(ch: &mut [Vec<f32>]) {
    for c in ch.iter_mut() {
        if c.is_empty() {
            continue;
        }
        let mean = c.iter().filter(|v| v.is_finite()).map(|&v| f64::from(v)).sum::<f64>() / c.len() as f64;
        let m = mean as f32;
        c.iter_mut().for_each(|s| *s -= m);
    }
}

/// Fades in over the first `fade_in` samples and out over the last `fade_out` samples.
pub fn fade(ch: &mut [Vec<f32>], fade_in: usize, fade_out: usize, shape: FadeShape) {
    for c in ch.iter_mut() {
        let len = c.len();
        let fi = fade_in.min(len);
        for (i, s) in c.iter_mut().take(fi).enumerate() {
            *s *= shape.gain(i as f32 / fi as f32);
        }
        let fo = fade_out.min(len);
        for (j, s) in c.iter_mut().skip(len - fo).enumerate() {
            *s *= shape.gain((fo - 1 - j) as f32 / fo as f32);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Generators and analysis

/// Generates a test signal. Periodic waveforms peak at `level_db`; noise is scaled so its peak
/// stays at or below `level_db`.
pub fn signal_generator(kind: Waveform, freq: f32, level_db: f32, sample_rate: f32, frames: usize, channels: usize) -> Vec<Vec<f32>> {
    let sr = sane_sr(sample_rate);
    let frames = frames.min(MAX_OFFLINE_SAMPLES);
    let channels = channels.min(crate::util::MAX_CHANNELS);
    let f = if freq.is_finite() { freq.clamp(0.0, sr * 0.49) } else { 1000.0 };
    let g = db_to_gain(if level_db.is_nan() { -20.0 } else { level_db.min(0.0) });
    let mut osc = Osc::new(0x1234_5678);
    let mono: Vec<f32> = (0..frames).map(|_| osc.next(kind, f / sr) * g).collect();
    (0..channels).map(|_| mono.clone()).collect()
}

/// Peak level of the buffers in dBFS (-144 for silence).
pub fn peak_db(ch: &[Vec<f32>]) -> f32 {
    gain_to_db(peak(ch))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fade_shapes_endpoints() {
        for s in FadeShape::ALL {
            assert!(s.gain(0.0).abs() < 1e-6, "{s:?}");
            assert!((s.gain(1.0) - 1.0).abs() < 1e-5, "{s:?}");
            assert!(s.gain(f32::NAN) == 0.0);
        }
    }

    #[test]
    fn fade_hits_zero_at_edges() {
        let mut ch = vec![vec![1.0f32; 100]];
        fade(&mut ch, 10, 10, FadeShape::Linear);
        assert_eq!(ch[0][0], 0.0);
        assert_eq!(ch[0][99], 0.0);
        assert_eq!(ch[0][50], 1.0);
        fade(&mut ch, 1000, 1000, FadeShape::SCurve);
    }
}
