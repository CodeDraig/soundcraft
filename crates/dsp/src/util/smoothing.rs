//! Sample-accurate parameter smoothing.

use super::time_coef;

/// One-pole exponential parameter smoother (no zipper noise).
#[derive(Debug, Clone, Copy)]
pub(crate) struct Smoother {
    cur: f32,
    target: f32,
    coef: f32,
}

impl Smoother {
    pub(crate) fn new(v: f32) -> Self {
        Self { cur: v, target: v, coef: 0.0 }
    }

    /// Sets the time constant (default ~20 ms is typical).
    pub(crate) fn set_time(&mut self, ms: f32, sr: f32) {
        self.coef = time_coef(ms, sr);
    }

    pub(crate) fn set(&mut self, v: f32) {
        if v.is_finite() {
            self.target = v;
        }
    }

    pub(crate) fn snap(&mut self) {
        self.cur = self.target;
    }

    pub(crate) fn value(&self) -> f32 {
        self.cur
    }

    pub(crate) fn settling(&self) -> bool {
        self.cur != self.target
    }

    #[inline]
    pub(crate) fn next(&mut self) -> f32 {
        if self.cur != self.target {
            self.cur = self.target + (self.cur - self.target) * self.coef;
            if (self.cur - self.target).abs() <= 1.0e-6 * (1.0 + self.target.abs()) {
                self.cur = self.target;
            }
        }
        self.cur
    }

    /// Advances `n` samples at once (used for block-rate coefficient updates).
    pub(crate) fn advance(&mut self, n: usize) -> f32 {
        if self.cur != self.target {
            let k = self.coef.powi(n.min(i32::MAX as usize) as i32);
            self.cur = self.target + (self.cur - self.target) * k;
            if (self.cur - self.target).abs() <= 1.0e-6 * (1.0 + self.target.abs()) {
                self.cur = self.target;
            }
        }
        self.cur
    }
}
