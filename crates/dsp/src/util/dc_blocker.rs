//! One-pole DC blocking.

use super::{flush, sane_sr};
use std::f32::consts::TAU;

/// DC blocking one-pole high-pass.
#[derive(Debug, Clone, Copy)]
pub(crate) struct DcBlocker {
    r: f32,
    x1: f32,
    y1: f32,
}

impl DcBlocker {
    pub(crate) fn new(cutoff_hz: f32, sr: f32) -> Self {
        let mut d = Self { r: 0.995, x1: 0.0, y1: 0.0 };
        d.set_cutoff(cutoff_hz, sr);
        d
    }

    pub(crate) fn set_cutoff(&mut self, cutoff_hz: f32, sr: f32) {
        let fc = if cutoff_hz.is_finite() { cutoff_hz.clamp(0.1, 1000.0) } else { 5.0 };
        self.r = (-TAU * fc / sane_sr(sr)).exp();
    }

    pub(crate) fn reset(&mut self) {
        self.x1 = 0.0;
        self.y1 = 0.0;
    }

    #[inline]
    pub(crate) fn process(&mut self, x: f32) -> f32 {
        let y = x - self.x1 + self.r * self.y1;
        self.x1 = x;
        self.y1 = flush(y);
        self.y1
    }
}
