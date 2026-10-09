//! Bounded MIDI velocity transformations.

use super::{DEFAULT_SEED, round_i64};
use crate::Note;
use crate::XorShift;

/// Velocity change for [`change_velocity`]. Results are clamped to 1..=127.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VelocityOp {
    Set(u8),
    Add(i32),
    /// Multiply (non-finite factors leave velocities unchanged).
    Scale(f32),
    /// Clamp into `min..=max` (bounds are swapped if reversed).
    Limit {
        min: u8,
        max: u8,
    },
    /// Linear ramp over the notes' time span: the earliest note gets `from`, the latest `to`.
    Ramp {
        from: u8,
        to: u8,
    },
    /// Random offset of up to ± the amount.
    Humanize(u8),
}

fn clamp_vel(v: i64) -> u8 {
    v.clamp(1, 127) as u8
}

/// Changes note-on velocities. See [`VelocityOp`].
pub fn change_velocity(notes: &mut [Note], op: VelocityOp) {
    change_velocity_with_seed(notes, op, DEFAULT_SEED);
}

/// [`change_velocity`] with an explicit seed for [`VelocityOp::Humanize`].
pub fn change_velocity_with_seed(notes: &mut [Note], op: VelocityOp, seed: u64) {
    match op {
        VelocityOp::Set(v) => notes.iter_mut().for_each(|n| n.velocity = clamp_vel(i64::from(v))),
        VelocityOp::Add(d) => notes.iter_mut().for_each(|n| n.velocity = clamp_vel(i64::from(n.velocity) + i64::from(d))),
        VelocityOp::Scale(f) => {
            if f.is_finite() {
                notes.iter_mut().for_each(|n| n.velocity = clamp_vel(round_i64(f64::from(n.velocity) * f64::from(f))));
            }
        }
        VelocityOp::Limit { min, max } => {
            let (lo, hi) = if min <= max { (min, max) } else { (max, min) };
            notes.iter_mut().for_each(|n| n.velocity = clamp_vel(i64::from(n.velocity.clamp(lo, hi))));
        }
        VelocityOp::Ramp { from, to } => {
            let first = notes.iter().map(|n| n.start).min().unwrap_or(0);
            let last = notes.iter().map(|n| n.start).max().unwrap_or(0);
            let span = last.saturating_sub(first) as f64;
            for n in notes.iter_mut() {
                let t = if span > 0.0 { n.start.saturating_sub(first) as f64 / span } else { 0.0 };
                let v = f64::from(from) + (f64::from(to) - f64::from(from)) * t;
                n.velocity = clamp_vel(round_i64(v));
            }
        }
        VelocityOp::Humanize(amount) => {
            let mut rng = XorShift::new(seed);
            for n in notes.iter_mut() {
                let d = round_i64(rng.next_signed() * f64::from(amount));
                n.velocity = clamp_vel(i64::from(n.velocity) + d);
            }
        }
    }
}
