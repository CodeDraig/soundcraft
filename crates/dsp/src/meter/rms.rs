//! Windowed RMS metering.

use crate::gain_to_db;
use crate::util::sane_sr;

/// Exponentially averaged RMS meter (300 ms time constant by default).
#[derive(Debug, Clone)]
pub struct RmsMeter {
    sr: f32,
    coef: f32,
    ms: f64,
}

impl RmsMeter {
    pub fn new(sample_rate: f32) -> Self {
        let mut m = Self { sr: sane_sr(sample_rate), coef: 0.0, ms: 0.0 };
        m.set_window_ms(300.0);
        m
    }

    pub fn set_window_ms(&mut self, ms: f32) {
        self.coef = crate::util::time_coef(if ms.is_finite() { ms.clamp(1.0, 10_000.0) } else { 300.0 }, self.sr);
    }

    pub fn process(&mut self, samples: &[f32]) {
        let k = f64::from(1.0 - self.coef);
        for &x in samples {
            let x = if x.is_finite() { f64::from(x) } else { 0.0 };
            self.ms += (x * x - self.ms) * k;
        }
        if self.ms < 1e-30 {
            self.ms = 0.0;
        }
    }

    /// Current RMS level in dBFS (sine-referenced: an RMS of 0.707 reads -3 dB).
    pub fn rms_db(&self) -> f32 {
        gain_to_db(self.ms.sqrt() as f32)
    }

    pub fn reset(&mut self) {
        self.ms = 0.0;
    }
}
