//! Windowed-sinc sample-rate conversion.

use super::MAX_OFFLINE_SAMPLES;
use std::f64::consts::PI;

// ---------------------------------------------------------------------------------------------
// Resampling (Kaiser-windowed sinc)

fn bessel_i0(x: f64) -> f64 {
    let mut sum = 1.0;
    let mut term = 1.0;
    let y = x * x / 4.0;
    for k in 1..64 {
        term *= y / (k as f64 * k as f64);
        sum += term;
        if term < sum * 1e-12 {
            break;
        }
    }
    sum
}

const SINC_ZC: f64 = 32.0;

const SINC_RES: usize = 512;

/// Precomputed one-sided windowed-sinc kernel for a given cutoff (fraction of input Nyquist).
struct SincTable {
    table: Vec<f32>,
    half_width: f64,
}

impl SincTable {
    fn new(cutoff: f64) -> Self {
        let cutoff = cutoff.clamp(0.01, 1.0);
        let half_width = SINC_ZC / cutoff;
        let n = (half_width * SINC_RES as f64).ceil() as usize + 2;
        let beta = 9.0;
        let i0b = bessel_i0(beta);
        let table = (0..n)
            .map(|i| {
                let t = i as f64 / SINC_RES as f64;
                let x = t / half_width;
                if x >= 1.0 {
                    return 0.0;
                }
                let arg = PI * cutoff * t;
                let sinc = if arg.abs() < 1e-12 { 1.0 } else { arg.sin() / arg };
                let w = bessel_i0(beta * (1.0 - x * x).max(0.0).sqrt()) / i0b;
                (cutoff * sinc * w) as f32
            })
            .collect();
        Self { table, half_width }
    }

    #[inline]
    fn at(&self, t: f64) -> f32 {
        let pos = t.abs() * SINC_RES as f64;
        let i = pos as usize;
        let f = (pos - i as f64) as f32;
        match (self.table.get(i), self.table.get(i + 1)) {
            (Some(a), Some(b)) => a + (b - a) * f,
            (Some(a), None) => *a,
            _ => 0.0,
        }
    }
}

/// Resamples one channel to exactly `out_len` samples, where output sample `n` is read at input
/// time `n / ratio`.
fn resample_channel(x: &[f32], ratio: f64, out_len: usize, table: &SincTable) -> Vec<f32> {
    let len = x.len() as i64;
    let hw = table.half_width;
    (0..out_len)
        .map(|n| {
            let t = n as f64 / ratio;
            let lo = (t - hw).ceil().max(0.0) as i64;
            let hi = ((t + hw).floor() as i64).min(len - 1);
            let mut acc = 0.0f32;
            let mut j = lo;
            while j <= hi {
                if let Some(&v) = x.get(j as usize) {
                    acc += v * table.at(t - j as f64);
                }
                j += 1;
            }
            acc
        })
        .collect()
}

pub(super) fn resample_ratio(ch: &[Vec<f32>], ratio: f64, out_len: usize) -> Vec<Vec<f32>> {
    if !ratio.is_finite() || ratio <= 0.0 {
        return ch.iter().map(|_| vec![0.0; out_len]).collect();
    }
    // Anti-alias below the lower of the two Nyquists, with a little transition room.
    let cutoff = if ratio < 1.0 { ratio * 0.94 } else { 0.97 };
    let table = SincTable::new(cutoff);
    ch.iter().map(|c| resample_channel(c, ratio, out_len, &table)).collect()
}

/// High-quality windowed-sinc sample-rate conversion from `from` Hz to `to` Hz.
pub fn resample(ch: &[Vec<f32>], from: u32, to: u32) -> Vec<Vec<f32>> {
    if from == 0 || to == 0 || from == to {
        return ch.to_vec();
    }
    let ratio = f64::from(to) / f64::from(from);
    ch.iter()
        .map(|c| {
            let out_len = ((c.len() as f64 * ratio).round() as usize).min(MAX_OFFLINE_SAMPLES);
            resample_ratio(std::slice::from_ref(c), ratio, out_len).into_iter().next().unwrap_or_default()
        })
        .collect()
}
