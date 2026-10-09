//! Swung-grid quantization with deterministic random offsets.

use super::{DEFAULT_SEED, round_i64, unit};
use crate::Note;
use crate::XorShift;

/// Options for [`quantize`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuantizeOptions {
    /// Grid spacing in ticks (e.g. 240 = 1/16 note at 960 PPQ). Values <= 0 make quantize a no-op.
    pub grid_ticks: i64,
    /// How far notes move towards the grid line, 0..=1 (1 = all the way).
    pub strength: f32,
    /// Swing, 0..=1: every second grid line is delayed by `swing * grid / 3`, so 1.0 gives a
    /// triplet feel.
    pub swing: f32,
    /// Only notes within this fraction of half a grid step of their target are moved
    /// (1 = all notes).
    pub include_within: f32,
    /// Notes closer than this fraction of half a grid step to their target are left alone
    /// (0 = none excluded).
    pub exclude_within: f32,
    /// Also quantize note ends to the grid.
    pub quantize_ends: bool,
    /// Keep each note's length when its start moves (otherwise the end stays put). Ignored when
    /// `quantize_ends` is set.
    pub preserve_duration: bool,
    /// Random offset after quantizing, 0..=1 fraction of half a grid step.
    pub random: f32,
}

impl Default for QuantizeOptions {
    fn default() -> Self {
        Self {
            grid_ticks: 240,
            strength: 1.0,
            swing: 0.0,
            include_within: 1.0,
            exclude_within: 0.0,
            quantize_ends: false,
            preserve_duration: true,
            random: 0.0,
        }
    }
}

/// The swung grid line nearest to `t`.
fn nearest_line(t: i64, grid: i64, swing: f64) -> i64 {
    let swing_off = round_i64(swing * grid as f64 / 3.0);
    let k = t.saturating_add(grid / 2).div_euclid(grid);
    let mut best = i64::MAX;
    let mut best_d = u64::MAX;
    for kk in [k.saturating_sub(1), k, k.saturating_add(1)] {
        let line = kk.saturating_mul(grid).saturating_add(if kk.rem_euclid(2) == 1 { swing_off } else { 0 });
        let d = line.abs_diff(t);
        if d < best_d {
            best_d = d;
            best = line;
        }
    }
    best
}

/// Quantizes note starts (and optionally ends) to a grid. See [`QuantizeOptions`].
pub fn quantize(notes: &mut [Note], opts: &QuantizeOptions) {
    quantize_with_seed(notes, opts, DEFAULT_SEED);
}

/// [`quantize`] with an explicit seed for the `random` component.
pub fn quantize_with_seed(notes: &mut [Note], opts: &QuantizeOptions, seed: u64) {
    let grid = opts.grid_ticks;
    if grid <= 0 {
        return;
    }
    let strength = unit(opts.strength);
    let swing = unit(opts.swing);
    let include = unit(opts.include_within);
    let exclude = unit(opts.exclude_within);
    let random = unit(opts.random);
    let half = grid as f64 / 2.0;
    let mut rng = XorShift::new(seed);

    for n in notes.iter_mut() {
        let old_start = n.start;
        let old_end = n.end();
        let target = nearest_line(old_start, grid, swing);
        let d = target.saturating_sub(old_start);
        let dist = d.unsigned_abs() as f64;
        if include < 1.0 && dist > include * half {
            continue;
        }
        if dist < exclude * half {
            continue;
        }
        let mut new_start = old_start.saturating_add(round_i64(d as f64 * strength));
        if random > 0.0 {
            new_start = new_start.saturating_add(round_i64(rng.next_signed() * random * half));
        }
        n.start = new_start;
        n.length = if opts.quantize_ends {
            let end_target = nearest_line(old_end, grid, swing);
            let new_end = old_end.saturating_add(round_i64(end_target.saturating_sub(old_end) as f64 * strength));
            new_end.saturating_sub(new_start)
        } else if opts.preserve_duration {
            n.length
        } else {
            old_end.saturating_sub(new_start)
        }
        .max(1);
    }
}
