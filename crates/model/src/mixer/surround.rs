//! Surround panner coordinates and their valid ranges.

use super::finite_or;

/// Surround panner position (Pro Tools-style X/Y puck). `None` on a mixer means "use the stereo
/// pan"; an explicit position activates the surround panner.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SurroundPan {
    /// -1 (left) ..= 1 (right).
    pub x: f32,
    /// -1 (back) ..= 1 (front).
    pub y: f32,
    /// Elevation 0 (ear level) ..= 1 (overhead), used by height formats.
    pub z: f32,
    /// 0 = point source ..= 1 = spread over every speaker.
    pub divergence: f32,
    /// Centre speaker share of front-centre images, 0..=100 % (0 = phantom centre on L/R).
    pub center: f32,
    /// LFE send level, dB (-144 = off).
    pub lfe_db: f32,
}

impl Default for SurroundPan {
    fn default() -> Self {
        SurroundPan { x: 0.0, y: 1.0, z: 0.0, divergence: 0.0, center: 100.0, lfe_db: -144.0 }
    }
}

impl SurroundPan {
    /// Puck at the stereo pan position (front edge).
    pub fn from_stereo(pan: f32) -> Self {
        SurroundPan { x: finite_or(pan, 0.0).clamp(-1.0, 1.0), ..SurroundPan::default() }
    }
    /// Clamp into range (NaN → defaults).
    pub fn sanitize(&mut self) {
        let d = SurroundPan::default();
        self.x = finite_or(self.x, d.x).clamp(-1.0, 1.0);
        self.y = finite_or(self.y, d.y).clamp(-1.0, 1.0);
        self.z = finite_or(self.z, d.z).clamp(0.0, 1.0);
        self.divergence = finite_or(self.divergence, d.divergence).clamp(0.0, 1.0);
        self.center = finite_or(self.center, d.center).clamp(0.0, 100.0);
        self.lfe_db = finite_or(self.lfe_db, d.lfe_db).clamp(-144.0, 12.0);
    }
}
