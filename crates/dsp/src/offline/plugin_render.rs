//! Bounded offline plugin rendering with tail support.

use super::{MAX_OFFLINE_SAMPLES, max_len};
use crate::Plugin;
use crate::util::sane_sr;

/// Runs a plugin over whole buffers in blocks, compensating its latency so the output lines up
/// with the input. The buffer length is unchanged (any tail past the end is dropped; see
/// [`apply_plugin_with_tail`]).
pub fn apply_plugin(ch: &mut [Vec<f32>], plugin: &mut dyn Plugin, sample_rate: f32) {
    let len = max_len(ch);
    let out = render(ch, plugin, sample_rate, len);
    for (c, o) in ch.iter_mut().zip(out) {
        for (d, s) in c.iter_mut().zip(o) {
            *d = s;
        }
    }
}

/// Like [`apply_plugin`] but returns new buffers extended by the plugin's tail (capped at 60 s).
pub fn apply_plugin_with_tail(ch: &[Vec<f32>], plugin: &mut dyn Plugin, sample_rate: f32) -> Vec<Vec<f32>> {
    let len = max_len(ch);
    plugin.prepare(sane_sr(sample_rate), 1024, ch.len());
    let tail = plugin.tail_samples().min((sane_sr(sample_rate) * 60.0) as usize);
    render(ch, plugin, sample_rate, (len + tail).min(MAX_OFFLINE_SAMPLES))
}

fn render(ch: &[Vec<f32>], plugin: &mut dyn Plugin, sample_rate: f32, out_len: usize) -> Vec<Vec<f32>> {
    const BLOCK: usize = 1024;
    let sr = sane_sr(sample_rate);
    let channels = ch.len();
    plugin.prepare(sr, BLOCK, channels);
    plugin.reset();
    let lat = plugin.latency().min(MAX_OFFLINE_SAMPLES);
    let mut out: Vec<Vec<f32>> = (0..channels).map(|_| vec![0.0; out_len]).collect();
    let mut scratch: Vec<Vec<f32>> = (0..channels).map(|_| vec![0.0; BLOCK]).collect();
    let total = out_len + lat;
    let mut done = 0usize;
    while done < total {
        let n = BLOCK.min(total - done);
        for (s, c) in scratch.iter_mut().zip(ch) {
            for (i, v) in s.iter_mut().enumerate() {
                *v = if i < n { c.get(done + i).copied().unwrap_or(0.0) } else { 0.0 };
            }
        }
        plugin.process(&mut scratch, n);
        for (s, o) in scratch.iter().zip(out.iter_mut()) {
            for (i, v) in s.iter().take(n).enumerate() {
                let idx = done + i;
                if idx >= lat
                    && let Some(d) = o.get_mut(idx - lat)
                {
                    *d = if v.is_finite() { *v } else { 0.0 };
                }
            }
        }
        done += n;
    }
    out
}
