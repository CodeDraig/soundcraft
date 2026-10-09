//! Low-frequency phase accumulation.

use std::f32::consts::TAU;

/// Sine LFO with phase in 0..1.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Lfo {
    phase: f32,
}

impl Lfo {
    pub(crate) fn new() -> Self {
        Self { phase: 0.0 }
    }

    pub(crate) fn reset(&mut self) {
        self.phase = 0.0;
    }

    /// Advances by `inc` cycles and returns sin at `offset` cycles ahead (for stereo spread).
    #[inline]
    pub(crate) fn tick(&mut self, inc: f32) -> f32 {
        let inc = if inc.is_finite() { inc.clamp(0.0, 0.5) } else { 0.0 };
        self.phase += inc;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        self.phase
    }

    #[inline]
    pub(crate) fn sin_at(phase: f32) -> f32 {
        (phase * TAU).sin()
    }
}
