//! Peak hold and clip indication.

use crate::MIN_DB;
use crate::gain_to_db;
use crate::util::sane_sr;

/// Peak meter with instant attack, linear-in-dB release, peak hold and a clip latch.
#[derive(Debug, Clone)]
pub struct PeakMeter {
    sr: f32,
    level_db: f32,
    hold_db: f32,
    hold_left: f32,
    hold_secs: f32,
    release_db_per_sec: f32,
    clipped: bool,
}

impl PeakMeter {
    /// A meter with 20 dB/s release and a 2 s peak hold.
    pub fn new(sample_rate: f32) -> Self {
        Self { sr: sane_sr(sample_rate), level_db: MIN_DB, hold_db: MIN_DB, hold_left: 0.0, hold_secs: 2.0, release_db_per_sec: 20.0, clipped: false }
    }

    pub fn set_release_db_per_sec(&mut self, rate: f32) {
        if rate.is_finite() {
            self.release_db_per_sec = rate.clamp(0.1, 1000.0);
        }
    }

    pub fn set_hold_secs(&mut self, secs: f32) {
        if secs.is_finite() {
            self.hold_secs = secs.clamp(0.0, 60.0);
        }
    }

    /// Feeds one block of samples.
    pub fn process(&mut self, samples: &[f32]) {
        let peak = samples.iter().filter(|v| v.is_finite()).fold(0.0f32, |m, v| m.max(v.abs()));
        self.feed(peak, samples.len());
    }

    /// Feeds an already-computed block peak (linear) covering `frames` samples.
    pub fn feed(&mut self, peak: f32, frames: usize) {
        let dt = frames as f32 / self.sr;
        if peak >= 1.0 {
            self.clipped = true;
        }
        let peak_db = gain_to_db(peak);
        let decayed = (self.level_db - self.release_db_per_sec * dt).max(MIN_DB);
        self.level_db = decayed.max(peak_db);
        if peak_db >= self.hold_db {
            self.hold_db = peak_db;
            self.hold_left = self.hold_secs;
        } else if self.hold_left > 0.0 {
            self.hold_left -= dt;
        } else {
            self.hold_db = (self.hold_db - self.release_db_per_sec * dt).max(self.level_db);
        }
    }

    /// Displayed level in dBFS.
    pub fn level_db(&self) -> f32 {
        self.level_db
    }

    /// Held peak in dBFS.
    pub fn peak_hold_db(&self) -> f32 {
        self.hold_db
    }

    /// True once any sample reached full scale, until [`PeakMeter::reset_clip`].
    pub fn clipped(&self) -> bool {
        self.clipped
    }

    pub fn reset_clip(&mut self) {
        self.clipped = false;
    }

    pub fn reset(&mut self) {
        self.level_db = MIN_DB;
        self.hold_db = MIN_DB;
        self.hold_left = 0.0;
        self.clipped = false;
    }
}
