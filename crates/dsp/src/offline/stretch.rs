//! WSOLA time stretching and pitch shifting.

use super::{MAX_OFFLINE_SAMPLES, max_len, resample_ratio};
use crate::util::sane_sr;
use std::f64::consts::PI;

// ---------------------------------------------------------------------------------------------
// Time stretch (WSOLA) and pitch shift

/// Pitch-preserving time stretch by WSOLA (waveform-similarity overlap-add).
/// `ratio` = output length / input length, clamped to 0.1 ..= 10. All channels share the same
/// segment choices so the stereo image stays coherent.
pub fn time_stretch(ch: &[Vec<f32>], ratio: f64, sample_rate: f32) -> Vec<Vec<f32>> {
    let len = max_len(ch);
    let ratio = if ratio.is_finite() { ratio.clamp(0.1, 10.0) } else { 1.0 };
    let out_len = ((len as f64 * ratio).round() as usize).min(MAX_OFFLINE_SAMPLES);
    if len == 0 || ch.is_empty() {
        return ch.iter().map(|_| vec![0.0; out_len]).collect();
    }
    if (ratio - 1.0).abs() < 1e-9 {
        return ch.iter().map(|c| c.iter().copied().chain(std::iter::repeat(0.0)).take(out_len).collect()).collect();
    }
    let sr = f64::from(sane_sr(sample_rate));
    let frame = (((sr * 0.04) as usize) & !1).clamp(64, 8192);
    let hop = frame / 2;
    let tol = ((sr * 0.010) as usize).clamp(8, 2048) as i64;
    let window: Vec<f32> = (0..frame).map(|i| (0.5 - 0.5 * (2.0 * PI * i as f64 / frame as f64).cos()) as f32).collect();
    let nch = ch.len() as f32;
    let guide: Vec<f32> = (0..len).map(|i| ch.iter().map(|c| c.get(i).copied().unwrap_or(0.0)).sum::<f32>() / nch).collect();
    let g = |i: i64| -> f32 { if i < 0 { 0.0 } else { guide.get(i as usize).copied().unwrap_or(0.0) } };

    let buf_len = out_len + frame;
    let mut out: Vec<Vec<f32>> = ch.iter().map(|_| vec![0.0; buf_len]).collect();
    let mut wsum = vec![0.0f32; buf_len];
    let mut prev_in: i64 = 0;
    let mut k = 0usize;
    loop {
        let out_pos = k * hop;
        if out_pos >= out_len {
            break;
        }
        let in_pos = if k == 0 {
            0
        } else {
            let nominal = (out_pos as f64 / ratio).round() as i64;
            let natural = prev_in + hop as i64;
            // Similarity of candidate start `p` with the natural continuation (stride 2).
            let score = |p: i64| -> f32 {
                let (mut xy, mut yy) = (0.0f32, 1e-9f32);
                let mut i = 0i64;
                while i < hop as i64 {
                    let a = g(natural + i);
                    let b = g(p + i);
                    xy += a * b;
                    yy += b * b;
                    i += 2;
                }
                xy / yy.sqrt()
            };
            let lo = (nominal - tol).max(0);
            let hi = nominal + tol;
            let mut best = nominal.max(0);
            let mut best_s = f32::NEG_INFINITY;
            let mut p = lo;
            while p <= hi {
                let s = score(p);
                if s > best_s {
                    best_s = s;
                    best = p;
                }
                p += 4;
            }
            let (rlo, rhi) = ((best - 3).max(lo), (best + 3).min(hi));
            for p in rlo..=rhi {
                let s = score(p);
                if s > best_s {
                    best_s = s;
                    best = p;
                }
            }
            best
        };
        for (c, o) in ch.iter().zip(out.iter_mut()) {
            for (i, w) in window.iter().enumerate() {
                let src = in_pos as usize + i;
                let v = c.get(src).copied().unwrap_or(0.0);
                if let Some(d) = o.get_mut(out_pos + i) {
                    *d += v * w;
                }
            }
        }
        for (i, w) in window.iter().enumerate() {
            if let Some(d) = wsum.get_mut(out_pos + i) {
                *d += w;
            }
        }
        prev_in = in_pos;
        k += 1;
    }
    for o in out.iter_mut() {
        for (s, w) in o.iter_mut().zip(&wsum) {
            if *w > 1e-3 {
                *s /= w;
            }
        }
        o.truncate(out_len);
    }
    out
}

/// Length-preserving pitch shift by `semitones` (clamped to ±48): WSOLA stretch followed by
/// windowed-sinc resampling back to the original length.
pub fn pitch_shift(ch: &[Vec<f32>], semitones: f32, sample_rate: f32) -> Vec<Vec<f32>> {
    let st = if semitones.is_finite() { semitones.clamp(-48.0, 48.0) } else { 0.0 };
    if st.abs() < 1e-4 {
        return ch.to_vec();
    }
    let factor = 2f64.powf(f64::from(st) / 12.0);
    let stretched = time_stretch(ch, factor, sample_rate);
    let len = max_len(ch);
    let slen = max_len(&stretched);
    if slen == 0 || len == 0 {
        return ch.iter().map(|c| vec![0.0; c.len()]).collect();
    }
    let ratio = len as f64 / slen as f64;
    let mut out = resample_ratio(&stretched, ratio, len);
    for (o, c) in out.iter_mut().zip(ch) {
        o.truncate(c.len());
    }
    out
}
