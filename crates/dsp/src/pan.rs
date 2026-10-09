//! Mono-to-stereo pan laws and the surround (multichannel) panner.

use std::f32::consts::FRAC_PI_4;

mod surround;
use surround::fin;
pub use surround::{MAX_SPEAKERS, SpeakerPos, SurroundParams, azimuth_to_puck, direction_gains, puck_azimuth, surround_gains};
mod ambisonic;
pub use ambisonic::{ambisonic_decode, ambisonic_encode};

/// Attenuation of a centered signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
pub enum PanLaw {
    /// Constant power: -3 dB at center.
    #[default]
    Minus3,
    /// Compromise: -4.5 dB at center.
    Minus4_5,
    /// Constant voltage: -6 dB at center.
    Minus6,
    /// Balance: 0 dB at center, the far side fades out linearly.
    Zero,
}

impl PanLaw {
    /// Center attenuation in dB (negative).
    pub fn center_db(self) -> f32 {
        match self {
            PanLaw::Minus3 => -3.0,
            PanLaw::Minus4_5 => -4.5,
            PanLaw::Minus6 => -6.0,
            PanLaw::Zero => 0.0,
        }
    }
}

/// Left/right gains for `pan` in -1 (hard left) ..= 1 (hard right). NaN pans center.
pub fn gains(pan: f32, law: PanLaw) -> (f32, f32) {
    let pan = if pan.is_nan() { 0.0 } else { pan.clamp(-1.0, 1.0) };
    if law == PanLaw::Zero {
        return ((1.0 - pan).min(1.0), (1.0 + pan).min(1.0));
    }
    let theta = (pan + 1.0) * FRAC_PI_4;
    let (l, r) = (theta.cos().max(0.0), theta.sin().max(0.0));
    // cos(pi/4) = -3.0103 dB; raise to a power to reach the requested center level.
    let exp = law.center_db() / -3.010_3;
    let l = if pan <= -1.0 { 1.0 } else { l.powf(exp) };
    let r = if pan >= 1.0 { 1.0 } else { r.powf(exp) };
    (l, r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gain_to_db;

    #[test]
    fn center_levels() {
        for law in [PanLaw::Minus3, PanLaw::Minus4_5, PanLaw::Minus6, PanLaw::Zero] {
            let (l, r) = gains(0.0, law);
            assert!((gain_to_db(l) - law.center_db()).abs() < 0.02, "{law:?}");
            assert!((l - r).abs() < 1e-6);
            let (l, r) = gains(-1.0, law);
            assert!((l - 1.0).abs() < 1e-6 && r.abs() < 1e-6);
        }
    }

    fn layout_51() -> Vec<SpeakerPos> {
        [(-30.0, false), (30.0, false), (0.0, false), (0.0, true), (-110.0, false), (110.0, false)]
            .iter()
            .map(|&(az, lfe)| SpeakerPos { az, el: 0.0, lfe })
            .collect()
    }

    fn layout_714() -> Vec<SpeakerPos> {
        let mut v: Vec<SpeakerPos> =
            [-30.0, 30.0, 0.0, 0.0, -150.0, 150.0, -90.0, 90.0].iter().enumerate().map(|(i, &az)| SpeakerPos { az, el: 0.0, lfe: i == 3 }).collect();
        for az in [-45.0, 45.0, -135.0, 135.0] {
            v.push(SpeakerPos { az, el: 45.0, lfe: false });
        }
        v
    }

    fn params(x: f32, y: f32) -> SurroundParams {
        SurroundParams { x, y, z: 0.0, divergence: 0.0, center: 1.0 }
    }

    #[test]
    fn surround_energy_is_constant() {
        for layout in [layout_51(), layout_714()] {
            let mut g = [0.0f32; MAX_SPEAKERS];
            for xi in -4..=4 {
                for yi in -4..=4 {
                    for div in [0.0, 0.3, 1.0] {
                        for z in [0.0, 0.5, 1.0] {
                            for center in [0.0, 0.5, 1.0] {
                                let p = SurroundParams { x: xi as f32 / 4.0, y: yi as f32 / 4.0, z, divergence: div, center };
                                surround_gains(&layout, &p, &mut g);
                                let e: f32 = g.iter().zip(layout.iter()).filter(|(_, s)| !s.lfe).map(|(x, _)| x * x).sum();
                                assert!((e - 1.0).abs() < 1e-4, "energy {e} at {p:?}");
                                assert!(g.iter().zip(layout.iter()).all(|(x, s)| !s.lfe || *x == 0.0), "LFE is never panned to");
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn surround_point_positions() {
        let l = layout_51();
        let mut g = [0.0f32; MAX_SPEAKERS];
        // Front centre with no divergence: C only.
        surround_gains(&l, &params(0.0, 1.0), &mut g);
        assert!((g[2] - 1.0).abs() < 1e-5 && g.iter().enumerate().all(|(i, x)| i == 2 || x.abs() < 1e-3), "{g:?}");
        // Divergence spreads the centre image.
        surround_gains(&l, &SurroundParams { divergence: 0.5, ..params(0.0, 1.0) }, &mut g);
        assert!(g[2] < 0.99 && g[0] > 0.05 && g[1] > 0.05);
        // Full left (front-left corner): L only.
        surround_gains(&l, &params(-1.0, 1.0), &mut g);
        assert!((g[0] - 1.0).abs() < 1e-5 && g.iter().enumerate().all(|(i, x)| i == 0 || x.abs() < 1e-3), "{g:?}");
        // Rear-right corner: Rs only.
        surround_gains(&l, &params(1.0, -1.0), &mut g);
        assert!((g[5] - 1.0).abs() < 1e-5, "{g:?}");
        // Phantom centre (centre 0 %): L and R at -3 dB, no C.
        surround_gains(&l, &SurroundParams { center: 0.0, ..params(0.0, 1.0) }, &mut g);
        assert!(g[2].abs() < 1e-6 && (g[0] - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-4);
        // Overhead on 7.1.4: heights only.
        let l = layout_714();
        surround_gains(&l, &SurroundParams { z: 1.0, ..params(-1.0, 1.0) }, &mut g);
        let top: f32 = g[8..12].iter().map(|x| x * x).sum();
        assert!((top - 1.0).abs() < 1e-4 && g[8] > 0.9, "{g:?}");
        // Corner positions map onto speaker angles and back.
        let (x, y) = azimuth_to_puck(&l, -150.0);
        assert!((x + 1.0).abs() < 1e-3 && (y + 1.0).abs() < 1e-3, "{x} {y}");
        assert!((puck_azimuth(&l, 0.0, -1.0).abs() - 180.0).abs() < 1e-3);
        // Hostile input never produces NaN.
        surround_gains(&l, &SurroundParams { x: f32::NAN, y: f32::INFINITY, z: f32::NAN, divergence: f32::NAN, center: f32::NAN }, &mut g);
        assert!(g.iter().all(|x| x.is_finite()));
    }
}
