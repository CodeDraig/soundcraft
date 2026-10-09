//! Audio activity and transient detection.

use super::max_len;
use crate::db_to_gain;
use crate::util::sane_sr;
use rustfft::FftPlanner;
use rustfft::num_complex::Complex;
use std::f64::consts::PI;

/// Onset detection by half-wave-rectified spectral flux with an adaptive threshold.
/// `sensitivity` 0..1 (higher finds more transients). Returns sample positions, ascending.
pub fn detect_transients(ch: &[Vec<f32>], sample_rate: f32, sensitivity: f32) -> Vec<usize> {
    let len = max_len(ch);
    let sr = sane_sr(sample_rate);
    let sens = if sensitivity.is_finite() { sensitivity.clamp(0.0, 1.0) } else { 0.5 };
    let n = ((sr * 0.023) as usize).clamp(64, 8192).next_power_of_two();
    let hop = n / 4;
    if len < n || ch.is_empty() {
        return Vec::new();
    }
    let nch = ch.len() as f32;
    let mono: Vec<f32> = (0..len).map(|i| ch.iter().map(|c| c.get(i).copied().filter(|v| v.is_finite()).unwrap_or(0.0)).sum::<f32>() / nch).collect();
    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(n);
    let window: Vec<f32> = (0..n).map(|i| (0.5 - 0.5 * (2.0 * PI * i as f64 / n as f64).cos()) as f32).collect();
    let frames = (len - n) / hop + 1;
    let mut prev = vec![0.0f32; n / 2 + 1];
    let mut buf = vec![Complex::new(0.0f32, 0.0); n];
    let mut flux = Vec::with_capacity(frames);
    for m in 0..frames {
        let start = m * hop;
        for (i, b) in buf.iter_mut().enumerate() {
            *b = Complex::new(mono.get(start + i).copied().unwrap_or(0.0) * window.get(i).copied().unwrap_or(0.0), 0.0);
        }
        fft.process(&mut buf);
        let mut f = 0.0;
        for (b, p) in buf.iter().zip(prev.iter_mut()) {
            let mag = (1.0 + 100.0 * b.norm()).ln();
            f += (mag - *p).max(0.0);
            *p = mag;
        }
        flux.push(if m == 0 { 0.0 } else { f });
    }
    let max_flux = flux.iter().copied().fold(0.0f32, f32::max);
    if max_flux <= 1e-6 {
        return Vec::new();
    }
    let w = ((0.1 * sr) as usize / hop).max(2);
    let mult = 1.0 + 2.5 * (1.0 - sens);
    let delta = max_flux * (0.02 + 0.25 * (1.0 - sens));
    let min_gap = ((0.05 * sr) as usize / hop).max(1);
    let mut out = Vec::new();
    let mut last: Option<usize> = None;
    for m in 1..flux.len() {
        let cur = flux.get(m).copied().unwrap_or(0.0);
        let lo = m.saturating_sub(w);
        let hi = (m + w + 1).min(flux.len());
        let local = flux.get(lo..hi).map(|s| s.iter().sum::<f32>() / s.len().max(1) as f32).unwrap_or(0.0);
        let prev_v = flux.get(m - 1).copied().unwrap_or(0.0);
        let next_v = flux.get(m + 1).copied().unwrap_or(0.0);
        if cur > local * mult + delta && cur >= prev_v && cur > next_v && last.is_none_or(|l| m - l >= min_gap) {
            last = Some(m);
            // Refine: first sample in the analysis frame reaching a quarter of the frame's peak.
            let start = m * hop;
            let seg = mono.get(start..(start + n).min(len)).unwrap_or(&[]);
            let pk = seg.iter().fold(0.0f32, |a, v| a.max(v.abs()));
            let off = seg.iter().position(|v| v.abs() >= 0.25 * pk).unwrap_or(n / 2);
            let pos = start + off;
            if out.last().is_none_or(|&p: &usize| pos > p) {
                out.push(pos);
            }
        }
    }
    out
}

/// Strip Silence: ranges `(start, end)` (end exclusive) of material whose peak across channels
/// exceeds `threshold_db`. Silent gaps shorter than `min_len` samples do not split a region.
/// Each region is padded by `pad_before`/`pad_after` (clamped to the buffer) and overlapping
/// regions are merged.
pub fn non_silent_ranges(ch: &[Vec<f32>], threshold_db: f32, min_len: usize, pad_before: usize, pad_after: usize) -> Vec<(usize, usize)> {
    let len = max_len(ch);
    let thr = db_to_gain(if threshold_db.is_nan() { -48.0 } else { threshold_db });
    let mut raw: Vec<(usize, usize)> = Vec::new();
    let mut start: Option<usize> = None;
    let mut last_loud = 0usize;
    for i in 0..len {
        let loud = ch.iter().any(|c| c.get(i).is_some_and(|v| v.is_finite() && v.abs() > thr));
        if !loud {
            continue;
        }
        match start {
            Some(s) if i - last_loud > min_len.max(1) => {
                raw.push((s, last_loud + 1));
                start = Some(i);
            }
            None => start = Some(i),
            _ => {}
        }
        last_loud = i;
    }
    if let Some(s) = start {
        raw.push((s, last_loud + 1));
    }
    let mut out: Vec<(usize, usize)> = Vec::with_capacity(raw.len());
    for (s, e) in raw {
        let s = s.saturating_sub(pad_before);
        let e = e.saturating_add(pad_after).min(len);
        match out.last_mut() {
            Some(last) if s <= last.1 => last.1 = last.1.max(e),
            _ => out.push((s, e)),
        }
    }
    out
}
